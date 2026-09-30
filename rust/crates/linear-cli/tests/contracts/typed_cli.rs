use clap::{ArgAction, Command};
use linear_cli::cli::{self, RootCommand};
use serde_json::Value;
fn parse(words: &[&str]) -> cli::Cli {
    cli::parse(&words.iter().map(Into::into).collect::<Vec<_>>()).expect("typed parse")
}
fn selected<'a>(root: &'a Command, path: &str) -> &'a Command {
    let mut command = root;
    for word in path.split_whitespace().skip(1) {
        command = command
            .find_subcommand(word)
            .expect("source route declared");
    }
    command
}
#[test]
fn derived_grammar_conforms_to_all_source_capabilities() {
    let source: Value =
        serde_json::from_str(include_str!("../../../../parity/manifest.json")).unwrap();
    let routes = source["routes"].as_array().unwrap();
    assert_eq!(routes.len(), 110);
    let mut command = cli::command();
    command.build();
    command.clone().debug_assert();
    let (mut options, mut repeated, mut negative, mut bulk) = (0, 0, 0, 0);
    for route in routes {
        let path = route["path"].as_str().unwrap();
        let native = selected(&command, path);
        assert_eq!(
            native.is_hide_set(),
            route["hidden"].as_bool().unwrap(),
            "{path}"
        );
        let aliases = native.get_all_aliases().collect::<Vec<_>>();
        for alias in route["aliases"].as_array().unwrap() {
            assert!(aliases.contains(&alias.as_str().unwrap()), "{path} {alias}");
        }
        assert!(
            native
                .get_arguments()
                .any(|arg| arg.get_long() == Some("workspace") && arg.is_global_set()),
            "{path} workspace"
        );
        let mut argv = path
            .split_whitespace()
            .map(str::to_owned)
            .collect::<Vec<_>>();
        for positional in route["arguments"].as_array().unwrap() {
            let name = positional["name"].as_str().unwrap();
            let arg = native
                .get_positionals()
                .find(|arg| {
                    arg.get_value_names()
                        .is_some_and(|names| names.iter().any(|n| n.as_str() == name))
                })
                .unwrap();
            assert_eq!(
                arg.is_required_set(),
                !positional["optional"].as_bool().unwrap(),
                "{path} {name}"
            );
            if arg.is_required_set() {
                argv.push("value".to_owned());
            }
        }
        for option in route["localOptions"].as_array().unwrap() {
            options += 1;
            let name = option["name"].as_str().unwrap();
            let flags = option["flags"].as_array().unwrap();
            let long = flags
                .iter()
                .filter_map(Value::as_str)
                .find_map(|flag| flag.strip_prefix("--"))
                .unwrap();
            let long = if path == "linear label list" && name == "workspace" {
                "workspace-only"
            } else {
                long
            };
            let arg = native
                .get_arguments()
                .find(|arg| arg.get_long() == Some(long))
                .unwrap_or_else(|| panic!("{path} {long}"));
            assert_eq!(
                arg.is_hide_set(),
                option["hidden"].as_bool().unwrap(),
                "{path} {long}"
            );
            for flag in flags
                .iter()
                .filter_map(Value::as_str)
                .filter(|f| !f.starts_with("--"))
            {
                assert!(
                    arg.get_short_and_visible_aliases()
                        .unwrap()
                        .contains(&flag.chars().nth(1).unwrap()),
                    "{path} {flag}"
                );
            }
            if option["collect"].as_bool().unwrap() {
                repeated += 1;
                assert!(
                    matches!(arg.get_action(), ArgAction::Append),
                    "{path} {name}"
                );
            }
            if name.starts_with("no-") {
                negative += 1;
                assert!(matches!(arg.get_action(), ArgAction::SetTrue));
            }
            if name == "bulk" {
                bulk += 1;
                assert_eq!(arg.get_num_args().unwrap().min_values(), 0);
            }
            if option["required"] == true {
                argv.push(format!("--{long}"));
                argv.push("value".to_owned());
            }
            let type_name = option["args"]
                .as_array()
                .unwrap()
                .first()
                .and_then(|arg| arg["type"].as_str());
            if let Some(definition) =
                route["localTypes"].as_array().unwrap().iter().find(|def| {
                    def["name"].as_str() == type_name && def["handlerKind"] == "EnumType"
                })
            {
                let actual = arg
                    .get_value_parser()
                    .possible_values()
                    .unwrap()
                    .map(|v| v.get_name().to_owned())
                    .collect::<Vec<_>>();
                let expected = definition["values"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|v| v.as_str().unwrap().to_owned())
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected, "{path} {name}");
            }
            if let Some(default) = option.get("default").filter(|v| !v.is_null()) {
                let expected = match default {
                    Value::Array(values) => values
                        .iter()
                        .map(|v| v.as_str().unwrap().to_owned())
                        .collect(),
                    Value::Number(number) => vec![number.to_string()],
                    _ => panic!("unexpected source default"),
                };
                assert_eq!(
                    arg.get_default_values()
                        .iter()
                        .map(|v| v.to_str().unwrap().to_owned())
                        .collect::<Vec<_>>(),
                    expected,
                    "{path} {name}"
                );
            }
        }
        cli::parse(&argv.iter().skip(1).map(Into::into).collect::<Vec<_>>())
            .unwrap_or_else(|error| panic!("{path}: {error}"));
    }
    assert_eq!((options, repeated, negative, bulk), (336, 23, 11, 5));
}
#[test]
fn supplied_mutation_values_and_negative_switches_keep_typed_intent() {
    let Some(RootCommand::Milestone(group)) = parse(&["milestone", "update", "M"]).command else {
        panic!()
    };
    let Some(cli::milestone::MilestoneCommand::Update(args)) = group.command else {
        panic!()
    };
    assert!(args.name.is_none() && args.description.is_none() && args.sort_order.is_none());
    let Some(RootCommand::Milestone(group)) = parse(&[
        "milestone",
        "update",
        "M",
        "--name",
        "New",
        "--sort-order",
        "-0.25",
    ])
    .command
    else {
        panic!()
    };
    let Some(cli::milestone::MilestoneCommand::Update(args)) = group.command else {
        panic!()
    };
    assert_eq!(args.name.as_deref(), Some("New"));
    assert_eq!(args.sort_order, Some(-0.25));
    for (flag, expected) in [(None, false), (Some("--no-pager"), true)] {
        let mut words = vec!["project", "view", "P"];
        if let Some(flag) = flag {
            words.push(flag);
        }
        let Some(RootCommand::Project(group)) = parse(&words).command else {
            panic!()
        };
        let Some(cli::project::ProjectCommand::View(args)) = group.command else {
            panic!()
        };
        assert_eq!(args.no_pager, expected);
    }
    let Some(RootCommand::Issue(group)) =
        parse(&["issue", "pull-request", "--template", "T", "--no-template"]).command
    else {
        panic!()
    };
    let Some(cli::issue::IssueCommand::PullRequest(args)) = group.command else {
        panic!()
    };
    assert_eq!(args.template.as_deref(), Some("T"));
    assert!(args.no_template);
}
#[test]
fn all_five_bulk_flags_distinguish_absent_bare_and_values() {
    for route in [
        ["issue", "archive"],
        ["issue", "delete"],
        ["initiative", "archive"],
        ["initiative", "delete"],
        ["document", "delete"],
    ] {
        for (suffix, expected) in [
            (vec![], None),
            (vec!["--bulk"], Some(vec![])),
            (
                vec!["--bulk", "A", "B"],
                Some(vec!["A".to_owned(), "B".to_owned()]),
            ),
        ] {
            let words = route.into_iter().chain(suffix).collect::<Vec<_>>();
            let bulk = match parse(&words).command.unwrap() {
                RootCommand::Issue(group) => match group.command.unwrap() {
                    cli::issue::IssueCommand::Archive(args) => args.bulk,
                    cli::issue::IssueCommand::Delete(args) => args.bulk,
                    _ => panic!(),
                },
                RootCommand::Initiative(group) => match group.command.unwrap() {
                    cli::initiative::InitiativeCommand::Archive(args) => args.bulk,
                    cli::initiative::InitiativeCommand::Delete(args) => args.bulk,
                    _ => panic!(),
                },
                RootCommand::Document(group) => match group.command.unwrap() {
                    cli::document::DocumentCommand::Delete(args) => args.bulk,
                    _ => panic!(),
                },
                _ => panic!(),
            };
            assert_eq!(bulk, expected, "{words:?}");
        }
    }
}
#[test]
fn native_global_workspace_and_positional_ids_are_distinct() {
    for words in [
        vec!["--workspace", "acme", "auth", "logout", "other"],
        vec!["auth", "logout", "other", "--workspace", "acme"],
    ] {
        let cli = parse(&words);
        assert_eq!(cli.workspace.as_deref(), Some("acme"));
        let Some(RootCommand::Auth(group)) = cli.command else {
            panic!()
        };
        let Some(cli::auth::AuthCommand::Logout(args)) = group.command else {
            panic!()
        };
        assert_eq!(args.workspace_name.as_deref(), Some("other"));
    }
    let cli = parse(&["label", "list", "--workspace", "acme", "--workspace-only"]);
    assert_eq!(cli.workspace.as_deref(), Some("acme"));
    let Some(RootCommand::Label(group)) = cli.command else {
        panic!()
    };
    let Some(cli::label::LabelCommand::List(args)) = group.command else {
        panic!()
    };
    assert!(args.workspace_only);
}

#[test]
fn collected_values_replace_defaults_and_retain_ordered_typed_payloads() {
    let Some(RootCommand::Issue(group)) = parse(&["issue", "mine"]).command else {
        panic!()
    };
    let Some(cli::issue::IssueCommand::Mine(args)) = group.command else {
        panic!()
    };
    assert_eq!(args.state, ["unstarted"]);
    assert_eq!(args.limit, 50.0);
    assert!(args.sort.is_none() && args.label.is_empty() && !args.no_pager);
    let Some(RootCommand::Issue(group)) = parse(&[
        "issue",
        "mine",
        "--state",
        "started",
        "--state",
        "completed",
        "--label",
        "first",
        "--label",
        "second",
    ])
    .command
    else {
        panic!()
    };
    let Some(cli::issue::IssueCommand::Mine(args)) = group.command else {
        panic!()
    };
    assert_eq!(args.state, ["started", "completed"]);
    assert_eq!(args.label, ["first", "second"]);
    let Some(RootCommand::Api(args)) = parse(&[
        "api",
        "--variable",
        "key=a=b",
        "--variable",
        "key=",
        "--variable",
        "=value",
    ])
    .command
    else {
        panic!()
    };
    assert_eq!(
        args.variable
            .iter()
            .map(|v| (v.key.as_str(), v.value.as_str()))
            .collect::<Vec<_>>(),
        [("key", "a=b"), ("key", ""), ("", "value")]
    );
}

#[test]
fn empty_default_references_are_preserved_for_business_resolution() {
    let Some(RootCommand::Team(group)) = parse(&["team", "states", ""]).command else {
        panic!()
    };
    let Some(cli::team::TeamCommand::States(args)) = group.command else {
        panic!()
    };
    assert_eq!(args.team.as_deref(), Some(""));
    let Some(RootCommand::Cycle(group)) = parse(&["cycle", "view", ""]).command else {
        panic!()
    };
    let Some(cli::cycle::CycleCommand::View(args)) = group.command else {
        panic!()
    };
    assert_eq!(args.cycle_ref, "");
    assert!(
        cli::parse(
            &["initiative", "view", ""]
                .iter()
                .map(Into::into)
                .collect::<Vec<_>>()
        )
        .is_err()
    );
}
