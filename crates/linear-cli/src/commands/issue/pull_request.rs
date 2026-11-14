//! `issue gh pull-request` and `issue jj pull-request`: open a GitHub pull
//! request for one or more issues with `gh`, which owns the browser, editor
//! and interactive flow. The jj command settles and pushes the head bookmark
//! first; the gh command leaves the repository and head to gh.
use crate::{
    cli::{issue::IssuePullRequest, values::IssueRef},
    client::LinearClient,
    commands::text_input,
    config::PrTemplateCli,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::issue::IssueDetails,
    platform::{
        gh,
        jj_pr::{self, Jj},
    },
};
use futures_util::{StreamExt, TryStreamExt, stream};
use std::collections::HashSet;
use std::path::Path;

/// Which command runs: the one that names the tool decides what is done to
/// the repository, never a setting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Route {
    /// `issue gh pr`: gh alone, from the git checkout.
    Gh,
    /// `issue jj pr`: push a jj bookmark, then gh with its repository and head.
    Jj,
}

impl Route {
    fn command(self) -> &'static str {
        match self {
            Self::Gh => "linear issue gh pr",
            Self::Jj => "linear issue jj pr",
        }
    }
}

/// What a failed fetch of one of several issues adds to its hint.
const NOTHING_PUBLISHED: &str = "No pull request was opened and no bookmark was created or pushed. Check the issue ID and access, then try again.";

pub fn run_gh(ctx: &Ctx, args: &IssuePullRequest) -> Result<()> {
    create_pull_request(ctx, args, Route::Gh).context("Failed to create pull request")
}

pub fn run_jj(ctx: &Ctx, args: &IssuePullRequest) -> Result<()> {
    create_pull_request(ctx, args, Route::Jj).context("Failed to create pull request")
}

fn create_pull_request(ctx: &Ctx, args: &IssuePullRequest, route: Route) -> Result<()> {
    // The issue is always named, by an ID or by `@git` / `@jj`; leaving it
    // out never reads it from the repository.
    if args.issue_ids.is_empty() {
        return Err(
            Error::invalid("No issue to open a pull request for").with_hint(format!(
                "Name it: an issue ID like `{} ENG-123`, `@git` for the issue in the git branch name, or `@jj` for the ones in the Linear-issue trailers of trunk()..@.",
                route.command()
            )),
        );
    }
    let selected = if args.no_template {
        PrTemplateCli::Disabled
    } else if let Some(path) = args.template.as_deref() {
        PrTemplateCli::Path(path)
    } else {
        PrTemplateCli::Unset
    };
    let template = ctx
        .options()
        .pr_template(selected)?
        .map(|path| read_template(path.path()))
        .transpose()?;
    if route == Route::Jj {
        jj_pr::check_head(args.head.as_deref())?;
    }
    let identifiers = distinct(ctx, &args.issue_ids)?;
    check_title(&identifiers, args.title.as_deref())?;
    // Credentials first: a missing login fails before jj snapshots the
    // working copy or anything else is touched.
    let client = ctx.client()?;
    if route == Route::Jj {
        // Nothing is pushed for a pull request that gh cannot open.
        gh::require(ctx.cwd(), &ctx.config().child_env)?;
    }
    // Every issue is fetched before the repository is touched, so a missing
    // one leaves nothing behind.
    let issues = ctx.spin(true, fetch_all(client, identifiers))?;
    let first = issues.first().expect("a pull request covers an issue");
    // gh reads the current git branch itself. jj has no current branch, so
    // its head is settled here, before anything is sent.
    let jj = Jj::new(ctx.cwd(), &ctx.config().child_env);
    let published = match route {
        Route::Gh => None,
        Route::Jj => {
            let survey = jj.survey(args.head.as_deref())?;
            // A new bookmark is named after the first issue.
            let head = survey.head(&first.details.branch_name)?;
            let remote = &survey.remote.name;
            if head.create {
                ctx.eprint(format!("Creating jj bookmark {}\n", head.name))?;
            }
            ctx.eprint(format!("Pushing jj bookmark {} to {remote}\n", head.name))?;
            ctx.eprint(jj.publish(&survey, &head)?)?;
            Some((survey.remote.url, head.name))
        }
    };
    let (repo, head) = match &published {
        Some((repo, head)) => (Some(repo.as_str()), Some(head.as_str())),
        None => (None, args.head.as_deref()),
    };
    let linkage = if args.references {
        Linkage::References
    } else {
        Linkage::Fixes
    };
    let argv = self::args(
        title(&issues, args.title.as_deref()),
        body(template.as_deref(), &issues, linkage),
        Options {
            repo,
            base: args.base.as_deref(),
            head,
            draft: args.draft,
            web: args.web,
        },
    );
    ctx.flush()?;
    gh::run(&argv, ctx.cwd(), &ctx.config().child_env)
}

/// The distinct issues the arguments name, in the order they are first
/// named: an argument's own issues in the order its reference reads them.
fn distinct(ctx: &Ctx, inputs: &[IssueRef]) -> Result<Vec<String>> {
    let several_inputs = inputs.len() > 1;
    let mut seen = HashSet::new();
    let mut identifiers = Vec::new();
    for input in inputs {
        let named = super::reference::require_all(ctx, input).map_err(|error| {
            if several_inputs {
                error.context(format!("Failed to resolve issue {input}"))
            } else {
                error
            }
        })?;
        for identifier in named {
            if seen.insert(identifier.clone()) {
                identifiers.push(identifier);
            }
        }
    }
    Ok(identifiers)
}

/// A pull request for several issues has no one issue to take its title
/// from, so it must be given.
fn check_title(identifiers: &[String], title: Option<&str>) -> Result<()> {
    let given = title.is_some_and(|title| !title.trim().is_empty());
    match identifiers {
        [] | [_] => Ok(()),
        _ if given => Ok(()),
        several => Err(Error::invalid(format!(
            "A title is required for a pull request covering several issues: {}",
            several.join(", ")
        ))
        .with_hint(
            "Pass --title \"<summary covering all issues>\". The title is used exactly as supplied.",
        )),
    }
}

/// An issue the pull request covers.
#[derive(Clone, Debug)]
pub struct Covered {
    pub identifier: String,
    pub details: IssueDetails,
}

/// Fetches every issue, five at a time, keeping their order. The first
/// failure stops it.
async fn fetch_all(client: &LinearClient, identifiers: Vec<String>) -> Result<Vec<Covered>> {
    let several = identifiers.len() > 1;
    stream::iter(identifiers)
        .map(|identifier| async move {
            match super::details::fetch(client, identifier.clone()).await {
                Ok(details) => Ok(Covered {
                    identifier,
                    details,
                }),
                Err(error) if several => {
                    let hint = match error.hint() {
                        Some(hint) => format!("{hint} {NOTHING_PUBLISHED}"),
                        None => NOTHING_PUBLISHED.to_owned(),
                    };
                    Err(error
                        .with_hint(hint)
                        .context(format!("Failed to fetch issue {identifier}")))
                }
                Err(error) => Err(error),
            }
        })
        .buffered(5)
        .try_collect()
        .await
}

/// Whether the generated clause closes the issues when the pull request
/// merges.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Linkage {
    Fixes,
    References,
}

impl Linkage {
    fn keyword(self) -> &'static str {
        match self {
            Self::Fixes => "Fixes",
            Self::References => "References",
        }
    }
}

/// One issue: `<ID> <title>`, the title being --title unless it is blank.
/// Several: --title as given, which [`check_title`] required.
pub fn title(issues: &[Covered], custom: Option<&str>) -> String {
    match issues {
        [issue] => format!(
            "{} {}",
            issue.identifier,
            custom
                .filter(|title| !title.trim().is_empty())
                .unwrap_or(&issue.details.title)
        ),
        _ => custom
            .expect("a pull request for several issues has a title")
            .to_owned(),
    }
}

/// The template, then Linear's linking clause, then each issue's URL. One
/// issue gets no clause unless it is [`Linkage::References`], so that its
/// body stays the template and the URL.
pub fn body(template: Option<&str>, issues: &[Covered], linkage: Linkage) -> String {
    let template = template.unwrap_or("").trim_end();
    let clause = match (issues, linkage) {
        ([_], Linkage::Fixes) => None,
        _ => {
            let identifiers: Vec<&str> = issues
                .iter()
                .map(|issue| issue.identifier.as_str())
                .collect();
            Some(format!("{} {}", linkage.keyword(), listed(&identifiers)))
        }
    };
    let urls: Vec<&str> = issues
        .iter()
        .map(|issue| issue.details.url.as_str())
        .collect();
    let urls = urls.join("\n");
    [Some(template), clause.as_deref(), Some(urls.as_str())]
        .into_iter()
        .flatten()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("\n\n")
}

/// `A`, `A and B`, `A, B, and C`: the list Linear reads after a magic word.
fn listed(items: &[&str]) -> String {
    match items {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [one, two] => format!("{one} and {two}"),
        [init @ .., last] => format!("{}, and {last}", init.join(", ")),
    }
}

pub const TEMPLATE_SUGGESTION: &str = "Pass a readable file to --template, fix the pr_template config option, or use --no-template to skip the template.";
fn unusable(reason: impl Into<String>) -> Error {
    Error::new(format!(
        "Cannot read pull request template: {}",
        reason.into()
    ))
    .with_hint(TEMPLATE_SUGGESTION)
}
pub fn read_template(path: &Path) -> Result<String, Error> {
    let display = path.to_string_lossy();
    if display.trim().is_empty() {
        return Err(unusable("the path is empty"));
    }
    let metadata = std::fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            unusable(format!("\"{display}\" does not exist"))
        } else {
            unusable(format!("\"{display}\" could not be read: {error}")).with_source(error)
        }
    })?;
    if metadata.is_dir() {
        return Err(unusable(format!(
            "\"{display}\" is a directory, not a file"
        )));
    }
    if !metadata.is_file() {
        return Err(unusable(format!("\"{display}\" is not a regular file")));
    }
    let contents = text_input::read_file(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::InvalidData {
            unusable(format!("\"{display}\" is not valid UTF-8 text")).with_source(error)
        } else {
            unusable(format!("\"{display}\" could not be read: {error}")).with_source(error)
        }
    })?;
    if contents.contains('\0') {
        return Err(unusable(format!("\"{display}\" is not a text file")));
    }
    Ok(contents)
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Options<'a> {
    /// The repository to open the pull request in, when gh is not to work
    /// it out from the checkout.
    pub repo: Option<&'a str>,
    pub base: Option<&'a str>,
    pub head: Option<&'a str>,
    pub draft: bool,
    pub web: bool,
}
pub fn args(title: String, body: String, options: Options<'_>) -> Vec<String> {
    let mut args = vec![
        "pr".to_owned(),
        "create".to_owned(),
        "--title".to_owned(),
        title,
        "--body".to_owned(),
        body,
    ];
    for (flag, value) in [
        ("--repo", options.repo),
        ("--base", options.base),
        ("--head", options.head),
    ] {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            args.push(flag.to_owned());
            args.push(value.to_owned());
        }
    }
    if options.draft {
        args.push("--draft".to_owned());
    }
    if options.web {
        args.push("--web".to_owned());
    }
    args
}

#[cfg(test)]
mod tests;
