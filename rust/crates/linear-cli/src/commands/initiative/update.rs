//! `initiative update`: fields from flags or prompts, then one mutation.
use chrono::NaiveDate;

use crate::cli::initiative::InitiativeUpdate;
use crate::cli::values::{date, hex_color};
use crate::client::LinearClient;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::initiative_update::{
    CurrentInitiative, GetInitiativeForUpdate, InitiativeUpdateInput, UpdateInitiative,
    UpdateVariables, UpdatedInitiative,
};
use crate::graphql::operations::initiative_view::DetailVariables;
use crate::graphql::operations::initiatives::InitiativeStatus;
use crate::graphql::scalars::TimelessDate;
use crate::platform::prompt::{Choice, Prompter, Text};

pub fn run(ctx: &Ctx, args: &InitiativeUpdate) -> Result<()> {
    update(ctx, args).context("Failed to update initiative")
}

fn update(ctx: &Ctx, args: &InitiativeUpdate) -> Result<()> {
    let original = &args.initiative_id;
    let reference = super::reference(ctx, original)?;
    let flags = Changes {
        name: args.name.clone(),
        description: args.description.clone(),
        status: args.status.map(Into::into),
        owner: args.owner.clone(),
        target_date: args.target_date,
        color: args.color.clone(),
        icon: args.icon.clone(),
    };
    super::check_owner(flags.owner.as_deref())?;
    let prompting = flags.is_empty();
    if prompting && !args.interactive {
        return Err(Error::new("No changes specified").with_hint(
            "Pass the fields to change, such as --name or --status, or -i to be prompted.",
        ));
    }
    if prompting && !ctx.interactive() {
        return Err(Error::new("Interactive mode needs a terminal")
            .with_hint("Pass the fields to change, such as --name or --status."));
    }
    let client = ctx.client()?;
    let (id, changes) = if prompting {
        let (id, current) = ctx.spin(true, async {
            let id = super::resolve(client, &reference, original, super::Archived::Exclude).await?;
            let current = details(client, &id, original).await?;
            Ok::<_, Error>((id, current))
        })?;
        ctx.print(format!("\nUpdating initiative: {}\n\n", current.name))?;
        let changes = prompt(&ctx.prompter()?, &current)?;
        if changes.is_empty() {
            return ctx.print("No changes specified\n");
        }
        (id, changes)
    } else {
        let id = ctx.spin(
            true,
            super::resolve(client, &reference, original, super::Archived::Exclude),
        )?;
        (id, flags)
    };
    let updated = ctx.spin(true, async {
        let owner_id = match &changes.owner {
            Some(owner) => Some(crate::commands::user::resolve(client, owner, "Owner").await?),
            None => None,
        };
        submit(client, &id, changes.into_input(owner_id)).await
    })?;
    let mut output = format!("✓ Updated initiative: {}\n", updated.name);
    if !updated.url.is_empty() {
        output.push_str(&format!("{}\n", updated.url));
    }
    ctx.print(output)
}

/// The fields to change; `None` leaves a field as it is.
#[derive(Default)]
struct Changes {
    name: Option<String>,
    description: Option<String>,
    status: Option<InitiativeStatus>,
    owner: Option<String>,
    target_date: Option<NaiveDate>,
    color: Option<String>,
    icon: Option<String>,
}

impl Changes {
    fn is_empty(&self) -> bool {
        self.name.is_none()
            && self.description.is_none()
            && self.status.is_none()
            && self.owner.is_none()
            && self.target_date.is_none()
            && self.color.is_none()
            && self.icon.is_none()
    }

    fn into_input(self, owner_id: Option<String>) -> InitiativeUpdateInput {
        InitiativeUpdateInput {
            name: self.name,
            description: self.description,
            status: self.status,
            owner_id,
            target_date: self.target_date.map(TimelessDate::from),
            color: self.color,
            icon: self.icon,
        }
    }
}

async fn details(client: &LinearClient, id: &str, original: &str) -> Result<CurrentInitiative> {
    let result: GetInitiativeForUpdate =
        client.query(DetailVariables { id: id.to_owned() }).await?;
    result
        .initiative
        .ok_or_else(|| Error::not_found("Initiative", original))
}

async fn submit(
    client: &LinearClient,
    id: &str,
    input: InitiativeUpdateInput,
) -> Result<UpdatedInitiative> {
    let result: UpdateInitiative = client
        .mutate(UpdateVariables {
            id: id.to_owned(),
            input,
        })
        .await?;
    if !result.initiative_update.success {
        return Err(Error::new("Linear did not update the initiative"));
    }
    Ok(result.initiative_update.initiative)
}

/// Asks for each field with its current value as the default; only changed
/// fields are returned.
fn prompt(prompter: &Prompter<'_>, current: &CurrentInitiative) -> Result<Changes> {
    let mut changes = Changes::default();
    let name = prompter.text(Text::new("Name:").required().with_default(&current.name))?;
    if name != current.name {
        changes.name = Some(name);
    }
    let default = current.description.as_deref().unwrap_or("");
    let value = prompter.text(Text::new("Description:").with_default(default))?;
    if value != default {
        changes.description = (!value.is_empty()).then_some(value);
    }
    let statuses = [
        (InitiativeStatus::Planned, "Planned"),
        (InitiativeStatus::Active, "Active"),
        (InitiativeStatus::Completed, "Completed"),
    ];
    let start = statuses
        .iter()
        .position(|(status, _)| Some(status) == current.status.as_ref())
        .unwrap_or(0);
    let choices = statuses
        .into_iter()
        .map(|(status, label)| Choice::new(label, status))
        .collect();
    let status = prompter.select_from("Status:", choices, start)?;
    if Some(&status) != current.status.as_ref() {
        changes.status = Some(status);
    }
    let default = current
        .target_date
        .as_ref()
        .map_or("", |date| date.0.as_str());
    let value = prompter.parsed(
        Text::new("Target date (YYYY-MM-DD):").with_default(default),
        &date,
    )?;
    if let Some(value) = value
        && value.format("%Y-%m-%d").to_string() != default
    {
        changes.target_date = Some(value);
    }
    let default = current.color.as_deref().unwrap_or("");
    let value = prompter.parsed(
        Text::new("Color (hex, e.g., #5E6AD2):").with_default(default),
        &hex_color,
    )?;
    if let Some(value) = value
        && value != default
    {
        changes.color = Some(value);
    }
    Ok(changes)
}
