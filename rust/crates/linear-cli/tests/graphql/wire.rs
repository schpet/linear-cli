use cynic::{MutationBuilder, QueryBuilder};
use linear_cli::graphql::edit::Edit;
use linear_cli::graphql::envelope::GraphQlRequest;
use linear_cli::graphql::operations::issue_update::{
    IssueUpdateInput, SlaDayCountType, UpdateIssue, UpdateIssueVariables,
};
use linear_cli::graphql::operations::teams::{
    GetTeams, GetTeamsVariables, StringComparator, TeamFilter,
};
use linear_cli::graphql::scalars::{Json, TimelessDate};
use serde_json::{Value, json, to_string, to_value};

fn update_operation(
    input: IssueUpdateInput,
) -> cynic::Operation<UpdateIssue, UpdateIssueVariables> {
    UpdateIssue::build(UpdateIssueVariables {
        id: "issue-1".to_owned(),
        input,
    })
}

fn update_variables(input: IssueUpdateInput) -> Value {
    to_value(&update_operation(input).variables).expect("variables serialize")
}

/// Serializes the variables struct directly so key order is the struct's
/// declaration order, not a `Value` map's.
fn update_variables_text(input: IssueUpdateInput) -> String {
    to_string(&update_operation(input).variables).expect("variables serialize")
}

#[test]
fn update_issue_title_only_sends_exactly_one_input_key() {
    let variables = update_variables(IssueUpdateInput {
        title: Edit::Set("x".to_owned()),
        ..IssueUpdateInput::default()
    });
    assert_eq!(variables, json!({"id": "issue-1", "input": {"title": "x"}}));
    let text = update_variables_text(IssueUpdateInput {
        title: Edit::Set("x".to_owned()),
        ..IssueUpdateInput::default()
    });
    assert_eq!(text, r#"{"id":"issue-1","input":{"title":"x"}}"#);
}

#[test]
fn update_issue_with_no_edits_sends_an_empty_input_object() {
    let variables = update_variables(IssueUpdateInput::default());
    assert_eq!(variables, json!({"id": "issue-1", "input": {}}));
}

#[test]
fn update_issue_clear_sends_explicit_null_for_each_cleared_field() {
    let variables = update_variables(IssueUpdateInput {
        assignee_id: Edit::Clear,
        due_date: Edit::Clear,
        parent_id: Edit::Clear,
        estimate: Edit::Clear,
        project_id: Edit::Clear,
        project_milestone_id: Edit::Clear,
        cycle_id: Edit::Clear,
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
                "cycleId": null
            }
        })
    );
}

#[test]
fn update_issue_preserves_false_zero_empty_string_and_empty_list() {
    let text = update_variables_text(IssueUpdateInput {
        title: Edit::Set(String::new()),
        estimate: Edit::Set(0),
        priority: Edit::Set(0),
        trashed: Edit::Set(false),
        label_ids: Some(Vec::new()),
        description: Edit::Set(String::new()),
        ..IssueUpdateInput::default()
    });
    assert_eq!(
        text,
        r#"{"id":"issue-1","input":{"title":"","priority":0,"estimate":0,"description":"","labelIds":[],"trashed":false}}"#
    );
}

#[test]
fn update_issue_trashed_clear_is_a_restore_null_not_an_omission() {
    let variables = update_variables(IssueUpdateInput {
        trashed: Edit::Clear,
        ..IssueUpdateInput::default()
    });
    assert_eq!(
        variables,
        json!({"id": "issue-1", "input": {"trashed": null}})
    );
    let variables = update_variables(IssueUpdateInput {
        trashed: Edit::Set(true),
        ..IssueUpdateInput::default()
    });
    assert_eq!(
        variables,
        json!({"id": "issue-1", "input": {"trashed": true}})
    );
}

#[test]
fn update_issue_custom_scalars_and_lists_keep_wire_form() {
    let variables = update_variables(IssueUpdateInput {
        due_date: Edit::Set(TimelessDate("2026-09-30".to_owned())),
        description_data: Edit::Set(Json(r#"{"type":"doc"}"#.to_owned())),
        added_label_ids: Some(vec!["l1".to_owned(), "l2".to_owned()]),
        removed_label_ids: Some(vec![]),
        team_id: Edit::Set("team-1".to_owned()),
        state_id: Edit::Set("state-1".to_owned()),
        ..IssueUpdateInput::default()
    });
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
                "stateId": "state-1"
            }
        })
    );
    // `JSON` is stringified: the value is a JSON string, not an embedded object.
    let description_data = &variables["input"]["descriptionData"];
    assert!(description_data.is_string());
}

#[test]
fn update_issue_enum_edit_sends_the_schema_spelling_or_null() {
    let text = update_variables_text(IssueUpdateInput {
        sla_type: Edit::Set(SlaDayCountType::OnlyBusinessDays),
        ..IssueUpdateInput::default()
    });
    assert_eq!(
        text,
        r#"{"id":"issue-1","input":{"slaType":"onlyBusinessDays"}}"#
    );
    let variables = update_variables(IssueUpdateInput {
        sla_type: Edit::Clear,
        ..IssueUpdateInput::default()
    });
    assert_eq!(
        variables,
        json!({"id": "issue-1", "input": {"slaType": null}})
    );
}

#[test]
fn unchanged_edit_refuses_direct_serialization() {
    let error = to_value(Edit::<i32>::Unchanged).expect_err("must not serialize");
    assert!(
        error
            .to_string()
            .contains("Edit::Unchanged must be omitted")
    );
    assert_eq!(to_value(Edit::<i32>::Clear).expect("null"), Value::Null);
    assert_eq!(to_value(Edit::Set(7)).expect("value"), json!(7));
}

#[test]
fn edit_refuses_deserialization_so_a_missing_field_can_never_become_clear() {
    for body in ["null", "true", "1", r#""x""#] {
        let error = serde_json::from_str::<Edit<bool>>(body).expect_err(body);
        assert!(
            error
                .to_string()
                .contains("Edit<T> is a write-only adapter and cannot be deserialized"),
            "{body}: {error}"
        );
    }
    #[derive(serde::Deserialize)]
    struct Probe {
        #[allow(dead_code)]
        trashed: Edit<bool>,
    }
    for body in ["{}", r#"{"trashed":null}"#, r#"{"trashed":true}"#] {
        assert!(serde_json::from_str::<Probe>(body).is_err(), "{body}");
    }
}

#[test]
fn edit_helpers_convert_options_and_default_to_unchanged() {
    assert_eq!(Edit::<i32>::default(), Edit::Unchanged);
    assert_eq!(Edit::set_or_clear(None::<i32>), Edit::Clear);
    assert_eq!(Edit::set_or_clear(Some(1)), Edit::Set(1));
    assert_eq!(Edit::set_or_unchanged(None::<i32>), Edit::Unchanged);
    assert_eq!(Edit::from(2), Edit::Set(2));
    assert!(Edit::<i32>::Unchanged.is_unchanged());
    assert!(!Edit::<i32>::Clear.is_unchanged());
}

#[test]
fn update_issue_document_matches_the_expected_selection() {
    let operation = UpdateIssue::build(UpdateIssueVariables {
        id: "issue-1".to_owned(),
        input: IssueUpdateInput::default(),
    });
    assert_eq!(operation.operation_name.as_deref(), Some("UpdateIssue"));
    assert_eq!(
        operation.query,
        "mutation UpdateIssue($id: String!, $input: IssueUpdateInput!) {\n  issueUpdate(id: $id, input: $input) {\n    success\n    issue {\n      id\n      identifier\n      url\n      title\n    }\n  }\n}\n"
    );
}

#[test]
fn get_teams_first_page_omits_after_and_filter_keys() {
    let operation = GetTeams::build(GetTeamsVariables {
        filter: None,
        first: Some(100),
        after: None,
    });
    let variables = to_value(&operation.variables).expect("variables");
    assert_eq!(variables, json!({"first": 100}));
    assert_eq!(
        to_string(&operation.variables).expect("string"),
        r#"{"first":100}"#
    );
}

#[test]
fn get_teams_unset_first_is_omitted_rather_than_null() {
    let operation = GetTeams::build(GetTeamsVariables {
        filter: None,
        first: None,
        after: None,
    });
    assert_eq!(to_value(&operation.variables).expect("value"), json!({}));
    assert_eq!(to_string(&operation.variables).expect("text"), "{}");
}

#[test]
fn get_teams_later_page_sends_the_cursor() {
    let operation = GetTeams::build(GetTeamsVariables {
        filter: None,
        first: Some(100),
        after: Some("cursor-a".to_owned()),
    });
    assert_eq!(
        to_string(&operation.variables).expect("string"),
        r#"{"first":100,"after":"cursor-a"}"#
    );
}

#[test]
fn get_teams_filter_omits_unset_nested_keys() {
    let operation = GetTeams::build(GetTeamsVariables {
        filter: Some(TeamFilter {
            name: Some(StringComparator {
                eq: Some("Engineering".to_owned()),
                contains_ignore_case: None,
            }),
            key: None,
        }),
        first: Some(100),
        after: None,
    });
    assert_eq!(
        to_value(&operation.variables).expect("variables"),
        json!({"filter": {"name": {"eq": "Engineering"}}, "first": 100})
    );
}

#[test]
fn get_teams_document_matches_the_expected_selection() {
    let operation = GetTeams::build(GetTeamsVariables {
        filter: None,
        first: Some(100),
        after: None,
    });
    assert_eq!(operation.operation_name.as_deref(), Some("GetTeams"));
    assert_eq!(
        operation.query,
        "query GetTeams($filter: TeamFilter, $first: Int, $after: String) {\n  teams(filter: $filter, first: $first, after: $after) {\n    nodes {\n      id\n      name\n      key\n      description\n      icon\n      color\n      cyclesEnabled\n      createdAt\n      updatedAt\n      archivedAt\n      organization {\n        id\n        name\n      }\n    }\n    pageInfo {\n      hasNextPage\n      endCursor\n    }\n  }\n}\n"
    );
}

#[test]
fn request_envelope_with_variables_carries_query_variables_and_operation_name() {
    let operation = GetTeams::build(GetTeamsVariables {
        filter: None,
        first: Some(100),
        after: None,
    });
    let query = operation.query.clone();
    let request = GraphQlRequest::with_variables(operation);
    let value = to_value(&request).expect("request");
    assert_eq!(
        value,
        json!({"query": query, "variables": {"first": 100}, "operationName": "GetTeams"})
    );
    let text = to_string(&request).expect("string");
    assert!(text.starts_with(r#"{"query":"query GetTeams("#));
    assert!(text.ends_with(r#","variables":{"first":100},"operationName":"GetTeams"}"#));
}

#[test]
fn request_envelope_without_variables_omits_the_variables_key() {
    let request = GraphQlRequest::<()> {
        query: "query Viewer { viewer { id } }".to_owned(),
        variables: None,
        operation_name: Some("Viewer".to_owned()),
    };
    assert_eq!(
        to_string(&request).expect("string"),
        r#"{"query":"query Viewer { viewer { id } }","operationName":"Viewer"}"#
    );
    let anonymous = GraphQlRequest::<()> {
        query: "{ viewer { id } }".to_owned(),
        variables: None,
        operation_name: None,
    };
    assert_eq!(
        to_string(&anonymous).expect("string"),
        r#"{"query":"{ viewer { id } }"}"#
    );
}

#[test]
fn raw_variables_keep_an_explicit_null_cursor_for_the_api_paginate_path() {
    // The future raw `api --paginate` path sends `after: null` on page one.
    // The envelope must carry that null through untouched; only the typed
    // built-in variables above omit the key.
    let request = GraphQlRequest {
        query: "query ($after: String) { teams(after: $after) { nodes { id } } }".to_owned(),
        variables: Some(json!({"first": 100, "after": null})),
        operation_name: None,
    };
    let text = to_string(&request).expect("string");
    assert!(
        text.ends_with(r#""variables":{"first":100,"after":null}}"#),
        "{text}"
    );
}
