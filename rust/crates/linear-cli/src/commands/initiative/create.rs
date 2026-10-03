//! `initiative create`: fields from flags or prompts, then one mutation.
use chrono::NaiveDate;
use cynic::MutationBuilder;

use crate::cli::initiative::InitiativeCreate;
use crate::cli::values::{InitiativeStatus, date};
use crate::commands::color;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiative_create::{
    CreateInitiative, CreateInitiativeVariables, CreatedInitiative, InitiativeCreateInput,
};
use crate::graphql::scalars::TimelessDate;
use crate::graphql::transport::GraphQlTransport;
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
        color: args.color.clone(),
        icon: args.icon.clone(),
    };
    if args.interactive && !ctx.interactive() {
        return Err(Error::new("Interactive mode needs a terminal")
            .with_hint("Pass --name and the other fields instead of --interactive."));
    }
    if ctx.interactive() && (fields.name.is_none() || args.interactive) {
        ctx.print("\nCreate a new initiative\n\n")?;
        prompt(&mut fields, &ctx.prompter()?)?;
    }
    let input = validate(fields)?;
    let client = ctx.client()?;
    let created = ctx.spin(true, async {
        let owner_id = match &input.owner {
            Some(owner) => Some(crate::commands::user::resolve(client, owner, "Owner").await?),
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
    owner: Option<String>,
    target_date: Option<NaiveDate>,
    color: Option<String>,
    icon: Option<String>,
}

/// Validated fields, ready to send once the owner is looked up.
struct Valid {
    name: String,
    description: Option<String>,
    status: Option<InitiativeStatus>,
    owner: Option<String>,
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

/// Asks for each field not given as a flag.
fn prompt(options: &mut Fields, prompter: &Prompter<'_>) -> Result<()> {
    if options.name.as_deref().is_none_or(str::is_empty) {
        options.name = Some(prompter.text(Text::new("Initiative name:").required())?);
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
    if options.owner.as_deref().is_none_or(str::is_empty) {
        let check = |owner: &str| {
            super::check_owner(Some(owner)).map_err(|error| error.message().to_owned())
        };
        options.owner = optional(prompter.text(
            Text::new("Owner (username, email, or @me - press Enter to skip):").with_check(&check),
        )?);
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
        .ok_or_else(|| Error::new("Initiative name is required. Use --name or -n flag."))?;
    let nonempty = |value: Option<String>| value.filter(|value| !value.is_empty());
    let owner = nonempty(fields.owner);
    super::check_owner(owner.as_deref())?;
    Ok(Valid {
        name,
        description: nonempty(fields.description),
        status: fields.status,
        owner,
        target_date: fields.target_date,
        color: nonempty(fields.color),
        icon: nonempty(fields.icon),
    })
}

/// Sends the mutation once. A failure after the request may have reached
/// Linear says the initiative may already exist; nothing is retried.
async fn submit(
    client: &GraphQlTransport,
    input: InitiativeCreateInput,
) -> Result<CreatedInitiative> {
    let request =
        GraphQlRequest::with_variables(CreateInitiative::build(CreateInitiativeVariables {
            input,
        }));
    let result: CreateInitiative = client
        .execute(&request)
        .await
        .map_err(|failure| failure.into_create_error("initiative"))?;
    if !result.initiative_create.success {
        return Err(Error::new("Linear did not create the initiative"));
    }
    Ok(result.initiative_create.initiative)
}

fn render(initiative: &CreatedInitiative) -> String {
    let mut output = format!(
        "✓ Created initiative: {}\n  Slug: {}\n",
        initiative.name, initiative.slug_id
    );
    if !initiative.url.is_empty() {
        output.push_str(&format!("  URL: {}\n", initiative.url));
    }
    output
}
