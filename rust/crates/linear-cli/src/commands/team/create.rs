//! `team create`: from flags, or from prompts when run in a terminal without
//! any field flag.
use std::io::{Read, Write};

use cynic::MutationBuilder;

use crate::cli::team::TeamCreate;
use crate::commands::milestone::create::outcome_unknown;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::team_create::{
    CreateTeam, CreateTeamVariables, CreatedTeam, TeamCreateInput,
};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};

pub fn run(ctx: &Ctx, args: &TeamCreate) -> Result<()> {
    create(ctx, args).context("Failed to create team")
}

fn create(ctx: &Ctx, args: &TeamCreate) -> Result<()> {
    let no_fields =
        args.name.is_none() && args.description.is_none() && args.key.is_none() && !args.private;
    let input = if no_fields && !args.no_interactive && ctx.stdin_tty() && ctx.stdout_tty() {
        prompt(ctx)?
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

fn prompt(ctx: &Ctx) -> Result<TeamCreateInput> {
    let mut session = ctx.prompts()?;
    let input = ask(&mut session);
    session.close()?;
    input
}

/// Asks for every field; empty optional answers are left out.
fn ask<R: Read, W: Write>(session: &mut PromptSession<R, W>) -> Result<TeamCreateInput> {
    let name = answer(session.text("Team name:", 0, |raw| {
        if raw.trim().is_empty() {
            Err("Team name is required".to_owned())
        } else {
            Ok(())
        }
    }))?;
    let description = answer(session.text("Team description (optional):", 0, |_| Ok(())))?;
    let key = answer(session.text(
        "Team key (optional, generated from the name if empty):",
        0,
        |_| Ok(()),
    ))?;
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
    let private = match answer(session.select(&PlainSelect {
        message: "Team visibility:",
        options: &visibility,
        default_index: 0,
        default_hint: Some("Public"),
    }))?
    .as_str()
    {
        "private" => true,
        "public" => false,
        other => unreachable!("the visibility prompt only offers its options, got {other:?}"),
    };
    Ok(TeamCreateInput {
        name,
        description: Some(description).filter(|value| !value.is_empty()),
        key: Some(key).filter(|value| !value.is_empty()),
        private: private.then_some(true),
    })
}

fn answer<T>(outcome: Result<PromptOutcome<T>>) -> Result<T> {
    match outcome? {
        PromptOutcome::Submitted(value) => Ok(value),
        PromptOutcome::Interrupted => Err(Error::cancelled()),
        PromptOutcome::EndOfInput => Err(Error::new(
            "Input ended before the team prompts were answered",
        )),
    }
}

/// Sends the mutation once. A failure after the request may have reached
/// Linear says the team may already exist; nothing is retried.
async fn submit(client: &GraphQlTransport, input: TeamCreateInput) -> Result<CreatedTeam> {
    let request = GraphQlRequest::with_variables(CreateTeam::build(CreateTeamVariables { input }));
    let result: CreateTeam = client.execute(&request).await.map_err(|failure| {
        let uncertain = outcome_unknown(&failure);
        let mut error = Error::from(failure);
        if uncertain {
            error.push_message("; team may already exist");
        }
        error
    })?;
    let payload = result.team_create;
    if !payload.success {
        return Err(Error::new("Linear did not create the team"));
    }
    payload
        .team
        .ok_or_else(|| Error::new("Linear reported success but returned no team"))
}
