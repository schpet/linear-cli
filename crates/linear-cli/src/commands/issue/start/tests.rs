use serde_json::json;

use super::filter;

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
