//! `label create`: a workspace or team label, from flags or prompts.
use crate::cli::label::LabelCreate;
use crate::commands::color;
use crate::commands::confirm;
use crate::commands::outcome;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::label::{
    CreateIssueLabel, CreateIssueLabelPayload, CreateIssueLabelVariables, IssueLabelCreateInput,
};
use crate::platform::prompt::{Choice, Prompter, Text};
use crate::refs::{self, team::ResolvedTeam, team::TeamReference};

/// Where the label lives.
enum Team {
    Workspace,
    /// A `--team` reference, resolved before the label is created.
    Reference(TeamReference),
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
    let optional = ctx.optional_prompts(args.interactive)?;
    if args.name.is_none() && !ctx.interactive() {
        return Err(ctx.missing_value("Label name is required", "--name"));
    }
    let team = args
        .team
        .as_deref()
        .map(|team| TeamReference::parse(team, &ctx.scope()?))
        .transpose()?;
    let typed = args.name.is_none() || optional;
    let fields = if typed {
        prompt(ctx, args, team, optional)?
    } else {
        Fields {
            name: args.name.clone().expect("non-interactive runs have a name"),
            color: args
                .color
                .clone()
                .map_or_else(|| color::INDIGO.to_owned(), String::from),
            description: args.description.clone(),
            team: team.map_or(Team::Workspace, Team::Reference),
        }
    };
    let question = format!("Create label \"{}\"?", fields.name);
    if typed && !confirm::proceed(ctx, args.confirm.yes, &question)? {
        return Ok(());
    }
    let client = ctx.client()?;
    let created = ctx.spin(true, async {
        let team_id = match &fields.team {
            Team::Workspace => None,
            Team::Reference(lookup) => Some(refs::team::resolve(client, lookup).await?.id),
            Team::Picked(id) => Some(id.clone()),
        };
        client
            .mutate::<CreateIssueLabel, _>(CreateIssueLabelVariables {
                input: IssueLabelCreateInput {
                    name: fields.name.clone(),
                    color: fields.color.clone(),
                    description: fields.description.clone(),
                    team_id,
                },
            })
            .await
            .map_err(|failure| failure.into_create_error("label"))
    })?;
    ctx.print(render(&created.issue_label_create)?)
}

/// Asks for the name when it is missing, and with `optional` for every
/// other field not given as a flag. The team list is fetched between the
/// description and team prompts.
fn prompt(
    ctx: &Ctx,
    args: &LabelCreate,
    team: Option<TeamReference>,
    optional: bool,
) -> Result<Fields> {
    ctx.print("\nCreate a new label\n\n")?;
    let prompter = ctx.prompter()?;
    let name = match &args.name {
        Some(name) => name.clone(),
        None => prompter.text(Text::new("Label name:").required())?,
    };
    let color = match &args.color {
        Some(color) => color.clone().into(),
        None if optional => pick_color(&prompter)?,
        None => color::INDIGO.to_owned(),
    };
    let description = match &args.description {
        Some(description) => Some(description.clone()),
        None if optional => Some(prompter.text(Text::new("Description (optional):"))?)
            .filter(|description| !description.is_empty()),
        None => None,
    };
    let team = match team {
        Some(lookup) => Team::Reference(lookup),
        None if optional => {
            let teams = ctx.spin(true, refs::team::fetch_all(ctx.client()?))?;
            pick_team(&prompter, teams, configured_team_key(ctx.options()))?
        }
        None => Team::Workspace,
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
    let mut choices: Vec<_> = color::PALETTE
        .iter()
        .map(|(name, hex)| Choice::new(color::label(name, hex), Some(*hex)))
        .collect();
    choices.push(Choice::new("Custom color", None));
    let indigo = color::PALETTE
        .iter()
        .position(|(_, hex)| *hex == color::INDIGO)
        .expect("the palette has indigo");
    match prompter.select_from("Color:", choices, indigo)? {
        Some(hex) => Ok(hex.to_owned()),
        None => color::custom(prompter),
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
    let mut output = outcome::done("Created", "label", &label.name, None);
    output.push_str(&format!("  Color: {}\n", label.color));
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
