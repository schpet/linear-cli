//! `team create`: from flags, or from prompts when run in a terminal without
//! any field flag.
use crate::cli::team::TeamCreate;
use crate::client::LinearClient;
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
    let no_fields =
        args.name.is_none() && args.description.is_none() && args.key.is_none() && !args.private;
    let input = if no_fields && !args.no_interactive && ctx.interactive() {
        ask(&ctx.prompter()?)?
    } else {
        TeamCreateInput {
            name: args.name.clone().ok_or_else(|| {
                Error::new("Team name is required")
                    .with_hint("Pass --name, or run without flags in a terminal to be prompted.")
            })?,
            description: args.description.clone(),
            key: args.key.clone(),
            private: args.private.then_some(true),
        }
    };
    let client = ctx.client()?;
    let team = ctx.spin(true, submit(client, input))?;
    ctx.print(format!("✓ Created team {}: {}\n", team.key, team.name))
}

/// Asks for every field; empty optional answers are left out.
fn ask(prompter: &Prompter<'_>) -> Result<TeamCreateInput> {
    let name = prompter.text(Text::new("Team name:").required())?;
    let description = prompter.text(Text::new("Team description (optional):"))?;
    let key = prompter.text(Text::new(
        "Team key (optional, generated from the name if empty):",
    ))?;
    let private = prompter.select(
        "Team visibility:",
        vec![Choice::new("Public", false), Choice::new("Private", true)],
    )?;
    Ok(TeamCreateInput {
        name,
        description: Some(description).filter(|value| !value.is_empty()),
        key: Some(key).filter(|value| !value.is_empty()),
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
