//! `label delete`: find the label by UUID or name, ask which one when names
//! repeat, confirm, delete it.
use crate::cli::label::LabelDelete;
use crate::client::LinearClient;
use crate::commands::confirm;
use crate::commands::outcome;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::common::NameVariables;
use crate::graphql::operations::label::{DeleteIssueLabel, GetLabelById, GetLabelByName, Label};
use crate::platform::prompt::Choice;
use crate::refs::{self, is_linear_uuid, reject_linear_url, team::TeamReference};

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
        .map(|team| TeamReference::parse(&team, &ctx.scope()?))
        .transpose()?;
    if !args.confirm.yes {
        ctx.require_tty("for confirmation", "--yes")?;
    }
    let client = ctx.client()?;
    let (labels, team_key) = ctx.spin(true, async {
        if by_id {
            return Ok::<_, Error>((vec![by_uuid(client, reference).await?], None));
        }
        let team_key = match &team {
            Some(lookup) => Some(refs::team::resolve(client, lookup).await?.key),
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
    if !confirm::proceed(ctx, args.confirm.yes, &question)? {
        return Ok(());
    }
    let result: DeleteIssueLabel = ctx.spin(
        true,
        client.mutate(IdVariables {
            id: label.id.inner().to_owned(),
        }),
    )?;
    if !result.issue_label_delete.success {
        return Err(Error::new("Linear did not delete the label"));
    }
    ctx.print(outcome::done("Deleted", "label", &display(&label), None))
}

async fn by_uuid(client: &LinearClient, id: &str) -> Result<Label> {
    match client
        .query::<GetLabelById, _>(IdVariables { id: id.to_owned() })
        .await
    {
        Ok(data) => Ok(data.issue_label),
        Err(failure) => Err(failure.or_not_found("Label", id)),
    }
}

/// Labels whose name matches, ignoring case, in server order.
async fn by_name(client: &LinearClient, name: &str) -> Result<Vec<Label>> {
    let data: GetLabelByName = client
        .query(NameVariables {
            name: name.to_owned(),
        })
        .await?;
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
    ctx.eprint(format!("Several labels are named \"{name}\".\n"))?;
    let label = ctx.prompter()?.select("Label:", choices)?;
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
