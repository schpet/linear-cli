//! `linear team`.
mod autolinks;
mod create;
mod delete;
mod id;
mod list;
mod members;
mod states;

use crate::cli::team::TeamCommand;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::transport::GraphQlTransport;
use crate::refs::{PreparedTeamLookup, prepare_team_lookup, resolve_team_with_transport};

pub fn run(ctx: &Ctx, command: &TeamCommand) -> Result<()> {
    match command {
        TeamCommand::Create(args) => create::run(ctx, args),
        TeamCommand::Delete(args) => delete::run(ctx, args),
        TeamCommand::List(args) => list::run(ctx, args),
        TeamCommand::Id(args) => id::run(ctx, args),
        TeamCommand::Autolinks(args) => autolinks::run(ctx, args),
        TeamCommand::Members(args) => members::run(ctx, args),
        TeamCommand::States(args) => states::run(ctx, args),
    }
}

/// A team given as a command argument (looked up by key, name, or ID), or
/// else the configured team key, which is used as is.
enum TeamArg {
    Lookup(PreparedTeamLookup),
    Configured(String),
}

impl TeamArg {
    fn prepare(ctx: &Ctx, team: Option<&str>) -> Result<Self> {
        match team {
            Some(team) => Ok(Self::Lookup(prepare_team_lookup(team, &ctx.scope()?)?)),
            None => configured_team_key(ctx.options())
                .map(Self::Configured)
                .ok_or_else(|| {
                    Error::new("No team given and none is configured").with_hint(
                        "Pass a team key, name, or ID, or run `linear config` to set a default team.",
                    )
                }),
        }
    }

    async fn key(self, client: &GraphQlTransport) -> Result<String> {
        match self {
            Self::Lookup(lookup) => Ok(resolve_team_with_transport(&lookup, client).await?.key),
            Self::Configured(key) => Ok(key),
        }
    }
}
