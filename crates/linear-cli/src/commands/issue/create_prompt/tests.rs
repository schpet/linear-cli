use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::prompt;
use crate::cli::values::UserRef;
use crate::commands::issue::create::{self, Fields};
use crate::commands::issue::write::{
    Backend, CreateSettings, Created, Label, Named, Parent, State, Ui, Updated,
};
use crate::config::AssignSelf;
use crate::error::Error;
use crate::graphql::operations::issue::IssueUpdateInput;
use crate::platform::prompt::{Choice, Text};
use crate::refs::team::ResolvedTeam;

/// Answers the lookups an interactive create makes and records which ran.
#[derive(Clone, Default)]
struct Linear {
    calls: Arc<Mutex<Vec<String>>>,
    parent: Option<Parent>,
    teams: Vec<ResolvedTeam>,
    labels: Vec<Label>,
    projects: Vec<Named>,
    empty_states: bool,
}

impl Linear {
    fn note(&self, call: &str) {
        self.calls.lock().expect("call log").push(call.to_owned());
    }

    fn called(&self, call: &str) -> bool {
        self.calls
            .lock()
            .expect("call log")
            .iter()
            .any(|c| c == call)
    }
}

impl Backend for Linear {
    async fn team(&self, _: String) -> Result<ResolvedTeam, Error> {
        unreachable!("team")
    }
    async fn find_team(&self, key: String) -> Result<Option<ResolvedTeam>, Error> {
        Ok(Some(ResolvedTeam {
            id: "team-id".into(),
            key,
            name: "Engineering".into(),
        }))
    }
    async fn teams(&self) -> Result<Vec<ResolvedTeam>, Error> {
        Ok(self.teams.clone())
    }
    async fn team_options(&self, _: String) -> Result<Vec<Named>, Error> {
        unreachable!("team options")
    }
    async fn viewer(&self) -> Result<String, Error> {
        self.note("viewer");
        Ok("self-id".into())
    }
    async fn auto_assign(&self) -> Result<bool, Error> {
        unreachable!("auto assign")
    }
    async fn user(&self, _: UserRef) -> Result<String, Error> {
        unreachable!("user")
    }
    async fn states(&self, _: String) -> Result<Vec<State>, Error> {
        if self.empty_states {
            return Ok(Vec::new());
        }
        let state = |id: &str, kind: &str, position| State {
            id: id.into(),
            name: id.into(),
            kind: kind.into(),
            position,
        };
        Ok(vec![
            state("started", "started", 0.0),
            state("later", "unstarted", 9.0),
            state("first", "unstarted", 1.0),
            state("tied", "unstarted", 1.0),
        ])
    }
    async fn state(&self, _: String, _: String) -> Result<String, Error> {
        unreachable!("state")
    }
    async fn label(&self, _: String, _: String) -> Result<Option<String>, Error> {
        unreachable!("label")
    }
    async fn label_options(&self, _: String, _: String) -> Result<Vec<Named>, Error> {
        unreachable!("label options")
    }
    async fn labels(&self, _: String) -> Result<Vec<Label>, Error> {
        Ok(self.labels.clone())
    }
    async fn project(&self, _: String) -> Result<Option<String>, Error> {
        unreachable!("project")
    }
    async fn project_options(&self, _: String) -> Result<Vec<Named>, Error> {
        unreachable!("project options")
    }
    async fn projects(&self, _: String) -> Result<Vec<Named>, Error> {
        self.note("projects");
        Ok(self.projects.clone())
    }
    async fn milestone(&self, _: String, _: String) -> Result<String, Error> {
        unreachable!("milestone")
    }
    async fn cycle(&self, _: String, _: String) -> Result<String, Error> {
        unreachable!("cycle")
    }
    async fn parent_id(&self, _: String) -> Result<String, Error> {
        Ok("parent-id".into())
    }
    async fn parent_metadata(&self, _: String) -> Result<Option<Parent>, Error> {
        Ok(self.parent.clone())
    }
    async fn issue_project(&self, _: String) -> Result<Option<String>, Error> {
        unreachable!("issue project")
    }
    async fn create(&self, _: create::Input) -> Result<Created, Error> {
        unreachable!("create")
    }
    async fn update(&self, _: String, _: IssueUpdateInput) -> Result<Updated, Error> {
        unreachable!("update")
    }
}

/// Gives scripted answers and records what was shown.
#[derive(Default)]
struct Script {
    answers: VecDeque<&'static str>,
    shown: Vec<String>,
    menus: Vec<Vec<String>>,
    defaults: Vec<usize>,
    checked: VecDeque<Vec<&'static str>>,
}

impl Script {
    fn answering(answers: &[&'static str]) -> Self {
        Self {
            answers: answers.iter().copied().collect(),
            ..Self::default()
        }
    }
    fn done(&self) {
        assert!(
            self.answers.is_empty(),
            "unused answers: {:?}",
            self.answers
        );
        assert!(
            self.checked.is_empty(),
            "unused checkbox answers: {:?}",
            self.checked
        );
    }
    fn selected(labels: &[String], answer: &str) -> usize {
        if let Some(index) = answer.strip_prefix('#') {
            let index = index.parse::<usize>().expect("scripted choice index");
            assert!(index < labels.len(), "scripted index in menu");
            return index;
        }
        let matches: Vec<_> = labels
            .iter()
            .enumerate()
            .filter(|(_, label)| *label == answer)
            .map(|(index, _)| index)
            .collect();
        match matches.as_slice() {
            [index] => *index,
            _ => panic!("scripted answer {answer:?} must select exactly one of {labels:?}"),
        }
    }
}

impl Ui for Script {
    fn text(&mut self, text: Text<'_>) -> Result<String, Error> {
        self.shown.push(text.message().into());
        let raw = self.answers.pop_front().expect("an answer");
        text.answer(raw).map_err(Error::new)
    }
    fn parsed<T>(
        &mut self,
        text: Text<'_>,
        parse: &dyn Fn(&str) -> Result<T, String>,
    ) -> Result<Option<T>, Error> {
        let answer = self.text(text)?;
        if answer.is_empty() {
            return Ok(None);
        }
        parse(&answer).map(Some).map_err(Error::new)
    }
    fn choose<T>(
        &mut self,
        message: &str,
        choices: Vec<Choice<T>>,
        default: usize,
    ) -> Result<T, Error> {
        self.shown.push(message.into());
        let labels: Vec<_> = choices.iter().map(ToString::to_string).collect();
        let index = self
            .answers
            .pop_front()
            .map_or(default, |answer| Self::selected(&labels, answer));
        self.menus.push(labels);
        self.defaults.push(default);
        Ok(choices
            .into_iter()
            .nth(index)
            .expect("selected choice exists")
            .value)
    }
    fn checkbox<T>(&mut self, message: &str, choices: Vec<Choice<T>>) -> Result<Vec<T>, Error> {
        self.shown.push(message.into());
        let labels: Vec<_> = choices.iter().map(ToString::to_string).collect();
        let answers = self.checked.pop_front().expect("checkbox answer");
        let selected: Vec<_> = answers
            .iter()
            .map(|answer| Self::selected(&labels, answer))
            .collect();
        self.menus.push(labels);
        Ok(choices
            .into_iter()
            .enumerate()
            .filter(|(index, _)| selected.contains(index))
            .map(|(_, choice)| choice.value)
            .collect())
    }
    fn note(&mut self, text: &str) -> Result<(), Error> {
        self.shown.push(text.into());
        Ok(())
    }
    fn discover_editor(&mut self) -> Result<Option<String>, Error> {
        Ok(None)
    }
    fn optional_editor(&mut self) -> Result<Option<String>, Error> {
        unreachable!("no editor was offered")
    }
}

fn settings(assign_self: AssignSelf, ask_project: bool) -> CreateSettings {
    CreateSettings {
        default_team: Some("ENG".into()),
        assign_self,
        ask_project,
    }
}

fn input(created: super::Interactive) -> Value {
    serde_json::to_value(created.input).expect("issue input")
}

#[tokio::test]
async fn a_submitted_issue_gets_trimmed_text_the_first_unstarted_state_and_its_creator() {
    let linear = Linear::default();
    let mut ui = Script::answering(&["  Title 界  ", "  body  ", "Submit issue", "No"]);
    let fields = Fields {
        use_default_template: true,
        ..Fields::default()
    };
    let created = prompt(
        &linear,
        &mut ui,
        &settings(AssignSelf::Always, false),
        &fields,
    )
    .await
    .expect("issue input");
    ui.done();
    assert!(!created.start);
    let input = input(created);
    assert_eq!(input["title"], "Title 界");
    assert_eq!(input["description"], "body");
    assert_eq!(input["stateId"], "first");
    assert_eq!(input["assigneeId"], "self-id");
    assert_eq!(input["teamId"], "team-id");
    assert_eq!(input["labelIds"], json!([]));
    assert_eq!(input["useDefaultTemplate"], true);
}

#[tokio::test]
async fn a_sub_issue_takes_the_parent_project_without_asking() {
    let linear = Linear {
        parent: Some(Parent {
            title: "Parent work".into(),
            identifier: "ENG-9".into(),
            project_id: None,
        }),
        ..Linear::default()
    };
    let mut ui = Script::answering(&["X", "", "Submit issue", "No"]);
    let fields = Fields {
        parent: Some("ENG-9".into()),
        ..Fields::default()
    };
    let created = prompt(
        &linear,
        &mut ui,
        &settings(AssignSelf::Never, true),
        &fields,
    )
    .await
    .expect("issue input");
    assert_eq!(
        ui.shown[0],
        "Creating sub-issue for: ENG-9: Parent work\n\n"
    );
    assert!(!linear.called("projects"));
    assert!(!linear.called("viewer"));
    let input = input(created);
    assert_eq!(input["parentId"], "parent-id");
    assert_eq!(input["projectId"], Value::Null);
    assert_eq!(input["useDefaultTemplate"], false);
}

#[test]
fn a_near_miss_offers_the_closest_names() {
    let named = |id: &str, name: &str| Named {
        id: id.into(),
        name: name.into(),
    };
    let mut ui = Script::default();
    assert_eq!(
        create::select_option(&mut ui, "Project", "P", &[]).expect("no candidates"),
        None
    );
    assert!(ui.shown.is_empty(), "nothing to ask about");

    let one = [named("a", "old"), named("a", "renamed")];
    assert_eq!(
        create::select_option(&mut ui, "Project", "P", &one).expect("one candidate"),
        Some("a".into())
    );
    assert_eq!(
        ui.shown,
        ["Project named P does not exist, but old exists. Is this what you meant?"]
    );

    let several = [named("10", "ten"), named("2", "two"), named("10", "TEN")];
    for (answer, chosen) in [("two", Some("2")), ("none of the above", None)] {
        let mut ui = Script::answering(&[answer]);
        assert_eq!(
            create::select_option(&mut ui, "Project", "missing", &several).expect("choice"),
            chosen.map(str::to_owned)
        );
        let labels: Vec<_> = ui.menus[0].iter().map(String::as_str).collect();
        assert_eq!(labels, ["ten", "two", "none of the above"]);
        ui.done();
    }
}

#[tokio::test]
async fn additional_fields_keep_menu_order_defaults_and_typed_values() {
    let linear = Linear {
        labels: vec![
            Label {
                id: "label-a".into(),
                name: "First label".into(),
                color: "#abcdef".into(),
            },
            Label {
                id: "label-b".into(),
                name: "Second label".into(),
                color: "#abcdef".into(),
            },
        ],
        projects: vec![Named {
            id: "release".into(),
            name: "Release".into(),
        }],
        ..Linear::default()
    };
    let mut ui = Script::answering(&[
        "Title",
        "",
        "Add more fields",
        "later (unstarted)",
        "Me",
        "⚠⚠⚠ Urgent",
        "0",
        "Release",
        "No",
    ]);
    ui.checked = [
        vec![
            "Project",
            "Estimate",
            "Labels",
            "Priority",
            "Assignee (unassigned)",
            "Workflow state (first)",
        ],
        vec!["Second label", "First label"],
    ]
    .into();
    let created = prompt(
        &linear,
        &mut ui,
        &settings(AssignSelf::Never, false),
        &Fields::default(),
    )
    .await
    .expect("issue input");
    ui.done();
    assert_eq!(
        ui.menus[1],
        [
            "Workflow state (first)",
            "Assignee (unassigned)",
            "Priority",
            "Labels",
            "Estimate",
            "Project"
        ]
    );
    assert_eq!(
        ui.menus[2],
        [
            "started (started)",
            "later (unstarted)",
            "first (unstarted)",
            "tied (unstarted)"
        ]
    );
    assert_eq!(ui.defaults[1], 2, "lowest unstarted state; first tie");
    let input = input(created);
    assert_eq!(input["stateId"], "later");
    assert_eq!(input["assigneeId"], "self-id");
    assert_eq!(input["priority"], 1);
    assert_eq!(input["estimate"], 0);
    assert_eq!(input["labelIds"], json!(["label-a", "label-b"]));
    assert_eq!(input["projectId"], "release");
}

#[tokio::test]
async fn more_fields_resets_default_state_and_omits_zero_priority() {
    let linear = Linear::default();
    let mut ui = Script::answering(&["Title", "", "Add more fields", "--- No priority", "No"]);
    ui.checked.push_back(vec!["Priority"]);
    let created = prompt(
        &linear,
        &mut ui,
        &settings(AssignSelf::Always, false),
        &Fields::default(),
    )
    .await
    .expect("issue input");
    ui.done();
    let input = input(created);
    assert!(input.get("stateId").is_none());
    assert!(input.get("priority").is_none());
    assert_eq!(input["assigneeId"], "self-id");
    assert_eq!(input["labelIds"], json!([]));
}

#[tokio::test]
async fn declining_self_assignment_overrides_the_auto_assignment() {
    let linear = Linear::default();
    let mut ui = Script::answering(&["Title", "", "Add more fields", "Unassigned", "No"]);
    ui.checked.push_back(vec!["Assignee (self)"]);
    let created = prompt(
        &linear,
        &mut ui,
        &settings(AssignSelf::Always, false),
        &Fields::default(),
    )
    .await
    .expect("issue input");
    ui.done();
    assert_eq!(
        ui.defaults,
        [0, 1, 0],
        "the assignee question defaults to yes"
    );
    assert!(input(created).get("assigneeId").is_none());
}

#[tokio::test]
async fn empty_state_and_label_lists_skip_their_selection_prompts() {
    let linear = Linear {
        empty_states: true,
        ..Linear::default()
    };
    let mut ui = Script::answering(&["Title", "", "Add more fields", "No"]);
    ui.checked.push_back(vec!["Workflow state", "Labels"]);
    let created = prompt(
        &linear,
        &mut ui,
        &settings(AssignSelf::Never, false),
        &Fields::default(),
    )
    .await
    .expect("issue input");
    ui.done();
    assert_eq!(ui.menus.len(), 3, "next action, fields and start only");
    assert!(
        ui.shown
            .contains(&"Team ENG has no workflow states to choose from.\n".to_owned())
    );
    assert!(
        ui.shown
            .contains(&"Team ENG has no labels to choose from.\n".to_owned())
    );
    let input = input(created);
    assert!(input.get("stateId").is_none());
    assert_eq!(input["labelIds"], json!([]));
}

#[tokio::test]
async fn project_selection_or_decline_survives_unrelated_additional_fields() {
    for (answer, project) in [("Release", json!("release")), ("No project", Value::Null)] {
        let linear = Linear {
            projects: vec![Named {
                id: "release".into(),
                name: "Release".into(),
            }],
            ..Linear::default()
        };
        let mut ui = Script::answering(&["Title", "", answer, "Add more fields", "No"]);
        ui.checked.push_back(vec![]);
        let created = prompt(
            &linear,
            &mut ui,
            &settings(AssignSelf::Never, true),
            &Fields::default(),
        )
        .await
        .expect("issue input");
        ui.done();
        assert_eq!(ui.menus[0], ["No project", "Release"]);
        assert!(!ui.menus[2].contains(&"Project".to_owned()));
        assert_eq!(input(created)["projectId"], project);
    }
}

#[tokio::test]
async fn a_team_picker_returns_the_selected_team_and_start_answer() {
    let linear = Linear {
        teams: vec![
            ResolvedTeam {
                id: "eng".into(),
                key: "ENG".into(),
                name: "Shared".into(),
            },
            ResolvedTeam {
                id: "ops".into(),
                key: "OPS".into(),
                name: "Shared".into(),
            },
        ],
        ..Linear::default()
    };
    let mut config = settings(AssignSelf::Never, false);
    config.default_team = None;
    let mut ui = Script::answering(&["Shared (OPS)", "Title", "", "Submit issue", "Yes"]);
    let created = prompt(&linear, &mut ui, &config, &Fields::default())
        .await
        .expect("issue input");
    ui.done();
    assert_eq!(ui.menus[0], ["Shared (ENG)", "Shared (OPS)"]);
    assert!(created.start);
    assert_eq!(input(created)["teamId"], "ops");
}

#[test]
fn near_miss_choices_distinguish_duplicate_labels_and_the_decline_label() {
    for (names, answer, expected) in [
        (["same", "same"], "#1", Some("second")),
        (["other", "none of the above"], "#1", Some("second")),
        (["other", "none of the above"], "#2", None),
    ] {
        let options = names
            .into_iter()
            .zip(["first", "second"])
            .map(|(name, id)| Named {
                id: id.into(),
                name: name.into(),
            })
            .collect::<Vec<_>>();
        let mut ui = Script::answering(&[answer]);
        assert_eq!(
            create::select_option(&mut ui, "Project", "missing", &options)
                .expect("choice")
                .as_deref(),
            expected
        );
        ui.done();
    }
    let mut ui = Script::answering(&["no"]);
    let options = [Named {
        id: "first".into(),
        name: "Only".into(),
    }];
    assert_eq!(
        create::select_option(&mut ui, "Project", "missing", &options).expect("decline"),
        None
    );
    ui.done();
}

#[tokio::test]
async fn no_accessible_teams_returns_an_error_without_opening_a_team_picker() {
    let linear = Linear::default();
    let mut config = settings(AssignSelf::Never, true);
    config.default_team = None;
    let mut ui = Script::answering(&[]);
    let Err(error) = prompt(&linear, &mut ui, &config, &Fields::default()).await else {
        panic!("no accessible team must prevent issue creation");
    };
    assert_eq!(
        error.message(),
        "This workspace has no teams you can access"
    );
    assert_eq!(
        error.hint(),
        Some("Ask a workspace admin to add you to a team, or check the API key's workspace.")
    );
    assert!(ui.shown.is_empty(), "the title is asked after the team");
    assert!(ui.menus.is_empty());
    assert!(!linear.called("projects"));
    assert!(!linear.called("viewer"));
    ui.done();
}

#[tokio::test]
async fn a_team_without_projects_says_so_instead_of_skipping_silently() {
    let linear = Linear::default();
    let mut ui = Script::answering(&["Title", "", "Submit issue", "No"]);
    let created = prompt(
        &linear,
        &mut ui,
        &settings(AssignSelf::Never, true),
        &Fields::default(),
    )
    .await
    .expect("issue input");
    ui.done();
    assert!(
        ui.shown
            .contains(&"Team ENG has no projects, so the issue gets none.\n".to_owned()),
        "{:?}",
        ui.shown
    );
    assert_eq!(input(created)["projectId"], Value::Null);
}
