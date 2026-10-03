//! `initiative unarchive`: find the archived initiative, confirm, restore it.
use crate::cli::initiative::InitiativeUnarchive;
use crate::client::LinearClient;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::initiative::{
    ArchivedLookupVariables, GetInitiativeForUnarchive, UnarchiveDetail, UnarchiveInitiative,
    UnarchiveVariables,
};
use crate::refs::{self, initiative::Archived};

pub fn run(ctx: &Ctx, args: &InitiativeUnarchive) -> Result<()> {
    unarchive(ctx, args).context("Failed to unarchive initiative")
}

fn unarchive(ctx: &Ctx, args: &InitiativeUnarchive) -> Result<()> {
    if !args.force {
        ctx.require_tty("--force")?;
    }
    let original = &args.initiative_id;
    let reference = super::reference(ctx, original)?;
    let client = ctx.client()?;
    let detail = ctx.spin(true, async {
        let id = refs::initiative::resolve(client, &reference, Archived::Include).await?;
        details(client, &id, original).await
    })?;
    if detail.archived_at.is_none() {
        return ctx.print(format!("Initiative \"{}\" is not archived.\n", detail.name));
    }
    let question = format!("Are you sure you want to unarchive \"{}\"?", detail.name);
    if !args.force && !ctx.confirm(&question, "--force")? {
        return outcome::canceled(ctx);
    }
    let result: UnarchiveInitiative = ctx.spin(
        true,
        client.mutate(UnarchiveVariables {
            id: detail.id.inner().to_owned(),
        }),
    )?;
    if !result.initiative_unarchive.success {
        return Err(Error::new("Linear did not unarchive the initiative"));
    }
    let url = result.initiative_unarchive.entity.map(|entity| entity.url);
    let output = outcome::done("Unarchived", "initiative", &detail.name, url.as_deref());
    ctx.print(output)
}

async fn details(client: &LinearClient, id: &str, original: &str) -> Result<UnarchiveDetail> {
    let data: GetInitiativeForUnarchive = client
        .query(ArchivedLookupVariables {
            id: cynic::Id::new(id),
        })
        .await?;
    data.initiatives
        .nodes
        .into_iter()
        .next()
        .ok_or_else(|| Error::not_found("Initiative", original))
}
