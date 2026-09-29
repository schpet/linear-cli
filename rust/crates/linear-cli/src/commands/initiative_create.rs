//! Create an initiative, preserving the source command's validation and write order.
use std::io::{Read, Write};

use cynic::MutationBuilder;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiative_create::{
    CreateInitiative, CreateInitiativeVariables, InitiativeCreateInput,
};
use crate::graphql::operations::initiatives::InitiativeStatus;
use crate::graphql::scalars::TimelessDate;
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};

pub const CREATE_CONTEXT: &str = "Failed to create initiative";

#[derive(Clone, Debug, Default)]
pub struct Options {
    pub name: Option<String>,
    pub description: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub target_date: Option<String>,
    pub color: Option<String>,
    pub icon: Option<String>,
    pub interactive: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptResult {
    Complete,
    Interrupted,
    EndOfInput,
}

pub fn should_prompt(options: &Options, stdout_tty: bool) -> bool {
    stdout_tty && (options.name.is_none() || options.interactive)
}

fn choice(label: &str, value: &str, token: &str) -> PlainOption {
    PlainOption {
        label: label.to_owned(),
        value: value.to_owned(),
        script_token: token.to_owned(),
    }
}

pub fn prompt<R: Read, W: Write>(
    options: &mut Options,
    session: &mut PromptSession<R, W>,
) -> Result<PromptResult, AppError> {
    macro_rules! answer {
        ($call:expr) => {
            match $call? {
                PromptOutcome::Submitted(value) => value,
                PromptOutcome::Interrupted => return Ok(PromptResult::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptResult::EndOfInput),
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
                |raw| if valid_color(raw) {
                    Ok(())
                } else {
                    Err("Please enter a valid hex color (e.g., #FF5733)".to_owned())
                }
            ))),
            _ => Some(selected),
        };
    }
    Ok(PromptResult::Complete)
}

fn optional(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

fn valid_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

fn valid_date(value: &str) -> bool {
    let [a, b, c, d, b'-', e, f, b'-', g, h] = value.as_bytes() else {
        return false;
    };
    [a, b, c, d, e, f, g, h].into_iter().all(u8::is_ascii_digit)
}

pub fn validate(options: &Options) -> Result<Option<InitiativeStatus>, AppError> {
    if options.name.as_deref().is_none_or(str::is_empty) {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Initiative name is required. Use --name or -n flag.",
        ));
    }
    let status = match options.status.as_deref() {
        None | Some("") => None,
        Some(value) if value.eq_ignore_ascii_case("planned") => Some(InitiativeStatus::Planned),
        Some(value) if value.eq_ignore_ascii_case("active") => Some(InitiativeStatus::Active),
        Some(value) if value.eq_ignore_ascii_case("completed") => Some(InitiativeStatus::Completed),
        Some(value) => {
            return Err(AppError::new(
                AppErrorKind::Validation,
                format!("Invalid status: {value}. Valid values: planned, active, completed"),
            ));
        }
    };
    if options
        .color
        .as_deref()
        .is_some_and(|value| !value.is_empty() && !valid_color(value))
    {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Color must be a valid hex code (e.g., #5E6AD2)",
        ));
    }
    if options
        .target_date
        .as_deref()
        .is_some_and(|value| !value.is_empty() && !valid_date(value))
    {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Target date must be in YYYY-MM-DD format",
        ));
    }
    Ok(status)
}

pub async fn resolve_owner(
    transport: &GraphQlTransport,
    owner: Option<&str>,
) -> Result<Option<String>, AppError> {
    match owner.filter(|value| !value.is_empty()) {
        Some(owner) => {
            crate::refs::reject_linear_url(owner, "an email, username, display name, or @me")?;
            let id = super::initiative_list::resolve_owner(transport, owner).await?;
            if id.inner().is_empty() {
                return Err(AppError::not_found("Owner", owner));
            }
            Ok(Some(id.inner().to_owned()))
        }
        None => Ok(None),
    }
}

pub async fn submit_create(
    transport: &GraphQlTransport,
    options: Options,
    status: Option<InitiativeStatus>,
    owner_id: Option<String>,
) -> Result<Vec<u8>, AppError> {
    let name = options
        .name
        .ok_or_else(|| AppError::new(AppErrorKind::Invariant, "validated name vanished"))?;
    let request =
        GraphQlRequest::with_variables(CreateInitiative::build(CreateInitiativeVariables {
            input: InitiativeCreateInput {
                name,
                description: options.description.filter(|value| !value.is_empty()),
                status,
                owner_id,
                target_date: options
                    .target_date
                    .filter(|value| !value.is_empty())
                    .map(TimelessDate),
                color: options.color.filter(|value| !value.is_empty()),
                icon: options.icon.filter(|value| !value.is_empty()),
            },
        }));
    let result: CreateInitiative = transport.execute(&request).await.map_err(|failure| {
        if matches!(failure, TransportFailure::Timeout { .. }) {
            AppError::new(
                AppErrorKind::Transport,
                format!("{failure}; initiative may already exist"),
            )
        } else {
            AppError::from(failure)
        }
    })?;
    if !result.initiative_create.success {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            "Failed to create initiative",
        ));
    }
    let initiative = result.initiative_create.initiative;
    let mut output = format!(
        "✓ Created initiative: {}\n  Slug: {}\n",
        initiative.name, initiative.slug_id
    );
    if !initiative.url.is_empty() {
        output.push_str(&format!("  URL: {}\n", initiative.url));
    }
    Ok(output.into_bytes())
}
