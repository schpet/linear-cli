//! What `initiative archive` and `initiative delete` share: one initiative or
//! many, confirmation, and the summary.
use crate::client::LinearClient;
use crate::commands::bulk::{self, BulkInput, BulkResult, Found, Skipped, Verb};
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::initiative::{
    ArchiveInitiative, DeleteInitiative, GetInitiativeForArchive, GetInitiativeForDelete,
};
use crate::platform::prompt::Text;
use crate::refs::{
    self,
    initiative::{Archived, InitiativeReference},
};

pub struct Request<'a> {
    pub initiative: Option<&'a str>,
    /// `--yes`: no prompt.
    pub yes: bool,
    pub bulk: BulkInput<'a>,
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

    /// Archived initiatives can be deleted but not archived again.
    const fn archived(self) -> Archived {
        match self {
            Self::Archive => Archived::Exclude,
            Self::Delete => Archived::Include,
        }
    }
}

const PERMANENT: &str = "\n⚠️  This action is PERMANENT and cannot be undone.\n\n";

pub fn run(ctx: &Ctx, mode: Mode, request: &Request<'_>) -> Result<()> {
    if !request.yes {
        ctx.require_tty("for confirmation", "--yes")?;
    }
    if request.bulk.requested() {
        return run_bulk(ctx, mode, request);
    }
    let original = request.initiative.ok_or_else(|| {
        Error::invalid("Initiative ID required").with_hint("Use --bulk for multiple initiatives.")
    })?;
    let reference = super::common::reference(ctx, original)?;
    let client = ctx.client()?;
    let details = ctx.spin(true, async {
        let id = refs::initiative::resolve(client, &reference, mode.archived()).await?;
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
        ctx.eprint(format!(
            "\n⚠️  Initiative \"{}\" has {} linked project(s).\nDeleting the initiative will unlink these projects.\n\n",
            details.name, details.linked_projects
        ))?;
    }
    if !request.yes && !confirm_single(ctx, mode, &details.name)? {
        return outcome::canceled(ctx);
    }
    ctx.spin(true, submit(client, &details.id, mode))?;
    let done = match mode {
        Mode::Archive => "Archived",
        Mode::Delete => "Permanently deleted",
    };
    ctx.print(outcome::done(done, "initiative", &details.name, None))
}

/// Archiving asks once; deleting also asks for the initiative's name.
fn confirm_single(ctx: &Ctx, mode: Mode, name: &str) -> Result<bool> {
    let question = match mode {
        Mode::Archive => format!("Archive initiative \"{name}\"?"),
        Mode::Delete => {
            ctx.eprint(PERMANENT)?;
            format!("Permanently delete initiative \"{name}\"?")
        }
    };
    if !ctx.confirm(&question, "--yes")? {
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
        ctx.eprint("Name does not match.\n")?;
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
    let scope = ctx.scope()?;
    let targets: Vec<_> = ids
        .into_iter()
        .map(|original| {
            let reference = InitiativeReference::parse(&original, &scope);
            (original, reference)
        })
        .collect();
    let client = ctx.client()?;
    let (found, missing) = bulk::look_up(ctx, targets, |(original, reference)| {
        look_up_item(client, original, reference, mode)
    });
    let verb = Verb {
        present: mode.verb(),
        past: mode.past(),
    };
    ctx.eprint(bulk::preview(&found, &missing, "initiative", verb))?;
    if found.is_empty() {
        return Err(Error::new("None of the listed initiatives could be found"));
    }
    if mode == Mode::Delete {
        ctx.eprint(PERMANENT)?;
    }
    let count = bulk::count(found.len(), "initiative");
    let question = match mode {
        Mode::Archive => format!("Archive {count}?"),
        Mode::Delete => format!("Permanently delete {count}?"),
    };
    if !request.yes && !ctx.confirm(&question, "--yes")? {
        return outcome::canceled(ctx);
    }
    let mut results = bulk::run(ctx, found, |found| async move {
        let outcome = if found.item.already_archived {
            Ok(())
        } else {
            submit(client, &found.item.id, mode).await
        };
        found.result(outcome)
    })?;
    results.extend(missing.into_iter().map(BulkResult::from));
    bulk::report(ctx, &results, "initiative", verb)
}

/// Looks up one listed initiative. Failures, including an unparseable
/// reference, become the reason it is skipped.
async fn look_up_item(
    client: &LinearClient,
    original: String,
    reference: Result<InitiativeReference>,
    mode: Mode,
) -> std::result::Result<Found<Details>, Skipped> {
    let looked_up = async {
        let id = refs::initiative::resolve(client, &reference?, mode.archived()).await?;
        details(client, &id, mode).await
    };
    match looked_up.await {
        Ok(Some(details)) => Ok(Found {
            original,
            name: if details.already_archived {
                format!("{} (already archived)", details.name)
            } else {
                details.name.clone()
            },
            item: details,
        }),
        Ok(None) => Err(Skipped {
            original,
            reason: "Initiative not found".to_owned(),
        }),
        Err(error) => Err(Skipped {
            original,
            reason: error.message().to_owned(),
        }),
    }
}

struct Details {
    id: String,
    name: String,
    already_archived: bool,
    linked_projects: usize,
}

async fn details(client: &LinearClient, id: &str, mode: Mode) -> Result<Option<Details>> {
    let variables = IdVariables { id: id.to_owned() };
    Ok(match mode {
        Mode::Archive => {
            let data: GetInitiativeForArchive = client.query(variables).await?;
            data.initiative.map(|initiative| Details {
                id: initiative.id.into_inner(),
                name: initiative.name,
                already_archived: initiative.archived_at.is_some(),
                linked_projects: 0,
            })
        }
        Mode::Delete => {
            let data: GetInitiativeForDelete = client.query(variables).await?;
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

async fn submit(client: &LinearClient, id: &str, mode: Mode) -> Result<()> {
    let variables = IdVariables { id: id.to_owned() };
    let success = match mode {
        Mode::Archive => {
            let data: ArchiveInitiative = client.mutate(variables).await?;
            data.initiative_archive.success
        }
        Mode::Delete => {
            let data: DeleteInitiative = client.mutate(variables).await?;
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
