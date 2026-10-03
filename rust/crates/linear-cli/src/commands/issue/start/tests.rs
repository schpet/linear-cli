use serde_json::json;

use super::{filter, free_suffix};

#[test]
fn an_existing_branch_gets_the_first_free_suffix() {
    let mut checked = Vec::new();
    let branch = free_suffix("eng-1", |name| {
        checked.push(name.to_owned());
        Ok(name != "eng-1-3")
    })
    .expect("free suffix");
    assert_eq!(branch, "eng-1-3");
    assert_eq!(checked, ["eng-1-1", "eng-1-2", "eng-1-3"]);
}

#[test]
fn the_picker_lists_unstarted_issues_for_the_chosen_assignees() {
    for (all, unassigned, assignee) in [
        (false, false, Some(json!({"isMe": {"eq": true}}))),
        (false, true, Some(json!({"null": true}))),
        (true, false, None),
    ] {
        let value = serde_json::to_value(filter("ENG", all, unassigned)).expect("filter");
        assert_eq!(value["team"], json!({"key": {"eq": "ENG"}}));
        assert_eq!(value["state"], json!({"type": {"in": ["unstarted"]}}));
        assert_eq!(value.get("assignee").cloned(), assignee);
    }
}
