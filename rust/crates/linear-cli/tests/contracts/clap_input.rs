use linear_cli::cli::ROUTES;
use linear_cli::cli::clap_input::{self, CollectedValue, Invocation, OptionValue, ValueOrigin};
use linear_cli::error::AppErrorKind;
use std::ffi::OsString;

fn parse(args: &[&str]) -> Result<Invocation, linear_cli::error::AppError> {
    let args = args.iter().map(OsString::from).collect::<Vec<_>>();
    clap_input::parse(&args)
}

#[test]
fn help_rows_keep_inherited_before_local_order_on_every_route() {
    let root_workspace = ROUTES
        .iter()
        .find(|route| route.path == "linear")
        .and_then(|route| {
            route
                .local_options
                .iter()
                .find(|option| option.name == "workspace" && option.global)
        })
        .expect("root workspace selector");
    assert_eq!(ROUTES.len(), 110);
    for route in ROUTES {
        let actual = linear_cli::cli::spelling::effective_help_options(route)
            .into_iter()
            .map(|(option, _)| (option.name, option.global))
            .collect::<Vec<_>>();
        let mut expected = if route.path == "linear label list" {
            vec![(root_workspace.name, root_workspace.global)]
        } else {
            route
                .inherited_global_options
                .iter()
                .map(|option| (option.name, option.global))
                .collect::<Vec<_>>()
        };
        expected.extend(
            route
                .local_options
                .iter()
                .map(|option| (option.name, option.global)),
        );
        assert_eq!(actual, expected, "{} help row order", route.path);
    }
}

fn assert_native_error(args: &[&str]) -> linear_cli::error::AppError {
    let words = std::iter::once("linear")
        .chain(args.iter().copied())
        .map(OsString::from)
        .collect::<Vec<_>>();
    let expected = linear_cli::cli::clap_tree::build()
        .unwrap()
        .try_get_matches_from(words)
        .unwrap_err();
    let actual = parse(args).expect_err("native parser failure");
    let native = actual.native_parser_error().expect("clap error retained");
    assert_eq!(native.kind(), expected.kind());
    assert_eq!(native.render().to_string(), expected.render().to_string());
    assert_eq!(native.use_stderr(), expected.use_stderr());
    assert_eq!(native.exit_code(), expected.exit_code());
    actual
}

fn action(args: &[&str]) -> clap_input::ParsedAction {
    match parse(args).expect("typed clap parse") {
        Invocation::Action(action) => action,
        _ => panic!("expected action"),
    }
}

#[test]
fn aliases_options_and_defaults_are_typed_with_origin() {
    let mine = action(&["issue", "l", "--state", "started", "--state", "completed"]);
    assert_eq!(mine.route.path, "linear issue mine");
    let state = mine.option("state").expect("state");
    assert_eq!(state.value.origin, ValueOrigin::Explicit);
    assert_eq!(
        state.value.value,
        OptionValue::Collected(vec![
            CollectedValue::String("started".to_owned()),
            CollectedValue::String("completed".to_owned())
        ])
    );
    assert_eq!(
        mine.option("limit").expect("limit").value.value,
        OptionValue::Number(50.0)
    );
    assert_eq!(
        mine.option("limit").expect("limit").value.origin,
        ValueOrigin::Default
    );
    let raw = action(&["api", "--variable", "key=a=b"]);
    assert_eq!(
        raw.option("variable").expect("variable").value.value,
        OptionValue::Collected(vec![CollectedValue::Variable {
            key: "key".to_owned(),
            value: "a=b".to_owned()
        }])
    );
}

#[test]
fn all_six_metadata_defaults_are_typed_and_sourced() {
    let expected = [
        (
            &["issue", "mine"][..],
            "state",
            OptionValue::Collected(vec![CollectedValue::String("unstarted".to_owned())]),
        ),
        (&["issue", "mine"][..], "limit", OptionValue::Number(50.0)),
        (&["issue", "query"][..], "limit", OptionValue::Number(50.0)),
        (
            &["project-update", "list", "PRJ"][..],
            "limit",
            OptionValue::PositiveInteger(std::num::NonZeroU32::new(10).unwrap()),
        ),
        (
            &["initiative-update", "list", "INI"][..],
            "limit",
            OptionValue::PositiveInteger(std::num::NonZeroU32::new(10).unwrap()),
        ),
        (
            &["document", "list"][..],
            "limit",
            OptionValue::PositiveInteger(std::num::NonZeroU32::new(50).unwrap()),
        ),
    ];
    for (argv, name, value) in expected {
        let parsed = action(argv);
        let actual = parsed.option(name).expect("metadata default");
        assert_eq!(actual.value.origin, ValueOrigin::Default, "{argv:?} {name}");
        assert_eq!(actual.value.value, value, "{argv:?} {name}");
    }
}

#[test]
fn milestone_sort_order_uses_strict_finite_decimal_boundary() {
    for (input, expected) in [
        ("+5", 5.0),
        (".5", 0.5),
        ("1e3", 1000.0),
        ("-1", -1.0),
        ("-0", 0.0),
    ] {
        let parsed = action(&["milestone", "update", "M1", "--sort-order", input]);
        let option = parsed.option("sort-order").expect("sort-order");
        assert_eq!(option.value.origin, ValueOrigin::Explicit);
        assert_eq!(option.value.value, OptionValue::Number(expected));
    }
    for input in [
        "",
        "NaN",
        "Infinity",
        "-Infinity",
        "0x",
        "1e999",
        "0x10",
        "0b1",
        "0o7",
        " 5 ",
        " ",
        "\u{00a0}5",
        "5\u{00a0}",
        "5 ",
        " 5",
    ] {
        let error = parse(&["milestone", "update", "M1", "--sort-order", input])
            .expect_err("malformed or nonfinite number");
        assert!(
            matches!(error.kind, AppErrorKind::Usage { .. }),
            "{input:?}"
        );
    }
    // The reviewed source-number gate must not widen unrelated flags.
    for input in ["0x10", "0b1", " 5 "] {
        assert!(parse(&["issue", "query", "--limit", input]).is_err());
    }
}

#[test]
fn workspace_spelling_and_global_source_stay_distinct() {
    let result = action(&["--workspace", "acme", "label", "list", "--workspace-only"]);
    assert_eq!(
        result.global_workspace.as_ref().expect("workspace").value,
        "acme"
    );
    assert_eq!(
        result.option("workspace").expect("filter").value.value,
        OptionValue::Switch(true)
    );
    let error = parse(&["label", "list", "--workspace"]).expect_err("value required");
    assert!(matches!(error.kind, AppErrorKind::Usage { .. }));
    assert_native_error(&["label", "list", "--workspace"]);
    let duplicate = parse(&["--workspace", "a", "issue", "mine", "--workspace", "b"])
        .expect_err("repeated credential selector");
    assert_eq!(
        duplicate.message,
        "Option \"--workspace\" can only occur once, but was found several times."
    );
    let flag_value = parse(&["--workspace", "--help"]).expect_err("flag-looking slug");
    assert!(matches!(flag_value.kind, AppErrorKind::Usage { .. }));
}

#[test]
fn literal_is_separate_and_missing_filepath_is_usage() {
    let root = action(&["--", "--help"]);
    assert_eq!(root.literal, ["--help"]);
    let error = parse(&["issue", "attach", "ABC-1", "--", "/tmp/x"]).expect_err("filepath missing");
    assert!(matches!(error.kind, AppErrorKind::Usage { .. }));
    assert!(error.message.contains("filepath"));
}

#[test]
fn every_negated_switch_exposes_positive_boolean_and_origin() {
    let mut count = 0;
    for route in ROUTES {
        for option in route
            .local_options
            .iter()
            .filter(|option| option.name.starts_with("no-"))
        {
            count += 1;
            let mut args = route.path.split(' ').skip(1).collect::<Vec<_>>();
            args.extend(std::iter::repeat_n(
                "X",
                route
                    .arguments
                    .iter()
                    .filter(|argument| !argument.optional)
                    .count(),
            ));
            let absent = action(&args);
            let value = absent.option(option.name).expect("negated default");
            assert_eq!(value.value.value, OptionValue::Switch(true));
            assert_eq!(value.value.origin, ValueOrigin::Default);
            args.push(
                option
                    .flags
                    .first()
                    .copied()
                    .expect("negated switch spelling"),
            );
            let present = action(&args);
            let value = present.option(option.name).expect("negated explicit");
            assert_eq!(value.value.value, OptionValue::Switch(false));
            assert_eq!(value.value.origin, ValueOrigin::Explicit);
        }
    }
    assert_eq!(count, 11);
}

#[test]
fn help_and_version_spelling_are_typed() {
    for args in [&["-h", "-V"][..], &["--help", "-V"]] {
        assert!(matches!(
            parse(args).expect("short version wins"),
            Invocation::Version { long: false }
        ));
    }
    assert!(matches!(
        parse(&["-h", "--version"]).expect("long version follows help"),
        Invocation::Help { long: false, .. }
    ));
    assert!(matches!(
        parse(&["issue", "--help"]).expect("help"),
        Invocation::Help { long: true, .. }
    ));
    assert!(matches!(
        parse(&["issue", "-h"]).expect("help"),
        Invocation::Help { long: false, .. }
    ));
    assert!(matches!(
        parse(&["-V"]).expect("version"),
        Invocation::Version { long: false }
    ));
    assert!(matches!(
        parse(&["--version"]).expect("version"),
        Invocation::Version { long: true }
    ));
}

#[test]
fn proposed_help_rows_and_clap_spellings_share_one_adapter() {
    use linear_cli::cli::{clap_tree, spelling};
    let tree = clap_tree::build().expect("tree");
    let mut aliases = 0;
    let mut inherited = 0;
    for route in ROUTES {
        let node = route.path.split(' ').skip(1).fold(&tree, |parent, name| {
            parent
                .get_subcommands()
                .find(|child| child.get_name() == name)
                .expect("child")
        });
        aliases += route.aliases.len();
        let rows = spelling::effective_help_options(route);
        for (option, flags) in rows {
            let id = if option.global {
                "global:workspace".to_owned()
            } else {
                format!("opt:{}", option.name)
            };
            let arg = node
                .get_arguments()
                .find(|arg| arg.get_id().as_str() == id)
                .expect("row registered");
            let mut accepted = Vec::new();
            if let Some(long) = arg.get_long() {
                accepted.push(format!("--{long}"));
            }
            if let Some(short) = arg.get_short() {
                accepted.push(format!("-{short}"));
            }
            accepted.extend(
                arg.get_all_aliases()
                    .into_iter()
                    .flatten()
                    .map(|alias| format!("--{alias}")),
            );
            accepted.extend(
                arg.get_all_short_aliases()
                    .into_iter()
                    .flatten()
                    .map(|alias| format!("-{alias}")),
            );
            accepted.sort();
            let mut expected = flags
                .iter()
                .map(|flag| (*flag).to_owned())
                .collect::<Vec<_>>();
            expected.sort();
            assert_eq!(accepted, expected, "{} {}", route.path, option.name);
            inherited += usize::from(option.global && route.path != "linear");
        }
    }
    assert_eq!(aliases, 36);
    assert_eq!(inherited, 104); // 103 manifest inherited rows and label-list override.
}

#[test]
fn every_route_and_alias_has_typed_help_identity() {
    let mut aliases = 0;
    for route in ROUTES {
        let mut path = route.path.split(' ').skip(1).collect::<Vec<_>>();
        path.push("--help");
        match parse(&path).expect("route help parses") {
            Invocation::Help {
                route: selected,
                long: true,
            } => assert_eq!(selected.path, route.path),
            other => panic!("{} selected {other:?}", route.path),
        }
        path.pop();
        for alias in route.aliases {
            aliases += 1;
            let mut aliased = path.clone();
            *aliased.last_mut().expect("alias has final segment") = alias;
            aliased.push("--help");
            match parse(&aliased).expect("alias help parses") {
                Invocation::Help {
                    route: selected,
                    long: true,
                } => assert_eq!(selected.path, route.path),
                other => panic!("{} alias {alias} selected {other:?}", route.path),
            }
        }
    }
    assert_eq!(aliases, 36);
}

#[test]
fn required_options_and_parent_surplus_are_semantic_usage_errors() {
    for argv in [
        &["milestone", "list"][..],
        &["milestone", "create", "--project", "PRJ"][..],
        &["milestone", "create", "--name", "M1"][..],
    ] {
        let error = parse(argv).expect_err("required option");
        assert!(matches!(error.kind, AppErrorKind::Usage { .. }), "{argv:?}");
        assert!(
            error.message.contains("Missing required option"),
            "{argv:?}: {}",
            error.message
        );
    }
    let error = parse(&["issue", "foo", "--workspace=x", "list"]).expect_err("parent surplus");
    assert!(matches!(error.kind, AppErrorKind::Usage { .. }));
    assert!(
        error.message.contains("Unknown command \"foo\""),
        "{}",
        error.message
    );
}

#[test]
fn bulk_help_and_literal_boundaries_are_explicit() {
    let help = parse(&["issue", "archive", "--bulk", "A", "--help"]).expect_err("combined help");
    assert!(matches!(help.kind, AppErrorKind::Usage { .. }));
    let bulk = action(&["issue", "archive", "--bulk", "A", "--", "B"]);
    assert_eq!(
        bulk.option("bulk").expect("bulk").value.value,
        OptionValue::Bulk(vec!["A".to_owned()])
    );
    assert_eq!(bulk.literal, ["B"]);
    assert!(bulk.positionals.is_empty());
}

#[test]
fn native_value_diagnostics_and_nested_error_routes() {
    for args in [
        vec!["issue", "create", "--title="],
        vec!["issue", "create", "--description-file="],
        vec!["api", "--variable", "badformat"],
        vec!["issue", "create", "--title", "x", "--title", "y"],
        vec!["--help=x"],
    ] {
        assert_native_error(&args);
    }
    let route = ROUTES
        .iter()
        .find(|route| route.path == "linear issue mine")
        .expect("mine");
    let unknown = parse(&["issue", "mine", "--bogus"]).expect_err("unknown");
    assert_eq!(unknown.kind, AppErrorKind::Usage { route: route.route });
    let parent = ROUTES
        .iter()
        .find(|route| route.path == "linear issue")
        .expect("issue");
    let unknown_parent = parse(&["issue", "--bogus"]).expect_err("unknown");
    assert_eq!(
        unknown_parent.kind,
        AppErrorKind::Usage {
            route: parent.route
        }
    );
    assert!(unknown_parent.message.contains("--bogus"));
    let child = ROUTES
        .iter()
        .find(|route| route.path == "linear issue comment add")
        .expect("child");
    let nested = parse(&["issue", "comment", "add", "--bogus"]).expect_err("nested unknown");
    assert_eq!(nested.kind, AppErrorKind::Usage { route: child.route });
    let competing = parse(&["issue", "foo", "--workspace=x", "list", "--bogus"])
        .expect_err("parent surplus wins");
    assert_eq!(
        competing.kind,
        AppErrorKind::Usage {
            route: parent.route
        }
    );
    assert!(
        competing.message.contains("--bogus"),
        "{}",
        competing.message
    );
}

#[test]
fn inherited_help_and_root_version_keep_source_precedence() {
    match parse(&["issue", "--help", "attach"]).expect("inherited help") {
        Invocation::Help { route, .. } => assert_eq!(route.path, "linear issue attach"),
        other => panic!("expected help, got {other:?}"),
    }
    assert!(matches!(
        parse(&["--help", "--version"]).expect("first help"),
        Invocation::Help { .. }
    ));
    let bundle = parse(&["-hV"]).expect_err("bundle conflict");
    assert_eq!(
        bundle.message,
        "Option \"--version\" cannot be combined with other options."
    );
    let reverse = parse(&["--version", "--help"]).expect_err("reverse conflict");
    assert_eq!(
        reverse.message,
        "Option \"--help\" cannot be combined with other options."
    );
    match parse(&["--workspace", "ws", "issue", "--help"]).expect("workspace before child help") {
        Invocation::Help { route, .. } => assert_eq!(route.path, "linear issue"),
        other => panic!("expected issue help, got {other:?}"),
    }
    assert!(matches!(
        parse(&["--workspace", "ws", "--version"]).expect("global preparse then version"),
        Invocation::Version { .. }
    ));
    let version = parse(&["-V", "issue"]).expect_err("version surplus");
    assert_eq!(version.message, "Too many arguments: issue");
    let alias = parse(&["--version", "issue", "list"]).expect_err("version alias surplus");
    assert_eq!(alias.message, "Too many arguments: issue list");
    let extra = parse(&["-V", "issue", "X"]).expect_err("version consumes surplus");
    assert_eq!(extra.message, "Too many arguments: issue X");
    let child_help = parse(&["-V", "issue", "--help"]).expect_err("child help conflict");
    assert_eq!(
        child_help.message,
        "Option \"--help\" cannot be combined with other options."
    );
    assert_eq!(
        child_help.kind,
        AppErrorKind::Usage {
            route: ROUTES.first().expect("root").route
        }
    );
    let child_workspace =
        parse(&["-V", "issue", "--workspace", "ws"]).expect_err("child workspace conflict");
    assert_eq!(
        child_workspace.message,
        "Option \"--version\" cannot be combined with other options."
    );
    let prepended_help = parse(&["--help", "-V", "issue"]).expect_err("preparsed help surplus");
    assert_eq!(prepended_help.message, "Too many arguments: issue");
    let root_workspace =
        parse(&["-V", "--workspace", "ws", "issue"]).expect_err("root workspace after version");
    assert_eq!(
        root_workspace.message,
        "Option \"--version\" cannot be combined with other options."
    );
    let root_help_workspace = parse(&["--help", "--workspace", "ws", "-V", "issue"])
        .expect_err("root help with workspace");
    assert_eq!(
        root_help_workspace.message,
        "Option \"--help\" cannot be combined with other options."
    );
    let root_workspace_help = parse(&["--workspace", "ws", "--help", "-V", "issue"])
        .expect_err("root workspace with help");
    assert_eq!(
        root_workspace_help.message,
        "Option \"--help\" cannot be combined with other options."
    );
    assert_eq!(
        version.kind,
        AppErrorKind::Usage {
            route: ROUTES.first().expect("root").route
        }
    );
}

#[test]
fn empty_help_suffix_preserves_values_and_rejects_standalone_switch() {
    let title = action(&["issue", "create", "--title", "--help="]);
    assert_eq!(
        title.option("title").expect("title option").value.value,
        OptionValue::String("--help=".to_owned())
    );
    assert!(matches!(
        parse(&["issue", "--help="])
            .expect_err("standalone empty help suffix")
            .kind,
        AppErrorKind::Usage { .. }
    ));
    let literal = action(&["--", "--help="]);
    assert_eq!(literal.literal, ["--help="]);
    let two_values = action(&[
        "issue",
        "create",
        "--title",
        "--help=",
        "--description",
        "-h=",
    ]);
    assert_eq!(
        two_values.option("title").expect("title").value.value,
        OptionValue::String("--help=".to_owned())
    );
    assert_eq!(
        two_values
            .option("description")
            .expect("description")
            .value
            .value,
        OptionValue::String("-h=".to_owned())
    );
    let mixed = parse(&["issue", "create", "--title", "--help=", "--help="])
        .expect_err("second empty help suffix is standalone");
    assert!(matches!(mixed.kind, AppErrorKind::Usage { .. }));
}

#[test]
fn standalone_conflicts_keep_the_parse_level_and_last_option() {
    for (args, path, message) in [
        (
            &["--help", "issue", "--workspace", "ws"][..],
            "linear issue",
            "Option \"--help\" cannot be combined with other options.",
        ),
        (
            &["issue", "--help", "mine", "--workspace", "ws"][..],
            "linear issue mine",
            "Option \"--help\" cannot be combined with other options.",
        ),
        (
            &["--help", "--workspace", "ws", "issue"][..],
            "linear",
            "Option \"--help\" cannot be combined with other options.",
        ),
        (
            &["issue", "--help", "--workspace", "ws", "mine"][..],
            "linear issue",
            "Option \"--help\" cannot be combined with other options.",
        ),
        (
            &["--help", "--version", "--workspace", "ws"][..],
            "linear",
            "Option \"--version\" cannot be combined with other options.",
        ),
        (
            &["-V", "issue", "list", "--team", "X"][..],
            "linear",
            "Unknown option \"--team\". Did you mean option \"--help\"?",
        ),
    ] {
        let error = parse(args).expect_err("standalone conflict");
        assert_eq!(error.message, message, "{args:?}");
        assert_eq!(
            error.kind,
            AppErrorKind::Usage {
                route: ROUTES
                    .iter()
                    .find(|route| route.path == path)
                    .expect("route")
                    .route
            },
            "{args:?}"
        );
    }
    let unknown = parse(&["-V", "issue", "list", "--bogus"]).expect_err("root unknown");
    assert!(unknown.message.starts_with("Unknown option \"--bogus\"."));
    assert_eq!(
        unknown.kind,
        AppErrorKind::Usage {
            route: ROUTES.first().expect("root").route
        }
    );
    for args in [
        &["-V", "issue", "list", "--workspace", "ws", "--team", "X"][..],
        &["-V", "issue", "list", "--team", "X", "--bogus"][..],
        &["-V", "issue", "list", "--team"][..],
    ] {
        let error = parse(args).expect_err("root unknown precedes child parsing");
        assert_eq!(
            error.message, "Unknown option \"--team\". Did you mean option \"--help\"?",
            "{args:?}"
        );
        assert_eq!(
            error.kind,
            AppErrorKind::Usage {
                route: ROUTES.first().expect("root").route
            }
        );
    }
    let limit = parse(&["-V", "issue", "list", "--limit", "abc"]).expect_err("root unknown limit");
    assert!(limit.message.starts_with("Unknown option \"--limit\"."));
    let first_global =
        parse(&["--help", "--workspace", "ws", "--version"]).expect_err("root global preparse");
    assert_eq!(
        first_global.message,
        "Option \"--help\" cannot be combined with other options."
    );
    for (args, message) in [
        (
            &["--workspace", "ws", "-hV"][..],
            "Option \"--help\" cannot be combined with other options.",
        ),
        (
            &["--workspace", "ws", "-hV", "issue"][..],
            "Option \"--help\" cannot be combined with other options.",
        ),
        (
            &["-hV", "issue"][..],
            "Option \"--version\" cannot be combined with other options.",
        ),
        (
            &["-hV", "issue", "list", "--team", "X"][..],
            "Unknown option \"--team\". Did you mean option \"--help\"?",
        ),
        (
            &["-V", "issue", "list", "--team=X"][..],
            "Unknown option \"--team\". Did you mean option \"--help\"?",
        ),
        (
            &["-V", "issue", "list", "-hx"][..],
            "Unknown option \"-x\". Did you mean option \"-h\"?",
        ),
        (
            &["-V", "issue", "--help=x"][..],
            "Option \"--help\" doesn't take a value, but got \"x\".",
        ),
        (
            &[
                "-V",
                "issue",
                "--workspace",
                "a",
                "--workspace",
                "b",
                "--bogus",
            ][..],
            "Option \"--workspace\" can only occur once, but was found several times.",
        ),
        (
            &["-V", "issue", "list", "--workspace=", "--team", "X"][..],
            "Option \"--version\" cannot be combined with other options.",
        ),
        (&["-V", "issue", "-"][..], "Too many arguments: issue -"),
    ] {
        let error = parse(args).expect_err("root lexical precedence");
        assert_eq!(error.message, message, "{args:?}");
        assert_eq!(
            error.kind,
            AppErrorKind::Usage {
                route: ROUTES.first().expect("root").route
            },
            "{args:?}"
        );
    }
}

#[test]
fn shared_limits_require_positive_u32_and_preserve_defaults() {
    use std::num::NonZeroU32;
    for prefix in [
        vec!["document", "list"],
        vec!["project-update", "list", "P1"],
        vec!["initiative-update", "list", "I1"],
    ] {
        for input in [
            "0",
            "-0",
            "-1",
            "0.5",
            "1.0",
            "1e3",
            "0x10",
            "0b1",
            "0o7",
            "",
            " ",
            " 5",
            "5 ",
            "\u{00a0}5",
            "5\u{00a0}",
            "NaN",
            "Infinity",
            "4294967296",
        ] {
            let mut args = prefix.clone();
            args.extend(["--limit", input]);
            let error = parse(&args).expect_err("strict limit must fail in parser");
            assert!(matches!(error.kind, AppErrorKind::Usage { .. }), "{args:?}");
            if !input.is_empty() {
                assert!(
                    error.message.contains("positive decimal integer"),
                    "{}",
                    error.message
                );
            }
        }
        for (input, value) in [
            ("1", 1),
            ("16", 16),
            ("2147483647", 2_147_483_647),
            ("2147483648", 2_147_483_648),
            ("4294967295", u32::MAX),
        ] {
            let mut args = prefix.clone();
            args.extend(["--limit", input]);
            let parsed = action(&args);
            assert_eq!(
                parsed.option("limit").unwrap().value.value,
                OptionValue::PositiveInteger(NonZeroU32::new(value).unwrap())
            );
        }
        assert_eq!(
            action(&prefix).option("limit").unwrap().value.origin,
            ValueOrigin::Default
        );
    }
}
