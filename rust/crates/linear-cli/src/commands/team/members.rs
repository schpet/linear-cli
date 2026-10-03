//! `team members`: a team's members, active ones only unless `--all`.
use super::TeamArg;
use crate::cli::team::TeamMembers;
use crate::client::LinearClient;
use crate::commands::user;
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::operations::organization_members::User;
use crate::graphql::operations::team_members::{GetTeamMembers, GetTeamMembersVariables};
use crate::graphql::pagination::{self, Page};

pub fn run(ctx: &Ctx, args: &TeamMembers) -> Result<()> {
    members(ctx, args).context("Failed to list team members")
}

fn members(ctx: &Ctx, args: &TeamMembers) -> Result<()> {
    let team = TeamArg::prepare(ctx, args.team.as_deref())?;
    let client = ctx.client()?;
    let members = ctx.spin(!args.json, async {
        let key = team.key(client).await?;
        fetch(client, &key, args.all).await
    })?;
    user::list::show(
        ctx,
        members,
        &user::list::Shown {
            all: args.all,
            limit: args.limit,
            json: args.json,
            place: "for this team",
        },
    )
}

/// Every member of the team, including disabled users with `all`.
async fn fetch(client: &LinearClient, team_key: &str, all: bool) -> Result<Vec<User>> {
    pagination::collect(None, |after, first| {
        let variables = GetTeamMembersVariables {
            team_key: team_key.to_owned(),
            include_disabled: all,
            first: Some(first),
            after,
        };
        async move {
            let data: GetTeamMembers = client.query(variables).await?;
            let page = data.team.members;
            Ok(Page {
                nodes: page.nodes,
                page_info: page.page_info,
            })
        }
    })
    .await
}
