//! `label create`: a workspace or team label, from flags or prompts.
use std::io::{Read, Write};

use cynic::MutationBuilder;

use crate::cli::label::{LabelCreate, hex_color};
use crate::commands::milestone::create::outcome_unknown;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::label_create::{
    CreateIssueLabel, CreateIssueLabelPayload, CreateIssueLabelVariables, IssueLabelCreateInput,
};
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};
use crate::refs::{
    PreparedTeamLookup, ResolvedTeam, fetch_all_teams_with_transport, prepare_team_lookup,
    resolve_team_with_transport,
};

const INDIGO: &str = "#5E6AD2";
const PALETTE: [(&str, &str); 10] = [
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
const CUSTOM_COLOR: &str = "custom";
const WORKSPACE: &str = "workspace";

/// Where the label lives.
enum Team {
    Workspace,
    /// A `--team` reference, resolved before the label is created.
    Reference(PreparedTeamLookup),
    /// A team picked at the prompt.
    Picked(String),
}

struct Fields {
    name: String,
    color: String,
    description: Option<String>,
    team: Team,
}

pub fn run(ctx: &Ctx, args: &LabelCreate) -> Result<()> {
    create(ctx, args).context("Failed to create label")
}

fn create(ctx: &Ctx, args: &LabelCreate) -> Result<()> {
    let interactive = args.interactive || args.name.is_none();
    if interactive && !ctx.stdin_tty() {
        return Err(match args.name {
            None => Error::new("Label name is required")
                .with_hint("Pass --name, or run in a terminal to be prompted."),
            Some(_) => Error::new("--interactive needs a terminal"),
        });
    }
    let team = args
        .team
        .as_deref()
        .map(|team| prepare_team_lookup(team, &ctx.scope()?))
        .transpose()?;
    let fields = if interactive {
        prompt(ctx, args, team)?
    } else {
        Fields {
            name: args.name.clone().expect("non-interactive runs have a name"),
            color: args.color.clone().unwrap_or_else(|| INDIGO.to_owned()),
            description: args.description.clone(),
            team: team.map_or(Team::Workspace, Team::Reference),
        }
    };
    let client = ctx.client()?;
    let created = ctx.spin(true, async {
        let team_id = match &fields.team {
            Team::Workspace => None,
            Team::Reference(lookup) => Some(resolve_team_with_transport(lookup, client).await?.id),
            Team::Picked(id) => Some(id.clone()),
        };
        let request =
            GraphQlRequest::with_variables(CreateIssueLabel::build(CreateIssueLabelVariables {
                input: IssueLabelCreateInput {
                    name: fields.name.clone(),
                    color: fields.color.clone(),
                    description: fields.description.clone(),
                    team_id,
                },
            }));
        client
            .execute::<CreateIssueLabel, _>(&request)
            .await
            .map_err(|failure| {
                let uncertain = outcome_unknown(&failure);
                let mut error = Error::from(failure);
                if uncertain {
                    error.push_message("; the label may have been created");
                }
                error
            })
    })?;
    ctx.print(render(&created.issue_label_create)?)
}

/// Asks for every field not given as a flag. The team list is fetched between
/// the description and team prompts.
fn prompt(ctx: &Ctx, args: &LabelCreate, team: Option<PreparedTeamLookup>) -> Result<Fields> {
    ctx.print("\nCreate a new label\n\n")?;
    let mut session = ctx.prompts()?;
    let result = prompt_with(ctx, args, team, &mut session);
    match session.finish_result(result)? {
        PromptOutcome::Submitted(fields) => Ok(fields),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new("Unexpected end of input at a prompt")),
    }
}

fn prompt_with<R: Read, W: Write>(
    ctx: &Ctx,
    args: &LabelCreate,
    team: Option<PreparedTeamLookup>,
    session: &mut PromptSession<R, W>,
) -> Result<PromptOutcome<Fields>> {
    macro_rules! answer {
        ($outcome:expr) => {
            match $outcome? {
                PromptOutcome::Submitted(value) => value,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
            }
        };
    }
    let name = match &args.name {
        Some(name) => name.clone(),
        None => answer!(session.text("Label name:", 1, |_| Ok(()))),
    };
    let color = match &args.color {
        Some(color) => color.clone(),
        None => {
            let mut choices: Vec<_> = PALETTE
                .iter()
                .map(|(name, color)| option(&format!("{name} ({color})"), color))
                .collect();
            choices.push(option("Custom color", CUSTOM_COLOR));
            let indigo = 6;
            let selected = answer!(session.select(&PlainSelect {
                message: "Color:",
                options: &choices,
                default_index: indigo,
                default_hint: choices.get(indigo).map(|choice| choice.label.as_str()),
            }));
            if selected == CUSTOM_COLOR {
                answer!(session.text("Enter hex color (e.g., #FF5733):", 0, |raw| {
                    hex_color(raw)
                        .map(drop)
                        .map_err(|_| "Please enter a valid hex color (e.g., #FF5733)".to_owned())
                }))
            } else {
                selected
            }
        }
    };
    let description = match &args.description {
        Some(description) => Some(description.clone()),
        None => Some(answer!(
            session.text("Description (optional):", 0, |_| Ok(()))
        ))
        .filter(|description| !description.is_empty()),
    };
    let team = match team {
        Some(lookup) => Team::Reference(lookup),
        None => {
            session.suspend()?;
            let client = ctx.client()?;
            let teams = ctx.spin(true, fetch_all_teams_with_transport(client))?;
            session.resume()?;
            answer!(pick_team(
                session,
                &teams,
                configured_team_key(ctx.options())
            ))
        }
    };
    Ok(PromptOutcome::Submitted(Fields {
        name,
        color,
        description,
        team,
    }))
}

fn option(label: &str, value: &str) -> PlainOption {
    PlainOption {
        label: label.to_owned(),
        value: value.to_owned(),
        script_token: value.to_owned(),
    }
}

/// The workspace first, then the teams; the configured team is the default.
fn pick_team<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    teams: &[ResolvedTeam],
    configured_key: Option<String>,
) -> Result<PromptOutcome<Team>> {
    let mut choices = vec![option("Workspace (shared by all teams)", WORKSPACE)];
    choices.extend(
        teams
            .iter()
            .map(|team| option(&format!("{} ({})", team.name, team.key), &team.id)),
    );
    let default_index = configured_key
        .and_then(|key| teams.iter().position(|team| team.key == key))
        .map_or(0, |index| index + 1);
    let picked = session.select(&PlainSelect {
        message: "Team:",
        options: &choices,
        default_index,
        default_hint: choices
            .get(default_index)
            .map(|choice| choice.label.as_str()),
    })?;
    Ok(match picked {
        PromptOutcome::Submitted(id) if id == WORKSPACE => {
            PromptOutcome::Submitted(Team::Workspace)
        }
        PromptOutcome::Submitted(id) => PromptOutcome::Submitted(Team::Picked(id)),
        PromptOutcome::Interrupted => PromptOutcome::Interrupted,
        PromptOutcome::EndOfInput => PromptOutcome::EndOfInput,
    })
}

fn render(payload: &CreateIssueLabelPayload) -> Result<String> {
    if !payload.success {
        return Err(Error::new("Linear did not create the label"));
    }
    let label = &payload.issue_label;
    let mut output = format!(
        "✓ Created label: {}\n  Color: {}\n",
        label.name, label.color
    );
    if let Some(description) = label.description.as_deref().filter(|text| !text.is_empty()) {
        output.push_str(&format!("  Description: {description}\n"));
    }
    let scope = match &label.team {
        Some(team) if !team.name.is_empty() => format!("{} ({})", team.name, team.key),
        Some(_) | None => "Workspace".to_owned(),
    };
    output.push_str(&format!("  Scope: {scope}\n"));
    Ok(output)
}
