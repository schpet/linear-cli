//! Project and initiative status updates: creating one, and the shared
//! table their `list` commands print.
use std::io::Stdin;

use chrono::{DateTime, Local, Utc};
use cynic::MutationBuilder;

use crate::cli::project_update::{Health, StatusUpdateArgs};
use crate::commands::display::{display_width, pad, truncate_text};
use crate::commands::relative_time::format_relative_time;
use crate::commands::table::underlined_header;
use crate::commands::text_input;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::initiatives::InitiativeUpdateHealthType;
use crate::graphql::operations::projects::ProjectUpdateHealthType;
use crate::graphql::operations::update_create::{
    CreateInitiativeUpdate, CreateProjectUpdate, InitiativeHealthInput, InitiativeInput,
    InitiativeVariables, ProjectHealthInput, ProjectInput, ProjectVariables,
};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::output::StdoutWriter;
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};
use crate::platform::prompt_text::TextOptions;
use crate::platform::style;
use crate::refs::{
    InitiativeReference, ProjectReference, prepare_initiative_lookup, prepare_project_lookup,
    resolve_initiative_with_transport, resolve_project_with_transport,
};

/// What a status update is posted to, as the user named it.
#[derive(Clone, Copy)]
pub enum Target<'a> {
    Project(&'a str),
    Initiative(&'a str),
}

enum Reference {
    Project(ProjectReference),
    Initiative(InitiativeReference),
}

/// Posts a status update: content from the flags, piped stdin, the editor,
/// or prompts, after the target is found.
pub fn create(ctx: &Ctx, target: Target<'_>, args: &StatusUpdateArgs) -> Result<()> {
    let terminal = ctx.stdin_tty() && ctx.stdout_tty();
    if args.interactive && !terminal {
        return Err(Error::new("Interactive mode needs a terminal")
            .with_hint("Pass --body, --body-file, or --health instead of --interactive."));
    }
    let interactive = args.interactive
        || (terminal && args.body.is_none() && args.body_file.is_none() && args.health.is_none());
    let body = match (&args.body, &args.body_file) {
        (Some(body), _) => Some(body.clone()),
        (None, Some(path)) => Some(text_input::read_file(path).map_err(|error| {
            Error::new(format!("Failed to read body file {path}: {error}")).with_source(error)
        })?),
        (None, None) if !ctx.stdin_tty() => text_input::read_stdin(std::io::stdin().lock())?,
        (None, None) => None,
    };
    let (original, reference) = match target {
        Target::Project(original) => (
            original,
            Reference::Project(prepare_project_lookup(original, &ctx.scope()?)?),
        ),
        Target::Initiative(original) => (
            original,
            Reference::Initiative(prepare_initiative_lookup(original, &ctx.scope()?)?),
        ),
    };
    let client = ctx.client()?;
    let id = ctx.spin(true, async {
        match &reference {
            Reference::Project(reference) => {
                resolve_project_with_transport(reference, original, client).await
            }
            Reference::Initiative(reference) => {
                resolve_initiative_with_transport(reference, original, client).await
            }
        }
    })?;
    let (body, health) = if interactive {
        let mut session = ctx.prompts()?;
        let result = prompt(ctx, &mut session, body, args.health);
        session.close()?;
        result?
    } else if body.is_none() && ctx.stdin_tty() && ctx.stdout_tty() {
        ctx.print("Opening editor for the update content...\n")?;
        let body = text_input::edited_body(&ctx.edit_text("")?);
        if body.is_none() {
            ctx.print("No content entered.\n")?;
        }
        (body, args.health)
    } else {
        (body, args.health)
    };
    let body = body.filter(|body| !body.trim().is_empty());
    let created = ctx.spin(true, submit(client, target, &id, body, health))?;
    let mut output = format!(
        "✓ Created status update for {}\n",
        created.name.as_deref().unwrap_or(original)
    );
    if let Some(health) = created.health {
        output.push_str(&format!("Health: {}\n", health.label()));
    }
    output.push_str(&format!("{}\n", created.url));
    ctx.print(output)
}

struct Created {
    name: Option<String>,
    health: Option<UpdateHealth>,
    url: String,
}

async fn submit(
    client: &GraphQlTransport,
    target: Target<'_>,
    id: &str,
    body: Option<String>,
    health: Option<Health>,
) -> Result<Created> {
    let (success, created) = match target {
        Target::Project(_) => {
            let request =
                GraphQlRequest::with_variables(CreateProjectUpdate::build(ProjectVariables {
                    input: ProjectInput {
                        project_id: id.to_owned(),
                        body,
                        health: health.map(|health| match health {
                            Health::OnTrack => ProjectHealthInput::OnTrack,
                            Health::AtRisk => ProjectHealthInput::AtRisk,
                            Health::OffTrack => ProjectHealthInput::OffTrack,
                        }),
                    },
                }));
            let data: CreateProjectUpdate = client.execute(&request).await?;
            let payload = data.project_update_create;
            let update = payload.project_update;
            (
                payload.success,
                Created {
                    name: update.project.map(|project| project.name),
                    health: update.health.as_ref().map(UpdateHealth::from),
                    url: update.url,
                },
            )
        }
        Target::Initiative(_) => {
            let request = GraphQlRequest::with_variables(CreateInitiativeUpdate::build(
                InitiativeVariables {
                    input: InitiativeInput {
                        initiative_id: id.to_owned(),
                        body,
                        health: health.map(|health| match health {
                            Health::OnTrack => InitiativeHealthInput::OnTrack,
                            Health::AtRisk => InitiativeHealthInput::AtRisk,
                            Health::OffTrack => InitiativeHealthInput::OffTrack,
                        }),
                    },
                },
            ));
            let data: CreateInitiativeUpdate = client.execute(&request).await?;
            let payload = data.initiative_update_create;
            let update = payload.initiative_update;
            (
                payload.success,
                Created {
                    name: update.initiative.map(|initiative| initiative.name),
                    health: update.health.as_ref().map(UpdateHealth::from),
                    url: update.url,
                },
            )
        }
    };
    if !success {
        return Err(Error::new("Linear did not create the status update"));
    }
    Ok(created)
}

/// Asks for the health and the content the flags left out.
fn prompt(
    ctx: &Ctx,
    session: &mut PromptSession<Stdin, StdoutWriter<'_>>,
    body: Option<String>,
    health: Option<Health>,
) -> Result<(Option<String>, Option<Health>)> {
    let health = match health {
        Some(health) => Some(health),
        None => {
            let options = [
                choice("Skip (no change)", "skip"),
                choice("On Track", "onTrack"),
                choice("At Risk", "atRisk"),
                choice("Off Track", "offTrack"),
            ];
            let selected = answer(session.select(&PlainSelect {
                message: "Health status",
                options: &options,
                default_index: 0,
                default_hint: None,
            })?)?;
            match selected.as_str() {
                "skip" => None,
                "onTrack" => Some(Health::OnTrack),
                "atRisk" => Some(Health::AtRisk),
                "offTrack" => Some(Health::OffTrack),
                other => unreachable!("health menu returned {other}"),
            }
        }
    };
    if body.is_some() {
        return Ok((body, health));
    }
    let methods = [
        choice("Skip (no content)", "skip"),
        choice("Enter inline", "inline"),
        choice("Open editor", "editor"),
        choice("Read from file", "file"),
    ];
    let method = answer(session.select(&PlainSelect {
        message: "How would you like to enter the update content?",
        options: &methods,
        default_index: 0,
        default_hint: None,
    })?)?;
    let body = match method.as_str() {
        "skip" => None,
        "inline" => text_input::edited_body(&answer(session.text_with_options(
            "Content (markdown)",
            TextOptions {
                required: false,
                default: None,
            },
        )?)?),
        "file" => {
            let path = answer(session.text_with_options(
                "File path",
                TextOptions {
                    required: true,
                    default: None,
                },
            )?)?;
            Some(text_input::read_file(&path).map_err(|error| {
                Error::new(format!("Failed to read {path}: {error}")).with_source(error)
            })?)
        }
        "editor" => {
            session.suspend()?;
            let edited = ctx.edit_text("");
            session.resume()?;
            let body = text_input::edited_body(&edited?);
            if let Some(body) = &body {
                session.print_line(&format!(
                    "Content entered ({} characters)",
                    body.chars().count()
                ))?;
            }
            body
        }
        other => unreachable!("content menu returned {other}"),
    };
    Ok((body, health))
}

fn choice(label: &str, value: &str) -> PlainOption {
    PlainOption {
        label: label.to_owned(),
        value: value.to_owned(),
        script_token: value.to_owned(),
    }
}

fn answer<T>(outcome: PromptOutcome<T>) -> Result<T> {
    match outcome {
        PromptOutcome::Submitted(answer) => Ok(answer),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => {
            Err(Error::new("Input ended before every question was answered"))
        }
    }
}

/// A status update's health as Linear reports it.
pub enum UpdateHealth {
    OnTrack,
    AtRisk,
    OffTrack,
    /// A health value newer than this program, shown as Linear spells it.
    Other(String),
}

impl UpdateHealth {
    fn label(&self) -> &str {
        match self {
            Self::OnTrack => "On Track",
            Self::AtRisk => "At Risk",
            Self::OffTrack => "Off Track",
            Self::Other(value) => value,
        }
    }

    fn paint(&self, text: &str, color: bool) -> String {
        match self {
            Self::OnTrack => style::green(text, color),
            Self::AtRisk => style::yellow(text, color),
            Self::OffTrack => style::red(text, color),
            Self::Other(_) => text.to_owned(),
        }
    }
}

impl From<&ProjectUpdateHealthType> for UpdateHealth {
    fn from(health: &ProjectUpdateHealthType) -> Self {
        match health {
            ProjectUpdateHealthType::OnTrack => Self::OnTrack,
            ProjectUpdateHealthType::AtRisk => Self::AtRisk,
            ProjectUpdateHealthType::OffTrack => Self::OffTrack,
            ProjectUpdateHealthType::Unknown(value) => Self::Other(value.clone()),
        }
    }
}

impl From<&InitiativeUpdateHealthType> for UpdateHealth {
    fn from(health: &InitiativeUpdateHealthType) -> Self {
        match health {
            InitiativeUpdateHealthType::OnTrack => Self::OnTrack,
            InitiativeUpdateHealthType::AtRisk => Self::AtRisk,
            InitiativeUpdateHealthType::OffTrack => Self::OffTrack,
            InitiativeUpdateHealthType::Unknown(value) => Self::Other(value.clone()),
        }
    }
}

/// One status update in a `list` table.
pub struct Row<'a> {
    pub id: &'a str,
    pub health: Option<UpdateHealth>,
    pub created_at: &'a str,
    pub author: &'a str,
    pub body: &'a str,
}

/// The status updates of `parent` (a project or initiative name) as a table,
/// each followed by a one-line preview of its content.
pub fn render_list(
    parent: &str,
    rows: &[Row<'_>],
    columns: usize,
    color: bool,
    now: DateTime<Utc>,
) -> String {
    if rows.is_empty() {
        return format!("No status updates found for {parent}\n");
    }
    let healths: Vec<_> = rows
        .iter()
        .map(|row| row.health.as_ref().map_or("-", UpdateHealth::label))
        .collect();
    let dates: Vec<_> = rows
        .iter()
        .map(|row| format_relative_time(row.created_at, now, &Local))
        .collect();
    let authors: Vec<_> = rows
        .iter()
        .map(|row| {
            if row.author.is_empty() {
                "-"
            } else {
                row.author
            }
        })
        .collect();
    let width = |cells: &[&str], header: usize| {
        cells
            .iter()
            .map(|cell| display_width(cell))
            .max()
            .unwrap_or(0)
            .max(header)
    };
    let health_width = width(&healths, 6);
    let date_width = width(&dates.iter().map(String::as_str).collect::<Vec<_>>(), 4);
    let author_width = width(&authors, 6);
    let preview_width = columns.saturating_sub(PREVIEW_INDENT.len() + 1).max(10);
    let mut output = format!("Status updates for {parent}\n\n");
    output.push_str(&underlined_header(
        &[
            pad("ID", 8),
            pad("HEALTH", health_width),
            pad("DATE", date_width),
            pad("AUTHOR", author_width),
        ],
        color,
    ));
    for (((row, health), date), author) in rows.iter().zip(&healths).zip(&dates).zip(&authors) {
        let short_id: String = row.id.chars().take(8).collect();
        let health = match &row.health {
            Some(value) => value.paint(&pad(health, health_width), color),
            None => pad(health, health_width),
        };
        output.push_str(&format!(
            "{} {health} {} {}\n",
            pad(&short_id, 8),
            style::gray(&pad(date, date_width), color),
            pad(author, author_width),
        ));
        let preview = row.body.split_whitespace().collect::<Vec<_>>().join(" ");
        if !preview.is_empty() {
            let preview = truncate_text(&preview, preview_width);
            output.push_str(&style::gray(&format!("{PREVIEW_INDENT}{preview}"), color));
            output.push('\n');
        }
    }
    output
}

const PREVIEW_INDENT: &str = "  ";
