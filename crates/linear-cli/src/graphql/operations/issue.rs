//! Issue lookups and mutations: create, update, archive, delete, relations, links and state changes.

use serde::Serialize;

use super::common::DeletePayload;
use super::common::IdVariablesFields;
use super::project::ProjectFilter;
use super::project::ProjectRef;
use super::team::TeamKey;
use super::team::TeamRef;
use crate::graphql::edit::Edit;
use crate::graphql::pagination::PageInfo;
use crate::graphql::scalars::DateTime;
use crate::graphql::scalars::Json;
use crate::graphql::scalars::TimelessDate;
use crate::graphql::schema;

/// What archive and delete confirm and report.
#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetIssueSummary {
    #[arguments(id: $id)]
    pub issue: Option<IssueSummary>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueSummary {
    pub identifier: String,
    pub title: String,
    pub archived_at: Option<DateTime>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct ArchiveIssue {
    #[arguments(id: $id)]
    pub issue_archive: SuccessPayload,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "IdVariables"
)]
pub struct DeleteIssue {
    #[arguments(id: $id)]
    pub issue_delete: SuccessPayload,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueArchivePayload")]
pub struct SuccessPayload {
    pub success: bool,
}

#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueCreateInput")]
pub struct IssueCreateInput {
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub title: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub assignee_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub due_date: Edit<TimelessDate>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub parent_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub priority: Edit<i32>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub estimate: Edit<i32>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub label_ids: Option<Vec<String>>,
    pub team_id: String,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub project_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub project_milestone_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub cycle_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub state_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub template_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub use_default_template: Edit<bool>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub description: Edit<String>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct CreateIssueVariables {
    pub input: IssueCreateInput,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateIssueVariables"
)]
pub struct CreateIssue {
    #[arguments(input:$input)]
    pub issue_create: CreatePayload,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssuePayload")]
pub struct CreatePayload {
    pub success: bool,
    pub issue: Option<CreatedIssue>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct CreatedIssue {
    pub identifier: String,
    pub title: String,
    pub url: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetIssueDetails {
    #[arguments(id: $id)]
    pub issue: IssueDetails,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueDetails {
    pub title: String,
    pub url: String,
    pub branch_name: String,
    pub team: TeamKey,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetIssueId {
    #[arguments(id: $id)]
    pub issue: IssueId,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueId {
    pub id: cynic::Id,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct LinkVariables {
    pub issue_id: String,
    pub url: String,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "LinkVariables"
)]
pub struct AttachmentLinkURL {
    #[arguments(issueId: $issue_id, url: $url, title: $title)]
    #[cynic(rename = "attachmentLinkURL")]
    pub attachment_link_url: LinkedPayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "AttachmentPayload")]
pub struct LinkedPayload {
    pub success: bool,
    pub attachment: LinkedAttachment,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Attachment")]
pub struct LinkedAttachment {
    pub id: cynic::Id,
    pub title: String,
    pub url: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct RelationsVariables {
    pub issue_id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "RelationsVariables"
)]
pub struct ListIssueRelations {
    #[arguments(id: $issue_id)]
    pub issue: ListedIssue,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct ListedIssue {
    pub id: cynic::Id,
    pub identifier: String,
    pub title: String,
    #[arguments(first: 100)]
    pub relations: Outgoing,
    #[arguments(first: 100)]
    pub inverse_relations: Incoming,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationConnection")]
pub struct Outgoing {
    pub nodes: Vec<OutgoingRelation>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelation")]
pub struct OutgoingRelation {
    pub id: cynic::Id,
    #[cynic(rename = "type")]
    pub relation_type: String,
    pub related_issue: DisplayIssue,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationConnection")]
pub struct Incoming {
    pub nodes: Vec<IncomingRelation>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelation")]
pub struct IncomingRelation {
    pub id: cynic::Id,
    #[cynic(rename = "type")]
    pub relation_type: String,
    pub issue: DisplayIssue,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct DisplayIssue {
    pub identifier: String,
    pub title: String,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct RelationsPageVariables {
    pub issue_id: String,
    pub first: i32,
    pub after: Option<String>,
}

/// A later page of the outgoing relations `issue relation list` shows.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "RelationsPageVariables"
)]
pub struct GetOutgoingRelationsPage {
    #[arguments(id: $issue_id)]
    pub issue: OutgoingRelationsPage,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "RelationsPageVariables"
)]
pub struct OutgoingRelationsPage {
    #[arguments(first: $first, after: $after)]
    pub relations: Outgoing,
}

/// A later page of the incoming relations `issue relation list` shows.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "RelationsPageVariables"
)]
pub struct GetIncomingRelationsPage {
    #[arguments(id: $issue_id)]
    pub issue: IncomingRelationsPage,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "RelationsPageVariables"
)]
pub struct IncomingRelationsPage {
    #[arguments(first: $first, after: $after)]
    pub inverse_relations: Incoming,
}

/// A page of an issue's outgoing relations.
#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "RelationsPageVariables"
)]
pub struct FindIssueRelation {
    #[arguments(id: $issue_id)]
    pub issue: FindIssue,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Issue",
    variables = "RelationsPageVariables"
)]
pub struct FindIssue {
    #[arguments(first: $first, after: $after)]
    pub relations: FoundRelations,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationConnection")]
pub struct FoundRelations {
    pub nodes: Vec<FoundRelation>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelation")]
pub struct FoundRelation {
    pub id: cynic::Id,
    #[cynic(rename = "type")]
    pub relation_type: String,
    pub related_issue: IssueId,
}

#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "IssueRelationType",
    rename_all = "lowercase"
)]
pub enum ApiRelationType {
    Blocks,
    Duplicate,
    Related,
    Similar,
}

impl ApiRelationType {
    pub fn spelling(self) -> &'static str {
        match self {
            Self::Blocks => "blocks",
            Self::Duplicate => "duplicate",
            Self::Related => "related",
            Self::Similar => "similar",
        }
    }
}

#[derive(cynic::InputObject, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationCreateInput")]
pub struct RelationInput {
    pub issue_id: String,
    pub related_issue_id: String,
    #[cynic(rename = "type")]
    pub relation_type: ApiRelationType,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct CreateVariables {
    pub input: RelationInput,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "CreateVariables"
)]
pub struct CreateIssueRelation {
    #[arguments(input: $input)]
    pub issue_relation_create: CreatedPayload,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueRelationPayload")]
pub struct CreatedPayload {
    pub success: bool,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct DeleteVariables {
    pub id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "DeleteVariables"
)]
pub struct DeleteIssueRelation {
    #[arguments(id: $id)]
    pub issue_relation_delete: DeletePayload,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct UpdateIssueStateVariables {
    pub issue_id: String,
    pub state_id: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UpdateIssueStateVariables"
)]
pub struct UpdateIssueState {
    #[arguments(id: $issue_id, input: { stateId: $state_id })]
    pub issue_update: StateResult,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssuePayload")]
pub struct StateResult {
    pub success: bool,
}

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct UpdateIssueVariables {
    pub id: String,
    pub input: IssueUpdateInput,
}

/// The `IssueUpdateInput` fields `issue update` can send, plus `trashed`.
///
/// Only provided fields are sent. Nullable scalar fields use [`Edit<T>`] so a
/// clear sends an explicit `null`; list fields use `Option<Vec<T>>` (omit or
/// set), because Cynic's derive check cannot accept `Edit<Vec<T>>`.
///
/// `trashed` is documented by Linear as "true to trash, or null to restore",
/// so `Edit::Clear` is a meaningful restore, not an absence of intent.
#[derive(cynic::InputObject, Clone, Debug, Default, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "IssueUpdateInput")]
pub struct IssueUpdateInput {
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub title: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub assignee_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub due_date: Edit<TimelessDate>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub parent_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub priority: Edit<i32>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub estimate: Edit<i32>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub description: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub description_data: Edit<Json>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub label_ids: Option<Vec<String>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub added_label_ids: Option<Vec<String>>,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub removed_label_ids: Option<Vec<String>>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub team_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub project_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub project_milestone_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub cycle_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub state_id: Edit<String>,
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub trashed: Edit<bool>,
    /// Not sent by any command yet; an `Edit<Enum>` field.
    #[cynic(skip_serializing_if = "Edit::is_unchanged")]
    pub sla_type: Edit<SlaDayCountType>,
}

/// Wire spellings are `all`/`onlyBusinessDays`.
#[derive(cynic::Enum, Clone, Copy, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "SLADayCountType",
    rename_all = "camelCase"
)]
pub enum SlaDayCountType {
    All,
    OnlyBusinessDays,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(
    schema = "linear",
    graphql_type = "Mutation",
    variables = "UpdateIssueVariables"
)]
#[serde(rename_all = "camelCase")]
pub struct UpdateIssue {
    #[arguments(id: $id, input: $input)]
    pub issue_update: IssuePayload,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear")]
pub struct IssuePayload {
    pub success: bool,
    pub issue: Option<UpdatedIssue>,
}

#[derive(cynic::QueryFragment, Serialize, Clone, Debug, PartialEq, Eq)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct UpdatedIssue {
    pub id: cynic::Id,
    pub identifier: String,
    pub url: String,
    pub title: String,
}

#[cfg(test)]
mod tests {
    use cynic::MutationBuilder;
    use serde_json::{Value, json, to_string, to_value};

    use super::{IssueUpdateInput, SlaDayCountType, UpdateIssue, UpdateIssueVariables};
    use crate::graphql::edit::Edit;
    use crate::graphql::scalars::{Json, TimelessDate};

    fn variables(input: IssueUpdateInput) -> UpdateIssueVariables {
        UpdateIssue::build(UpdateIssueVariables {
            id: "issue-1".to_owned(),
            input,
        })
        .variables
    }

    fn value(input: IssueUpdateInput) -> Value {
        to_value(variables(input)).expect("variables serialize")
    }

    #[test]
    fn only_edited_fields_are_sent() {
        assert_eq!(
            value(IssueUpdateInput::default()),
            json!({"id": "issue-1", "input": {}})
        );
        assert_eq!(
            to_string(&variables(IssueUpdateInput {
                title: Edit::Set("x".to_owned()),
                ..IssueUpdateInput::default()
            }))
            .expect("variables serialize"),
            r#"{"id":"issue-1","input":{"title":"x"}}"#
        );
    }

    #[test]
    fn clear_sends_an_explicit_null_for_each_cleared_field() {
        let variables = value(IssueUpdateInput {
            assignee_id: Edit::Clear,
            due_date: Edit::Clear,
            parent_id: Edit::Clear,
            estimate: Edit::Clear,
            project_id: Edit::Clear,
            project_milestone_id: Edit::Clear,
            cycle_id: Edit::Clear,
            // Linear restores a trashed issue when `trashed` is null.
            trashed: Edit::Clear,
            sla_type: Edit::Clear,
            ..IssueUpdateInput::default()
        });
        assert_eq!(
            variables,
            json!({
                "id": "issue-1",
                "input": {
                    "assigneeId": null,
                    "dueDate": null,
                    "parentId": null,
                    "estimate": null,
                    "projectId": null,
                    "projectMilestoneId": null,
                    "cycleId": null,
                    "trashed": null,
                    "slaType": null
                }
            })
        );
    }

    #[test]
    fn falsy_values_are_sent_not_omitted() {
        assert_eq!(
            value(IssueUpdateInput {
                title: Edit::Set(String::new()),
                estimate: Edit::Set(0),
                priority: Edit::Set(0),
                trashed: Edit::Set(false),
                label_ids: Some(Vec::new()),
                description: Edit::Set(String::new()),
                ..IssueUpdateInput::default()
            }),
            json!({
                "id": "issue-1",
                "input": {
                    "title": "",
                    "priority": 0,
                    "estimate": 0,
                    "description": "",
                    "labelIds": [],
                    "trashed": false
                }
            })
        );
    }

    #[test]
    fn scalars_lists_and_enums_keep_their_wire_form() {
        let variables = value(IssueUpdateInput {
            due_date: Edit::Set(TimelessDate(
                chrono::NaiveDate::from_ymd_opt(2026, 9, 30).expect("valid date"),
            )),
            description_data: Edit::Set(Json(r#"{"type":"doc"}"#.to_owned())),
            added_label_ids: Some(vec!["l1".to_owned(), "l2".to_owned()]),
            removed_label_ids: Some(vec![]),
            team_id: Edit::Set("team-1".to_owned()),
            state_id: Edit::Set("state-1".to_owned()),
            sla_type: Edit::Set(SlaDayCountType::OnlyBusinessDays),
            ..IssueUpdateInput::default()
        });
        // `JSON` is stringified: the value is a JSON string, not an embedded object.
        assert_eq!(
            variables,
            json!({
                "id": "issue-1",
                "input": {
                    "dueDate": "2026-09-30",
                    "descriptionData": "{\"type\":\"doc\"}",
                    "addedLabelIds": ["l1", "l2"],
                    "removedLabelIds": [],
                    "teamId": "team-1",
                    "stateId": "state-1",
                    "slaType": "onlyBusinessDays"
                }
            })
        );
    }
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query")]
pub struct GetUserSettings {
    pub user_settings: UserSettings,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear")]
pub struct UserSettings {
    pub auto_assign_to_self: bool,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct LabelVariables {
    pub name: String,
    pub team_key: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LabelVariables"
)]
pub struct GetIssueLabelIdByNameForTeam {
    #[arguments(filter:{name:{eqIgnoreCase:$name},or:[{team:{key:{eq:$team_key}}},{team:{null:true}}]})]
    pub issue_labels: LookupLabelConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "LabelVariables"
)]
pub struct GetIssueLabelIdOptionsByNameForTeam {
    #[arguments(filter:{name:{containsIgnoreCase:$name},or:[{team:{key:{eq:$team_key}}},{team:{null:true}}]})]
    pub issue_labels: LookupLabelConnection,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabelConnection")]
pub struct LookupLabelConnection {
    pub nodes: Vec<LookupLabel>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "IssueLabel")]
pub struct LookupLabel {
    pub id: cynic::Id,
    pub name: String,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct ProjectsVariables {
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub filter: Option<ProjectFilter>,
    pub first: i32,
    #[cynic(skip_serializing_if = "Option::is_none")]
    pub after: Option<String>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(
    schema = "linear",
    graphql_type = "Query",
    variables = "ProjectsVariables"
)]
pub struct GetProjectsForTeam {
    #[arguments(filter: $filter, first: $first, after: $after)]
    pub projects: ProjectsPage,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "ProjectConnection")]
pub struct ProjectsPage {
    pub nodes: Vec<ProjectRef>,
    pub page_info: PageInfo,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetParentIssueData {
    #[arguments(id:$id)]
    pub issue: Option<ParentIssue>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct ParentIssue {
    pub title: String,
    pub identifier: String,
    pub project: Option<ProjectId>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Project")]
pub struct ProjectId {
    pub id: cynic::Id,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "IdVariables")]
pub struct GetIssueProjectId {
    #[arguments(id:$id)]
    pub issue: Option<IssueProject>,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Issue")]
pub struct IssueProject {
    pub project: Option<ProjectId>,
}

#[derive(cynic::QueryVariables, Clone, Debug)]
pub struct TeamSubstring {
    pub team: String,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "Query", variables = "TeamSubstring")]
pub struct GetTeamIdOptionsByKey {
    #[arguments(filter:{key:{containsIgnoreCase:$team}})]
    pub teams: TeamNames,
}

#[derive(cynic::QueryFragment, Clone, Debug)]
#[cynic(schema = "linear", graphql_type = "TeamConnection")]
pub struct TeamNames {
    pub nodes: Vec<TeamRef>,
}
