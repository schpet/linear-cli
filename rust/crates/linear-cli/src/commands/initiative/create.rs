//! `initiative create`: fields from flags or prompts, then one mutation.
use std::io::{Read, Write};

use cynic::MutationBuilder;

use crate::cli::initiative::InitiativeCreate;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiative_create::{
    CreateInitiative, CreateInitiativeVariables, CreatedInitiative, InitiativeCreateInput,
};
use crate::graphql::operations::initiatives::InitiativeStatus;
use crate::graphql::scalars::TimelessDate;
use crate::graphql::transport::GraphQlTransport;
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};

pub fn run(ctx: &Ctx, args: &InitiativeCreate) -> Result<()> {
    create(ctx, args).context("Failed to create initiative")
}

fn create(ctx: &Ctx, args: &InitiativeCreate) -> Result<()> {
    let mut fields = Fields {
        name: args.name.clone(),
        description: args.description.clone(),
        status: args.status.clone(),
        owner: args.owner.clone(),
        target_date: args.target_date.clone(),
        color: args.color.clone(),
        icon: args.icon.clone(),
    };
    if ctx.stdout_tty() && (fields.name.is_none() || args.interactive) {
        ctx.print("\nCreate a new initiative\n\n")?;
        let mut session = ctx.prompts()?;
        let result = prompt(&mut fields, &mut session);
        match session.finish_result(result)? {
            PromptOutcome::Submitted(()) => {}
            PromptOutcome::Interrupted => return Err(Error::cancelled()),
            PromptOutcome::EndOfInput => {
                return Err(Error::new("Unexpected end of input at a prompt"));
            }
        }
    }
    let input = validate(fields)?;
    let client = ctx.client()?;
    let created = ctx.spin(true, async {
        let owner_id = match &input.owner {
            Some(owner) => Some(super::owner_id(client, owner).await?),
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
    status: Option<String>,
    owner: Option<String>,
    target_date: Option<String>,
    color: Option<String>,
    icon: Option<String>,
}

/// Validated fields, ready to send once the owner is looked up.
struct Valid {
    name: String,
    description: Option<String>,
    status: Option<InitiativeStatus>,
    owner: Option<String>,
    target_date: Option<TimelessDate>,
    color: Option<String>,
    icon: Option<String>,
}

impl Valid {
    fn into_create(self, owner_id: Option<String>) -> InitiativeCreateInput {
        InitiativeCreateInput {
            name: self.name,
            description: self.description,
            status: self.status,
            owner_id,
            target_date: self.target_date,
            color: self.color,
            icon: self.icon,
        }
    }
}

fn choice(label: &str, value: &str, token: &str) -> PlainOption {
    PlainOption {
        label: label.to_owned(),
        value: value.to_owned(),
        script_token: token.to_owned(),
    }
}

/// Asks for each field not given as a flag.
fn prompt<R: Read, W: Write>(
    options: &mut Fields,
    session: &mut PromptSession<R, W>,
) -> Result<PromptOutcome<()>> {
    macro_rules! answer {
        ($call:expr) => {
            match $call? {
                PromptOutcome::Submitted(value) => value,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
            }
        };
    }
    if options.name.as_deref().is_none_or(str::is_empty) {
        options.name = Some(answer!(session.text("Initiative name:", 1, |_| Ok(()))));
    }
    if options.description.as_deref().is_none_or(str::is_empty) {
        options.description =
            optional(answer!(
                session.text("Description (optional):", 0, |_| Ok(()))
            ));
    }
    if options.status.as_deref().is_none_or(str::is_empty) {
        let choices = [
            choice("Planned", "Planned", "Planned"),
            choice("Active", "Active", "Active"),
            choice("Completed", "Completed", "Completed"),
        ];
        options.status = Some(answer!(session.select(&PlainSelect {
            message: "Status:",
            options: &choices,
            default_index: 0,
            default_hint: Some("planned"),
        })));
    }
    if options.owner.as_deref().is_none_or(str::is_empty) {
        options.owner = optional(answer!(session.text(
            "Owner (username, email, or @me - press Enter to skip):",
            0,
            |_| Ok(())
        )));
    }
    if options.target_date.as_deref().is_none_or(str::is_empty) {
        options.target_date = optional(answer!(session.text(
            "Target date (YYYY-MM-DD - press Enter to skip):",
            0,
            |_| Ok(())
        )));
    }
    if options.color.as_deref().is_none_or(str::is_empty) {
        let colors = [
            choice("Skip (use default)", "__skip__", "skip"),
            choice("Red (#EB5757)", "#EB5757", "#EB5757"),
            choice("Orange (#F2994A)", "#F2994A", "#F2994A"),
            choice("Yellow (#F2C94C)", "#F2C94C", "#F2C94C"),
            choice("Green (#27AE60)", "#27AE60", "#27AE60"),
            choice("Teal (#0D9488)", "#0D9488", "#0D9488"),
            choice("Blue (#2F80ED)", "#2F80ED", "#2F80ED"),
            choice("Indigo (#5E6AD2)", "#5E6AD2", "#5E6AD2"),
            choice("Purple (#8B5CF6)", "#8B5CF6", "#8B5CF6"),
            choice("Pink (#BB6BD9)", "#BB6BD9", "#BB6BD9"),
            choice("Gray (#6B6F76)", "#6B6F76", "#6B6F76"),
            choice("Custom color", "__custom__", "custom"),
        ];
        let selected = answer!(session.select(&PlainSelect {
            message: "Color (optional):",
            options: &colors,
            default_index: 0,
            default_hint: Some("__skip__"),
        }));
        options.color = match selected.as_str() {
            "__skip__" => None,
            "__custom__" => Some(answer!(session.text(
                "Enter hex color (e.g., #FF5733):",
                0,
                |raw| {
                    super::parse_color(raw)
                        .map(drop)
                        .map_err(|_| "Please enter a valid hex color (e.g., #FF5733)".to_owned())
                }
            ))),
            _ => Some(selected),
        };
    }
    Ok(PromptOutcome::Submitted(()))
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
    let status = nonempty(fields.status)
        .map(|value| super::parse_status(&value))
        .transpose()?;
    let color = nonempty(fields.color)
        .map(|value| super::parse_color(&value))
        .transpose()?;
    let target_date = nonempty(fields.target_date)
        .map(|value| super::parse_target_date(&value))
        .transpose()?;
    let owner = nonempty(fields.owner);
    super::check_owner(owner.as_deref())?;
    Ok(Valid {
        name,
        description: nonempty(fields.description),
        status,
        owner,
        target_date,
        color,
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
    let result: CreateInitiative = client.execute(&request).await.map_err(|failure| {
        let uncertain = crate::commands::milestone::create::outcome_unknown(&failure);
        let mut error = Error::from(failure);
        if uncertain {
            error.push_message("; initiative may already exist");
        }
        error
    })?;
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
