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
