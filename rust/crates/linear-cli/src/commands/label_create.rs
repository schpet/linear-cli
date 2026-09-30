//! Label create prompts pause after description to fetch the team picker.
use std::io::{Read, Write};

use cynic::MutationBuilder;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::label_create::{
    CreateIssueLabel, CreateIssueLabelPayload, CreateIssueLabelVariables, IssueLabelCreateInput,
};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};
use crate::refs::ResolvedTeam;

pub const CONTEXT: &str = "Failed to create label";
pub const PROMPT_HEADER: &[u8] = b"\nCreate a new label\n\n";
const INDIGO: &str = "#5E6AD2";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub name: Option<String>,
    pub color: Option<String>,
    pub description: Option<String>,
    pub team: Option<String>,
    pub interactive: bool,
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

/// Ask only for absent fields, stopping before the team network request.
pub fn prompt_fields<R: Read, W: Write>(
    options: &mut Options,
    session: &mut PromptSession<R, W>,
) -> Result<PromptOutcome<()>, AppError> {
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
        options.name = Some(answer!(session.text("Label name:", 1, |_| Ok(()))));
    }
    if options.color.as_deref().is_none_or(str::is_empty) {
        let palette = [
            ("Red", "#EB5757"),
            ("Orange", "#F2994A"),
            ("Yellow", "#F2C94C"),
            ("Green", "#27AE60"),
            ("Teal", "#0D9488"),
            ("Blue", "#2F80ED"),
            ("Indigo", INDIGO),
            ("Purple", "#8B5CF6"),
            ("Pink", "#BB6BD9"),
            ("Gray", "#6B6F76"),
        ];
        let mut choices: Vec<_> = palette
            .iter()
            .map(|(name, color)| choice(&format!("{name} ({color})"), color, color))
            .collect();
        choices.push(choice("Custom color", "custom", "custom"));
        let selected = answer!(session.select(&PlainSelect {
            message: "Color:",
            options: &choices,
            default_index: 6,
            default_hint: choices.get(6).map(|choice| choice.label.as_str()),
        }));
        options.color = Some(if selected == "custom" {
            answer!(session.text("Enter hex color (e.g., #FF5733):", 0, |raw| {
                if valid_color(raw) {
                    Ok(())
                } else {
                    Err("Please enter a valid hex color (e.g., #FF5733)".to_owned())
                }
            }))
        } else {
            selected
        });
    }
    if options.description.as_deref().is_none_or(str::is_empty) {
        let description = answer!(session.text("Description (optional):", 0, |_| Ok(())));
        options.description = (!description.is_empty()).then_some(description);
    }
    Ok(PromptOutcome::Submitted(()))
}

/// Teams are already sorted by the typed source paging service. Workspace is
/// first; the configured uppercased key selects a default or falls back to it.
pub fn prompt_team<R: Read, W: Write>(
    options: &mut Options,
    session: &mut PromptSession<R, W>,
    teams: &[ResolvedTeam],
    configured_key: Option<&str>,
) -> Result<PromptOutcome<()>, AppError> {
    let mut choices = vec![choice(
        "Workspace (shared by all teams)",
        "__workspace__",
        "workspace",
    )];
    choices.extend(teams.iter().map(|team| {
        choice(
            &format!("{} ({})", team.name, team.key),
            &team.key,
            &team.key,
        )
    }));
    let default_index = configured_key
        .and_then(|key| choices.iter().position(|c| c.value == key))
        .unwrap_or(0);
    match session.select(&PlainSelect {
        message: "Team:",
        options: &choices,
        default_index,
        default_hint: choices.get(default_index).map(|c| c.label.as_str()),
    })? {
        PromptOutcome::Submitted(selected) => {
            options.team = (selected != "__workspace__").then_some(selected);
            Ok(PromptOutcome::Submitted(()))
        }
        PromptOutcome::Interrupted => Ok(PromptOutcome::Interrupted),
        PromptOutcome::EndOfInput => Ok(PromptOutcome::EndOfInput),
    }
}

fn valid_color(value: &str) -> bool {
    value.len() == 7 && value.starts_with('#') && value[1..].bytes().all(|b| b.is_ascii_hexdigit())
}

pub fn validate(options: &Options) -> Result<(), AppError> {
    if options.name.as_deref().is_none_or(str::is_empty) {
        return Err(
            AppError::new(AppErrorKind::Validation, "Label name is required")
                .with_suggestion("Use --name or -n flag to specify a label name."),
        );
    }
    if options
        .color
        .as_deref()
        .is_some_and(|c| !c.is_empty() && !valid_color(c))
    {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "Color must be a valid hex code (e.g., #EB5757)",
        ));
    }
    Ok(())
}

pub fn request(
    options: &Options,
    team_id: Option<String>,
) -> Result<GraphQlRequest<CreateIssueLabelVariables>, AppError> {
    validate(options)?;
    let name = options
        .name
        .clone()
        .ok_or_else(|| AppError::new(AppErrorKind::Invariant, "validated label name vanished"))?;
    Ok(GraphQlRequest::with_variables(CreateIssueLabel::build(
        CreateIssueLabelVariables {
            input: IssueLabelCreateInput {
                name,
                color: options
                    .color
                    .clone()
                    .filter(|c| !c.is_empty())
                    .unwrap_or_else(|| INDIGO.to_owned()),
                description: options.description.clone().filter(|d| !d.is_empty()),
                team_id,
            },
        },
    )))
}

/// Send once; post-write uncertainty never implies a retry.
pub async fn submit(
    transport: &GraphQlTransport,
    options: &Options,
    team_id: Option<String>,
) -> Result<Vec<u8>, AppError> {
    let result: CreateIssueLabel = transport
        .execute(&request(options, team_id)?)
        .await
        .map_err(|failure| {
            let uncertain = super::milestone_create::outcome_unknown(&failure);
            let mut error = AppError::from(failure);
            if uncertain {
                error.message.push_str("; label may already exist");
            }
            error
        })?;
    render(&result.issue_label_create)
}

pub fn render(payload: &CreateIssueLabelPayload) -> Result<Vec<u8>, AppError> {
    if !payload.success {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            "Failed to create label",
        ));
    }
    let label = &payload.issue_label;
    let mut output = format!(
        "✓ Created label: {}\n  Color: {}\n",
        label.name, label.color
    );
    if let Some(description) = &label.description
        && !description.is_empty()
    {
        output.push_str(&format!("  Description: {description}\n"));
    }
    let scope = match &label.team {
        Some(team) if !team.name.is_empty() => format!("{} ({})", team.name, team.key),
        Some(_) | None => "Workspace".to_owned(),
    };
    output.push_str(&format!("  Scope: {scope}\n"));
    Ok(output.into_bytes())
}
