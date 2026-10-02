//! `team delete`: optionally move the team's issues elsewhere, then delete it.
use crate::{
    error::Error,
    graphql::{edit::Edit, envelope::GraphQlRequest, operations::team_delete::*},
};
use cynic::{MutationBuilder, QueryBuilder};
use std::{collections::HashSet, future::Future};
pub const CONTEXT: &str = "Failed to delete team";
pub const MOVE_CONTEXT: &str = "Failed to move issues";
pub fn details_request(id: &str) -> GraphQlRequest<IdVariables> {
    GraphQlRequest::with_variables(GetTeamDetails::build(IdVariables { id: id.to_owned() }))
}
pub fn delete_request(id: &str) -> GraphQlRequest<IdVariables> {
    GraphQlRequest::with_variables(DeleteTeam::build(IdVariables { id: id.to_owned() }))
}
pub fn move_request(id: &str, team_id: &str) -> GraphQlRequest<MoveVariables> {
    GraphQlRequest::with_variables(MoveIssueToTeam::build(MoveVariables {
        id: id.to_owned(),
        team_id: team_id.to_owned(),
    }))
}
pub fn page_request(team_id: &str, after: Edit<String>) -> GraphQlRequest<MovePageVariables> {
    GraphQlRequest::with_variables(GetTeamIssuesForMove::build(MovePageVariables {
        team_id: team_id.to_owned(),
        first: Some(100),
        after,
    }))
}
pub async fn all_issues<F, Fut>(team_id: &str, mut fetch: F) -> Result<Vec<MoveIssue>, Error>
where
    F: FnMut(GraphQlRequest<MovePageVariables>) -> Fut,
    Fut: Future<Output = Result<GetTeamIssuesForMove, Error>>,
{
    let mut nodes = Vec::new();
    let mut after = Edit::Unchanged;
    let mut seen = HashSet::new();
    loop {
        let data = fetch(page_request(team_id, after)).await?;
        let Some(team) = data.team else { break };
        let issues = team.issues;
        nodes.extend(issues.nodes);
        if !issues.page_info.has_next_page {
            break;
        }
        let cursor = issues.page_info.end_cursor.ok_or_else(|| {
            Error::new("Linear reported more team issues but returned no pagination cursor")
        })?;
        if !seen.insert(cursor.clone()) {
            return Err(Error::new("Linear repeated a team issue pagination cursor"));
        }
        after = Edit::Set(cursor);
    }
    Ok(nodes)
}
/// `success: false` is ignored: every update that does not error counts as moved.
/// No delete/confirmation is permitted until this sequential future returns.
pub async fn move_all<F, Fut, P>(
    issues: &[MoveIssue],
    target: &str,
    mut submit: F,
    mut progress: P,
) -> Result<usize, Error>
where
    F: FnMut(GraphQlRequest<MoveVariables>) -> Fut,
    Fut: Future<Output = Result<MoveIssueToTeam, Error>>,
    P: FnMut(usize, usize) -> Result<(), Error>,
{
    let mut moved = 0;
    for issue in issues {
        let _response = submit(move_request(issue.id.inner(), target)).await?;
        moved += 1;
        progress(moved, issues.len())?
    }
    Ok(moved)
}
pub fn warning(team: &TeamDetails) -> Vec<u8> {
    format!("\n⚠️  Team {} ({}) has {} issue(s).\nYou must move these issues to another team before deletion.\n\n",team.key,team.name,team.issues.nodes.len()).into_bytes()
}
pub fn deleted(team: &TeamDetails) -> Vec<u8> {
    format!("✓ Successfully deleted team: {}: {}\n", team.key, team.name).into_bytes()
}
