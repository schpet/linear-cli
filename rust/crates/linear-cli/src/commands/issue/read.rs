//! Fetching issue lists: the server-side sort, paging and search.
use std::num::NonZeroU32;

use crate::client::LinearClient;
use crate::config::IssueSort;
use crate::error::Error;
use crate::graphql::operations::issue_read::*;
use crate::graphql::pagination::{self, Page};

pub fn sort_payload(priority: bool) -> Vec<IssueSortInput> {
    let mut sort = vec![IssueSortInput {
        workflow_state: Some(WorkflowStateSort {
            order: Some(PaginationSortOrder::Ascending),
        }),
        ..Default::default()
    }];
    if priority {
        sort.push(IssueSortInput {
            priority: Some(PrioritySort {
                order: Some(PaginationSortOrder::Descending),
                nulls: Some(PaginationNulls::Last),
            }),
            ..Default::default()
        });
    }
    sort.push(IssueSortInput {
        manual: Some(ManualSort {
            order: Some(PaginationSortOrder::Ascending),
            nulls: Some(PaginationNulls::Last),
        }),
        ..Default::default()
    });
    sort
}
pub async fn mine(
    client: &LinearClient,
    filter: IssueFilter,
    priority: bool,
    limit: Option<NonZeroU32>,
) -> Result<Vec<ListedIssue>, Error> {
    let mut rows = pagination::collect(limit, |after, first| {
        let variables = GetIssuesForStateVariables {
            sort: Some(sort_payload(priority)),
            filter: filter.clone(),
            first: Some(first),
            after,
        };
        async move {
            let data: GetIssuesForState = client.query(variables).await?;
            Ok(Page {
                nodes: data.issues.nodes,
                page_info: data.issues.page_info,
            })
        }
    })
    .await?;
    super::list_view::sort(&mut rows);
    Ok(rows)
}
pub async fn query(
    client: &LinearClient,
    filter: Option<IssueFilter>,
    priority: bool,
    limit: Option<NonZeroU32>,
    archived: bool,
) -> Result<Vec<ListedIssue>, Error> {
    let mut rows = pagination::collect(limit, |after, first| {
        let variables = GetIssuesForQueryVariables {
            sort: Some(sort_payload(priority)),
            filter: filter.clone(),
            first: Some(first),
            after,
            include_archived: archived.then_some(true),
        };
        async move {
            let data: GetIssuesForQuery = client.query(variables).await?;
            Ok(Page {
                nodes: data.issues.nodes,
                page_info: data.issues.page_info,
            })
        }
    })
    .await?;
    super::list_view::sort(&mut rows);
    Ok(rows)
}
pub async fn search(
    client: &LinearClient,
    filter: Option<IssueFilter>,
    term: String,
    limit: Option<NonZeroU32>,
    archived: bool,
    comments: bool,
) -> Result<Vec<SearchIssuesSearchIssuesNodes>, Error> {
    pagination::collect(limit, |after, first| {
        let variables = SearchIssuesVariables {
            term: term.clone(),
            filter: filter.clone(),
            first: Some(first),
            after,
            include_archived: archived.then_some(true),
            include_comments: comments.then_some(true),
            order_by: None,
        };
        async move {
            let data: SearchIssues = client.query(variables).await?;
            Ok(Page {
                nodes: data.search_issues.nodes,
                page_info: data.search_issues.page_info,
            })
        }
    })
    .await
}
impl From<SearchIssuesSearchIssuesNodes> for ListedIssue {
    /// A search hit as a listed issue; the search metadata is not shown in
    /// tables.
    fn from(r: SearchIssuesSearchIssuesNodes) -> Self {
        Self {
            id: r.id,
            identifier: r.identifier,
            title: r.title,
            url: r.url,
            priority: r.priority,
            priority_label: r.priority_label,
            estimate: r.estimate,
            created_at: r.created_at,
            updated_at: r.updated_at,
            state: r.state,
            assignee: r.assignee,
            team: r.team,
            project: r.project,
            project_milestone: r.project_milestone,
            cycle: r.cycle,
            labels: r.labels,
            inverse_relations: r.inverse_relations,
        }
    }
}

/// Whether issues sort by priority: `--sort`, else the configured sort.
pub(super) fn priority_sort(ctx: &crate::ctx::Ctx, sort: Option<IssueSort>) -> bool {
    ctx.options().issue_sort(sort).0 == IssueSort::Priority
}
