use std::collections::BTreeSet;

use clap::{Arg, ArgAction, ArgMatches, Command, error::ErrorKind};
use linear_cli::cli::{ROUTES, RouteMeta, clap_tree};
use linear_cli::error::AppErrorKind;

fn command_for<'a>(root: &'a Command, route: &RouteMeta) -> &'a Command {
    route.path.split(' ').skip(1).fold(root, |parent, name| {
        parent
            .get_subcommands()
            .find(|child| child.get_name() == name)
            .expect("canonical route exists")
    })
}

fn argument<'a>(command: &'a Command, id: &str) -> &'a Arg {
    command
        .get_arguments()
        .find(|arg| arg.get_id().as_str() == id)
        .expect("registered argument exists")
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

#[test]
fn positionals_switches_and_workspace_are_covered_by_manifest_metadata() {
    let tree = clap_tree::build().expect("generated tree builds");
    tree.clone().debug_assert();
    let mut switches = 0;
    let mut positionals = 0;
    let mut inherited = 0;
    let mut label_overrides = 0;
    for route in ROUTES {
        let node = command_for(&tree, route);
        let mut expected_ids = BTreeSet::new();
        for (offset, descriptor) in route.arguments.iter().enumerate() {
            let id = format!("pos:{}", descriptor.name);
            assert!(expected_ids.insert(id.clone()), "{} {id}", route.path);
            let arg = argument(node, &id);
            assert_eq!(arg.get_index(), Some(offset + 1), "{} {id}", route.path);
            assert_eq!(
                arg.is_required_set(),
                !descriptor.optional,
                "{} {id}",
                route.path
            );
            assert!(
                matches!(arg.get_action(), ArgAction::Set),
                "{} {id}",
                route.path
            );
            if descriptor.variadic {
                let range = arg.get_num_args().expect("variadic range");
                assert_eq!(range.min_values(), 0);
                assert_eq!(range.max_values(), usize::MAX);
            }
            positionals += 1;
        }
        for option in route
            .local_options
            .iter()
            .filter(|option| option.args.is_empty())
        {
            let id = format!("opt:{}", option.name);
            assert!(expected_ids.insert(id.clone()), "{} {id}", route.path);
            let arg = argument(node, &id);
            assert!(
                matches!(arg.get_action(), ArgAction::SetTrue),
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
            let expected_flags = if route.path == "linear label list" && option.name == "workspace"
            {
                label_overrides += 1;
                ["--workspace-only".to_owned()].into_iter().collect()
            } else {
                option.flags.iter().map(|flag| (*flag).to_owned()).collect()
            };
            assert_eq!(spellings(arg), expected_flags, "{} {id}", route.path);
            switches += 1;
        }
        let has_workspace = route.path == "linear"
            || !route.inherited_global_options.is_empty()
            || route.path == "linear label list";
        if has_workspace {
            assert!(expected_ids.insert("global:workspace".to_owned()));
            let arg = argument(node, "global:workspace");
            assert_eq!(spellings(arg), ["--workspace".to_owned()].into());
            assert!(matches!(arg.get_action(), ArgAction::Set));
            assert!(!arg.is_required_set());
            if route.path != "linear" {
                inherited += usize::from(route.path != "linear label list");
            }
        }
        let actual_ids = node
            .get_arguments()
            .filter(|arg| {
                !arg.get_id().as_str().starts_with("opt:")
                    || matches!(arg.get_action(), ArgAction::SetTrue)
            })
            .map(|arg| arg.get_id().as_str().to_owned())
            .collect::<BTreeSet<_>>();
        assert_eq!(actual_ids, expected_ids, "{} arguments", route.path);
        if route.path.starts_with("linear completions") {
            assert!(!actual_ids.contains("global:workspace"), "{}", route.path);
        }
    }
    assert_eq!(switches, 122);
    assert_eq!(positionals, 66);
    assert_eq!(inherited, 103);
    assert_eq!(label_overrides, 1);
}

#[test]
fn positional_workspace_is_distinct_from_credential_workspace() {
    for verb in ["logout", "default"] {
        let argv = ["linear", "auth", verb, "personal"];
        let matches = parse(&argv);
        assert_eq!(
            selected(&matches)
                .get_one::<String>("pos:workspace")
                .map(String::as_str),
            Some("personal")
        );
        let argv = ["linear", "--workspace", "work", "auth", verb, "personal"];
        let matches = parse(&argv);
        assert_eq!(
            matches
                .get_one::<String>("global:workspace")
                .map(String::as_str),
            Some("work")
        );
        assert_eq!(
            selected(&matches)
                .get_one::<String>("pos:workspace")
                .map(String::as_str),
            Some("personal")
        );
    }
}

#[test]
fn label_workspace_filter_and_global_selection_are_distinct() {
    for argv in [
        &["linear", "label", "list", "--workspace-only"][..],
        &[
            "linear",
            "--workspace",
            "acme",
            "label",
            "list",
            "--workspace-only",
        ][..],
        &[
            "linear",
            "label",
            "list",
            "--workspace",
            "acme",
            "--workspace-only",
        ][..],
    ] {
        let matches = parse(argv);
        assert_eq!(
            selected(&matches).get_one::<bool>("opt:workspace"),
            Some(&true)
        );
    }
    let missing = clap_tree::build()
        .expect("tree")
        .try_get_matches_from(["linear", "label", "list", "--workspace"])
        .expect_err("workspace slug is required");
    assert_eq!(missing.kind(), ErrorKind::InvalidValue);
}

#[test]
fn workspace_selection_across_levels_is_unique() {
    for argv in [
        &["linear", "--workspace", "acme", "auth", "list"][..],
        &["linear", "auth", "--workspace", "acme", "list"][..],
        &["linear", "auth", "list", "--workspace", "acme"][..],
        &[
            "linear",
            "--workspace",
            "acme",
            "label",
            "list",
            "--workspace-only",
        ][..],
        &[
            "linear",
            "label",
            "list",
            "--workspace",
            "acme",
            "--workspace-only",
        ][..],
        &[
            "linear",
            "--workspace",
            "acme",
            "completions",
            "complete",
            "foo",
        ][..],
    ] {
        let matches = parse(argv);
        let workspace = clap_tree::selected_workspace(&matches).expect("single workspace");
        assert_eq!(workspace.as_deref(), Some("acme"), "{argv:?}");
    }
    for argv in [
        &[
            "linear",
            "--workspace",
            "a",
            "auth",
            "list",
            "--workspace",
            "b",
        ][..],
        &[
            "linear",
            "auth",
            "--workspace",
            "a",
            "list",
            "--workspace",
            "b",
        ][..],
        &[
            "linear",
            "--workspace",
            "a",
            "label",
            "list",
            "--workspace",
            "b",
        ][..],
    ] {
        let matches = parse(argv);
        let error = clap_tree::selected_workspace(&matches).expect_err("repeated workspace");
        assert!(matches!(error.kind, AppErrorKind::Usage { .. }), "{argv:?}");
    }
    for argv in [
        &[
            "linear",
            "--workspace",
            "a",
            "--workspace",
            "b",
            "auth",
            "list",
        ][..],
        &[
            "linear",
            "auth",
            "list",
            "--workspace",
            "a",
            "--workspace",
            "b",
        ][..],
    ] {
        assert!(
            clap_tree::build()
                .expect("tree")
                .try_get_matches_from(argv)
                .is_err(),
            "same-level repeated workspace must fail: {argv:?}"
        );
    }
    assert!(
        clap_tree::build()
            .expect("tree")
            .try_get_matches_from([
                "linear",
                "completions",
                "complete",
                "foo",
                "--workspace",
                "acme"
            ])
            .is_err()
    );
}

#[test]
fn switches_accept_aliases_and_reject_duplicates() {
    for argv in [
        &["linear", "issue", "view", "ABC-1", "-j"][..],
        &["linear", "issue", "view", "ABC-1", "--json"][..],
    ] {
        let matches = parse(argv);
        assert_eq!(selected(&matches).get_one::<bool>("opt:json"), Some(&true));
    }
    let matches = parse(&["linear", "issue", "view", "ABC-1", "--no-comments"]);
    assert_eq!(
        selected(&matches).get_one::<bool>("opt:no-comments"),
        Some(&true)
    );
    let references = parse(&["linear", "issue", "describe", "ABC-1", "--ref"]);
    assert_eq!(
        selected(&references).get_one::<bool>("opt:references"),
        Some(&true)
    );
    let hidden = parse(&["linear", "issue", "mine", "--all-assignees"]);
    assert_eq!(
        selected(&hidden).get_one::<bool>("opt:all-assignees"),
        Some(&true)
    );
    assert!(
        clap_tree::build()
            .expect("tree")
            .try_get_matches_from(["linear", "issue", "view", "ABC-1", "-j", "--json"])
            .is_err()
    );
}

#[test]
fn required_optional_and_variadic_positionals_have_distinct_shapes() {
    let issue = parse(&["linear", "issue", "attach", "ABC-1", "/tmp/file"]);
    assert_eq!(
        selected(&issue)
            .get_one::<String>("pos:issueId")
            .map(String::as_str),
        Some("ABC-1")
    );
    assert_eq!(
        selected(&issue)
            .get_one::<String>("pos:filepath")
            .map(String::as_str),
        Some("/tmp/file")
    );
    assert!(
        clap_tree::build()
            .expect("tree")
            .try_get_matches_from(["linear", "issue", "attach", "ABC-1"])
            .is_err()
    );

    let optional = parse(&["linear", "issue", "view"]);
    assert_eq!(selected(&optional).get_one::<String>("pos:issueId"), None);

    let complete = parse(&["linear", "completions", "complete", "next", "issue", "view"]);
    let names = selected(&complete)
        .get_many::<String>("pos:command")
        .expect("optional variadic values")
        .map(String::as_str)
        .collect::<Vec<_>>();
    assert_eq!(names, ["issue", "view"]);
    let no_tail = parse(&["linear", "completions", "complete", "next"]);
    assert!(
        selected(&no_tail)
            .get_many::<String>("pos:command")
            .is_none()
    );
}
