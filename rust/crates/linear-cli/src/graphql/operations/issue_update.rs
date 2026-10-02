//! `UpdateIssue`: the `issueUpdate` mutation with a three-state input.
//!
//! Only provided fields are sent, and clear flags send an explicit `null`.
//! Nullable scalar fields use [`Edit<T>`]; list fields use `Option<Vec<T>>`
//! (omit or set). See the `edit` module
//! for why lists cannot be `Edit<Vec<T>>` under Cynic's derive check.

use serde::Serialize;

use crate::graphql::edit::Edit;
use crate::graphql::scalars::{Json, TimelessDate};
use crate::graphql::schema;

#[derive(cynic::QueryVariables, Clone, Debug, PartialEq, Eq)]
pub struct UpdateIssueVariables {
    pub id: String,
    pub input: IssueUpdateInput,
}

/// The `IssueUpdateInput` fields `issue update` can send, plus `trashed`.
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
