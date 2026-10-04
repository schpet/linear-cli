//! `initiative create`: fields from flags or prompts, then one mutation.
use crate::commands::outcome;
use crate::refs;
use chrono::NaiveDate;

use crate::cli::initiative::InitiativeCreate;
use crate::cli::values::{InitiativeStatus, UserRef, date};
use crate::client::LinearClient;
use crate::commands::color;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::initiative::{
    CreateInitiative, CreateInitiativeVariables, CreatedInitiative, InitiativeCreateInput,
};
use crate::graphql::scalars::TimelessDate;
use crate::platform::prompt::{Choice, Prompter, Text};

pub fn run(ctx: &Ctx, args: &InitiativeCreate) -> Result<()> {
    create(ctx, args).context("Failed to create initiative")
}

fn create(ctx: &Ctx, args: &InitiativeCreate) -> Result<()> {
    let mut fields = Fields {
        name: args.name.clone(),
        description: args.description.clone(),
        status: args.status,
        owner: args.owner.clone(),
        target_date: args.target_date,
        color: args.color.clone().map(String::from),
        icon: args.icon.clone(),
    };
    let optional = ctx.optional_prompts(args.interactive)?;
    if ctx.interactive() && (fields.name.is_none() || optional) {
        ctx.print("\nCreate a new initiative\n\n")?;
        prompt(&mut fields, &ctx.prompter()?, optional)?;
    }
    let input = validate(fields)?;
    let client = ctx.client()?;
    let created = ctx.spin(true, async {
        let owner_id = match &input.owner {
            Some(owner) => Some(refs::user::resolve(client, owner, "Owner").await?),
            None => None,
        };
        submit(client, input.into_create(owner_id)).await
    })?;
    ctx.print(render(&created))
}

/// The fields as given on the command line or at the prompts.
#[derive(Default)]
struct Fields {
    name: Option<String>,
    description: Option<String>,
    status: Option<InitiativeStatus>,
    owner: Option<UserRef>,
    target_date: Option<NaiveDate>,
    color: Option<String>,
    icon: Option<String>,
}

/// Validated fields, ready to send once the owner is looked up.
struct Valid {
    name: String,
    description: Option<String>,
    status: Option<InitiativeStatus>,
    owner: Option<UserRef>,
    target_date: Option<NaiveDate>,
    color: Option<String>,
    icon: Option<String>,
}

impl Valid {
    fn into_create(self, owner_id: Option<String>) -> InitiativeCreateInput {
        InitiativeCreateInput {
            name: self.name,
            description: self.description,
            status: self.status.map(Into::into),
            owner_id,
            target_date: self.target_date.map(TimelessDate::from),
            color: self.color,
            icon: self.icon,
        }
    }
}

/// Asks for the name when it is missing, and with `all` for every other
/// field not given as a flag.
fn prompt(options: &mut Fields, prompter: &Prompter<'_>, all: bool) -> Result<()> {
    if options.name.as_deref().is_none_or(str::is_empty) {
        options.name = Some(prompter.text(Text::new("Initiative name:").required())?);
    }
    if !all {
        return Ok(());
    }
    if options.description.as_deref().is_none_or(str::is_empty) {
        options.description = optional(prompter.text(Text::new("Description (optional):"))?);
    }
    if options.status.is_none() {
        let choices = [
            ("Planned", InitiativeStatus::Planned),
            ("Active", InitiativeStatus::Active),
            ("Completed", InitiativeStatus::Completed),
        ]
        .into_iter()
        .map(|(label, status)| Choice::new(label, status))
        .collect();
        options.status = Some(prompter.select("Status:", choices)?);
    }
    if options.owner.is_none() {
        options.owner = prompter.parsed(
            Text::new("Owner (username, email, or @me - press Enter to skip):"),
            &str::parse::<UserRef>,
        )?;
    }
    if options.target_date.is_none() {
        options.target_date = prompter.parsed(
            Text::new("Target date (YYYY-MM-DD - press Enter to skip):"),
            &date,
        )?;
    }
    if options.color.as_deref().is_none_or(str::is_empty) {
        let mut colors = vec![Choice::new("Skip (use default)", Color::Skip)];
        colors.extend(
            color::PALETTE
                .into_iter()
                .map(|(name, hex)| Choice::new(color::label(name, hex), Color::Hex(hex))),
        );
        colors.push(Choice::new("Custom color", Color::Custom));
        options.color = match prompter.select("Color (optional):", colors)? {
            Color::Skip => None,
            Color::Hex(hex) => Some(hex.to_owned()),
            Color::Custom => Some(color::custom(prompter)?),
        };
    }
    Ok(())
}

enum Color {
    Skip,
    Hex(&'static str),
    Custom,
}

fn optional(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn validate(fields: Fields) -> Result<Valid> {
    let name = fields
        .name
        .filter(|name| !name.is_empty())
        .ok_or_else(|| Error::invalid("Initiative name is required").with_hint("Pass --name."))?;
    let nonempty = |value: Option<String>| value.filter(|value| !value.is_empty());
    Ok(Valid {
        name,
        description: nonempty(fields.description),
        status: fields.status,
        owner: fields.owner,
        target_date: fields.target_date,
        color: nonempty(fields.color),
        icon: nonempty(fields.icon),
    })
}

/// Sends the mutation once. A failure after the request may have reached
/// Linear says the initiative may already exist; nothing is retried.
async fn submit(client: &LinearClient, input: InitiativeCreateInput) -> Result<CreatedInitiative> {
    let result: CreateInitiative = client
        .mutate(CreateInitiativeVariables { input })
        .await
        .map_err(|failure| failure.into_create_error("initiative"))?;
    if !result.initiative_create.success {
        return Err(Error::new("Linear did not create the initiative"));
    }
    Ok(result.initiative_create.initiative)
}

fn render(initiative: &CreatedInitiative) -> String {
    outcome::done(
        "Created",
        "initiative",
        &initiative.name,
        Some(&initiative.url),
    )
}
