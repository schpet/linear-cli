//! `initiative update`: fields from flags or prompts, then one mutation.
use std::io::{Read, Write};

use cynic::{MutationBuilder, QueryBuilder};

use crate::cli::initiative::InitiativeUpdate;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiative_update::{
    CurrentInitiative, GetInitiativeForUpdate, InitiativeUpdateInput, UpdateInitiative,
    UpdateVariables, UpdatedInitiative,
};
use crate::graphql::operations::initiative_view::DetailVariables;
use crate::graphql::operations::initiatives::InitiativeStatus;
use crate::graphql::transport::GraphQlTransport;
use crate::platform::prompt::{
    PlainOption, PlainSelect, PromptOutcome, PromptSession, escaped_display,
};
use crate::platform::prompt_text::TextOptions;

pub fn run(ctx: &Ctx, args: &InitiativeUpdate) -> Result<()> {
    update(ctx, args).context("Failed to update initiative")
}

fn update(ctx: &Ctx, args: &InitiativeUpdate) -> Result<()> {
    let original = &args.initiative_id;
    let reference = super::reference(ctx, original)?;
    let flags = Changes {
        name: args.name.clone(),
        description: args.description.clone(),
        status: args
            .status
            .as_deref()
            .map(super::parse_status)
            .transpose()?,
        owner: args.owner.clone(),
        target_date: args.target_date.clone(),
        color: args.color.as_deref().map(super::parse_color).transpose()?,
        icon: args.icon.clone(),
    };
    if let Some(date) = &flags.target_date {
        super::parse_target_date(date)?;
    }
    super::check_owner(flags.owner.as_deref())?;
    let prompting = flags.is_empty();
    if prompting && !args.interactive {
        return Err(Error::new("No changes specified").with_hint(
            "Pass the fields to change, such as --name or --status, or -i to be prompted.",
        ));
    }
    if prompting && !ctx.stdout_tty() {
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
        ctx.print(format!(
            "\nUpdating initiative: {}\n\n",
            escaped_display(&current.name)
        ))?;
        let mut session = ctx.prompts()?;
        let result = prompt(&mut session, &current);
        let changes = match session.finish_result(result)? {
            PromptOutcome::Submitted(changes) => changes,
            PromptOutcome::Interrupted => return Err(Error::cancelled()),
            PromptOutcome::EndOfInput => {
                return Err(Error::new("Unexpected end of input at a prompt"));
            }
        };
        if changes.is_empty() {
            return ctx.print("No changes specified\n");
        }
        if let Some(date) = &changes.target_date {
            super::parse_target_date(date)?;
        }
        if let Some(color) = &changes.color {
            super::parse_color(color)?;
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
            Some(owner) => Some(super::owner_id(client, owner).await?),
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
    target_date: Option<String>,
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
            target_date: self.target_date.map(crate::graphql::scalars::TimelessDate),
            color: self.color,
            icon: self.icon,
        }
    }
}

async fn details(client: &GraphQlTransport, id: &str, original: &str) -> Result<CurrentInitiative> {
    let request = GraphQlRequest::with_variables(GetInitiativeForUpdate::build(DetailVariables {
        id: id.to_owned(),
    }));
    let result: GetInitiativeForUpdate = client.execute(&request).await?;
    result
        .initiative
        .ok_or_else(|| Error::not_found("Initiative", original))
}

async fn submit(
    client: &GraphQlTransport,
    id: &str,
    input: InitiativeUpdateInput,
) -> Result<UpdatedInitiative> {
    let request = GraphQlRequest::with_variables(UpdateInitiative::build(UpdateVariables {
        id: id.to_owned(),
        input,
    }));
    let result: UpdateInitiative = client.execute(&request).await?;
    if !result.initiative_update.success {
        return Err(Error::new("Linear did not update the initiative"));
    }
    Ok(result.initiative_update.initiative)
}

/// Asks for each field with its current value as the default; only changed
/// fields are returned.
fn prompt<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    current: &CurrentInitiative,
) -> Result<PromptOutcome<Changes>> {
    let mut changes = Changes::default();
    macro_rules! answer {
        ($call:expr) => {
            match $call? {
                PromptOutcome::Submitted(value) => value,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
            }
        };
    }
    let text = |default| TextOptions {
        required: false,
        default: Some(default),
    };
    let name = answer!(session.text_with_display_default("Name:", text(&current.name)));
    if name != current.name {
        changes.name = Some(name);
    }
    let default = current.description.as_deref().unwrap_or("");
    let value = answer!(session.text_with_display_default("Description:", text(default)));
    if value != default {
        changes.description = (!value.is_empty()).then_some(value);
    }
    let statuses = [
        (InitiativeStatus::Planned, "Planned"),
        (InitiativeStatus::Active, "Active"),
        (InitiativeStatus::Completed, "Completed"),
    ];
    let options: Vec<_> = statuses
        .iter()
        .map(|(_, label)| PlainOption {
            label: (*label).to_owned(),
            value: (*label).to_owned(),
            script_token: label.to_lowercase(),
        })
        .collect();
    let index = statuses
        .iter()
        .position(|(status, _)| Some(status) == current.status.as_ref())
        .unwrap_or(0);
    let selected = answer!(session.select(&PlainSelect {
        message: "Status:",
        options: &options,
        default_index: index,
        default_hint: None,
    }));
    let (status, _) = statuses
        .into_iter()
        .find(|(_, label)| *label == selected)
        .expect("the selected status is one of the options");
    if Some(&status) != current.status.as_ref() {
        changes.status = Some(status);
    }
    let default = current
        .target_date
        .as_ref()
        .map_or("", |date| date.0.as_str());
    let value =
        answer!(session.text_with_display_default("Target date (YYYY-MM-DD):", text(default)));
    if value != default {
        changes.target_date = (!value.is_empty()).then_some(value);
    }
    let default = current.color.as_deref().unwrap_or("");
    let value =
        answer!(session.text_with_display_default("Color (hex, e.g., #5E6AD2):", text(default)));
    if value != default {
        changes.color = (!value.is_empty()).then_some(value);
    }
    Ok(PromptOutcome::Submitted(changes))
}
