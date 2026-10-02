//! `team create`: prompts only when no create flag was given, otherwise
//! requires a name; either way it prints its progress line before building
//! the client and sends one typed mutation.
use std::io::{Read, Write};

use cynic::MutationBuilder;

use crate::error::Error;
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::team_create::{
    CreateTeam, CreateTeamPayload, CreateTeamVariables, TeamCreateInput,
};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};

/// Prefix for every `team create` failure.
pub const CONTEXT: &str = "Failed to create team";

/// Printed before the prompts.
pub const PROMPT_HEADER: &[u8] = b"Creating a new team...\n\n";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub name: Option<String>,
    pub description: Option<String>,
    pub key: Option<String>,
    pub private: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    /// No create flag and an interactive terminal: prompt for every field.
    Prompt,
    /// Anything else, including `--private` alone or a redirected stdout.
    Flags,
}

/// `--no-interactive` and a redirected stdout both disable prompts and the
/// flag-mode spinner.
pub fn interactive(no_interactive: bool, stdout_tty: bool) -> bool {
    !no_interactive && stdout_tty
}

/// `--no-interactive` is not a create flag; `--private` is.
pub fn mode(options: &Options, interactive: bool) -> Mode {
    let no_flags = options.name.is_none()
        && options.description.is_none()
        && options.key.is_none()
        && !options.private;
    if no_flags && interactive {
        Mode::Prompt
    } else {
        Mode::Flags
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PromptResult {
    Complete,
    Interrupted,
    EndOfInput,
}

/// Ask for every field. Empty optional answers are left out, and the
/// visibility choice only ever turns `private` on.
pub fn prompt<R: Read, W: Write>(
    options: &mut Options,
    session: &mut PromptSession<R, W>,
) -> Result<PromptResult, Error> {
    macro_rules! answer {
        ($call:expr) => {
            match $call? {
                PromptOutcome::Submitted(value) => value,
                PromptOutcome::Interrupted => return Ok(PromptResult::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptResult::EndOfInput),
            }
        };
    }
    options.name = Some(answer!(session.text("Team name:", 0, |raw| {
        if raw.trim().is_empty() {
            Err("Team name is required".to_owned())
        } else {
            Ok(())
        }
    })));
    options.description = optional(answer!(session.text(
        "Team description (optional):",
        0,
        |_| Ok(())
    )));
    options.key = optional(answer!(session.text(
        "Team key (optional, will be generated from name if not provided):",
        0,
        |_| Ok(())
    )));
    let visibility = [
        PlainOption {
            label: "Public".to_owned(),
            value: "public".to_owned(),
            script_token: "public".to_owned(),
        },
        PlainOption {
            label: "Private".to_owned(),
            value: "private".to_owned(),
            script_token: "private".to_owned(),
        },
    ];
    let choice = answer!(session.select(&PlainSelect {
        message: "Team visibility:",
        options: &visibility,
        default_index: 0,
        // The hint shows the default option's label, not its value.
        default_hint: Some("Public"),
    }));
    options.private = match choice.as_str() {
        "private" => true,
        "public" => false,
        other => {
            return Err(Error::new(format!(
                "unexpected team visibility choice {other:?}"
            )));
        }
    };
    Ok(PromptResult::Complete)
}

fn optional(value: String) -> Option<String> {
    if value.is_empty() { None } else { Some(value) }
}

/// The name the progress line and mutation use. Flag mode requires one;
/// prompt mode always has one after a completed prompt.
pub fn required_name(options: &Options) -> Result<&str, Error> {
    match options.name.as_deref() {
        Some(name) if !name.is_empty() => Ok(name),
        _ => Err(
            Error::new("Team name is required when not using interactive mode")
                .with_hint("Use --name or run without any flags for interactive mode."),
        ),
    }
}

/// The line printed before the client is built. The prompt path adds a
/// leading blank line and an ellipsis.
pub fn announcement(name: &str, mode: Mode) -> Vec<u8> {
    match mode {
        Mode::Prompt => format!("\nCreating team \"{name}\"...\n"),
        Mode::Flags => format!("Creating team \"{name}\"\n"),
    }
    .into_bytes()
}

pub fn request(options: &Options) -> Result<GraphQlRequest<CreateTeamVariables>, Error> {
    let name = required_name(options)?.to_owned();
    Ok(GraphQlRequest::with_variables(CreateTeam::build(
        CreateTeamVariables {
            input: TeamCreateInput {
                name,
                description: options
                    .description
                    .clone()
                    .filter(|value| !value.is_empty()),
                key: options.key.clone().filter(|value| !value.is_empty()),
                private: options.private.then_some(true),
            },
        },
    )))
}

/// Sends the mutation once. Failures after the request may have reached
/// Linear say the team may already exist; nothing is retried.
pub async fn submit(transport: &GraphQlTransport, options: &Options) -> Result<Vec<u8>, Error> {
    let request = request(options)?;
    let result: CreateTeam = transport.execute(&request).await.map_err(|failure| {
        let uncertain = super::milestone_create::outcome_unknown(&failure);
        let mut error = Error::from(failure);
        if uncertain {
            error.push_message("; team may already exist");
        }
        error
    })?;
    render(&result.team_create)
}

/// `success: false` is reported before a missing team.
pub fn render(payload: &CreateTeamPayload) -> Result<Vec<u8>, Error> {
    if !payload.success {
        return Err(Error::new("Team creation failed"));
    }
    let Some(team) = &payload.team else {
        return Err(Error::new("Team creation failed - no team returned"));
    };
    Ok(format!("✓ Created team {}: {}\n", team.key, team.name).into_bytes())
}
