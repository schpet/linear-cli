//! What `issue archive` and `issue delete` share: one issue or a bulk list,
//! confirmed unless `--yes` is given.
use crate::client::{LinearClient, RequestError};
use crate::graphql::operations::common::IdVariables;
use crate::{
    commands::bulk::{self, BulkInput, BulkResult, Found, Skipped, Verb},
    commands::outcome,
    commands::team_key::configured_team_key,
    ctx::Ctx,
    error::{Error, Result},
    graphql::operations::issue::*,
    refs::{self, WorkspaceScope},
};

pub struct Request<'a> {
    pub issue_id: Option<&'a str>,
    /// `--yes`: no prompt.
    pub yes: bool,
    pub bulk: BulkInput<'a>,
}

pub fn run(ctx: &Ctx, mode: Mode, request: &Request<'_>) -> Result<()> {
    if !request.yes {
        ctx.require_tty("for confirmation", "--yes")?;
    }
    if request.bulk.requested() {
        return run_bulk(ctx, mode, request);
    }
    let identifier = match (mode, request.issue_id) {
        (Mode::Delete, None) => {
            return Err(
                Error::invalid("Issue ID required").with_hint("Use --bulk for multiple issues.")
            );
        }
        (Mode::Archive, input) => super::require(ctx, input)?,
        (Mode::Delete, Some(input)) => {
            super::resolve(ctx, Some(input))?.ok_or_else(|| Error::not_found("Issue", input))?
        }
    };
    let client = ctx.client()?;
    let details = ctx.spin(true, single_details(client, &identifier))?;
    if mode == Mode::Archive && details.archived {
        return ctx.print(format!(
            "Issue \"{}\" is already archived.\n",
            details.name()
        ));
    }
    let question = format!("{} issue \"{}\"?", mode.title(), details.name());
    if !request.yes && !ctx.confirm(&question, "--yes")? {
        return outcome::canceled(ctx);
    }
    ctx.spin(true, submit_single(client, &identifier, mode))?;
    ctx.print(outcome::done(mode.done(), "issue", &details.name(), None))
}

fn run_bulk(ctx: &Ctx, mode: Mode, request: &Request<'_>) -> Result<()> {
    let ids = bulk::collect_ids(&request.bulk, &mut std::io::stdin().lock())?;
    if ids.is_empty() {
        return Err(Error::new(format!(
            "No issue identifiers provided for bulk {}",
            mode.verb()
        )));
    }
    let scope = ctx.scope()?;
    let team = configured_team_key(ctx.options());
    let targets: Vec<_> = ids
        .into_iter()
        .map(|id| Target::prepare(id, team.as_deref(), &scope))
        .collect();
    let client = ctx.client()?;
    let (found, missing) = bulk::look_up(ctx, targets, |target| look_up_item(client, target));
    let verb = Verb {
        present: mode.verb(),
        past: mode.past(),
    };
    ctx.eprint(bulk::preview(&found, &missing, "issue", verb))?;
    if found.is_empty() {
        return Err(bulk::none_found(&missing, "issues"));
    }
    let question = format!("{} {}?", mode.title(), bulk::count(found.len(), "issue"));
    if !request.yes && !ctx.confirm(&question, "--yes")? {
        return outcome::canceled(ctx);
    }
    let mut results = bulk::run(ctx, found, |found| apply_item(client, found, mode))?;
    results.extend(missing.into_iter().map(BulkResult::from));
    bulk::report(ctx, &results, "issue", verb)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Archive,
    Delete,
}
impl Mode {
    const fn verb(self) -> &'static str {
        match self {
            Self::Archive => "archive",
            Self::Delete => "delete",
        }
    }
    const fn past(self) -> &'static str {
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
    const fn done(self) -> &'static str {
        match self {
            Self::Archive => "Archived",
            Self::Delete => "Deleted",
        }
    }
}
enum ReferenceOutcome {
    Resolved(String),
    Unresolved,
    Failed(Error),
}
struct Target {
    original: String,
    reference: ReferenceOutcome,
}
impl Target {
    fn prepare(original: String, team: Option<&str>, scope: &WorkspaceScope<'_>) -> Self {
        let reference = match refs::prepare_issue_reference(&original, team, scope) {
            Ok(Some(id)) => ReferenceOutcome::Resolved(id),
            Ok(None) => ReferenceOutcome::Unresolved,
            Err(error) => ReferenceOutcome::Failed(error),
        };
        Self {
            original,
            reference,
        }
    }
}
struct Details {
    identifier: String,
    title: String,
    archived: bool,
}
impl Details {
    fn name(&self) -> String {
        format!("{}: {}", self.identifier, self.title)
    }
}
/// The issue's identifier, title and archive state; `None` when it does not exist.
async fn summary(client: &LinearClient, id: &str) -> Result<Option<Details>, Error> {
    let data: GetIssueSummary = match client.query(IdVariables { id: id.to_owned() }).await {
        Ok(data) => data,
        Err(failure) if failure.is_not_found() => return Ok(None),
        Err(failure) => return Err(failure.into()),
    };
    Ok(data.issue.map(|issue| Details {
        identifier: issue.identifier,
        title: issue.title,
        archived: issue.archived_at.is_some(),
    }))
}
async fn single_details(client: &LinearClient, id: &str) -> Result<Details, Error> {
    summary(client, id)
        .await?
        .ok_or_else(|| Error::not_found("Issue", id))
}
/// Archives or deletes issue `id`; `true` when Linear reports success.
async fn mutate(client: &LinearClient, id: &str, mode: Mode) -> Result<bool, RequestError> {
    let variables = IdVariables { id: id.to_owned() };
    Ok(match mode {
        Mode::Archive => {
            let data: ArchiveIssue = client.mutate(variables).await?;
            data.issue_archive.success
        }
        Mode::Delete => {
            let data: DeleteIssue = client.mutate(variables).await?;
            data.issue_delete.success
        }
    })
}
async fn submit_single(client: &LinearClient, id: &str, mode: Mode) -> Result<(), Error> {
    let success = mutate(client, id, mode)
        .await
        .map_err(|failure| failure.or_not_found("Issue", id))?;
    if !success {
        return Err(Error::new(format!(
            "Linear did not {} the issue",
            mode.verb()
        )));
    }
    Ok(())
}
/// A listed issue: its identifier, and whether it is already archived.
struct Listed {
    id: String,
    archived: bool,
}

async fn look_up_item(client: &LinearClient, target: Target) -> Result<Found<Listed>, Skipped> {
    let not_found = || Skipped::not_found(target.original.clone(), "Issue");
    let id = match target.reference {
        ReferenceOutcome::Resolved(id) => id,
        ReferenceOutcome::Unresolved => return Err(not_found()),
        ReferenceOutcome::Failed(error) => return Err(Skipped::failed(target.original, &error)),
    };
    match summary(client, &id).await {
        Ok(Some(details)) => Ok(Found {
            name: if details.archived {
                format!("{} (already archived)", details.name())
            } else {
                details.name()
            },
            original: target.original,
            item: Listed {
                id,
                archived: details.archived,
            },
        }),
        Ok(None) => Err(not_found()),
        Err(error) => Err(Skipped::failed(target.original, &error)),
    }
}

/// Archives or deletes one looked-up issue; archiving an archived issue
/// changes nothing and succeeds.
async fn apply_item(client: &LinearClient, found: Found<Listed>, mode: Mode) -> BulkResult {
    if mode == Mode::Archive && found.item.archived {
        return found.result(Ok(()));
    }
    let outcome = match mutate(client, &found.item.id, mode).await {
        Ok(true) => Ok(()),
        Ok(false) => Err(Error::new(format!("{} operation failed", mode.title()))),
        Err(failure) => Err(Error::from(failure)),
    };
    found.result(outcome)
}
