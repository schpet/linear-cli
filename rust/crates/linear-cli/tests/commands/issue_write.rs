use linear_cli::{
    commands::{
        issue::create,
        issue::update,
        issue::write::{
            self as shared, AssignSelf, Backend, CreateSettings, Created, Label, Named, Parent,
            State, Team, Ui, Updated,
        },
    },
    error::Error,
    graphql::operations::issue_update::IssueUpdateInput,
};
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
};
#[derive(Clone, Default)]
struct Fake {
    calls: Arc<Mutex<Vec<String>>>,
    state_fails: bool,
    empty_project: bool,
    parent_project: Option<Option<String>>,
}
impl Fake {
    fn note(&self, message: impl Into<String>) {
        self.calls
            .lock()
            .expect("ledger poisoned")
            .push(message.into())
    }
    fn calls(&self) -> Vec<String> {
        self.calls.lock().expect("ledger poisoned").clone()
    }
}
impl Backend for Fake {
    async fn team(&self, value: String) -> Result<Team, Error> {
        self.note(format!("Team:{value}"));
        Ok(Team {
            id: "team-id".into(),
            key: "ENG".into(),
            name: "Engineering".into(),
        })
    }
    async fn find_team(&self, value: String) -> Result<Option<Team>, Error> {
        self.note(format!("FindTeam:{value}"));
        Ok(Some(Team {
            id: "team-id".into(),
            key: value,
            name: "Engineering".into(),
        }))
    }
    async fn teams(&self) -> Result<Vec<Team>, Error> {
        panic!("unexpected all-team request")
    }
    async fn team_options(&self, _: String) -> Result<Vec<Named>, Error> {
        panic!("unexpected team options")
    }
    async fn viewer(&self) -> Result<String, Error> {
        self.note("Viewer");
        Ok("self-id".into())
    }
    async fn auto_assign(&self) -> Result<bool, Error> {
        self.note("Auto");
        Ok(true)
    }
    async fn user(&self, value: String) -> Result<String, Error> {
        self.note(format!("User:{value}"));
        Ok(format!("user-{value}"))
    }
    async fn states(&self, key: String) -> Result<Vec<State>, Error> {
        self.note(format!("States:{key}"));
        Ok(Vec::new())
    }
    async fn state(&self, key: String, value: String) -> Result<String, Error> {
        self.note(format!("State:{key}:{value}"));
        if self.state_fails {
            Err(shared::validation("missing state"))
        } else {
            Ok("state-id".into())
        }
    }
    async fn label(&self, key: String, value: String) -> Result<Option<String>, Error> {
        self.note(format!("Label:{key}:{value}"));
        Ok(Some(value.to_lowercase()))
    }
    async fn label_options(&self, _: String, _: String) -> Result<Vec<Named>, Error> {
        panic!("unexpected label options")
    }
    async fn labels(&self, key: String) -> Result<Vec<Label>, Error> {
        self.note(format!("Labels:{key}"));
        Ok(Vec::new())
    }
    async fn project(&self, value: String) -> Result<Option<String>, Error> {
        self.note(format!("Project:{value}"));
        Ok(Some(
            if self.empty_project { "" } else { "project-id" }.into(),
        ))
    }
    async fn project_options(&self, _: String) -> Result<Vec<Named>, Error> {
        panic!("unexpected project options")
    }
    async fn projects(&self, key: String) -> Result<Vec<Named>, Error> {
        self.note(format!("Projects:{key}"));
        Ok(if self.empty_project {
            vec![Named {
                id: "chosen-project".into(),
                name: "Chosen project".into(),
            }]
        } else {
            Vec::new()
        })
    }
    async fn milestone(&self, project: String, value: String) -> Result<String, Error> {
        self.note(format!("Milestone:{project}:{value}"));
        Ok("milestone-id".into())
    }
    async fn cycle(&self, team: String, value: String) -> Result<String, Error> {
        self.note(format!("Cycle:{team}:{value}"));
        Ok("cycle-id".into())
    }
    async fn parent_id(&self, value: String) -> Result<String, Error> {
        self.note(format!("Parent:{value}"));
        Ok("parent-id".into())
    }
    async fn parent_metadata(&self, id: String) -> Result<Option<Parent>, Error> {
        self.note(format!("ParentMetadata:{id}"));
        Ok(self.parent_project.clone().map(|project_id| Parent {
            title: "P".into(),
            identifier: "ENG-9".into(),
            project_id,
        }))
    }
    async fn issue_project(&self, id: String) -> Result<Option<String>, Error> {
        self.note(format!("IssueProject:{id}"));
        Ok(Some("existing-project".into()))
    }
    async fn create(&self, _: create::Input) -> Result<Created, Error> {
        panic!("unexpected CreateIssue")
    }
    async fn update(&self, _: String, _: IssueUpdateInput) -> Result<Updated, Error> {
        panic!("unexpected UpdateIssue")
    }
}
impl create::Templates for Fake {
    async fn issue_template(&self, value: String, team: String) -> Result<String, Error> {
        self.note(format!("Template:{team}:{value}"));
        Ok("template-id".into())
    }
}
#[derive(Default)]
struct Prompt {
    answers: VecDeque<String>,
    messages: Vec<String>,
    menus: Vec<(String, Vec<Named>)>,
    selected_fields: Vec<String>,
}
impl Ui for Prompt {
    fn text(
        &mut self,
        message: &str,
        required: bool,
        default: Option<&str>,
    ) -> Result<String, Error> {
        self.messages.push(message.into());
        let raw = self.answers.pop_front().expect("missing answer");
        linear_cli::platform::prompt_text::TextOptions { required, default }
            .answer(&raw)
            .map_err(shared::validation)
    }
    fn choose(
        &mut self,
        message: &str,
        options: &[Named],
        default: usize,
        _: Option<linear_cli::commands::issue::write::Search>,
    ) -> Result<String, Error> {
        self.messages.push(message.into());
        self.menus.push((message.into(), options.to_vec()));
        Ok(self
            .answers
            .pop_front()
            .unwrap_or_else(|| options[default].id.clone()))
    }
    fn checkbox(
        &mut self,
        message: &str,
        options: &[Named],
        _: bool,
    ) -> Result<Vec<String>, Error> {
        self.menus.push((message.into(), options.to_vec()));
        Ok(if message == "Select additional fields to configure" {
            self.selected_fields.clone()
        } else {
            Vec::new()
        })
    }
    fn suspend(&mut self) -> Result<(), Error> {
        Ok(())
    }
    fn output(&mut self, text: &str) -> Result<(), Error> {
        self.messages.push(text.into());
        Ok(())
    }
    fn error(&mut self, text: &str) -> Result<(), Error> {
        self.messages.push(text.into());
        Ok(())
    }
    fn discover_editor(&mut self) -> Result<Option<String>, Error> {
        Ok(None)
    }
    fn optional_editor(&mut self) -> Result<Option<String>, Error> {
        panic!("unexpected editor")
    }
}
fn settings() -> CreateSettings {
    CreateSettings {
        default_team: Some("ENG".into()),
        assign_self: AssignSelf::Always,
        ask_project: false,
    }
}
#[tokio::test]
async fn failing_state_precedes_always_self_and_mutation() {
    let backend = Fake {
        state_fails: true,
        ..Default::default()
    };
    let mut ui = Prompt::default();
    let fields = create::Fields {
        title: Some("X".into()),
        state: Some("missing".into()),
        template: Some("Issue template".into()),
        use_default_template: true,
        ..Default::default()
    };
    let error = create::flag_input(&backend, &mut ui, &settings(), &fields, None, false)
        .await
        .err()
        .expect("must fail");
    assert_eq!(error.message(), "missing state");
    assert_eq!(
        backend.calls(),
        [
            "FindTeam:ENG",
            "Template:team-id:Issue template",
            "State:ENG:missing"
        ]
    );
}
#[tokio::test]
async fn create_override_still_performs_always_self_read_and_keeps_duplicate_labels() {
    let backend = Fake::default();
    let mut ui = Prompt::default();
    let fields = create::Fields {
        title: Some("X".into()),
        state: Some("Started".into()),
        assignee: Some("other".into()),
        labels: vec!["Bug".into(), "bug".into()],
        use_default_template: true,
        ..Default::default()
    };
    let value = create::flag_input(&backend, &mut ui, &settings(), &fields, None, false)
        .await
        .expect("assembly");
    assert_eq!(
        backend.calls(),
        [
            "FindTeam:ENG",
            "State:ENG:Started",
            "Viewer",
            "User:other",
            "Label:ENG:Bug",
            "Label:ENG:bug"
        ]
    );
    let json = serde_json::to_value(value.input).expect("wire");
    assert_eq!(json["labelIds"], serde_json::json!(["bug", "bug"]));
    assert_eq!(json["assigneeId"], "user-other");
    assert_eq!(json["useDefaultTemplate"], true);
}
#[tokio::test]
async fn flag_parent_valid_null_project_differs_from_failed_metadata() {
    for (metadata, expected) in [(Some(None), Some(serde_json::Value::Null)), (None, None)] {
        let backend = Fake {
            parent_project: metadata,
            ..Default::default()
        };
        let mut ui = Prompt::default();
        let fields = create::Fields {
            title: Some("X".into()),
            parent: Some("ENG-9".into()),
            use_default_template: true,
            ..Default::default()
        };
        let value = create::flag_input(&backend, &mut ui, &settings(), &fields, None, false)
            .await
            .expect("assembly");
        let json = serde_json::to_value(value.input).expect("wire");
        assert_eq!(json.get("projectId").cloned(), expected);
        assert_eq!(
            backend.calls(),
            [
                "FindTeam:ENG",
                "Viewer",
                "Parent:ENG-9",
                "ParentMetadata:parent-id"
            ]
        );
    }
}
#[tokio::test]
async fn update_no_flags_always_sends_team_and_empty_input_is_not_noop() {
    let backend = Fake::default();
    let input = update::input(&backend, "ENG-1", &update::Fields::default(), None)
        .await
        .expect("assembly");
    assert_eq!(backend.calls(), ["Team:ENG"]);
    assert_eq!(
        serde_json::to_value(input).expect("wire"),
        serde_json::json!({"teamId":"team-id"})
    );
}
#[tokio::test]
async fn update_label_dedupe_overlap_stops_before_project_parent_and_mutation() {
    let backend = Fake::default();
    let fields = update::Fields {
        add_labels: Some(vec!["Bug".into(), "bug".into()]),
        remove_labels: Some(vec!["BUG".into()]),
        project: Some("P".into()),
        parent: Some("ENG-9".into()),
        ..Default::default()
    };
    let error = update::input(&backend, "ENG-1", &fields, None)
        .await
        .map_or_else(std::convert::identity, |_| panic!("overlap"));
    assert_eq!(
        error.message(),
        "Cannot add and remove the same label in one update"
    );
    assert_eq!(
        backend.calls(),
        [
            "Team:ENG",
            "Label:ENG:Bug",
            "Label:ENG:bug",
            "Label:ENG:BUG"
        ]
    );
}
#[tokio::test]
async fn update_clear_wire_order_and_lossy_file_success_are_independent_of_parser_only_controls() {
    let backend = Fake::default();
    let fields = update::Fields {
        unassign: true,
        clear_due_date: true,
        clear_parent: true,
        priority: Some(linear_cli::cli::values::Priority::None),
        clear_estimate: true,
        clear_project: true,
        clear_milestone: true,
        clear_cycle: true,
        ..Default::default()
    };
    let input = update::input(&backend, "ENG-1", &fields, None)
        .await
        .expect("assembly");
    assert_eq!(
        serde_json::to_string(&input).expect("wire"),
        "{\"assigneeId\":null,\"dueDate\":null,\"parentId\":null,\"priority\":0,\"estimate\":null,\"teamId\":\"team-id\",\"projectId\":null,\"projectMilestoneId\":null,\"cycleId\":null}"
    );
}
#[test]
fn integer_menu_source_prefix_and_default_state_stable_lowest() {
    for (input, expected) in [
        ("", None),
        ("NaN", None),
        ("3.8points", Some(3)),
        ("0x10rest", Some(16)),
        ("-2tail", Some(-2)),
    ] {
        assert_eq!(shared::menu_estimate(input).expect("parse"), expected)
    }
    assert!(shared::menu_estimate("2147483648").is_err());
    let state = |id: &str, kind: &str, position| State {
        id: id.into(),
        name: id.into(),
        kind: kind.into(),
        position,
    };
    assert_eq!(
        shared::default_state(&[
            state("A", "started", 0.0),
            state("B", "unstarted", 9.0),
            state("C", "unstarted", 1.0),
            state("D", "unstarted", 1.0)
        ])
        .expect("default"),
        Some("C".into())
    );
}
#[test]
fn fallback_one_match_is_yes_no_multiple_has_none_and_no_matches_do_not_prompt() {
    let mut ui = Prompt::default();
    assert_eq!(
        create::select_option(&mut ui, "Project", "P", &[]).expect("none"),
        None
    );
    assert!(ui.messages.is_empty());
    let named = Named {
        id: "id".into(),
        name: "Project X".into(),
    };
    assert_eq!(
        create::select_option(&mut ui, "Project", "P", &[named]).expect("one"),
        Some("id".into())
    );
    assert_eq!(
        ui.messages,
        ["Project named P does not exist, but Project X exists. Is this what you meant?"]
    );
}

fn network(
    replies: Vec<serde_json::Value>,
) -> (
    linear_cli::commands::issue::write_network::NetworkBackend,
    std::thread::JoinHandle<Vec<serde_json::Value>>,
) {
    network_with_content_types(
        replies
            .into_iter()
            .map(|value| (Some("application/json"), value.to_string()))
            .collect(),
    )
}
pub(super) fn network_with_content_types(
    replies: Vec<(Option<&'static str>, String)>,
) -> (
    linear_cli::commands::issue::write_network::NetworkBackend,
    std::thread::JoinHandle<Vec<serde_json::Value>>,
) {
    use linear_cli::config::{ConfigInputs, ConfigOptions, OptionInputs, OsFamily, SelectedEnv};
    let (transport, server) = super::project_write_server::serve_with_content_types(replies);
    let env = ConfigInputs {
        cwd: "/fake-issue-write".into(),
        os: OsFamily::Unix,
        process_env: std::collections::BTreeMap::new(),
    };
    let dotenv = SelectedEnv {
        applied: std::collections::BTreeMap::new(),
        source_path: None,
        diagnostics: Vec::new(),
    };
    let options = ConfigOptions::from_inputs(OptionInputs {
        env: &env,
        dotenv: &dotenv,
        project: None,
        global: None,
    })
    .unwrap();
    (
        linear_cli::commands::issue::write_network::NetworkBackend {
            transport,
            options,
            cli_workspace: None,
            default_workspace: None,
        },
        server,
    )
}
#[tokio::test]
async fn parent_optional_observation_distinguishes_absence_transport_failure_and_present_wrong_fields()
 {
    use serde_json::json;
    for reply in [
        json!({"data":{"issue":null}}),
        json!({"data":{}}),
        json!({"errors":[{"message":"optional failure"}]}),
        json!({"data":{"issue":{"title":"Parent","identifier":"ENG-9","project":null}}}),
    ] {
        let valid = reply.pointer("/data/issue/title").is_some();
        let (backend, server) = network(vec![reply]);
        let result = backend
            .parent_metadata("opaque-parent".into())
            .await
            .unwrap();
        assert_eq!(result.is_some(), valid);
        if let Some(parent) = result {
            assert_eq!(parent.project_id, None);
        }
        let sent = server.join().unwrap();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0]["variables"], json!({"id":"opaque-parent"}));
    }
    let (backend, server) = network(vec![
        json!({"data":{"issue":{"title":7,"identifier":"ENG-9","project":null}}}),
    ]);
    assert!(
        backend
            .parent_metadata("opaque".into())
            .await
            .unwrap_err()
            .message()
            .contains("unexpected shape")
    );
    assert_eq!(server.join().unwrap().len(), 1);
    for reply in [
        json!({"data":{}}),
        json!({"data":{"issue":null}}),
        json!({"data":{"issue":{}}}),
        json!({"data":{"issue":{"id":""}}}),
    ] {
        let (backend, server) = network(vec![reply]);
        assert_eq!(
            backend
                .parent_id("ENG-9".into())
                .await
                .unwrap_err()
                .message(),
            "Parent issue not found: ENG-9"
        );
        assert_eq!(server.join().unwrap().len(), 1);
    }
    let (backend, server) = network(vec![json!({"data":{"issue":[]}})]);
    assert!(backend.parent_id("ENG-9".into()).await.is_err());
    server.join().unwrap();
}
#[tokio::test]
async fn full_mutation_decode_precedes_false_null_and_preserves_raw_first_error() {
    use serde_json::json;
    let created =
        json!({"id":"opaque","identifier":"ENG-9","url":"server-url","team":{"key":"ENG"}});
    for (success, issue, want) in [
        (false, created.clone(), "Issue creation failed"),
        (
            true,
            serde_json::Value::Null,
            "Issue creation failed - no issue returned",
        ),
    ] {
        let (backend, server) = network(vec![
            json!({"data":{"issueCreate":{"success":success,"issue":issue}}}),
        ]);
        assert_eq!(
            backend
                .create(create::Input {
                    team_id: "team".into(),
                    ..Default::default()
                })
                .await
                .err()
                .unwrap()
                .message(),
            want
        );
        assert_eq!(server.join().unwrap().len(), 1);
    }
    for reply in [
        json!({"data":{"issueCreate":{"success":false,"issue":{"id":"opaque","identifier":"ENG-1","url":12,"team":{"key":"ENG"}}}}}),
        json!({"data":{"issueCreate":{"issue":created}}}),
    ] {
        let (backend, server) = network(vec![reply]);
        let error = backend
            .create(create::Input {
                team_id: "team".into(),
                ..Default::default()
            })
            .await
            .err()
            .unwrap();
        assert_ne!(error.message(), "Issue creation failed");
        assert_eq!(server.join().unwrap().len(), 1);
    }
    let (backend, server) = network(vec![json!({"errors":[{"message":""},{"message":"later"}]})]);
    let error = backend
        .update("opaque".into(), IssueUpdateInput::default())
        .await
        .err()
        .unwrap();
    assert!(error.message().contains("later"));
    assert_eq!(server.join().unwrap().len(), 1);
}
#[test]
fn issue_and_project_templates_share_scope_rules_with_exact_project_regression() {
    use linear_cli::{
        commands::issue::template_scope::{self as issue_template_scope, TemplateScope},
        graphql::operations::templates::Template,
    };
    let template = |kind: &str| {
        serde_json::from_value::<Template>(serde_json::json!({"id":"t","name":"Plan","description":null,"type":kind,"icon":null,"color":null,"hasFormFields":false,"lastAppliedAt":null,"sortOrder":0,"createdAt":"x","updatedAt":"x","team":null,"inheritedFrom":null,"creator":null,"templateData":"not parsed JSON"})).unwrap()
    };
    assert_eq!(
        issue_template_scope::select("PLAN", vec![template("issue")], &[], TemplateScope::Issue)
            .unwrap()
            .id
            .inner(),
        "t"
    );
    let error =
        issue_template_scope::select("Plan", vec![template("issue")], &[], TemplateScope::Project)
            .unwrap_err();
    assert_eq!(
        error.message(),
        "Template \"Plan\" is an issue template, not a project template"
    );
    assert_eq!(
        error.hint(),
        Some("Run `linear template list --type project` to see the project templates.")
    );
    let error =
        issue_template_scope::select("Plan", vec![template("project")], &[], TemplateScope::Issue)
            .unwrap_err();
    assert_eq!(
        error.message(),
        "Template \"Plan\" is a project template, not an issue template"
    );
    assert_eq!(
        error.hint(),
        Some("Run `linear template list --type issue` to see the issue templates.")
    );
}
#[test]
fn fallback_menu_dedupes_ids_in_order_and_declining_returns_none() {
    let named = |id: &str, name: &str| Named {
        id: id.into(),
        name: name.into(),
    };
    let options = [named("10", "ten"), named("2", "two"), named("10", "TEN")];
    let mut ui = Prompt::default();
    assert_eq!(
        create::select_option(&mut ui, "Project", "missing", &options).unwrap(),
        Some("10".into())
    );
    let names: Vec<_> = ui.menus[0].1.iter().map(|o| o.name.as_str()).collect();
    assert_eq!(names, ["ten", "two", "none of the above"]);
    let mut ui = Prompt {
        answers: VecDeque::from(["1".to_owned()]),
        ..Default::default()
    };
    assert_eq!(
        create::select_option(&mut ui, "Project", "missing", &options).unwrap(),
        Some("2".into())
    );
    let mut ui = Prompt {
        answers: VecDeque::from(["2".to_owned()]),
        ..Default::default()
    };
    assert_eq!(
        create::select_option(&mut ui, "Project", "missing", &options).unwrap(),
        None
    );
    let mut ui = Prompt::default();
    assert_eq!(
        create::select_option(
            &mut ui,
            "Project",
            "missing",
            &[named("a", "old"), named("a", "new")]
        )
        .unwrap(),
        Some("a".into())
    );
    assert!(ui.messages[0].contains("but old exists"));
}

#[test]
fn interactive_defaults_more_fields_discard_and_parent_null_suppresses_project_queries() {
    use linear_cli::commands::issue::create_prompt as issue_create_prompt;
    for (next, expected_viewers) in [("submit", 1), ("more_fields", 2)] {
        let backend = Fake::default();
        let mut ui = Prompt {
            answers: VecDeque::from([
                "  Title 界  ".into(),
                "  body  ".into(),
                next.into(),
                "no".into(),
            ]),
            messages: Vec::new(),
            ..Default::default()
        };
        let output = issue_create_prompt::prompt(
            &backend,
            &mut ui,
            &settings(),
            &create::Fields {
                use_default_template: true,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(output.title, "Title 界");
        assert!(!output.start);
        let json = serde_json::to_value(output.input).unwrap();
        assert_eq!(json["projectId"], serde_json::Value::Null);
        assert_eq!(json["description"], "body");
        assert_eq!(json["labelIds"], serde_json::json!([]));
        assert_eq!(json["useDefaultTemplate"], true);
        assert_eq!(
            backend
                .calls()
                .iter()
                .filter(|call| call.as_str() == "Viewer")
                .count(),
            expected_viewers
        );
        assert_eq!(
            backend
                .calls()
                .iter()
                .filter(|call| call.starts_with("States:"))
                .count(),
            1
        );
        assert_eq!(
            backend
                .calls()
                .iter()
                .filter(|call| call.starts_with("Labels:"))
                .count(),
            1
        );
    }
    let backend = Fake {
        parent_project: Some(None),
        ..Default::default()
    };
    let mut ui = Prompt {
        answers: VecDeque::from(["X".into(), "".into(), "submit".into(), "no".into()]),
        messages: Vec::new(),
        ..Default::default()
    };
    let mut settings = settings();
    settings.ask_project = true;
    settings.assign_self = AssignSelf::Never;
    let output = issue_create_prompt::prompt(
        &backend,
        &mut ui,
        &settings,
        &create::Fields {
            parent: Some("ENG-9".into()),
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        serde_json::to_value(output.input).unwrap()["projectId"],
        serde_json::Value::Null
    );
    assert!(
        !backend
            .calls()
            .iter()
            .any(|call| call.starts_with("Projects:") || call == "Viewer")
    );
    assert_eq!(
        ui.messages.first().unwrap(),
        "Creating sub-issue for: ENG-9: P\n\n"
    );
}
#[test]
fn blank_title_is_rejected_and_default_template_false_remains_explicit() {
    let mut ui = Prompt {
        answers: VecDeque::from([" Title ".into(), "".into(), "submit".into(), "no".into()]),
        messages: Vec::new(),
        ..Default::default()
    };
    let mut settings = settings();
    settings.assign_self = AssignSelf::Never;
    let output = linear_cli::commands::issue::create_prompt::prompt(
        &Fake::default(),
        &mut ui,
        &settings,
        &create::Fields::default(),
    )
    .unwrap();
    let json = serde_json::to_value(output.input).unwrap();
    assert_eq!(json["title"], "Title");
    assert_eq!(json["useDefaultTemplate"], false);
    assert!(
        linear_cli::platform::prompt_text::TextOptions {
            required: true,
            default: None
        }
        .answer(" \t ")
        .is_err()
    );
}
#[tokio::test]
async fn update_false_corrupt_model_fails_full_decode_before_false_message() {
    let (backend, server) = network(vec![
        serde_json::json!({"data":{"issueUpdate":{"success":false,"issue":{"id":"i","identifier":"ENG-1","title":false,"url":"url"}}}}),
    ]);
    let error = backend
        .update("opaque".into(), IssueUpdateInput::default())
        .await
        .err()
        .unwrap();
    assert_ne!(error.message(), "Issue update failed");
    assert_eq!(server.join().unwrap().len(), 1);
}

#[tokio::test]
async fn lookup_operations_and_variables_match_and_settings_has_no_variables() {
    use serde_json::json;
    let (backend, server) = network(vec![
        json!({"data":{"teams":{"nodes":[{"id":"team","key":"ENG","name":"Engineering"}]}}}),
        json!({"data":{"users":{"nodes":[{"id":"user","email":"dummy@example.invalid","displayName":"DUMMY other","name":"DUMMY other"}]}}}),
        json!({"data":{"issueLabels":{"nodes":[{"id":"label","name":"DUMMY label"}]}}}),
        json!({"data":{"userSettings":{"autoAssignToSelf":true}}}),
    ]);
    backend.team("ENG".into()).await.unwrap();
    backend.user("DUMMY other".into()).await.unwrap();
    backend
        .label("ENG".into(), "DUMMY label".into())
        .await
        .unwrap();
    assert!(backend.auto_assign().await.unwrap());
    let mut requests = server.join().unwrap();
    let settings = requests.pop().unwrap();
    assert!(settings.get("variables").is_none());
    let mut expected: Vec<serde_json::Value> =
        serde_json::from_str(include_str!("fixtures/c069-lookups-wire.json")).unwrap();
    // Query text formatting is not part of the contract.
    for request in requests.iter_mut().chain(expected.iter_mut()) {
        request.as_object_mut().unwrap().remove("query");
    }
    assert_eq!(requests, expected);
}

#[tokio::test]
async fn update_empty_returned_label_ids_stop_at_each_label_prefix() {
    use serde_json::json;
    for mode in ["replace", "add", "remove"] {
        let (backend, server) = network(vec![
            json!({"data":{"teams":{"nodes":[{"id":"team","key":"ENG","name":"Engineering"}]}}}),
            json!({"data":{"issueLabels":{"nodes":[{"id":"","name":"DUMMY label"}]}}}),
        ]);
        let mut fields = update::Fields {
            project: Some("never read".into()),
            ..Default::default()
        };
        let names = Some(vec!["DUMMY label".into(), "never looked up".into()]);
        match mode {
            "replace" => fields.labels = names,
            "add" => fields.add_labels = names,
            "remove" => fields.remove_labels = names,
            _ => unreachable!(),
        }
        let error = update::input(&backend, "ENG-7", &fields, None)
            .await
            .map_or_else(std::convert::identity, |_| {
                panic!("empty returned label must be NotFound")
            });
        assert_eq!(
            error.message(),
            "Issue label not found: DUMMY label",
            "{mode}"
        );
        assert_eq!(
            error.hint(),
            Some("Run `linear label list --team ENG` to see available labels.")
        );
        let sent = server.join().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0]["operationName"], "ResolveTeam");
        assert_eq!(sent[1]["operationName"], "GetIssueLabelIdByNameForTeam");
        assert_eq!(
            sent[1]["variables"],
            json!({"name":"DUMMY label","teamKey":"ENG"})
        );
    }
}
#[tokio::test]
async fn update_empty_returned_assignee_ids_stop_before_labels_or_mutation() {
    use serde_json::json;
    for (reference, reply, operation) in [
        ("self", json!({"data":{"viewer":{"id":""}}}), "GetViewerId"),
        (
            "DUMMY user",
            json!({"data":{"users":{"nodes":[{"id":"","name":"DUMMY user","displayName":"DUMMY user","email":"fixture@example.invalid"}]}}}),
            "LookupUser",
        ),
    ] {
        let (backend, server) = network(vec![
            json!({"data":{"teams":{"nodes":[{"id":"team","key":"ENG","name":"Engineering"}]}}}),
            reply,
        ]);
        let fields = update::Fields {
            assignee: Some(reference.into()),
            labels: Some(vec!["never looked up".into()]),
            project: Some("never read".into()),
            ..Default::default()
        };
        let error = update::input(&backend, "ENG-7", &fields, None)
            .await
            .map_or_else(std::convert::identity, |_| {
                panic!("empty returned user must be NotFound")
            });
        assert_eq!(error.message(), format!("User not found: {reference}"));
        assert!(error.hint().is_none());
        let sent = server.join().unwrap();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0]["operationName"], "ResolveTeam");
        assert_eq!(sent[1]["operationName"], operation);
    }
}

#[tokio::test]
async fn m2_empty_project_name_continues_slug_before_create_or_update() {
    use serde_json::json;
    for create_mode in [false, true] {
        let (backend, server) = network(vec![
            json!({"data":{"teams":{"nodes":[{"id":"team","key":"ENG","name":"Engineering"}]}}}),
            json!({"data":{"projects":{"nodes":[{"id":""}]}}}),
            json!({"data":{"projects":{"nodes":[]}}}),
        ]);
        let error = if create_mode {
            let mut settings = settings();
            settings.assign_self = AssignSelf::Never;
            create::flag_input(
                &backend,
                &mut Prompt::default(),
                &settings,
                &create::Fields {
                    title: Some("X".into()),
                    project: Some("Name".into()),
                    ..Default::default()
                },
                None,
                false,
            )
            .await
            .err()
            .expect("expected prefix refusal")
        } else {
            update::input(
                &backend,
                "ENG-7",
                &update::Fields {
                    project: Some("Name".into()),
                    ..Default::default()
                },
                None,
            )
            .await
            .map_or_else(std::convert::identity, |_| {
                panic!("expected prefix refusal")
            })
        };
        assert_eq!(error.message(), "Project not found: Name");
        let sent = server.join().unwrap();
        assert_eq!(
            sent.iter()
                .map(|v| v["operationName"].as_str().unwrap())
                .collect::<Vec<_>>(),
            ["ResolveTeam", "GetProjectIdByName", "GetProjectIdBySlugId"]
        );
        assert_eq!(sent[1]["variables"], json!({"name":"Name"}));
        assert_eq!(sent[2]["variables"], json!({"slugId":"Name"}));
    }
}
#[tokio::test]
async fn m2_empty_existing_project_stops_update_before_milestone_lookup() {
    use serde_json::json;
    let (backend, server) = network(vec![
        json!({"data":{"teams":{"nodes":[{"id":"team","key":"ENG","name":"Engineering"}]}}}),
        json!({"data":{"issue":{"project":{"id":""}}}}),
    ]);
    let error = update::input(
        &backend,
        "ENG-7",
        &update::Fields {
            milestone: Some("Name".into()),
            ..Default::default()
        },
        None,
    )
    .await
    .map_or_else(std::convert::identity, |_| {
        panic!("expected prefix refusal")
    });
    assert_eq!(
        error.message(),
        "Cannot resolve milestone \"Name\" without --project"
    );
    assert_eq!(
        error.hint(),
        Some(
            "Pass a milestone UUID, or specify --project so the milestone name can be looked up within that project."
        )
    );
    let sent = server.join().unwrap();
    assert_eq!(
        sent.iter()
            .map(|v| v["operationName"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["ResolveTeam", "GetIssueProjectId"]
    );
    assert_eq!(sent[1]["variables"], json!({"id":"ENG-7"}));
}
#[tokio::test]
async fn m2_empty_slug_project_stops_create_before_milestone_lookup_but_uuid_bypasses() {
    use serde_json::json;
    let (backend, server) = network(vec![
        json!({"data":{"teams":{"nodes":[{"id":"team","key":"ENG","name":"Engineering"}]}}}),
        json!({"data":{"projects":{"nodes":[]}}}),
        json!({"data":{"projects":{"nodes":[{"id":""}]}}}),
    ]);
    let mut settings = settings();
    settings.assign_self = AssignSelf::Never;
    let error = create::flag_input(
        &backend,
        &mut Prompt::default(),
        &settings,
        &create::Fields {
            title: Some("X".into()),
            project: Some("X".into()),
            milestone: Some("Name".into()),
            ..Default::default()
        },
        None,
        false,
    )
    .await
    .err()
    .expect("expected prefix refusal");
    assert_eq!(
        error.message(),
        "Cannot resolve milestone \"Name\" without --project"
    );
    let sent = server.join().unwrap();
    assert_eq!(
        sent.iter()
            .map(|v| v["operationName"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["ResolveTeam", "GetProjectIdByName", "GetProjectIdBySlugId"]
    );
    let id = "00000000-0000-4000-8000-000000000001";
    assert_eq!(backend.milestone("".into(), id.into()).await.unwrap(), id);
}
#[tokio::test]
async fn m2_shared_read_empty_project_name_uses_slug_and_keeps_query_lf() {
    use serde_json::json;
    let (backend, server) = network(vec![
        json!({"data":{"projects":{"nodes":[{"id":""}]}}}),
        json!({"data":{"projects":{"nodes":[{"id":"slug-id"}]}}}),
    ]);
    assert_eq!(
        linear_cli::commands::issue::read::project_id(
            &backend.transport,
            &linear_cli::refs::ProjectReference::NameOrSlug("Name".into())
        )
        .await
        .unwrap(),
        Some("slug-id".into())
    );
    let sent = server.join().unwrap();
    assert_eq!(
        sent.iter()
            .map(|v| v["operationName"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["GetProjectIdByName", "GetProjectIdBySlugId"]
    );
    assert!(
        sent.iter()
            .all(|v| v["query"].as_str().unwrap().ends_with('\n'))
    );
}
#[test]
fn m2_empty_initial_project_keeps_project_menu_and_priority_glyphs() {
    use linear_cli::commands::issue::create_prompt as issue_create_prompt;
    for ask_project in [true, false] {
        let backend = Fake {
            empty_project: true,
            ..Default::default()
        };
        let mut settings = settings();
        settings.ask_project = ask_project;
        settings.assign_self = AssignSelf::Never;
        let mut ui = Prompt {
            answers: VecDeque::from(
                if ask_project {
                    vec!["X", "", "chosen-project", "submit", "no"]
                } else {
                    vec!["X", "", "more_fields", "2", "chosen-project", "no"]
                }
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>(),
            ),
            selected_fields: vec!["priority".into(), "project".into()],
            ..Default::default()
        };
        let output = issue_create_prompt::prompt(
            &backend,
            &mut ui,
            &settings,
            &create::Fields {
                project: Some("X".into()),
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            serde_json::to_value(output.input).unwrap()["projectId"],
            "chosen-project"
        );
        assert!(backend.calls().contains(&"Projects:ENG".to_string()));
        if !ask_project {
            let fields = ui
                .menus
                .iter()
                .find(|(name, _)| name == "Select additional fields to configure")
                .unwrap();
            assert!(fields.1.iter().any(|v| v.id == "project"));
            let priority = ui
                .menus
                .iter()
                .find(|(name, _)| name == "What priority should this issue have?")
                .unwrap();
            assert_eq!(
                priority
                    .1
                    .iter()
                    .map(|v| (v.id.as_str(), v.name.as_str()))
                    .collect::<Vec<_>>(),
                [
                    ("0", "--- No priority"),
                    ("1", "⚠⚠⚠ Urgent"),
                    ("2", "▄▆█ High"),
                    ("3", "▄▆  Medium"),
                    ("4", "▄   Low")
                ]
            );
        }
    }
}
