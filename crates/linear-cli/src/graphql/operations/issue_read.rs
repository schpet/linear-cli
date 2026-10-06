//! Typed selections for issue list/query/view.

use serde::Serialize;

use super::project::ProjectRef;
use super::team::TeamKey;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::scalars::DateTimeOrDuration;
use crate::graphql::scalars::Float;
use crate::graphql::scalars::JsonObject;
use crate::graphql::scalars::WholeNumber;
use crate::graphql::schema;

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "IssueFilter")]
pub struct IssueFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub state: Option<WorkflowStateFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub assignee: Option<NullableUserFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub project: Option<NullableProjectFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub cycle: Option<NullableCycleFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub project_milestone: Option<NullableProjectMilestoneFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub labels: Option<IssueLabelCollectionFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub created_at: Option<DateComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateComparator>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "TeamFilter")]
pub struct TeamFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub key: Option<StringComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub or: Option<Vec<TeamFilter>>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "WorkflowStateFilter")]
pub struct WorkflowStateFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub team: Option<TeamFilter>,
    #[cynic(rename = "type", skip_serializing_if = "Option::is_none")]
    pub r#type: Option<StringComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub id: Option<IDComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub or: Option<Vec<WorkflowStateFilter>>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "StringComparator")]
pub struct StringComparator {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub eq: Option<String>,
    #[cynic(rename = "in", skip_serializing_if = "Option::is_none")]
    pub r#in: Option<Vec<String>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub eq_ignore_case: Option<String>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "IDComparator")]
pub struct IDComparator {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub eq: Option<cynic::Id>,
    #[cynic(rename = "in", skip_serializing_if = "Option::is_none")]
    pub r#in: Option<Vec<cynic::Id>>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "EntityIdentifierIDComparator")]
pub struct EntityIdentifierIDComparator {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub eq: Option<cynic::Id>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "NullableUserFilter")]
pub struct NullableUserFilter {
    #[cynic(rename = "null", skip_serializing_if = "Option::is_none")]
    pub r#null: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub id: Option<IDComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub is_me: Option<BooleanComparator>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "BooleanComparator")]
pub struct BooleanComparator {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub eq: Option<bool>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "NullableProjectFilter")]
pub struct NullableProjectFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub id: Option<EntityIdentifierIDComparator>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub labels: Option<ProjectLabelCollectionFilter>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "ProjectLabelCollectionFilter")]
pub struct ProjectLabelCollectionFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<StringComparator>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "NullableCycleFilter")]
pub struct NullableCycleFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub id: Option<IDComparator>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "NullableProjectMilestoneFilter")]
pub struct NullableProjectMilestoneFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub id: Option<IDComparator>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "IssueLabelCollectionFilter")]
pub struct IssueLabelCollectionFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub some: Option<IssueLabelFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub and: Option<Vec<IssueLabelCollectionFilter>>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "IssueLabelFilter")]
pub struct IssueLabelFilter {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub name: Option<StringComparator>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "DateComparator")]
pub struct DateComparator {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub gte: Option<DateTimeOrDuration>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "IssueSortInput")]
pub struct IssueSortInput {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub workflow_state: Option<WorkflowStateSort>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub priority: Option<PrioritySort>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub manual: Option<ManualSort>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "WorkflowStateSort")]
pub struct WorkflowStateSort {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub order: Option<PaginationSortOrder>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "PrioritySort")]
pub struct PrioritySort {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub nulls: Option<PaginationNulls>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub order: Option<PaginationSortOrder>,
}

#[derive(cynic::InputObject, Clone, Debug, Default)]
#[cynic(schema = "linear", graphql_type = "ManualSort")]
pub struct ManualSort {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub nulls: Option<PaginationNulls>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub order: Option<PaginationSortOrder>,
}

#[derive(cynic::Enum, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "PaginationSortOrder")]
pub enum PaginationSortOrder {
    #[cynic(rename = "Ascending")]
    Ascending,
    #[cynic(rename = "Descending")]
    Descending,
}

#[derive(cynic::Enum, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "PaginationNulls")]
pub enum PaginationNulls {
    #[cynic(rename = "first")]
    First,
    #[cynic(rename = "last")]
    Last,
}

#[derive(cynic::Enum, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "PaginationOrderBy")]
pub enum PaginationOrderBy {
    #[cynic(rename = "createdAt")]
    CreatedAt,
    #[cynic(rename = "updatedAt")]
    UpdatedAt,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetIssuesForStateVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Vec<IssueSortInput>>,
    pub filter: IssueFilter,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "WorkflowState")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForStateIssuesNodesState {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub r#type: String,
    pub position: Float,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Cycle")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForStateIssuesNodesCycle {
    pub id: cynic::Id,
    pub number: WholeNumber,
    pub name: Option<String>,
    pub is_active: bool,
    pub is_next: bool,
    pub is_previous: bool,
    pub is_future: bool,
    pub is_past: bool,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Cycle")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForStateIssuesNodesTeamActiveCycle {
    pub number: WholeNumber,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabel")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForStateIssuesNodesLabelsNodes {
    pub id: cynic::Id,
    pub name: String,
    pub color: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabelConnection")]
#[serde(transparent)]
pub struct GetIssuesForStateIssuesNodesLabels {
    pub nodes: Vec<GetIssuesForStateIssuesNodesLabelsNodes>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "WorkflowState")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForStateIssuesNodesInverseRelationsNodesIssueState {
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub r#type: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForStateIssuesNodesInverseRelationsNodesIssue {
    pub id: cynic::Id,
    pub identifier: String,
    pub state: GetIssuesForStateIssuesNodesInverseRelationsNodesIssueState,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueRelation")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForStateIssuesNodesInverseRelationsNodes {
    pub id: cynic::Id,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub r#type: String,
    pub issue: GetIssuesForStateIssuesNodesInverseRelationsNodesIssue,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueRelationConnection")]
#[serde(transparent)]
pub struct GetIssuesForStateIssuesNodesInverseRelations {
    pub nodes: Vec<GetIssuesForStateIssuesNodesInverseRelationsNodes>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetIssuesForStateVariables"
)]
pub struct GetIssuesForState {
    #[arguments(filter : $filter, sort : $sort, first : $first, after : $after)]
    pub issues: ListedIssues,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetIssuesForQueryVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub sort: Option<Vec<IssueSortInput>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<IssueFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub include_archived: Option<bool>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForQueryIssuesNodesAssignee {
    pub id: cynic::Id,
    pub name: String,
    pub display_name: String,
    pub initials: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForQueryIssuesNodesTeam {
    pub id: cynic::Id,
    pub key: String,
    pub name: String,
    pub cycles_enabled: bool,
    pub active_cycle: Option<GetIssuesForStateIssuesNodesTeamActiveCycle>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestone")]
#[serde(rename_all = "camelCase")]
pub struct GetIssuesForQueryIssuesNodesProjectMilestone {
    pub id: cynic::Id,
    pub name: String,
}
/// One issue in `issue list`, `issue query` and `issue start` lists.
#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
#[serde(rename_all = "camelCase")]
pub struct ListedIssue {
    pub id: cynic::Id,
    pub identifier: String,
    pub title: String,
    pub url: String,
    pub priority: WholeNumber,
    pub priority_label: String,
    pub estimate: Option<Float>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub state: GetIssuesForStateIssuesNodesState,
    pub assignee: Option<GetIssuesForQueryIssuesNodesAssignee>,
    pub team: GetIssuesForQueryIssuesNodesTeam,
    pub project: Option<ProjectRef>,
    pub project_milestone: Option<GetIssuesForQueryIssuesNodesProjectMilestone>,
    pub cycle: Option<GetIssuesForStateIssuesNodesCycle>,
    pub labels: GetIssuesForStateIssuesNodesLabels,
    #[arguments(first : 100)]
    pub inverse_relations: GetIssuesForStateIssuesNodesInverseRelations,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueConnection")]
pub struct ListedIssues {
    pub nodes: Vec<ListedIssue>,
    pub page_info: crate::graphql::pagination::PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetIssuesForQueryVariables"
)]
pub struct GetIssuesForQuery {
    #[arguments(filter : $filter, sort : $sort, first : $first, after : $after, includeArchived : $include_archived)]
    pub issues: ListedIssues,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct SearchIssuesVariables {
    pub term: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<IssueFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub include_archived: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub include_comments: Option<bool>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub order_by: Option<PaginationOrderBy>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueSearchResult")]
#[serde(rename_all = "camelCase")]
pub struct SearchIssuesSearchIssuesNodes {
    pub id: cynic::Id,
    pub identifier: String,
    pub title: String,
    pub url: String,
    pub priority: WholeNumber,
    pub priority_label: String,
    pub estimate: Option<Float>,
    pub created_at: DateTime,
    pub updated_at: DateTime,
    pub state: GetIssuesForStateIssuesNodesState,
    pub assignee: Option<GetIssuesForQueryIssuesNodesAssignee>,
    pub team: GetIssuesForQueryIssuesNodesTeam,
    pub project: Option<ProjectRef>,
    pub project_milestone: Option<GetIssuesForQueryIssuesNodesProjectMilestone>,
    pub cycle: Option<GetIssuesForStateIssuesNodesCycle>,
    pub labels: GetIssuesForStateIssuesNodesLabels,
    #[arguments(first : 100)]
    pub inverse_relations: GetIssuesForStateIssuesNodesInverseRelations,
    pub metadata: JsonObject,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueSearchPayload")]
pub struct SearchIssuesSearchIssues {
    pub nodes: Vec<SearchIssuesSearchIssuesNodes>,
    pub page_info: crate::graphql::pagination::PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "SearchIssuesVariables"
)]
pub struct SearchIssues {
    #[arguments(term : $term, filter : $filter, first : $first, after : $after, includeArchived : $include_archived, includeComments : $include_comments, orderBy : $order_by)]
    pub search_issues: SearchIssuesSearchIssues,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetWorkflowStatesInScopeVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<WorkflowStateFilter>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub first: Option<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "WorkflowState")]
#[serde(rename_all = "camelCase")]
pub struct GetWorkflowStatesInScopeWorkflowStatesNodes {
    pub id: cynic::Id,
    pub name: String,
    #[cynic(rename = "type")]
    #[serde(rename = "type")]
    pub r#type: String,
    pub position: crate::graphql::scalars::Float,
    pub team: TeamKey,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "WorkflowStateConnection")]
pub struct GetWorkflowStatesInScopeWorkflowStates {
    pub nodes: Vec<GetWorkflowStatesInScopeWorkflowStatesNodes>,
    pub page_info: crate::graphql::pagination::PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetWorkflowStatesInScopeVariables"
)]
pub struct GetWorkflowStatesInScope {
    #[arguments(filter : $filter, first : $first, after : $after)]
    pub workflow_states: GetWorkflowStatesInScopeWorkflowStates,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetIssueDetailsVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "WorkflowState")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsIssueState {
    pub name: String,
    pub color: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "User")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsIssueAssignee {
    pub name: String,
    pub display_name: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsIssueProject {
    pub name: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectMilestone")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsIssueProjectMilestone {
    pub name: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Team")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsIssueTeam {
    pub active_cycle: Option<GetIssuesForStateIssuesNodesTeamActiveCycle>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsIssueParent {
    pub identifier: String,
    pub title: String,
    pub state: GetIssueDetailsIssueState,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueConnection")]
#[serde(transparent)]
pub struct GetIssueDetailsIssueChildren {
    pub nodes: Vec<GetIssueDetailsIssueParent>,
    #[serde(skip)]
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Attachment")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsIssueAttachmentsNodes {
    pub id: cynic::Id,
    pub title: String,
    pub url: String,
    pub subtitle: Option<String>,
    pub source_type: Option<String>,
    pub metadata: JsonObject,
    pub created_at: DateTime,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "AttachmentConnection")]
#[serde(transparent)]
pub struct GetIssueDetailsIssueAttachments {
    pub nodes: Vec<GetIssueDetailsIssueAttachmentsNodes>,
    #[serde(skip)]
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Document")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsIssueDocumentsNodes {
    pub id: cynic::Id,
    pub title: String,
    pub slug_id: String,
    pub url: String,
    pub created_at: DateTime,
    pub updated_at: DateTime,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "DocumentConnection")]
#[serde(transparent)]
pub struct GetIssueDetailsIssueDocuments {
    pub nodes: Vec<GetIssueDetailsIssueDocumentsNodes>,
    #[serde(skip)]
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsIssue {
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub url: String,
    pub branch_name: String,
    pub state: GetIssueDetailsIssueState,
    pub assignee: Option<GetIssueDetailsIssueAssignee>,
    pub delegate: Option<GetIssueDetailsIssueAssignee>,
    pub priority: WholeNumber,
    pub project: Option<GetIssueDetailsIssueProject>,
    pub project_milestone: Option<GetIssueDetailsIssueProjectMilestone>,
    pub cycle: Option<GetIssuesForStateIssuesNodesCycle>,
    pub team: GetIssueDetailsIssueTeam,
    #[arguments(first : 50)]
    pub labels: IssueLabels,
    pub parent: Option<GetIssueDetailsIssueParent>,
    #[arguments(first : 250)]
    pub children: GetIssueDetailsIssueChildren,
    #[arguments(first : 50)]
    pub attachments: GetIssueDetailsIssueAttachments,
    #[arguments(first : 50)]
    pub documents: GetIssueDetailsIssueDocuments,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetIssueDetailsVariables"
)]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetails {
    #[arguments(id : $id)]
    pub issue: Option<GetIssueDetailsIssue>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetIssueDetailsWithCommentsVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ExternalUser")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsWithCommentsIssueCommentsNodesExternalUser {
    pub name: String,
    pub display_name: String,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsWithCommentsIssueCommentsNodesParent {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Comment")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsWithCommentsIssueCommentsNodes {
    pub id: cynic::Id,
    pub body: String,
    pub quoted_text: Option<String>,
    pub created_at: DateTime,
    pub url: String,
    pub resolved_at: Option<DateTime>,
    pub resolving_comment_id: Option<String>,
    pub resolving_user: Option<GetIssueDetailsIssueAssignee>,
    pub user: Option<GetIssueDetailsIssueAssignee>,
    pub external_user: Option<GetIssueDetailsWithCommentsIssueCommentsNodesExternalUser>,
    pub parent: Option<GetIssueDetailsWithCommentsIssueCommentsNodesParent>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "CommentConnection")]
#[serde(transparent)]
pub struct GetIssueDetailsWithCommentsIssueComments {
    pub nodes: Vec<GetIssueDetailsWithCommentsIssueCommentsNodes>,
    #[serde(skip)]
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsWithCommentsIssue {
    pub identifier: String,
    pub title: String,
    pub description: Option<String>,
    pub url: String,
    pub branch_name: String,
    pub state: GetIssueDetailsIssueState,
    pub assignee: Option<GetIssueDetailsIssueAssignee>,
    pub delegate: Option<GetIssueDetailsIssueAssignee>,
    pub priority: WholeNumber,
    pub project: Option<GetIssueDetailsIssueProject>,
    pub project_milestone: Option<GetIssueDetailsIssueProjectMilestone>,
    pub cycle: Option<GetIssuesForStateIssuesNodesCycle>,
    pub team: GetIssueDetailsIssueTeam,
    #[arguments(first : 50)]
    pub labels: IssueLabels,
    pub parent: Option<GetIssueDetailsIssueParent>,
    #[arguments(first : 250)]
    pub children: GetIssueDetailsIssueChildren,
    #[arguments(first : 50, orderBy : createdAt)]
    pub comments: GetIssueDetailsWithCommentsIssueComments,
    #[arguments(first : 50)]
    pub attachments: GetIssueDetailsIssueAttachments,
    #[arguments(first : 50)]
    pub documents: GetIssueDetailsIssueDocuments,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetIssueDetailsWithCommentsVariables"
)]
#[serde(rename_all = "camelCase")]
pub struct GetIssueDetailsWithComments {
    #[arguments(id : $id)]
    pub issue: Option<GetIssueDetailsWithCommentsIssue>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct GetProjectIdOptionsByNameVariables {
    pub name: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct GetProjectIdOptionsByNameProjects {
    pub nodes: Vec<super::project::ProjectChoice>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "GetProjectIdOptionsByNameVariables"
)]
pub struct GetProjectIdOptionsByName {
    #[arguments(filter : { name : { containsIgnoreCase : $name } })]
    pub projects: GetProjectIdOptionsByNameProjects,
}

/// An issue's labels with the paging fields for fetching the rest.
#[derive(cynic::QueryFragment, Serialize, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabelConnection")]
#[serde(transparent)]
pub struct IssueLabels {
    pub nodes: Vec<GetIssuesForStateIssuesNodesLabelsNodes>,
    #[serde(skip)]
    pub page_info: PageInfo,
}

/// One page of a connection nested in an issue.
#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct IssuePageVariables {
    pub id: String,
    pub first: i32,
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "IssuePageVariables"
)]
pub struct GetIssueLabelsPage {
    #[arguments(id: $id)]
    pub issue: IssueLabelsPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "IssuePageVariables"
)]
pub struct IssueLabelsPage {
    #[arguments(first: $first, after: $after)]
    pub labels: IssueLabels,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "IssuePageVariables"
)]
pub struct GetIssueChildrenPage {
    #[arguments(id: $id)]
    pub issue: IssueChildrenPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "IssuePageVariables"
)]
pub struct IssueChildrenPage {
    #[arguments(first: $first, after: $after)]
    pub children: GetIssueDetailsIssueChildren,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "IssuePageVariables"
)]
pub struct GetIssueAttachmentsPage {
    #[arguments(id: $id)]
    pub issue: IssueAttachmentsPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "IssuePageVariables"
)]
pub struct IssueAttachmentsPage {
    #[arguments(first: $first, after: $after)]
    pub attachments: GetIssueDetailsIssueAttachments,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "IssuePageVariables"
)]
pub struct GetIssueDocumentsPage {
    #[arguments(id: $id)]
    pub issue: IssueDocumentsPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "IssuePageVariables"
)]
pub struct IssueDocumentsPage {
    #[arguments(first: $first, after: $after)]
    pub documents: GetIssueDetailsIssueDocuments,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "IssuePageVariables"
)]
pub struct GetIssueCommentsPage {
    #[arguments(id: $id)]
    pub issue: IssueCommentsPage,
}

#[derive(cynic::QueryFragment, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "IssuePageVariables"
)]
pub struct IssueCommentsPage {
    #[arguments(first: $first, after: $after, orderBy: createdAt)]
    pub comments: GetIssueDetailsWithCommentsIssueComments,
}
