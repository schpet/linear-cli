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
