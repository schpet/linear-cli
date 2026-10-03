//! `label delete`: find the label by UUID or name, ask which one when names
//! repeat, confirm, delete it.
use cynic::{MutationBuilder, QueryBuilder};

use crate::cli::label::LabelDelete;
use crate::client::LinearClient;
use crate::commands::confirm;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::label_delete::{
    DeleteIssueLabel, GetLabelById, GetLabelByName, IdVariables, Label, NameVariables,
};
use crate::platform::prompt::Choice;
use crate::refs::{
    is_linear_uuid, prepare_team_lookup, reject_linear_url, resolve_team_with_transport,
};

pub fn run(ctx: &Ctx, args: &LabelDelete) -> Result<()> {
    delete(ctx, args).context("Failed to delete label")
}

fn delete(ctx: &Ctx, args: &LabelDelete) -> Result<()> {
    let reference = &args.name_or_id;
    reject_linear_url(reference, "a label name or UUID")?;
    let by_id = is_linear_uuid(reference);
    // A name is looked up in --team (or the configured team) and the workspace.
    let team = match &args.team {
        Some(team) => Some(team.clone()),
        None if by_id => None,
        None => configured_team_key(ctx.options()),
    };
    let team = team
        .map(|team| prepare_team_lookup(&team, &ctx.scope()?))
        .transpose()?;
    if !args.force {
        ctx.require_tty("--force")?;
    }
    let client = ctx.client()?;
    let (labels, team_key) = ctx.spin(true, async {
        if by_id {
            return Ok::<_, Error>((vec![by_uuid(client, reference).await?], None));
        }
        let team_key = match &team {
            Some(lookup) => Some(resolve_team_with_transport(lookup, client).await?.key),
            None => None,
        };
        let labels = by_name(client, reference).await?;
        Ok((scoped(labels, team_key.as_deref()), team_key))
    })?;
    let label = match labels.as_slice() {
        [] => {
            let error = Error::not_found("Label", reference);
            return Err(match team_key {
                Some(key) => error.with_hint(format!("Searched in team {key} and the workspace.")),
                None => error,
            });
        }
        [label] => label.clone(),
        _ => choose(ctx, reference, &labels)?,
    };
    let question = format!(
        "Are you sure you want to delete label \"{}\"?",
        display(&label)
    );
    if !confirm::deletion(ctx, args.force, &question)? {
        return Ok(());
    }
    let request = GraphQlRequest::with_variables(DeleteIssueLabel::build(IdVariables {
        id: label.id.inner().to_owned(),
    }));
    let result: DeleteIssueLabel = ctx.spin(true, client.execute(&request))?;
    if !result.issue_label_delete.success {
        return Err(Error::new("Linear did not delete the label"));
    }
    ctx.print(format!("✓ Deleted label: {}\n", display(&label)))
}

async fn by_uuid(client: &LinearClient, id: &str) -> Result<Label> {
    let request =
        GraphQlRequest::with_variables(GetLabelById::build(IdVariables { id: id.to_owned() }));
    match client.execute::<GetLabelById, _>(&request).await {
        Ok(data) => Ok(data.issue_label),
        Err(failure) => Err(failure.or_not_found("Label", id)),
    }
}

/// Labels whose name matches, ignoring case, in server order.
async fn by_name(client: &LinearClient, name: &str) -> Result<Vec<Label>> {
    let request = GraphQlRequest::with_variables(GetLabelByName::build(NameVariables {
        name: name.to_owned(),
    }));
    let data: GetLabelByName = client.execute(&request).await?;
    Ok(data.issue_labels.nodes)
}

/// With a team, its labels win; workspace labels are the fallback.
fn scoped(labels: Vec<Label>, team_key: Option<&str>) -> Vec<Label> {
    let Some(key) = team_key else {
        return labels;
    };
    let (team, rest): (Vec<Label>, Vec<Label>) = labels
        .into_iter()
        .partition(|label| label.team.as_ref().is_some_and(|team| team.key == key));
    if team.is_empty() {
        rest.into_iter()
            .filter(|label| label.team.is_none())
            .collect()
    } else {
        team
    }
}

/// Asks which of several same-named labels to delete.
fn choose(ctx: &Ctx, name: &str, labels: &[Label]) -> Result<Label> {
    if !ctx.interactive() {
        return Err(
            Error::new(format!("Multiple labels named \"{name}\" found"))
                .with_hint("Pass --team to pick one, or delete it by UUID."),
        );
    }
    let choices = labels
        .iter()
        .map(|label| Choice::new(format!("{} - {}", display(label), label.color), label))
        .collect();
    let label = ctx.prompter()?.select(
        &format!("Multiple labels named \"{name}\" found. Which one?"),
        choices,
    )?;
    Ok(label.clone())
}

fn display(label: &Label) -> String {
    let team = label
        .team
        .as_ref()
        .map(|team| team.key.as_str())
        .filter(|key| !key.is_empty())
        .unwrap_or("Workspace");
    format!("{} ({team})", label.name)
}
