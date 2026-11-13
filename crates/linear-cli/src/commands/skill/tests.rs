use clap::Command;

use super::guide;
use crate::cli;

fn rendered() -> String {
    guide::render(&cli::command())
}

/// Every command path a user can run, such as `linear issue comment add`.
fn runnable_paths(command: &Command, parent: &str, hidden: bool, out: &mut Vec<(String, bool)>) {
    for child in command.get_subcommands() {
        if child.get_name() == "help" {
            continue;
        }
        let path = format!("{parent} {}", child.get_name());
        let hidden = hidden || child.is_hide_set();
        out.push((path.clone(), hidden));
        runnable_paths(child, &path, hidden, out);
    }
}

#[test]
fn the_guide_fills_every_placeholder_and_stays_short() {
    let guide = rendered();
    assert!(!guide.contains("{{"), "an unfilled placeholder remains");
    // An agent reads the whole guide on every load.
    let lines = guide.lines().count();
    assert!(lines < 600, "the guide is {lines} lines");
}

#[test]
fn the_guide_names_the_version_that_printed_it() {
    let heading = format!("# Linear CLI (linear {})", env!("CARGO_PKG_VERSION"));
    assert_eq!(rendered().lines().next(), Some(heading.as_str()));
    assert_eq!(
        cli::command().get_version(),
        Some(env!("CARGO_PKG_VERSION"))
    );
}

#[test]
fn the_guide_carries_the_exit_statuses_and_markdown_rules_help_prints() {
    let guide = rendered();
    assert!(guide.contains(cli::EXIT_STATUS));
    assert!(guide.contains(include_str!("../../cli/markdown.txt").trim_end()));
}

#[test]
fn the_command_index_lists_every_visible_command_and_no_hidden_one() {
    let mut root = cli::command();
    root.build();
    let index = guide::command_index(&root);
    let listed: Vec<&str> = index
        .lines()
        .filter(|line| !line.is_empty())
        .map(|line| line.split("  ").next().unwrap_or_default())
        .collect();
    let mut paths = Vec::new();
    runnable_paths(&root, "linear", false, &mut paths);
    assert!(paths.len() > 50, "the command tree was not walked");
    for (path, hidden) in paths {
        let unlisted = hidden || path.starts_with("linear completions");
        assert_eq!(
            listed.contains(&path.as_str()),
            !unlisted,
            "{path} (hidden: {hidden})"
        );
    }
    let described = index
        .lines()
        .filter(|line| line.starts_with("linear issue list "))
        .count();
    assert_eq!(described, 1, "each command has its description beside it");
}

/// The words of one shell command line: quotes group, a backslash escapes,
/// and the line ends at a comment, pipe or redirection outside a word.
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut in_word = false;
    let mut chars = line.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' => {
                in_word = true;
                word.extend(chars.by_ref().take_while(|&c| c != '\''));
            }
            '"' => {
                in_word = true;
                while let Some(c) = chars.next() {
                    match c {
                        '"' => break,
                        '\\' => word.extend(chars.next()),
                        c => word.push(c),
                    }
                }
            }
            '\\' => {
                in_word = true;
                word.extend(chars.next());
            }
            '#' | '|' | '>' if !in_word => break,
            // A redirection or heredoc; `<ID>` is a placeholder word.
            '<' if !in_word && chars.peek().is_none_or(|&c| c == '<' || c.is_whitespace()) => {
                break;
            }
            c if c.is_whitespace() => {
                if in_word {
                    words.push(std::mem::take(&mut word));
                    in_word = false;
                }
            }
            c => {
                in_word = true;
                word.push(c);
            }
        }
    }
    if in_word {
        words.push(word);
    }
    words
}

/// The `linear ...` command lines in the guide's `bash` fences, with
/// backslash continuations joined. A fence opened as `bash no-check` holds
/// shell that is not a plain `linear` invocation and is skipped.
fn guide_examples() -> Vec<String> {
    let mut examples = Vec::new();
    let mut checking = false;
    let mut pending = String::new();
    for line in include_str!("guide.md").lines() {
        if let Some(info) = line.strip_prefix("```") {
            checking = info == "bash";
            pending.clear();
            continue;
        }
        if !checking {
            continue;
        }
        let joined = format!("{pending}{line}");
        if let Some(continued) = joined.strip_suffix('\\') {
            pending = continued.to_owned();
            continue;
        }
        pending.clear();
        if joined.starts_with("linear ") {
            examples.push(joined);
        }
    }
    examples
}

#[test]
fn shell_words_follow_quotes_and_stop_at_comments_and_pipes() {
    assert_eq!(
        shell_words(r#"linear issue update ENG-1 --state "In Review"   # note"#),
        ["linear", "issue", "update", "ENG-1", "--state", "In Review"]
    );
    assert_eq!(
        shell_words("linear api '{ viewer { id } }' | jq .data"),
        ["linear", "api", "{ viewer { id } }"]
    );
    assert_eq!(
        shell_words("linear api --variable first=5 <<'GRAPHQL'"),
        ["linear", "api", "--variable", "first=5"]
    );
    assert_eq!(
        shell_words(r##"linear label create --color "#EB5757""##),
        ["linear", "label", "create", "--color", "#EB5757"]
    );
    assert_eq!(
        shell_words("linear project view <ID> < input"),
        ["linear", "project", "view", "<ID>"]
    );
}

#[test]
fn every_example_in_the_guide_parses() {
    let examples = guide_examples();
    assert!(examples.len() > 40, "found {} examples", examples.len());
    for example in examples {
        if let Err(error) = cli::command().try_get_matches_from(shell_words(&example)) {
            panic!("the guide's example does not parse: {example}\n{error}");
        }
    }
}
