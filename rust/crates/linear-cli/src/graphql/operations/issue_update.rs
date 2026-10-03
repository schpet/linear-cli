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
            due_date: Edit::Set(TimelessDate("2026-09-30".to_owned())),
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
