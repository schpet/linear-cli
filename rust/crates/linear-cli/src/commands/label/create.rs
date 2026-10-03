//! `label create`: a workspace or team label, from flags or prompts.
use cynic::MutationBuilder;

use crate::cli::label::LabelCreate;
use crate::cli::values::hex_color;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::label_create::{
    CreateIssueLabel, CreateIssueLabelPayload, CreateIssueLabelVariables, IssueLabelCreateInput,
};
use crate::platform::prompt::{Choice, Prompter, Text};
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
            .map_err(|failure| failure.into_create_error("label"))
    })?;
    ctx.print(render(&created.issue_label_create)?)
}

/// Asks for every field not given as a flag. The team list is fetched between
/// the description and team prompts.
fn prompt(ctx: &Ctx, args: &LabelCreate, team: Option<PreparedTeamLookup>) -> Result<Fields> {
    ctx.print("\nCreate a new label\n\n")?;
    let prompter = ctx.prompter()?;
    let name = match &args.name {
        Some(name) => name.clone(),
        None => prompter.text(Text::new("Label name:").required())?,
    };
    let color = match &args.color {
        Some(color) => color.clone(),
        None => pick_color(&prompter)?,
    };
    let description = match &args.description {
        Some(description) => Some(description.clone()),
        None => Some(prompter.text(Text::new("Description (optional):"))?)
            .filter(|description| !description.is_empty()),
    };
    let team = match team {
        Some(lookup) => Team::Reference(lookup),
        None => {
            let teams = ctx.spin(true, fetch_all_teams_with_transport(ctx.client()?))?;
            pick_team(&prompter, teams, configured_team_key(ctx.options()))?
        }
    };
    Ok(Fields {
        name,
        color,
        description,
        team,
    })
}

/// A palette color, or a custom hex color; Indigo is the default.
fn pick_color(prompter: &Prompter<'_>) -> Result<String> {
    let mut choices: Vec<_> = PALETTE
        .iter()
        .map(|(name, color)| Choice::new(format!("{name} ({color})"), Some(*color)))
        .collect();
    choices.push(Choice::new("Custom color", None));
    let indigo = PALETTE
        .iter()
        .position(|(_, color)| *color == INDIGO)
        .expect("the palette has indigo");
    match prompter.select_from("Color:", choices, indigo)? {
        Some(color) => Ok(color.to_owned()),
        None => {
            let check = |raw: &str| {
                hex_color(raw)
                    .map(drop)
                    .map_err(|_| "Please enter a valid hex color (e.g., #FF5733)".to_owned())
            };
            prompter.text(
                Text::new("Enter hex color (e.g., #FF5733):")
                    .required()
                    .with_check(&check),
            )
        }
    }
}

/// The workspace first, then the teams; the configured team is the default.
fn pick_team(
    prompter: &Prompter<'_>,
    teams: Vec<ResolvedTeam>,
    configured_key: Option<String>,
) -> Result<Team> {
    let start = configured_key
        .and_then(|key| teams.iter().position(|team| team.key == key))
        .map_or(0, |index| index + 1);
    let mut choices = vec![Choice::new(
        "Workspace (shared by all teams)",
        Team::Workspace,
    )];
    choices.extend(teams.into_iter().map(|team| {
        Choice::new(
            format!("{} ({})", team.name, team.key),
            Team::Picked(team.id),
        )
    }));
    prompter.select_from("Team:", choices, start)
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
