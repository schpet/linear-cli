//! `issue archive`/`delete`, single or bulk.
use crate::client::LinearClient;
use crate::{
    cli::issue::{IssueArchive, IssueDelete},
    commands::bulk::{self, BulkInput, BulkOutcome, BulkResult, Verb},
    commands::team_key::configured_team_key,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::{envelope::LegacyRequest, operations::issue_archive_delete::*},
    refs::{self, IssueReference, WorkspaceScope},
};
use cynic::{MutationBuilder, QueryBuilder};

pub fn archive(ctx: &Ctx, args: &IssueArchive) -> Result<()> {
    let request = Request {
        issue_id: args.issue_id.as_deref(),
        confirmed: args.confirm,
        bulk: BulkInput {
            argv: args.bulk.as_deref(),
            file: args.bulk_file.as_deref().map(std::path::Path::new),
            stdin: args.bulk_stdin,
        },
    };
    run(ctx, Mode::Archive, &request).context("Failed to archive issue")
}

pub fn delete(ctx: &Ctx, args: &IssueDelete) -> Result<()> {
    let request = Request {
        issue_id: args.issue_id.as_deref(),
        confirmed: args.confirm,
        bulk: BulkInput {
            argv: args.bulk.as_deref(),
            file: args.bulk_file.as_deref().map(std::path::Path::new),
            stdin: args.bulk_stdin,
        },
    };
    run(ctx, Mode::Delete, &request).context("Failed to delete issue")
}

struct Request<'a> {
    issue_id: Option<&'a str>,
    /// `--confirm`: no prompt.
    confirmed: bool,
    bulk: BulkInput<'a>,
}

fn run(ctx: &Ctx, mode: Mode, request: &Request<'_>) -> Result<()> {
    if !request.confirmed {
        ctx.require_tty("--confirm")?;
    }
    if request.bulk.requested() {
        return run_bulk(ctx, mode, request);
    }
    let identifier = match (mode, request.issue_id) {
        (Mode::Delete, None) => {
            return Err(
                Error::new("Issue ID required").with_hint("Use --bulk for multiple issues.")
            );
        }
        (Mode::Archive, input) => super::require(ctx, input)?,
        (Mode::Delete, Some(input)) => {
            super::resolve(ctx, Some(input))?.ok_or_else(|| Error::not_found("Issue", input))?
        }
    };
    let client = ctx.client()?;
    let details = ctx.spin(true, single_details(client, &identifier, mode))?;
    if details.already_archived {
        return ctx.print(format!(
            "Issue \"{}\" is already archived.\n",
            details.name()
        ));
    }
    let question = format!(
        "Are you sure you want to {} \"{}\"?",
        mode.verb(),
        details.name()
    );
    if !request.confirmed && !ctx.confirm(&question, "--confirm")? {
        return ctx.print(format!("{} cancelled.\n", mode.title()));
    }
    ctx.print(ctx.spin(true, submit_single(client, &identifier, &details, mode))?)
}

fn run_bulk(ctx: &Ctx, mode: Mode, request: &Request<'_>) -> Result<()> {
    let ids = bulk::collect_ids(&request.bulk, &mut std::io::stdin().lock())?;
    if ids.is_empty() {
        return Err(Error::new(format!(
            "No issue identifiers provided for bulk {}",
            mode.verb()
        )));
    }
    ctx.print(format!(
        "Found {} issue(s) to {}.\n",
        ids.len(),
        mode.verb()
    ))?;
    let question = format!("{} {} issue(s)?", mode.title(), ids.len());
    if !request.confirmed && !ctx.confirm(&question, "--confirm")? {
        return ctx.print(format!("Bulk {} cancelled.\n", mode.verb()));
    }
    let scope = ctx.scope()?;
    let team = configured_team_key(ctx.options());
    let targets: Vec<_> = ids
        .into_iter()
        .map(|id| Target::prepare(id, team.as_deref(), &scope))
        .collect();
    let client = ctx.client()?;
    let results = bulk::run(ctx, targets, |target| run_item(client, target, mode))?;
    bulk::report(
        ctx,
        &results,
        "issue",
        Verb {
            present: mode.verb(),
            past: mode.past(),
        },
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Archive,
    Delete,
}
impl Mode {
    pub const fn verb(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Delete => "delete",
        }
    }
    pub const fn past(self) -> &'static str {
        match self {
            Self::Archive => "archived",
            Self::Delete => "deleted",
        }
    }
    const fn title(self) -> &'static str {
        match self {
            Self::Archive => "Archive",
            Self::Delete => "Delete",
        }
    }
}
pub enum ReferenceOutcome {
    Resolved(String),
    Unresolved,
    Failed(Error),
}
pub struct Target {
    pub original: String,
    pub reference: ReferenceOutcome,
}
impl Target {
    pub fn prepare(original: String, team: Option<&str>, scope: &WorkspaceScope<'_>) -> Self {
        let reference = match refs::prepare_issue_reference(Some(&original), team, scope) {
            Ok(IssueReference::Identifier(id)) => ReferenceOutcome::Resolved(id),
            Ok(IssueReference::Unresolved) => ReferenceOutcome::Unresolved,
            Ok(IssueReference::Inferred) => ReferenceOutcome::Failed(Error::new(
                "explicit bulk issue reference requested inference",
            )),
            Err(error) => ReferenceOutcome::Failed(error),
        };
        Self {
            original,
            reference,
        }
    }
}
pub struct Details {
    pub identifier: String,
    pub title: String,
    pub already_archived: bool,
}
impl Details {
    pub fn name(&self) -> String {
        format!("{}: {}", self.identifier, self.title)
    }
}
pub fn details_request(id: &str, mode: Mode, bulk: bool) -> LegacyRequest<IdVariables> {
    let variables = IdVariables { id: id.to_owned() };
    match (mode, bulk) {
        (Mode::Archive, false) => {
            LegacyRequest::with_variables(GetIssueArchiveDetails::build(variables))
        }
        (Mode::Archive, true) => {
            LegacyRequest::with_variables(GetIssueDetailsForBulkArchive::build(variables))
        }
        (Mode::Delete, false) => {
            LegacyRequest::with_variables(GetIssueDeleteDetails::build(variables))
        }
        (Mode::Delete, true) => {
            LegacyRequest::with_variables(GetIssueDetailsForBulkDelete::build(variables))
        }
    }
}
pub fn mutation_request(id: &str, mode: Mode, bulk: bool) -> LegacyRequest<IdVariables> {
    let variables = IdVariables { id: id.to_owned() };
    match (mode, bulk) {
        (Mode::Archive, false) => LegacyRequest::with_variables(ArchiveIssue::build(variables)),
        (Mode::Archive, true) => LegacyRequest::with_variables(BulkArchiveIssue::build(variables)),
        (Mode::Delete, false) => LegacyRequest::with_variables(DeleteIssue::build(variables)),
        (Mode::Delete, true) => LegacyRequest::with_variables(BulkDeleteIssue::build(variables)),
    }
}
/// A request whose "not found" answer means issue `id` does not exist.
async fn exchange<T: serde::de::DeserializeOwned>(
    client: &LinearClient,
    request: &LegacyRequest<IdVariables>,
    id: &str,
) -> Result<T, Error> {
    client
        .execute_legacy(request)
        .await
        .map_err(|failure| failure.or_not_found("Issue", id))
}
pub async fn single_details(client: &LinearClient, id: &str, mode: Mode) -> Result<Details, Error> {
    let request = details_request(id, mode, false);
    let details = match mode {
        Mode::Archive => {
            let data: GetIssueArchiveDetails = exchange(client, &request, id).await?;
            data.issue.map(|issue| Details {
                identifier: issue.identifier,
                title: issue.title,
                already_archived: issue.archived_at.is_some(),
            })
        }
        Mode::Delete => {
            let data: GetIssueDeleteDetails = exchange(client, &request, id).await?;
            data.issue.map(|issue| Details {
                identifier: issue.identifier,
                title: issue.title,
                already_archived: false,
            })
        }
    };
    details.ok_or_else(|| Error::not_found("Issue", id))
}
pub async fn submit_single(
    client: &LinearClient,
    id: &str,
    details: &Details,
    mode: Mode,
) -> Result<Vec<u8>, Error> {
    let request = mutation_request(id, mode, false);
    let success = match mode {
        Mode::Archive => {
            let data: ArchiveIssue = exchange(client, &request, id).await?;
            data.issue_archive.success
        }
        Mode::Delete => {
            let data: DeleteIssue = exchange(client, &request, id).await?;
            data.issue_delete.success
        }
    };
    if !success {
        return Err(Error::new(match mode {
            Mode::Archive => "Linear reported the archive as unsuccessful",
            Mode::Delete => "Failed to delete issue",
        }));
    }
    Ok(format!("✓ Successfully {} issue: {}\n", mode.past(), details.name()).into_bytes())
}
async fn bulk_resolved(client: &LinearClient, id: &str, mode: Mode) -> Result<BulkResult, Error> {
    let request = details_request(id, mode, true);
    let not_found = || BulkResult {
        id: id.to_owned(),
        name: None,
        outcome: BulkOutcome::Failed("Issue not found".to_owned()),
    };
    let (name, already_archived) = match mode {
        Mode::Archive => {
            let data: GetIssueDetailsForBulkArchive = match client.execute_legacy(&request).await {
                Ok(data) => data,
                Err(failure) if failure.is_not_found() => return Ok(not_found()),
                Err(failure) => return Err(failure.into()),
            };
            let Some(issue) = data.issue else {
                return Ok(not_found());
            };
            (
                format!("{}: {}", issue.identifier, issue.title),
                issue.archived_at.is_some(),
            )
        }
        Mode::Delete => {
            // A failed details lookup only loses the title in the summary; the
            // delete still runs.
            let data: Result<GetIssueDetailsForBulkDelete, _> =
                client.execute_legacy(&request).await;
            let issue = match data {
                Ok(data) => data.issue,
                Err(_) => None,
            };
            let name = issue.map_or_else(
                || id.to_owned(),
                |issue| {
                    if issue.title.is_empty() {
                        issue.identifier
                    } else {
                        format!("{}: {}", issue.identifier, issue.title)
                    }
                },
            );
            (name, false)
        }
    };
    let success = if already_archived {
        true
    } else {
        let request = mutation_request(id, mode, true);
        match mode {
            Mode::Archive => {
                let data: BulkArchiveIssue = client.execute_legacy(&request).await?;
                data.issue_archive.success
            }
            Mode::Delete => {
                let data: BulkDeleteIssue = client.execute_legacy(&request).await?;
                data.issue_delete.success
            }
        }
    };
    Ok(BulkResult {
        id: id.to_owned(),
        name: Some(name),
        outcome: if success {
            BulkOutcome::Succeeded
        } else {
            BulkOutcome::Failed(format!(
                "{} operation failed",
                match mode {
                    Mode::Archive => "Archive",
                    Mode::Delete => "Delete",
                }
            ))
        },
    })
}
pub async fn run_item(client: &LinearClient, target: Target, mode: Mode) -> BulkResult {
    let result = match target.reference {
        ReferenceOutcome::Resolved(id) => bulk_resolved(client, &id, mode).await,
        ReferenceOutcome::Unresolved => Ok(BulkResult {
            id: target.original.clone(),
            name: None,
            outcome: BulkOutcome::Failed("Issue not found".to_owned()),
        }),
        // Bulk rows show only the error message, not its suggestion or context.
        ReferenceOutcome::Failed(error) => Err(error),
    };
    result.unwrap_or_else(|error| BulkResult {
        id: target.original,
        name: None,
        outcome: BulkOutcome::Failed(error.message().to_owned()),
    })
}
