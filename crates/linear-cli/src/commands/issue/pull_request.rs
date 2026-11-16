//! `issue gh pull-request`: open a GitHub pull request for one or more
//! issues with `gh`, which owns the browser, editor and interactive flow.
//! linear builds the title and body; everything after `--` goes to
//! `gh pr create` unchanged, after them. linear creates, moves and pushes
//! nothing itself.
use crate::{
    cli::{
        issue::IssuePullRequest,
        values::{IssueRef, TextSource},
    },
    client::LinearClient,
    commands::text_input,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::operations::issue::IssueDetails,
    platform::gh,
};
use futures_util::{StreamExt, TryStreamExt, stream};
use std::collections::HashSet;

/// What a failed fetch of one of several issues adds to its hint.
const NOTHING_OPENED: &str =
    "No pull request was opened. Check the issue ID and access, then try again.";

pub fn run(ctx: &Ctx, args: &IssuePullRequest) -> Result<()> {
    create_pull_request(ctx, args).context("Failed to create pull request")
}

fn create_pull_request(ctx: &Ctx, args: &IssuePullRequest) -> Result<()> {
    retired_template(args)?;
    moved_to_gh(args)?;
    // The issue is always named, by an ID or by `@git` / `@jj`; leaving it
    // out never reads it from the repository.
    if args.issue_ids.is_empty() {
        return Err(Error::invalid("No issue to open a pull request for").with_hint(
            "Name it: an issue ID like `linear issue gh pr ENG-123`, `@git` for the issue in the git branch name, or `@jj` for the ones in the Linear-issue trailers of trunk()..@.",
        ));
    }
    let description = match (args.body.as_deref(), args.body_file.as_ref()) {
        (Some(body), _) => Some(body.trim_end().to_owned()),
        (None, Some(source)) => Some(read_body_file(source)?),
        (None, None) => None,
    };
    let identifiers = distinct(ctx, &args.issue_ids)?;
    check_title(&identifiers, args.title.as_deref())?;
    let client = ctx.client()?;
    // Every issue is fetched before gh runs, so a missing one opens nothing.
    let issues = ctx.spin(true, fetch_all(client, identifiers))?;
    let linkage = if args.references {
        Linkage::References
    } else {
        Linkage::Fixes
    };
    let argv = self::args(
        title(&issues, args.title.as_deref()),
        body(description.as_deref(), &issues, linkage),
        &args.gh_args,
    );
    ctx.flush()?;
    gh::run(&argv, ctx.cwd(), &ctx.config().child_env)
}

/// Refuses the 3.x flags that now go to gh after `--`, naming where to put
/// them.
fn moved_to_gh(args: &IssuePullRequest) -> Result<()> {
    let moved: Vec<(&str, &str)> = [
        ("--base", args.base.is_some(), " <BRANCH>"),
        ("--draft", args.draft, ""),
        ("--web", args.web, ""),
        ("--head", args.head.is_some(), " <BRANCH>"),
    ]
    .into_iter()
    .filter(|(_, given, _)| *given)
    .map(|(flag, _, value)| (flag, value))
    .collect();
    if moved.is_empty() {
        return Ok(());
    }
    let flags: Vec<&str> = moved.iter().map(|(flag, _)| *flag).collect();
    let forms: Vec<String> = moved
        .iter()
        .map(|(flag, value)| format!("{flag}{value}"))
        .collect();
    let (subject, verb) = match flags.as_slice() {
        [one] => ((*one).to_owned(), "is"),
        several => (listed(several), "are"),
    };
    Err(
        Error::invalid(format!("{subject} {verb} now passed to gh")).with_hint(format!(
            "Put {} after --: linear issue gh pr <ISSUE> -- {}",
            if flags.len() == 1 { "it" } else { "them" },
            forms.join(" ")
        )),
    )
}

/// Refuses the 3.x template flags, retired with `pr_template`.
fn retired_template(args: &IssuePullRequest) -> Result<()> {
    let flag = if args.template.is_some() {
        "--template"
    } else if args.no_template {
        "--no-template"
    } else {
        return Ok(());
    };
    Err(
        Error::invalid(format!("{flag} was removed in 4.0, with pr_template")).with_hint(
            "-F <FILE> gives the same body: the file's text, then the issue links. Add -- --editor to fill it in before creating.",
        ),
    )
}

/// The description in a `--body-file`, which must hold some.
fn read_body_file(source: &TextSource) -> Result<String> {
    text_input::read_source(source)
        .map_err(|error| {
            Error::new(format!("Failed to read body file: {source}"))
                .with_hint(format!("Error: {error}"))
                .with_source(error)
        })?
        .ok_or_else(|| {
            Error::invalid(format!("Body file is empty: {source}")).with_hint(
                "Write the description into the file, or leave out --body-file for a body of just the issue links.",
            )
        })
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
/// from, so it must be given. clap has refused a blank --title.
fn check_title(identifiers: &[String], title: Option<&str>) -> Result<()> {
    match identifiers {
        [] | [_] => Ok(()),
        _ if title.is_some() => Ok(()),
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
                        Some(hint) => format!("{hint} {NOTHING_OPENED}"),
                        None => NOTHING_OPENED.to_owned(),
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

/// --title as given; without it, `<ID> <title>` for the one issue.
/// [`check_title`] required it for several.
pub fn title(issues: &[Covered], custom: Option<&str>) -> String {
    match (custom, issues) {
        (Some(custom), _) => custom.to_owned(),
        (None, [issue]) => format!("{} {}", issue.identifier, issue.details.title),
        (None, _) => unreachable!("a pull request for several issues has a title"),
    }
}

/// The description, then Linear's linking clause, then each issue's URL on
/// its own line.
pub fn body(description: Option<&str>, issues: &[Covered], linkage: Linkage) -> String {
    let description = description.unwrap_or("").trim_end();
    let identifiers: Vec<&str> = issues
        .iter()
        .map(|issue| issue.identifier.as_str())
        .collect();
    let clause = format!("{} {}", linkage.keyword(), listed(&identifiers));
    let urls: Vec<&str> = issues
        .iter()
        .map(|issue| issue.details.url.as_str())
        .collect();
    let urls = urls.join("\n");
    [description, clause.as_str(), urls.as_str()]
        .into_iter()
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

/// The `gh pr create` arguments: linear's title and body, then `gh_args`
/// unchanged, so that a flag repeated there wins.
pub fn args(title: String, body: String, gh_args: &[String]) -> Vec<String> {
    let mut args = vec![
        "pr".to_owned(),
        "create".to_owned(),
        "--title".to_owned(),
        title,
        "--body".to_owned(),
        body,
    ];
    args.extend(gh_args.iter().cloned());
    args
}

#[cfg(test)]
mod tests;
