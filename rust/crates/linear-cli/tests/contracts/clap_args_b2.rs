use std::collections::BTreeSet;

use clap::{Arg, ArgAction, ArgMatches, Command, error::ErrorKind};
use linear_cli::cli::{OptionMeta, ROUTES, RouteMeta, TypeHandler, clap_tree};

fn command_for<'a>(root: &'a Command, route: &RouteMeta) -> &'a Command {
    route.path.split(' ').skip(1).fold(root, |parent, name| {
        parent
            .get_subcommands()
            .find(|child| child.get_name() == name)
            .expect("canonical route exists")
    })
}

fn spellings(arg: &Arg) -> BTreeSet<String> {
    let mut flags = BTreeSet::new();
    if let Some(long) = arg.get_long() {
        flags.insert(format!("--{long}"));
    }
    if let Some(short) = arg.get_short() {
        flags.insert(format!("-{short}"));
    }
    for long in arg.get_all_aliases().into_iter().flatten() {
        flags.insert(format!("--{long}"));
    }
    for short in arg.get_all_short_aliases().into_iter().flatten() {
        flags.insert(format!("-{short}"));
    }
    flags
}

fn parse(args: &[&str]) -> ArgMatches {
    clap_tree::build()
        .expect("generated tree builds")
        .try_get_matches_from(args)
        .expect("valid shadow argv")
}

fn selected(matches: &ArgMatches) -> &ArgMatches {
    matches
        .subcommand()
        .map_or(matches, |(_, child)| selected(child))
}

fn argv_for_option(route: &RouteMeta, option: &OptionMeta) -> Vec<String> {
    let mut argv = route.path.split(' ').map(str::to_owned).collect::<Vec<_>>();
    argv.extend(
        route
            .arguments
            .iter()
            .filter(|argument| !argument.optional)
            .map(|_| "sample".to_owned()),
    );
    for required in route
        .local_options
        .iter()
        .filter(|required| required.required && required.name != option.name)
    {
        argv.push(required.flags.first().expect("required flag").to_string());
        argv.push("sample".to_owned());
    }
    argv
}

#[test]
fn ordinary_valued_options_match_every_manifest_descriptor() {
    let tree = clap_tree::build().expect("generated tree builds");
    tree.clone().debug_assert();
    let mut count = 0;
    let mut required = 0;
    let mut separated_hyphen_text = 0;
    let mut separated_hyphen_numbers = 0;
    for route in ROUTES {
        let node = command_for(&tree, route);
        let descriptors = route.local_options.iter().filter(|option| {
            if option.args.is_empty() || option.collect || option.name == "bulk" {
                return false;
            }
            route.path != "linear" || option.name != "workspace"
        });
        let mut expected_ids = BTreeSet::new();
        for option in descriptors {
            let id = format!("opt:{}", option.name);
            assert!(expected_ids.insert(id.clone()), "{} {id}", route.path);
            let arg = node
                .get_arguments()
                .find(|arg| arg.get_id().as_str() == id)
                .expect("ordinary option registered");
            assert!(
                matches!(arg.get_action(), ArgAction::Set),
                "{} {id}",
                route.path
            );
            assert_eq!(
                arg.is_required_set(),
                option.required,
                "{} {id}",
                route.path
            );
            assert_eq!(arg.is_hide_set(), option.hidden, "{} {id}", route.path);
            let value_type = option.args.first().expect("one value descriptor").type_name;
            if value_type == "string" {
                separated_hyphen_text += usize::from(arg.is_allow_hyphen_values_set());
            } else if value_type == "number" {
                separated_hyphen_numbers += usize::from(arg.is_allow_hyphen_values_set());
            } else {
                assert!(!arg.is_allow_hyphen_values_set(), "{} {id}", route.path);
            }
            assert_eq!(
                spellings(arg),
                option.flags.iter().map(|flag| (*flag).to_owned()).collect(),
                "{} {id}",
                route.path
            );
            let value_name = option.args.first().expect("one value descriptor").name;
            assert_eq!(
                arg.get_value_names()
                    .expect("value name")
                    .first()
                    .map(|name| name.as_str()),
                Some(value_name),
                "{} {id}",
                route.path
            );
            count += 1;
            required += usize::from(option.required);
        }
        let actual_ids = node
            .get_arguments()
            .filter(|arg| {
                arg.get_id().as_str().starts_with("opt:")
                    && matches!(arg.get_action(), ArgAction::Set)
                    && arg.get_id().as_str() != "opt:bulk"
            })
            .map(|arg| arg.get_id().as_str().to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(
            actual_ids, expected_ids,
            "{} ordinary valued options",
            route.path
        );
    }
    assert_eq!(count, 185);
    assert_eq!(required, 3);
    assert_eq!(separated_hyphen_text, 38);
    assert_eq!(separated_hyphen_numbers, 11);
}

#[test]
fn separated_hyphen_text_is_exactly_the_reviewed_descriptor_set() {
    let tree = clap_tree::build().expect("generated tree builds");
    let mut expected = BTreeSet::new();
    let mut actual = BTreeSet::new();
    for route in ROUTES {
        let node = command_for(&tree, route);
        for option in route
            .local_options
            .iter()
            .filter(|option| !option.args.is_empty())
        {
            let pair = (route.path, option.name);
            if !route.path.starts_with("linear completions ")
                && matches!(
                    option.name,
                    "body" | "comment" | "content" | "description" | "name" | "search" | "title"
                )
            {
                expected.insert(pair);
            }
            let id = format!("opt:{}", option.name);
            if node
                .get_arguments()
                .find(|arg| arg.get_id().as_str() == id)
                .is_some_and(Arg::is_allow_hyphen_values_set)
                && option
                    .args
                    .first()
                    .is_some_and(|argument| argument.type_name == "string")
            {
                actual.insert(pair);
            }
        }
    }
    assert_eq!(expected.len(), 38);
    assert_eq!(actual, expected);
}

#[test]
fn every_ordinary_descriptor_enforces_its_value_type() {
    let tree = clap_tree::build().expect("generated tree builds");
    let mut checked = 0;
    let mut numbers = 0;
    let mut enums = 0;
    for route in ROUTES {
        let node = command_for(&tree, route);
        for option in route.local_options.iter().filter(|option| {
            if option.args.is_empty() || option.collect || option.name == "bulk" {
                return false;
            }
            route.path != "linear" || option.name != "workspace"
        }) {
            let argument = option.args.first().expect("one ordinary value");
            let flag = option
                .flags
                .iter()
                .find(|flag| flag.starts_with("--"))
                .or_else(|| option.flags.first())
                .expect("ordinary option has a flag");
            let base = argv_for_option(route, option);
            let expected_empty_kind =
                if argument.type_name == "string" || argument.type_name == "number" {
                    ErrorKind::ValueValidation
                } else {
                    ErrorKind::InvalidValue
                };
            for value in [format!("{flag}="), (*flag).to_owned()] {
                let mut argv = base.clone();
                argv.push(value);
                if argv.last().is_some_and(|value| value == *flag) {
                    argv.push(String::new());
                }
                let error = tree
                    .clone()
                    .try_get_matches_from(argv)
                    .expect_err("empty ordinary value rejected");
                assert_eq!(
                    error.kind(),
                    expected_empty_kind,
                    "{} {}",
                    route.path,
                    option.name
                );
            }
            let arg = node
                .get_arguments()
                .find(|arg| arg.get_id().as_str() == format!("opt:{}", option.name))
                .expect("ordinary option registered");
            if argument.type_name == "number" {
                numbers += 1;
                let mut valid = base.clone();
                valid.extend([(*flag).to_owned(), "-1".to_owned()]);
                let matches = tree
                    .clone()
                    .try_get_matches_from(valid)
                    .expect("negative number accepted");
                assert_eq!(
                    selected(&matches)
                        .get_one::<String>(&format!("opt:{}", option.name))
                        .map(String::as_str),
                    Some("-1")
                );
                let mut invalid = base;
                invalid.extend([(*flag).to_owned(), "x".to_owned()]);
                let error = tree
                    .clone()
                    .try_get_matches_from(invalid)
                    .expect_err("malformed number rejected");
                assert_eq!(
                    error.kind(),
                    ErrorKind::ValueValidation,
                    "{} {}",
                    route.path,
                    option.name
                );
            } else if argument.type_name != "string" {
                enums += 1;
                let definition = route
                    .local_types
                    .iter()
                    .find(|definition| definition.name == argument.type_name)
                    .expect("enum type registered");
                let expected = match definition.handler {
                    TypeHandler::Enum(values) => values.to_vec(),
                    TypeHandler::Variable => panic!("ordinary variable option"),
                };
                let possible_values = arg.get_possible_values();
                let actual = possible_values
                    .iter()
                    .map(|value| value.get_name())
                    .collect::<Vec<_>>();
                assert_eq!(actual, expected, "{} {}", route.path, option.name);
            }
            checked += 1;
        }
    }
    assert_eq!(checked, 185);
    assert_eq!(numbers, 11);
    assert_eq!(enums, 4);
}

#[test]
fn required_options_and_long_aliases_are_parsed() {
    for argv in [
        &["linear", "milestone", "list", "--project", "PRJ"][..],
        &[
            "linear",
            "milestone",
            "create",
            "--project",
            "PRJ",
            "--name",
            "M1",
        ][..],
    ] {
        parse(argv);
    }
    for argv in [
        &["linear", "milestone", "list"][..],
        &["linear", "milestone", "create", "--project", "PRJ"][..],
        &["linear", "milestone", "create", "--name", "M1"][..],
    ] {
        let error = clap_tree::build()
            .expect("tree")
            .try_get_matches_from(argv)
            .expect_err("required option missing");
        assert_eq!(error.kind(), ErrorKind::MissingRequiredArgument, "{argv:?}");
    }
    let matches = parse(&[
        "linear",
        "issue",
        "comment",
        "add",
        "ABC-1",
        "--body",
        "hello",
        "--reply-to",
        "COMMENT",
    ]);
    assert_eq!(
        selected(&matches)
            .get_one::<String>("opt:parent")
            .map(String::as_str),
        Some("COMMENT")
    );
}

#[test]
fn ordinary_values_reject_duplicates_without_clobbering() {
    for argv in [
        &[
            "linear", "issue", "create", "--title", "one", "--title", "two",
        ][..],
        &[
            "linear",
            "issue",
            "comment",
            "add",
            "ABC-1",
            "--body",
            "hello",
            "--parent",
            "A",
            "--reply-to",
            "B",
        ][..],
    ] {
        let error = clap_tree::build()
            .expect("tree")
            .try_get_matches_from(argv)
            .expect_err("duplicate ordinary value rejected");
        assert_eq!(error.kind(), ErrorKind::ArgumentConflict, "{argv:?}");
    }
}

#[test]
fn enum_values_are_exact_and_case_sensitive() {
    let mine = parse(&["linear", "issue", "mine", "--sort", "priority"]);
    assert_eq!(
        selected(&mine)
            .get_one::<String>("opt:sort")
            .map(String::as_str),
        Some("priority")
    );
    let query = parse(&["linear", "issue", "query", "--sort", "manual"]);
    assert_eq!(
        selected(&query)
            .get_one::<String>("opt:sort")
            .map(String::as_str),
        Some("manual")
    );
    let session = parse(&[
        "linear",
        "issue",
        "agent-session",
        "list",
        "--status",
        "awaitingInput",
    ]);
    assert_eq!(
        selected(&session)
            .get_one::<String>("opt:status")
            .map(String::as_str),
        Some("awaitingInput")
    );
    let template = parse(&["linear", "template", "list", "--type", "document"]);
    assert_eq!(
        selected(&template)
            .get_one::<String>("opt:type")
            .map(String::as_str),
        Some("document")
    );
    for argv in [
        &["linear", "issue", "mine", "--sort", "Priority"][..],
        &["linear", "issue", "query", "--sort", "Manual"][..],
        &[
            "linear",
            "issue",
            "agent-session",
            "list",
            "--status",
            "awaitinginput",
        ][..],
        &["linear", "template", "list", "--type", "unknown"][..],
    ] {
        assert!(
            clap_tree::build()
                .expect("tree")
                .try_get_matches_from(argv)
                .is_err()
        );
    }
}

#[test]
fn finite_numbers_accept_negative_text_but_reject_nonfinite_or_malformed() {
    for (input, expected) in [("-1", "-1"), ("-2.5", "-2.5"), ("1e3", "1e3")] {
        let matches = parse(&["linear", "milestone", "update", "M1", "--sort-order", input]);
        assert_eq!(
            selected(&matches)
                .get_one::<String>("opt:sort-order")
                .map(String::as_str),
            Some(expected)
        );
    }
    for input in ["NaN", "inf", "-inf", "not-a-number"] {
        assert!(
            clap_tree::build()
                .expect("tree")
                .try_get_matches_from([
                    "linear",
                    "milestone",
                    "update",
                    "M1",
                    "--sort-order",
                    input
                ])
                .is_err(),
            "{input}"
        );
    }
    for input in ["-.5", "-1e+2"] {
        let inline = format!("--sort-order={input}");
        let matches = parse(&["linear", "milestone", "update", "M1", &inline]);
        assert_eq!(
            selected(&matches)
                .get_one::<String>("opt:sort-order")
                .map(String::as_str),
            Some(input)
        );
        let separated = parse(&["linear", "milestone", "update", "M1", "--sort-order", input]);
        assert_eq!(
            selected(&separated)
                .get_one::<String>("opt:sort-order")
                .map(String::as_str),
            Some(input)
        );
    }
}

#[test]
fn free_text_and_identifier_hyphen_rules_are_explicit() {
    for (option, value) in [("--title", "-foo"), ("--description", "- item")] {
        let matches = parse(&["linear", "issue", "create", option, value]);
        let id = format!("opt:{}", option.trim_start_matches("--"));
        assert_eq!(
            selected(&matches)
                .get_one::<String>(&id)
                .map(String::as_str),
            Some(value)
        );
    }
    assert!(
        clap_tree::build()
            .expect("tree")
            .try_get_matches_from(["linear", "issue", "create", "--team", "-foo"])
            .is_err()
    );
    let identifier = parse(&["linear", "issue", "create", "--team=-foo"]);
    assert_eq!(
        selected(&identifier)
            .get_one::<String>("opt:team")
            .map(String::as_str),
        Some("-foo")
    );
    let literal_option = parse(&["linear", "issue", "create", "--title=--json"]);
    assert_eq!(
        selected(&literal_option)
            .get_one::<String>("opt:title")
            .map(String::as_str),
        Some("--json")
    );
    assert!(
        clap_tree::build()
            .expect("tree")
            .try_get_matches_from(["linear", "completions", "bash", "--name", "-foo"])
            .is_err()
    );
    let command_name = parse(&["linear", "completions", "bash", "--name=-foo"]);
    assert_eq!(
        selected(&command_name)
            .get_one::<String>("opt:name")
            .map(String::as_str),
        Some("-foo")
    );
}

#[test]
fn short_hyphen_values_and_hidden_valued_options_are_accepted() {
    let priority = parse(&["linear", "issue", "create", "-p", "-1"]);
    assert_eq!(
        selected(&priority)
            .get_one::<String>("opt:priority")
            .map(String::as_str),
        Some("-1")
    );
    let title = parse(&["linear", "issue", "create", "-t", "-foo"]);
    assert_eq!(
        selected(&title)
            .get_one::<String>("opt:title")
            .map(String::as_str),
        Some("-foo")
    );
    let assignee = parse(&["linear", "issue", "mine", "--assignee", "USER"]);
    assert_eq!(
        selected(&assignee)
            .get_one::<String>("opt:assignee")
            .map(String::as_str),
        Some("USER")
    );
    let comment = parse(&["linear", "issue", "comment", "add", "ABC-1", "--id", "UUID"]);
    assert_eq!(
        selected(&comment)
            .get_one::<String>("opt:id")
            .map(String::as_str),
        Some("UUID")
    );
}

#[test]
fn empty_valued_options_are_usage_errors() {
    for argv in [
        &["linear", "issue", "create", "--title="][..],
        &["linear", "issue", "create", "--team="][..],
        &["linear", "issue", "mine", "--sort="][..],
        &["linear", "milestone", "update", "M1", "--sort-order="][..],
        &["linear", "--workspace=", "issue", "mine"][..],
    ] {
        assert!(
            clap_tree::build()
                .expect("tree")
                .try_get_matches_from(argv)
                .is_err(),
            "{argv:?}"
        );
    }
}

#[test]
fn help_and_workspace_hyphen_policy_are_explicit() {
    // Clap treats a pending value as taking precedence over built-in flags.
    // R01C must decide whether to preserve this behavior at runtime.
    for (value, expected) in [("--help", "--help"), ("--json", "--json"), ("--", "--")] {
        let matches = parse(&["linear", "issue", "create", "--title", value]);
        assert_eq!(
            selected(&matches)
                .get_one::<String>("opt:title")
                .map(String::as_str),
            Some(expected)
        );
    }
    for argv in [
        &[
            "linear",
            "milestone",
            "update",
            "M1",
            "--sort-order",
            "--help",
        ][..],
        &["linear", "milestone", "update", "M1", "--sort-order", "--"][..],
    ] {
        let error = clap_tree::build()
            .expect("tree")
            .try_get_matches_from(argv)
            .expect_err("pending numeric value rejected");
        assert_eq!(error.kind(), ErrorKind::ValueValidation, "{argv:?}");
    }
    for argv in [
        &["linear", "issue", "create", "--team", "--help"][..],
        &["linear", "issue", "create", "--team", "--"][..],
        &["linear", "issue", "mine", "--sort", "--help"][..],
        &["linear", "issue", "mine", "--sort", "--"][..],
    ] {
        let error = clap_tree::build()
            .expect("tree")
            .try_get_matches_from(argv)
            .expect_err("pending value rejected");
        assert_eq!(error.kind(), ErrorKind::InvalidValue, "{argv:?}");
    }
    let help_after = clap_tree::build()
        .expect("tree")
        .try_get_matches_from(["linear", "issue", "create", "--title", "-foo", "--help"])
        .expect_err("help requested after title");
    assert_eq!(help_after.kind(), ErrorKind::DisplayHelp);
    assert!(
        clap_tree::build()
            .expect("tree")
            .try_get_matches_from(["linear", "--workspace", "-foo", "issue", "mine"])
            .is_err()
    );
    let workspace = parse(&["linear", "--workspace=-foo", "issue", "mine"]);
    assert_eq!(
        clap_tree::selected_workspace(&workspace).expect("workspace"),
        Some("-foo".to_owned())
    );
    let pending_workspace = clap_tree::build()
        .expect("tree")
        .try_get_matches_from(["linear", "--workspace", "--help"])
        .expect_err("workspace value cannot be a flag");
    assert_eq!(pending_workspace.kind(), ErrorKind::InvalidValue);
}
