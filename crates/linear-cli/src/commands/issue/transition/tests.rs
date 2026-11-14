use super::{Kind, Settled, Verb, destination, settled};
use crate::graphql::operations::team::WorkflowState;
use crate::graphql::scalars::Float;

fn state(name: &str, state_type: &str, position: f64) -> WorkflowState {
    WorkflowState {
        id: cynic::Id::new(format!("id-{name}")),
        name: name.to_owned(),
        state_type: state_type.to_owned(),
        position: Float(serde_json::Number::from_f64(position).expect("finite")),
    }
}

#[test]
fn start_leaves_started_and_closed_issues_alone() {
    for (kind, expected) in [
        (Kind::Triage, None),
        (Kind::Backlog, None),
        (Kind::Unstarted, None),
        (Kind::Started, Some(Settled::Already)),
        (Kind::Completed, Some(Settled::Closed)),
        (Kind::Canceled, Some(Settled::Closed)),
        (Kind::Duplicate, Some(Settled::Closed)),
    ] {
        assert_eq!(settled(Verb::Start, kind), expected, "{kind:?}");
    }
}

#[test]
fn complete_and_cancel_only_skip_an_issue_already_there() {
    assert_eq!(
        settled(Verb::Complete, Kind::Completed),
        Some(Settled::Already)
    );
    assert_eq!(
        settled(Verb::Cancel, Kind::Canceled),
        Some(Settled::Already)
    );
    assert_eq!(settled(Verb::Complete, Kind::Canceled), None);
    assert_eq!(settled(Verb::Cancel, Kind::Completed), None);
    assert_eq!(settled(Verb::Cancel, Kind::Started), None);
}

#[test]
fn start_takes_the_first_started_status_by_position() {
    let states = vec![
        state("Started", "unstarted", 0.0),
        state("In Review", "started", 3.0),
        state("In Progress", "started", 2.0),
    ];
    let chosen = destination(Verb::Start, "ENG", "ENG-1", states).expect("a started status");
    assert_eq!(chosen.name, "In Progress");
}

#[test]
fn cancel_refuses_to_choose_between_several_canceled_statuses() {
    let states = vec![
        state("Duplicate", "canceled", 5.0),
        state("Canceled", "canceled", 4.0),
        state("Done", "completed", 3.0),
    ];
    let error = destination(Verb::Cancel, "ENG", "ENG-1", states.clone())
        .expect_err("two canceled statuses");
    assert_eq!(
        error.to_string(),
        "Team ENG has several canceled statuses: \"Canceled\", \"Duplicate\""
    );
    assert_eq!(
        error.hint(),
        Some("Choose one with `linear issue update ENG-1 --state <name>`.")
    );
    let chosen = destination(Verb::Complete, "ENG", "ENG-1", states).expect("one completed");
    assert_eq!(chosen.name, "Done");
}

#[test]
fn a_team_without_a_status_of_the_type_is_an_error() {
    let error = destination(
        Verb::Complete,
        "ENG",
        "ENG-1",
        vec![state("Todo", "unstarted", 1.0)],
    )
    .expect_err("no completed status");
    assert_eq!(error.to_string(), "Team ENG has no completed status");
}
