use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use serde_json::{Value, json};

use super::prompt;
use crate::commands::issue::create::{self, Fields};
use crate::commands::issue::write::{
    AssignSelf, Backend, CreateSettings, Created, Label, Named, Parent, State, Team, Ui, Updated,
};
use crate::error::Error;
use crate::graphql::operations::issue_update::IssueUpdateInput;
use crate::platform::prompt::Text;

/// Answers the lookups an interactive create makes and records which ran.
#[derive(Clone, Default)]
struct Linear {
    calls: Arc<Mutex<Vec<String>>>,
    parent: Option<Parent>,
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
    async fn team(&self, _: String) -> Result<Team, Error> {
        unreachable!("team")
    }
    async fn find_team(&self, key: String) -> Result<Option<Team>, Error> {
        Ok(Some(Team {
            id: "team-id".into(),
            key,
            name: "Engineering".into(),
        }))
    }
    async fn teams(&self) -> Result<Vec<Team>, Error> {
        unreachable!("teams")
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
    async fn user(&self, _: String) -> Result<String, Error> {
        unreachable!("user")
    }
    async fn states(&self, _: String) -> Result<Vec<State>, Error> {
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
        Ok(Vec::new())
    }
    async fn project(&self, _: String) -> Result<Option<String>, Error> {
        unreachable!("project")
    }
    async fn project_options(&self, _: String) -> Result<Vec<Named>, Error> {
        unreachable!("project options")
    }
    async fn projects(&self, _: String) -> Result<Vec<Named>, Error> {
        self.note("projects");
        Ok(Vec::new())
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
    menus: Vec<Vec<Named>>,
}

impl Script {
    fn answering(answers: &[&'static str]) -> Self {
        Self {
            answers: answers.iter().copied().collect(),
            ..Self::default()
        }
    }
}

impl Ui for Script {
    fn text(&mut self, text: Text<'_>) -> Result<String, Error> {
        self.shown.push(text.message().into());
        let raw = self.answers.pop_front().expect("an answer");
        text.answer(raw).map_err(Error::new)
    }
    fn choose(
        &mut self,
        message: &str,
        options: &[Named],
        default: usize,
    ) -> Result<String, Error> {
        self.shown.push(message.into());
        self.menus.push(options.to_vec());
        Ok(self
            .answers
            .pop_front()
            .map_or_else(|| options[default].id.clone(), str::to_owned))
    }
    fn checkbox(&mut self, _: &str, _: &[Named]) -> Result<Vec<String>, Error> {
        Ok(Vec::new())
    }
    fn output(&mut self, text: &str) -> Result<(), Error> {
        self.shown.push(text.into());
        Ok(())
    }
    fn error(&mut self, text: &str) -> Result<(), Error> {
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
    let mut ui = Script::answering(&["  Title 界  ", "  body  ", "submit", "no"]);
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
    assert_eq!(created.title, "Title 界");
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
    let mut ui = Script::answering(&["X", "", "submit", "no"]);
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
    for (answer, chosen) in [("1", Some("2")), ("2", None)] {
        let mut ui = Script::answering(&[answer]);
        assert_eq!(
            create::select_option(&mut ui, "Project", "missing", &several).expect("choice"),
            chosen.map(str::to_owned)
        );
        let labels: Vec<_> = ui.menus[0].iter().map(|o| o.name.as_str()).collect();
        assert_eq!(labels, ["ten", "two", "none of the above"]);
    }
}
