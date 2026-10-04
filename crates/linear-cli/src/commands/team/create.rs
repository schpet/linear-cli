//! `team create`: from flags, asking for the name when it is missing and,
//! with `--interactive`, for the other fields too.
use crate::cli::team::TeamCreate;
use crate::client::LinearClient;
use crate::commands::outcome;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::operations::team::{
    CreateTeam, CreateTeamVariables, CreatedTeam, TeamCreateInput,
};
use crate::platform::prompt::{Choice, Prompter, Text};

pub fn run(ctx: &Ctx, args: &TeamCreate) -> Result<()> {
    create(ctx, args).context("Failed to create team")
}

fn create(ctx: &Ctx, args: &TeamCreate) -> Result<()> {
    let optional = ctx.optional_prompts(args.interactive)?;
    let input = if args.name.is_none() || optional {
        if !ctx.interactive() {
            return Err(ctx.missing_value("Team name is required", "--name"));
        }
        ask(&ctx.prompter()?, args, optional)?
    } else {
        TeamCreateInput {
            name: args.name.clone().expect("checked above"),
            description: args.description.clone(),
            key: args.key.clone(),
            private: args.private.then_some(true),
        }
    };
    let client = ctx.client()?;
    let team = ctx.spin(true, submit(client, input))?;
    ctx.print(outcome::done(
        "Created",
        "team",
        &format!("{}: {}", team.key, team.name),
        None,
    ))
}

/// Asks for the name when it is missing, and with `optional` for every other
/// field the flags left out; empty optional answers are left out.
fn ask(prompter: &Prompter<'_>, args: &TeamCreate, optional: bool) -> Result<TeamCreateInput> {
    let name = match &args.name {
        Some(name) => name.clone(),
        None => prompter.text(Text::new("Team name:").required())?,
    };
    let description = match &args.description {
        Some(description) => Some(description.clone()),
        None if optional => Some(prompter.text(Text::new("Team description (optional):"))?)
            .filter(|value| !value.is_empty()),
        None => None,
    };
    let key = match &args.key {
        Some(key) => Some(key.clone()),
        None if optional => Some(prompter.text(Text::new(
            "Team key (optional, generated from the name if empty):",
        ))?)
        .filter(|value| !value.is_empty()),
        None => None,
    };
    let private = args.private
        || (optional
            && prompter.select(
                "Team visibility:",
                vec![Choice::new("Public", false), Choice::new("Private", true)],
            )?);
    Ok(TeamCreateInput {
        name,
        description,
        key,
        private: private.then_some(true),
    })
}

/// Sends the mutation once. A failure after the request may have reached
/// Linear says the team may already exist; nothing is retried.
async fn submit(client: &LinearClient, input: TeamCreateInput) -> Result<CreatedTeam> {
    let result: CreateTeam = client
        .mutate(CreateTeamVariables { input })
        .await
        .map_err(|failure| failure.into_create_error("team"))?;
    let payload = result.team_create;
    if !payload.success {
        return Err(Error::new("Linear did not create the team"));
    }
    payload
        .team
        .ok_or_else(|| Error::new("Linear reported success but returned no team"))
}
