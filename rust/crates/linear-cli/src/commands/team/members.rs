//! `team members`: a team's members, active ones only unless `--all`.
use chrono::Utc;
use cynic::QueryBuilder;
use serde::Serialize;

use super::TeamArg;
use crate::cli::team::TeamMembers;
use crate::commands::user;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::organization_members::User;
use crate::graphql::operations::team_members::{GetTeamMembers, GetTeamMembersVariables};
use crate::graphql::operations::teams::PageInfo;
use crate::graphql::pagination::{self, Page, PaginationError};
use crate::graphql::transport::GraphQlTransport;
use crate::platform::collation;

pub fn run(ctx: &Ctx, args: &TeamMembers) -> Result<()> {
    members(ctx, args).context("Failed to list team members")
}

fn members(ctx: &Ctx, args: &TeamMembers) -> Result<()> {
    let team = TeamArg::prepare(ctx, args.team.as_deref())?;
    let client = ctx.client()?;
    let (mut members, page_info) = ctx.spin(!args.json, async {
        let key = team.key(client).await?;
        fetch(client, &key, args.all).await
    })?;
    members.sort_by(|left, right| {
        collation::compare(
            &left.display_name.to_lowercase(),
            &right.display_name.to_lowercase(),
        )
    });
    let fetched = members.len();
    if !args.all {
        members.retain(|member| member.active);
    }
    if args.json {
        ctx.print(render_json(&members, &page_info))
    } else if fetched == 0 {
        ctx.print("No members found for this team.\n")
    } else if members.is_empty() {
        ctx.print("No active members found for this team. Use --all to include inactive members.\n")
    } else {
        ctx.print(user::list::table(&members, Utc::now()).render_for(ctx))
    }
}

/// Every member of the team, including disabled users with `all`.
async fn fetch(
    client: &GraphQlTransport,
    team_key: &str,
    all: bool,
) -> Result<(Vec<User>, PageInfo)> {
    let result = pagination::paginate(|after| {
        let request =
            GraphQlRequest::with_variables(GetTeamMembers::build(GetTeamMembersVariables {
                team_key: team_key.to_owned(),
                include_disabled: all,
                first: Some(100),
                after,
            }));
        async move {
            let data: GetTeamMembers = client.execute(&request).await?;
            let page = data.team.members;
            Ok::<Page<User>, Error>(Page {
                nodes: page.nodes,
                page_info: page.page_info.into(),
            })
        }
    })
    .await
    .map_err(|error| match error {
        PaginationError::Fetch { source, .. } => source,
        PaginationError::MissingCursor { .. } => {
            Error::new("Linear reported more team members but returned no pagination cursor")
                .with_hint("Retry the command.")
        }
        PaginationError::RepeatedCursor { page, .. } => Error::new(format!(
            "Linear repeated a team member pagination cursor on page {page}"
        ))
        .with_hint("Retry the command."),
    })?;
    let page_info = PageInfo {
        has_next_page: result.page_info.has_next_page,
        end_cursor: result.page_info.end_cursor,
    };
    Ok((result.nodes, page_info))
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JsonConnection<'a> {
    nodes: &'a [User],
    page_info: &'a PageInfo,
}

fn render_json(members: &[User], page_info: &PageInfo) -> Vec<u8> {
    let mut output = serde_json::to_vec_pretty(&JsonConnection {
        nodes: members,
        page_info,
    })
    .expect("team member JSON always serializes");
    output.push(b'\n');
    output
}
