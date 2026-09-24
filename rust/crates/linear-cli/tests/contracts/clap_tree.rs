use std::collections::BTreeSet;

use linear_cli::cli::{ROUTES, Route, RouteKind, RouteMeta, clap_tree};

fn select(args: &[&str], route: &RouteMeta) -> Route {
    let mut argv = args.to_vec();
    argv.extend(
        route
            .arguments
            .iter()
            .filter(|argument| !argument.optional)
            .map(|_| "sample"),
    );
    for option in route.local_options.iter().filter(|option| option.required) {
        argv.push(option.flags.first().expect("required option has a flag"));
        argv.push("sample");
    }
    let matches = clap_tree::build()
        .expect("valid generated inventory")
        .try_get_matches_from(argv)
        .expect("registered route parses");
    clap_tree::selected_route(&matches).expect("selected route exists")
}

#[test]
fn every_canonical_route_and_alias_selects_its_generated_identity() {
    let mut canonical = BTreeSet::new();
    let mut aliases = 0;
    let mut kinds = (0, 0, 0);
    for route in ROUTES {
        assert!(
            canonical.insert(route.path),
            "duplicate path: {}",
            route.path
        );
        let path = route.path.split(' ').collect::<Vec<_>>();
        assert_eq!(
            select(&path, route),
            route.route,
            "canonical path: {}",
            route.path
        );
        for alias in route.aliases {
            let mut alias_path = path.clone();
            let last = alias_path.last_mut().expect("route has a name");
            *last = alias;
            assert_eq!(
                select(&alias_path, route),
                route.route,
                "alias {alias}: {}",
                route.path
            );
            aliases += 1;
        }
        match route.kind {
            RouteKind::ParentRoute => kinds.0 += 1,
            RouteKind::SourceLeaf => kinds.1 += 1,
            RouteKind::GeneratedCompletionChild => kinds.2 += 1,
        }
    }
    assert_eq!(canonical.len(), 110);
    assert_eq!(aliases, 36);
    assert_eq!(kinds, (20, 86, 4));
}

#[test]
fn clap_tree_has_each_parent_child_edge_and_hidden_setting() {
    let tree = clap_tree::build().expect("valid generated inventory");
    tree.clone().debug_assert();
    for route in ROUTES {
        let mut node = &tree;
        for name in route.path.split(' ').skip(1) {
            node = node
                .get_subcommands()
                .find(|candidate| candidate.get_name() == name)
                .expect("canonical child exists");
        }
        assert_eq!(node.is_hide_set(), route.hidden, "{} hidden", route.path);
        let child_names = node
            .get_subcommands()
            .map(|child| child.get_name())
            .collect::<Vec<_>>();
        assert_eq!(child_names, route.children, "{} children", route.path);
        let node_aliases = node.get_all_aliases().collect::<Vec<_>>();
        assert_eq!(node_aliases, route.aliases, "{} aliases", route.path);
    }
}

#[test]
fn every_node_with_children_accepts_bare_selection() {
    let nodes = ROUTES
        .iter()
        .filter(|route| !route.children.is_empty())
        .collect::<Vec<_>>();
    assert_eq!(nodes.len(), 21);
    for route in nodes {
        let path = route.path.split(' ').collect::<Vec<_>>();
        assert_eq!(
            select(&path, route),
            route.route,
            "bare route: {}",
            route.path
        );
    }
}

#[test]
fn built_in_help_subcommand_is_disabled_but_help_flag_remains() {
    for args in [
        ["linear", "help"].as_slice(),
        ["linear", "issue", "help"].as_slice(),
    ] {
        let matches = clap_tree::build()
            .expect("valid generated inventory")
            .try_get_matches_from(args)
            .expect("surplus positional captures unregistered help child");
        let selected = matches.subcommand().map_or(&matches, |(_, child)| child);
        assert_eq!(
            selected
                .get_many::<String>("internal:surplus")
                .expect("surplus")
                .map(String::as_str)
                .collect::<Vec<_>>(),
            ["help"]
        );
    }
    let help = clap_tree::build()
        .expect("valid generated inventory")
        .try_get_matches_from(["linear", "--help"])
        .expect("C1 registers ordinary help flag");
    assert_eq!(help.get_count("help:long"), 1);
}
