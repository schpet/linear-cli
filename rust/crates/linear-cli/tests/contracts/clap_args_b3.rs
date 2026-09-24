use std::collections::BTreeSet;

use clap::{Arg, ArgAction, ArgMatches, Command, builder::ValueRange, error::ErrorKind};
use linear_cli::cli::{ROUTES, RouteMeta, clap_tree, clap_tree::VariableAssignment};

fn command_for<'a>(root: &'a Command, route: &RouteMeta) -> &'a Command {
    route.path.split(' ').skip(1).fold(root, |parent, name| {
        parent
            .get_subcommands()
            .find(|child| child.get_name() == name)
            .expect("canonical route exists")
    })
}

fn selected(matches: &ArgMatches) -> &ArgMatches {
    matches
        .subcommand()
        .map_or(matches, |(_, child)| selected(child))
}

fn parse(args: &[String]) -> ArgMatches {
    clap_tree::build()
        .expect("generated tree builds")
        .try_get_matches_from(args)
        .expect("valid shadow argv")
}

fn parse_tokens(args: &[&str]) -> ArgMatches {
    parse(
        &args
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>(),
    )
}

fn route_at(path: &str) -> &'static RouteMeta {
    ROUTES
        .iter()
        .find(|route| route.path == path)
        .expect("route exists")
}

fn argv(route: &RouteMeta, suffix: &[&str]) -> Vec<String> {
    route
        .path
        .split(' ')
        .chain(suffix.iter().copied())
        .map(str::to_owned)
        .collect()
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

fn bulk_values(matches: &ArgMatches) -> Vec<String> {
    selected(matches)
        .get_many::<String>("opt:bulk")
        .expect("bulk values")
        .cloned()
        .collect()
}

#[test]
fn every_node_has_exactly_the_manifest_argument_ids_and_collected_shapes() {
    let mut tree = clap_tree::build().expect("generated tree builds");
    tree.build();
    tree.clone().debug_assert();
    let mut local_count = 0;
    let mut inherited_count = 0;
    let mut collected_count = 0;
    let mut variable_count = 0;
    let mut bulk_paths = BTreeSet::new();
    for route in ROUTES {
        let node = command_for(&tree, route);
        let mut expected = route
            .arguments
            .iter()
            .map(|argument| format!("pos:{}", argument.name))
            .collect::<BTreeSet<_>>();
        for option in route.local_options {
            local_count += 1;
            let id = if route.path == "linear" && option.name == "workspace" {
                "global:workspace".to_owned()
            } else {
                format!("opt:{}", option.name)
            };
            assert!(expected.insert(id.clone()), "{} {id}", route.path);
            let arg = node
                .get_arguments()
                .find(|arg| arg.get_id().as_str() == id)
                .expect("local descriptor registered");
            let expected_flags = if route.path == "linear label list" && option.name == "workspace"
            {
                ["--workspace-only"]
                    .into_iter()
                    .map(str::to_owned)
                    .collect()
            } else {
                option.flags.iter().map(|flag| (*flag).to_owned()).collect()
            };
            assert_eq!(spellings(arg), expected_flags, "{} {id}", route.path);
            assert_eq!(arg.is_hide_set(), option.hidden, "{} {id}", route.path);
            assert!(!arg.is_required_set(), "{} {id}", route.path);
            if matches!(arg.get_action(), ArgAction::SetTrue) {
                assert_eq!(
                    arg.get_default_values(),
                    ["false"],
                    "{} {id} clap's implicit switch default",
                    route.path
                );
            } else {
                assert!(arg.get_default_values().is_empty(), "{} {id}", route.path);
            }
            if option.name == "bulk" {
                assert!(bulk_paths.insert(route.path), "duplicate bulk route");
                assert!(
                    !option.collect,
                    "{} bulk unexpectedly collected",
                    route.path
                );
                assert!(
                    matches!(arg.get_action(), ArgAction::Set),
                    "{} {id}",
                    route.path
                );
                assert_eq!(
                    arg.get_num_args(),
                    Some(ValueRange::new(1..)),
                    "{} {id}",
                    route.path
                );
                assert!(!arg.is_allow_hyphen_values_set(), "{} {id}", route.path);
            } else if option.collect {
                collected_count += 1;
                assert!(
                    matches!(arg.get_action(), ArgAction::Append),
                    "{} {id}",
                    route.path
                );
                assert_eq!(
                    arg.get_num_args(),
                    Some(ValueRange::SINGLE),
                    "{} {id}",
                    route.path
                );
                assert!(!arg.is_allow_hyphen_values_set(), "{} {id}", route.path);
                let argument = option.args.first().expect("collected value descriptor");
                if argument.type_name == "variable" {
                    variable_count += 1;
                    assert_eq!((route.path, option.name), ("linear api", "variable"));
                } else {
                    assert_eq!(argument.type_name, "string", "{} {id}", route.path);
                }
            } else if option.args.is_empty() {
                assert!(
                    matches!(arg.get_action(), ArgAction::SetTrue),
                    "{} {id}",
                    route.path
                );
            } else {
                assert!(
                    matches!(arg.get_action(), ArgAction::Set),
                    "{} {id}",
                    route.path
                );
            }
            if let Some(argument) = option.args.first() {
                assert_eq!(
                    arg.get_value_names()
                        .expect("value names")
                        .first()
                        .map(|name| name.as_str()),
                    Some(argument.name),
                    "{} {id}",
                    route.path
                );
            }
        }
        inherited_count += route.inherited_global_options.len();
        if !route.inherited_global_options.is_empty() || route.path == "linear label list" {
            assert!(
                expected.insert("global:workspace".to_owned()),
                "{} global",
                route.path
            );
        }
        let actual = node
            .get_arguments()
            .filter(|arg| {
                !arg.get_id().as_str().starts_with("help:")
                    && !arg.get_id().as_str().starts_with("version:")
                    && !arg.get_id().as_str().starts_with("internal:")
            })
            .map(|arg| arg.get_id().as_str().to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual, expected, "{} full argument set", route.path);
        if route.path.starts_with("linear completions") {
            assert!(
                !actual.contains("global:workspace"),
                "{} workspace",
                route.path
            );
        }
    }
    assert_eq!(local_count, 336);
    assert_eq!(inherited_count, 103);
    assert_eq!(collected_count, 23);
    assert_eq!(variable_count, 1);
    assert_eq!(
        bulk_paths,
        BTreeSet::from([
            "linear issue archive",
            "linear issue delete",
            "linear initiative archive",
            "linear initiative delete",
            "linear document delete",
        ])
    );
}

#[test]
fn collected_options_preserve_repeat_order_aliases_and_absence() {
    let mine = parse_tokens(&[
        "linear", "issue", "mine", "--state", "Started", "-s", "Backlog", "--label", "one", "-l",
        "two",
    ]);
    let leaf = selected(&mine);
    assert_eq!(
        leaf.get_many::<String>("opt:state")
            .expect("state values")
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["Started", "Backlog"]
    );
    assert_eq!(
        leaf.get_many::<String>("opt:label")
            .expect("label values")
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["one", "two"]
    );
    let mine_alias = parse_tokens(&["linear", "issue", "l", "-s", "A", "-s", "B"]);
    assert_eq!(
        selected(&mine_alias)
            .get_many::<String>("opt:state")
            .expect("alias state values")
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
    let query = parse_tokens(&["linear", "issue", "q", "--team", "A", "--team", "B"]);
    assert_eq!(
        selected(&query)
            .get_many::<String>("opt:team")
            .expect("team values")
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
    let attachment = parse_tokens(&[
        "linear", "issue", "comment", "add", "ABC-1", "-a", "one.png", "--attach", "two.png",
    ]);
    assert_eq!(
        selected(&attachment)
            .get_many::<String>("opt:attach")
            .expect("attachment values")
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["one.png", "two.png"]
    );
    let project = parse_tokens(&[
        "linear", "project", "create", "--name", "P", "--lead", "USER", "-t", "A", "--team", "B",
    ]);
    assert_eq!(
        selected(&project)
            .get_many::<String>("opt:team")
            .expect("project team values")
            .map(String::as_str)
            .collect::<Vec<_>>(),
        ["A", "B"]
    );
    let no_state = parse_tokens(&["linear", "issue", "mine"]);
    assert!(
        selected(&no_state)
            .get_many::<String>("opt:state")
            .is_none()
    );
}

#[test]
fn every_collected_string_descriptor_rejects_empty_and_preserves_repeats() {
    let mut checked = 0;
    for route in ROUTES {
        let tree = clap_tree::build().expect("tree");
        let node = command_for(&tree, route);
        for option in route.local_options.iter().filter(|option| option.collect) {
            if option.name == "variable" {
                continue;
            }
            checked += 1;
            let id = format!("opt:{}", option.name);
            let arg = node
                .get_arguments()
                .find(|arg| arg.get_id().as_str() == id)
                .expect("collected argument");
            let flag = format!("--{}", arg.get_long().expect("collected long spelling"));
            let repeated = Command::new("value")
                .arg(arg.clone())
                .try_get_matches_from(["value", &flag, "A", &flag, "B"])
                .expect("repeated collected values");
            assert_eq!(
                repeated
                    .get_many::<String>(&id)
                    .expect("collected values")
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                ["A", "B"],
                "{} {}",
                route.path,
                option.name
            );
            let empty = Command::new("value")
                .arg(arg.clone())
                .try_get_matches_from(["value".to_owned(), format!("{flag}=")])
                .expect_err("empty collected value rejected");
            assert_eq!(
                empty.kind(),
                ErrorKind::ValueValidation,
                "{} {}",
                route.path,
                option.name
            );
        }
    }
    assert_eq!(checked, 22);
}

#[test]
fn variable_assignments_keep_typed_first_equals_split_and_repeat_order() {
    let cases = [
        ("key=", "key", ""),
        ("=value", "", "value"),
        ("key=a=b", "key", "a=b"),
    ];
    for (raw, key, value) in cases {
        let matches = parse_tokens(&[
            "linear",
            "api",
            "query { viewer { id } }",
            "--variable",
            raw,
        ]);
        let assignments = selected(&matches)
            .get_many::<VariableAssignment>("opt:variable")
            .expect("one variable")
            .collect::<Vec<_>>();
        assert_eq!(assignments.len(), 1);
        assert_eq!(
            assignments
                .first()
                .map(|assignment| assignment.key.as_str()),
            Some(key)
        );
        assert_eq!(
            assignments
                .first()
                .map(|assignment| assignment.value.as_str()),
            Some(value)
        );
    }
    let repeated = parse_tokens(&[
        "linear",
        "api",
        "query { viewer { id } }",
        "--variable",
        "key=",
        "--variable=other=a=b",
    ]);
    let assignments = selected(&repeated)
        .get_many::<VariableAssignment>("opt:variable")
        .expect("repeated variables")
        .cloned()
        .collect::<Vec<_>>();
    assert_eq!(
        assignments,
        [
            VariableAssignment {
                key: "key".to_owned(),
                value: String::new()
            },
            VariableAssignment {
                key: "other".to_owned(),
                value: "a=b".to_owned()
            },
        ]
    );
    let hyphen_key = parse_tokens(&["linear", "api", "--variable=-k=v"]);
    assert_eq!(
        selected(&hyphen_key)
            .get_many::<VariableAssignment>("opt:variable")
            .expect("hyphen key")
            .cloned()
            .collect::<Vec<_>>(),
        [VariableAssignment {
            key: "-k".to_owned(),
            value: "v".to_owned()
        }]
    );
    for suffix in [
        &["--variable", "key"][..],
        &["--variable", ""][..],
        &["--variable="][..],
    ] {
        let args = argv(route_at("linear api"), suffix);
        let error = clap_tree::build()
            .expect("tree")
            .try_get_matches_from(args)
            .expect_err("invalid variable rejected");
        assert_eq!(error.kind(), ErrorKind::ValueValidation, "{suffix:?}");
    }
    let empty_label = clap_tree::build()
        .expect("tree")
        .try_get_matches_from(["linear", "issue", "mine", "--label="])
        .expect_err("empty collected label rejected");
    assert_eq!(empty_label.kind(), ErrorKind::ValueValidation);
}

#[test]
fn all_bulk_routes_preserve_values_and_optional_positional_separately() {
    for path in [
        "linear issue archive",
        "linear issue delete",
        "linear initiative archive",
        "linear initiative delete",
        "linear document delete",
    ] {
        let route = route_at(path);
        let positional = format!("pos:{}", route.arguments.first().expect("optional ID").name);
        assert_eq!(route.arguments.len(), 1, "{path}");

        let spaced = parse(&argv(route, &["--bulk", "A", "B"]));
        assert_eq!(bulk_values(&spaced), ["A", "B"], "{path}");
        assert!(
            selected(&spaced).get_one::<String>(&positional).is_none(),
            "{path}"
        );

        let inline = parse(&argv(route, &["--bulk=A", "B"]));
        assert_eq!(bulk_values(&inline), ["A"], "{path}");
        assert_eq!(
            selected(&inline)
                .get_one::<String>(&positional)
                .map(String::as_str),
            Some("B"),
            "{path}"
        );

        let delimiter = parse(&argv(route, &["--bulk", "A", "--", "B"]));
        assert_eq!(bulk_values(&delimiter), ["A"], "{path}");
        assert_eq!(
            selected(&delimiter)
                .get_many::<String>("internal:literal")
                .expect("post-delimiter literal")
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["B"],
            "{path}"
        );

        let terminal_delimiter = parse(&argv(route, &["--bulk", "A", "B", "--"]));
        assert_eq!(bulk_values(&terminal_delimiter), ["A", "B"], "{path}");
        assert!(
            selected(&terminal_delimiter)
                .get_one::<String>(&positional)
                .is_none(),
            "{path}"
        );

        let hyphen_id = parse(&argv(route, &["--bulk=-x", "B"]));
        assert_eq!(bulk_values(&hyphen_id), ["-x"], "{path}");
        assert_eq!(
            selected(&hyphen_id)
                .get_one::<String>(&positional)
                .map(String::as_str),
            Some("B"),
            "{path}"
        );
    }
}

#[test]
fn all_bulk_routes_reject_empty_repeated_and_flag_looking_values() {
    for path in [
        "linear issue archive",
        "linear issue delete",
        "linear initiative archive",
        "linear initiative delete",
        "linear document delete",
    ] {
        let route = route_at(path);
        for (suffix, kind) in [
            (
                &["--bulk", "A", "B", "--json"][..],
                ErrorKind::UnknownArgument,
            ),
            (&["--bulk", "--json"][..], ErrorKind::UnknownArgument),
            (
                &["--bulk", "A", "--bulk", "B"][..],
                ErrorKind::ArgumentConflict,
            ),
            (&["--bulk", "", "B"][..], ErrorKind::ValueValidation),
            (&["--bulk", "A", ""][..], ErrorKind::ValueValidation),
            (&["--bulk="][..], ErrorKind::ValueValidation),
        ] {
            let error = clap_tree::build()
                .expect("tree")
                .try_get_matches_from(argv(route, suffix))
                .expect_err("invalid bulk rejected");
            assert_eq!(error.kind(), kind, "{path} {suffix:?}");
        }
        let help = parse(&argv(route, &["--bulk", "A", "--help"]));
        assert_eq!(selected(&help).get_count("help:long"), 1, "{path}");
    }
}

#[test]
fn all_bulk_routes_apply_known_switches_after_values() {
    for (path, switch) in [
        ("linear issue archive", "confirm"),
        ("linear issue delete", "confirm"),
        ("linear initiative archive", "force"),
        ("linear initiative delete", "force"),
        ("linear document delete", "yes"),
    ] {
        let route = route_at(path);
        let parsed = parse(&argv(route, &["--bulk", "A", "B", "-y"]));
        assert_eq!(bulk_values(&parsed), ["A", "B"], "{path}");
        assert_eq!(
            selected(&parsed).get_one::<bool>(&format!("opt:{switch}")),
            Some(&true),
            "{path}"
        );
    }
}

#[test]
fn bulk_routes_are_reachable_through_command_aliases() {
    let issue = parse_tokens(&["linear", "issue", "d", "--bulk", "A", "B"]);
    assert_eq!(bulk_values(&issue), ["A", "B"]);
    assert_eq!(
        clap_tree::selected_route(&issue).expect("issue alias route"),
        route_at("linear issue delete").route
    );
    let document = parse_tokens(&["linear", "document", "d", "--bulk", "A"]);
    assert_eq!(bulk_values(&document), ["A"]);
    assert_eq!(
        clap_tree::selected_route(&document).expect("document alias route"),
        route_at("linear document delete").route
    );
}
