//! `initiative archive`/`delete`, for one initiative or in bulk.
use cynic::{MutationBuilder, QueryBuilder};
use futures_util::{StreamExt, stream};

use crate::cli::initiative::{InitiativeArchive, InitiativeDelete};
use crate::commands::bulk::{self, BulkInput, BulkOutcome, BulkResult, Progress};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiative_bulk::{
    ArchiveInitiative, DeleteInitiative, GetInitiativeForArchive, GetInitiativeForDelete,
    IdVariables,
};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::prompt::Text;
use crate::refs::InitiativeReference;

use super::Archived;

pub fn archive(ctx: &Ctx, args: &InitiativeArchive) -> Result<()> {
    let request = Request {
        initiative: args.initiative_id.as_deref(),
        force: args.force,
        bulk: BulkInput {
            argv: args.bulk.as_deref(),
            file: args.bulk_file.as_deref().map(std::path::Path::new),
            stdin: args.bulk_stdin,
        },
    };
    run(ctx, Mode::Archive, &request).context("Failed to archive initiative")
}

pub fn delete(ctx: &Ctx, args: &InitiativeDelete) -> Result<()> {
    let request = Request {
        initiative: args.initiative_id.as_deref(),
        force: args.force,
        bulk: BulkInput {
            argv: args.bulk.as_deref(),
            file: args.bulk_file.as_deref().map(std::path::Path::new),
            stdin: args.bulk_stdin,
        },
    };
    run(ctx, Mode::Delete, &request).context("Failed to delete initiative")
}

struct Request<'a> {
    initiative: Option<&'a str>,
    /// `--force`: no prompt.
    force: bool,
    bulk: BulkInput<'a>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Mode {
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

    /// Archived initiatives can be deleted but not archived again.
    const fn archived(self) -> Archived {
        match self {
            Self::Archive => Archived::Exclude,
            Self::Delete => Archived::Include,
        }
    }
}

const PERMANENT: &str = "\n⚠️  This action is PERMANENT and cannot be undone.\n\n";

fn run(ctx: &Ctx, mode: Mode, request: &Request<'_>) -> Result<()> {
    if !request.force {
        ctx.require_tty("--force")?;
    }
    if request.bulk.requested() {
        return run_bulk(ctx, mode, request);
    }
    let original = request.initiative.ok_or_else(|| {
        Error::new("Initiative ID required").with_hint("Use --bulk for multiple initiatives.")
    })?;
    let reference = super::reference(ctx, original)?;
    let client = ctx.client()?;
    let details = ctx.spin(true, async {
        let id = super::resolve(client, &reference, original, mode.archived()).await?;
        details(client, &id, mode)
            .await?
            .ok_or_else(|| Error::not_found("Initiative", original))
    })?;
    if details.already_archived {
        return ctx.print(format!(
            "Initiative \"{}\" is already archived.\n",
            details.name
        ));
    }
    if details.linked_projects > 0 {
        ctx.print(format!(
            "\n⚠️  Initiative \"{}\" has {} linked project(s).\nDeleting the initiative will unlink these projects.\n\n",
            details.name, details.linked_projects
        ))?;
    }
    if !request.force && !confirm_single(ctx, mode, &details.name)? {
        return ctx.print(format!("{} cancelled.\n", mode.title()));
    }
    ctx.spin(true, submit(client, &details.id, mode))?;
    let done = match mode {
        Mode::Archive => "Archived",
        Mode::Delete => "Permanently deleted",
    };
    ctx.print(format!("✓ {done} initiative: {}\n", details.name))
}

/// Archiving asks once; deleting also asks for the initiative's name.
fn confirm_single(ctx: &Ctx, mode: Mode, name: &str) -> Result<bool> {
    let question = match mode {
        Mode::Archive => format!("Archive initiative \"{name}\"?"),
        Mode::Delete => {
            ctx.print(PERMANENT)?;
            format!("Are you sure you want to permanently delete \"{name}\"?")
        }
    };
    if !ctx.confirm(&question, "--force")? {
        return Ok(false);
    }
    if mode == Mode::Archive {
        return Ok(true);
    }
    let answer = ctx
        .prompter()?
        .text(Text::new("Type the initiative name to confirm deletion:"))?;
    if answer == name.trim() {
        Ok(true)
    } else {
        ctx.print("Name does not match.\n")?;
        Ok(false)
    }
}

fn run_bulk(ctx: &Ctx, mode: Mode, request: &Request<'_>) -> Result<()> {
    let ids = bulk::collect_ids(&request.bulk, &mut std::io::stdin().lock())?;
    if ids.is_empty() {
        return Err(Error::new(format!(
            "No initiative IDs provided for bulk {}",
            mode.verb()
        )));
    }
    ctx.print(format!(
        "Found {} initiative(s) to {}.\n",
        ids.len(),
        mode.verb()
    ))?;
    if mode == Mode::Delete {
        ctx.print(PERMANENT)?;
    }
    let question = match mode {
        Mode::Archive => format!("Archive {} initiative(s)?", ids.len()),
        Mode::Delete => format!("Permanently delete {} initiative(s)?", ids.len()),
    };
    if !request.force && !ctx.confirm(&question, "--force")? {
        return ctx.print(format!("Bulk {} cancelled.\n", mode.verb()));
    }
    let scope = ctx.scope()?;
    let targets: Vec<_> = ids
        .into_iter()
        .map(|original| {
            let reference = crate::refs::prepare_initiative_lookup(&original, &scope);
            (original, reference)
        })
        .collect();
    let client = ctx.client()?;
    let show_progress = ctx.terminal().stderr_tty;
    let total = targets.len();
    let results = ctx.block_on(async {
        let mut rows = stream::iter(targets)
            .map(|(original, reference)| run_item(client, original, reference, mode))
            .buffered(5);
        let mut results: Vec<BulkResult> = Vec::with_capacity(total);
        while let Some(row) = rows.next().await {
            results.push(row);
            if show_progress {
                let progress = Progress {
                    completed: results.len(),
                    total,
                    succeeded: results.iter().filter(|row| row.succeeded()).count(),
                };
                ctx.eprint(progress.render())?;
            }
        }
        Ok::<_, Error>(results)
    })?;
    if show_progress {
        ctx.eprint(bulk::PROGRESS_CLEAR)?;
    }
    let (output, failed) = summary(&results, mode);
    ctx.print(output)?;
    if failed {
        return Err(Error::reported());
    }
    Ok(())
}

/// One bulk row. Failures, including an unparseable reference, become the
/// row's message rather than stopping the other items.
async fn run_item(
    client: &GraphQlTransport,
    original: String,
    reference: Result<InitiativeReference>,
    mode: Mode,
) -> BulkResult {
    let row = async {
        let id = super::resolve(client, &reference?, &original, mode.archived()).await?;
        let Some(details) = details(client, &id, mode).await? else {
            return Ok(BulkResult {
                id: original.clone(),
                name: None,
                outcome: BulkOutcome::Failed("Initiative not found".to_owned()),
            });
        };
        if !details.already_archived {
            submit(client, &details.id, mode).await?;
        }
        Ok::<_, Error>(BulkResult {
            id: details.id,
            name: Some(details.name),
            outcome: BulkOutcome::Succeeded,
        })
    };
    row.await.unwrap_or_else(|error| BulkResult {
        id: original.clone(),
        name: None,
        outcome: BulkOutcome::Failed(error.message().to_owned()),
    })
}

struct Details {
    id: String,
    name: String,
    already_archived: bool,
    linked_projects: usize,
}

async fn details(client: &GraphQlTransport, id: &str, mode: Mode) -> Result<Option<Details>> {
    let variables = IdVariables { id: id.to_owned() };
    Ok(match mode {
        Mode::Archive => {
            let request = GraphQlRequest::with_variables(GetInitiativeForArchive::build(variables));
            let data: GetInitiativeForArchive = client.execute(&request).await?;
            data.initiative.map(|initiative| Details {
                id: initiative.id.into_inner(),
                name: initiative.name,
                already_archived: initiative.archived_at.is_some(),
                linked_projects: 0,
            })
        }
        Mode::Delete => {
            let request = GraphQlRequest::with_variables(GetInitiativeForDelete::build(variables));
            let data: GetInitiativeForDelete = client.execute(&request).await?;
            data.initiative.map(|initiative| Details {
                id: initiative.id.into_inner(),
                name: initiative.name,
                already_archived: false,
                linked_projects: initiative
                    .projects
                    .map_or(0, |projects| projects.nodes.len()),
            })
        }
    })
}

async fn submit(client: &GraphQlTransport, id: &str, mode: Mode) -> Result<()> {
    let variables = IdVariables { id: id.to_owned() };
    let success = match mode {
        Mode::Archive => {
            let request = GraphQlRequest::with_variables(ArchiveInitiative::build(variables));
            let data: ArchiveInitiative = client.execute(&request).await?;
            data.initiative_archive.success
        }
        Mode::Delete => {
            let request = GraphQlRequest::with_variables(DeleteInitiative::build(variables));
            let data: DeleteInitiative = client.execute(&request).await?;
            data.initiative_delete.success
        }
    };
    if !success {
        return Err(Error::new(format!(
            "Linear did not {} the initiative",
            mode.verb()
        )));
    }
    Ok(())
}

fn summary(results: &[BulkResult], mode: Mode) -> (String, bool) {
    let total = results.len();
    let succeeded = results.iter().filter(|result| result.succeeded()).count();
    let failed = total - succeeded;
    let plural = |count: usize| if count == 1 { "" } else { "s" };
    let mut output = String::from("\n");
    if failed == 0 {
        output.push_str(&format!(
            "✓ Successfully {} {succeeded} initiative{}\n",
            mode.past(),
            plural(succeeded)
        ));
        return (output, false);
    }
    if succeeded == 0 {
        output.push_str(&format!(
            "✗ Failed to {} all {total} initiative{}\n",
            mode.verb(),
            plural(total)
        ));
    } else {
        output.push_str(&format!(
            "Completed: {succeeded}/{total} initiative{} {}\n  ✓ Succeeded: {succeeded}\n  ✗ Failed: {failed}\n",
            plural(total),
            mode.past()
        ));
    }
    output.push_str("\nFailed operations:\n");
    for result in results {
        if let BulkOutcome::Failed(error) = &result.outcome {
            let name = result
                .name
                .as_deref()
                .filter(|name| !name.is_empty())
                .map_or_else(String::new, |name| format!(" ({name})"));
            output.push_str(&format!("  - {}{name}: {error}\n", result.id));
        }
    }
    (output, true)
}
