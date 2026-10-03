use clap::{CommandFactory, Parser};

use super::Cli;

fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
    Cli::try_parse_from(std::iter::once("linear").chain(args.iter().copied()))
}

#[track_caller]
fn parses(args: &[&str]) -> Cli {
    parse(args).unwrap_or_else(|error| panic!("{args:?} should parse: {error}"))
}

#[test]
fn the_grammar_is_consistent() {
    Cli::command().debug_assert();
}

#[test]
fn enum_values_are_kebab_case_and_keep_their_camel_case_spellings() {
    for health in [
        "on-track",
        "onTrack",
        "at-risk",
        "atRisk",
        "off-track",
        "offTrack",
    ] {
        parses(&["project-update", "create", "P", "--health", health]);
    }
    for status in ["awaiting-input", "awaitingInput"] {
        parses(&["issue", "agent-session", "list", "--status", status]);
    }
}

/// Whether `--yes` (or an old spelling of it) was given to a destructive command.
fn confirmed(cli: &Cli) -> bool {
    use super::RootCommand as Root;
    use super::{auth, document, initiative, issue, label, milestone, project, team};
    match &cli.command {
        Root::Auth(group) => match &group.command {
            auth::AuthCommand::Logout(args) => args.confirm.yes,
            other => panic!("not a destructive command: {other:?}"),
        },
        Root::Issue(group) => match &group.command {
            issue::IssueCommand::Archive(args) => args.confirm.yes,
            issue::IssueCommand::Delete(args) => args.confirm.yes,
            other => panic!("not a destructive command: {other:?}"),
        },
        Root::Label(group) => match &group.command {
            label::LabelCommand::Delete(args) => args.confirm.yes,
            other => panic!("not a destructive command: {other:?}"),
        },
        Root::Milestone(group) => match &group.command {
            milestone::MilestoneCommand::Delete(args) => args.confirm.yes,
            other => panic!("not a destructive command: {other:?}"),
        },
        Root::Project(group) => match &group.command {
            project::ProjectCommand::Delete(args) => args.confirm.yes,
            other => panic!("not a destructive command: {other:?}"),
        },
        Root::Team(group) => match &group.command {
            team::TeamCommand::Delete(args) => args.confirm.yes,
            other => panic!("not a destructive command: {other:?}"),
        },
        Root::Initiative(group) => match &group.command {
            initiative::InitiativeCommand::Archive(args) => args.confirm.yes,
            initiative::InitiativeCommand::Unarchive(args) => args.confirm.yes,
            initiative::InitiativeCommand::Delete(args) => args.confirm.yes,
            initiative::InitiativeCommand::RemoveProject(args) => args.confirm.yes,
            other => panic!("not a destructive command: {other:?}"),
        },
        Root::Document(group) => match &group.command {
            document::DocumentCommand::Delete(args) => args.confirm.yes,
            other => panic!("not a destructive command: {other:?}"),
        },
        other => panic!("not a destructive command: {other:?}"),
    }
}

#[test]
fn destructive_commands_take_yes_and_every_old_spelling() {
    let commands: [&[&str]; 12] = [
        &["auth", "logout", "acme"],
        &["issue", "archive", "ENG-1"],
        &["issue", "delete", "ENG-1"],
        &["label", "delete", "Bug"],
        &["milestone", "delete", "m-1"],
        &["project", "delete", "p-1"],
        &["team", "delete", "ENG"],
        &["initiative", "archive", "i-1"],
        &["initiative", "unarchive", "i-1"],
        &["initiative", "delete", "i-1"],
        &["initiative", "remove-project", "i-1", "p-1"],
        &["document", "delete", "d-1"],
    ];
    for command in commands {
        assert!(!confirmed(&parses(command)), "{command:?}");
        for flag in ["--yes", "-y", "--force", "-f", "--confirm"] {
            let mut args = command.to_vec();
            args.push(flag);
            assert!(confirmed(&parses(&args)), "{args:?}");
        }
    }
}

#[test]
fn issue_list_answers_to_its_old_names() {
    use super::RootCommand;
    use super::issue::IssueCommand;
    for name in ["list", "mine", "l"] {
        let cli = parses(&["issue", name, "-A"]);
        let RootCommand::Issue(group) = cli.command else {
            panic!("{name} is an issue command");
        };
        let IssueCommand::List(args) = group.command else {
            panic!("{name} is issue list");
        };
        assert!(args.filters.all_assignees, "{name}");
    }
}

#[test]
fn renamed_and_hidden_flags_still_parse() {
    parses(&["label", "list", "--all"]);
    parses(&["issue", "query", "--all-assignees", "--all-states"]);
    parses(&["issue", "create", "--no-interactive", "-t", "x"]);
    parses(&["team", "create", "--no-interactive"]);
    parses(&["--no-interactive", "issue", "list"]);
}

/// Every command, depth first, with its path.
fn commands() -> Vec<(String, clap::Command)> {
    fn walk(path: String, command: clap::Command, out: &mut Vec<(String, clap::Command)>) {
        for sub in command.get_subcommands() {
            walk(format!("{path} {}", sub.get_name()), sub.clone(), out);
        }
        out.push((path, command));
    }
    let mut command = Cli::command();
    command.build();
    let mut out = Vec::new();
    walk("linear".to_owned(), command, &mut out);
    out
}

#[test]
fn reserved_short_flags_keep_one_meaning_everywhere() {
    for (path, command) in commands() {
        for arg in command.get_arguments() {
            let Some(long) = arg.get_long() else { continue };
            let expected = match long {
                "yes" => Some('y'),
                "interactive" => Some('i'),
                "json" => Some('j'),
                _ => None,
            };
            if let Some(short) = arg.get_short() {
                for (reserved, owner) in [('y', "yes"), ('i', "interactive"), ('j', "json")] {
                    assert!(
                        short != reserved || long == owner,
                        "{path}: -{short} is --{long}, but -{reserved} is reserved for --{owner}"
                    );
                }
            }
            if let Some(expected) = expected {
                assert_eq!(arg.get_short(), Some(expected), "{path} --{long}");
            }
        }
    }
}

#[test]
fn every_argument_and_command_has_help() {
    for (path, command) in commands() {
        if path != "linear" {
            assert!(command.get_about().is_some(), "{path} has no about");
        }
        for arg in command.get_arguments() {
            if arg.is_hide_set() || ["help", "version"].contains(&arg.get_id().as_str()) {
                continue;
            }
            assert!(
                arg.get_help().is_some(),
                "{path} {} has no help",
                arg.get_id()
            );
        }
    }
}
