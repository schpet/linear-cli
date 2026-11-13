use std::cell::Cell;
use std::collections::BTreeSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use clap::Command;

use super::apply::{self, Files, RealFiles};
use super::guide;
use super::plan::{self, Change, Reason, State, Stubs, inspect};
use super::slots::{self, Layout, Location, Scope, SkillEnv};
use crate::cli;
use crate::cli::skill::Agent;
use crate::config::{FileKind, FileSource, RealFileSource};

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

// The installed skill file.

const STUB: &str = Stubs::SHIPPED.current;

/// The `key: value` lines of the stub's frontmatter, nested or not.
fn frontmatter() -> Vec<(&'static str, &'static str)> {
    let body = STUB.strip_prefix("---\n").expect("the stub opens with ---");
    let (yaml, _) = body
        .split_once("\n---\n")
        .expect("the frontmatter closes with ---");
    yaml.lines()
        .map(|line| {
            let line = line.trim_start();
            line.split_once(": ")
                .unwrap_or((line.trim_end_matches(':'), ""))
        })
        .collect()
}

#[test]
fn the_stub_is_a_skill_every_agent_can_load() {
    // A stub is recognized by its exact bytes, so a checkout that rewrote
    // its line endings would install a file other builds call foreign.
    assert!(
        !STUB.contains('\r'),
        "the stub was checked out with CRLF line endings; .gitattributes pins it"
    );
    let fields = frontmatter();
    let value = |key: &str| {
        fields
            .iter()
            .find(|(name, _)| *name == key)
            .map(|(_, value)| *value)
    };
    let name = value("name").expect("a name");
    assert_eq!(name, slots::SKILL_NAME, "the name is the directory's name");
    assert!(
        name.len() <= 64
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
    );
    let description = value("description").expect("a description");
    assert!(
        description.len() < 1024,
        "agents drop or cut a longer description"
    );

    // No YAML parser is linked, so the values keep to what every parser
    // reads the same way: a plain scalar, or a double-quoted one with nothing
    // to escape. A stray `: ` makes Pi skip the skill.
    for (key, value) in fields {
        let plain = !value.contains(": ")
            && !value.contains(" #")
            && !value.ends_with(':')
            && value.chars().next().is_none_or(char::is_alphanumeric);
        let quoted = value.len() >= 2
            && value.starts_with('"')
            && value.ends_with('"')
            && !value
                .get(1..value.len() - 1)
                .is_some_and(|inner| inner.contains(['"', '\\']));
        assert!(plain || quoted, "{key}: {value}");
    }
}

#[test]
fn the_stub_tells_the_agent_to_run_the_command_that_prints_the_guide() {
    assert!(STUB.contains("Run `linear skill show`"));
    cli::command()
        .try_get_matches_from(["linear", "skill", "show"])
        .expect("the stub's command parses");
}

// Where skills live.

fn env(home: &Path) -> SkillEnv {
    SkillEnv {
        home: Some(home.to_path_buf()),
        ..SkillEnv::default()
    }
}

fn paths(layout: &Layout, location: Location) -> Vec<PathBuf> {
    layout
        .slots
        .iter()
        .filter(|slot| slot.location == location)
        .map(|slot| slot.path.clone())
        .collect()
}

#[test]
fn an_account_has_two_install_directories_and_three_others_are_watched() {
    let layout = Layout::user(&env(Path::new("/home/a"))).expect("layout");
    assert_eq!(layout.root, Path::new("/home/a"));
    assert_eq!(
        paths(&layout, Location::Canonical),
        [
            Path::new("/home/a/.agents/skills/linear-cli"),
            Path::new("/home/a/.claude/skills/linear-cli"),
        ]
    );
    assert_eq!(
        paths(&layout, Location::Alternate),
        [
            Path::new("/home/a/.codex/skills/linear-cli"),
            Path::new("/home/a/.pi/agent/skills/linear-cli"),
            Path::new("/home/a/.config/opencode/skills/linear-cli"),
        ]
    );
    let shared = layout.slots.first().expect("the shared slot");
    assert_eq!(
        shared.agents,
        [Agent::Codex, Agent::Opencode, Agent::Pi],
        "three agents read the shared directory"
    );
}

#[test]
fn each_agent_s_own_variable_moves_its_directory() {
    let layout = Layout::user(&SkillEnv {
        claude_config_dir: Some("/etc/claude".into()),
        codex_home: Some("/etc/codex".into()),
        pi_agent_dir: Some("/etc/pi".into()),
        xdg_config_home: Some("/etc/xdg".into()),
        ..env(Path::new("/home/a"))
    })
    .expect("layout");
    let all: Vec<PathBuf> = layout.slots.iter().map(|slot| slot.path.clone()).collect();
    assert_eq!(
        all,
        [
            Path::new("/home/a/.agents/skills/linear-cli"),
            Path::new("/etc/claude/skills/linear-cli"),
            Path::new("/etc/codex/skills/linear-cli"),
            Path::new("/etc/pi/skills/linear-cli"),
            Path::new("/etc/xdg/opencode/skills/linear-cli"),
        ]
    );
}

#[test]
fn a_relative_directory_variable_is_refused() {
    let error = slots::absolute("CLAUDE_CONFIG_DIR", "claude".into()).expect_err("relative");
    assert!(error.to_string().contains("CLAUDE_CONFIG_DIR"), "{error}");
    assert!(error.hint().is_some());
}

#[test]
fn no_home_is_an_error_for_an_account_and_not_for_a_project() {
    let dir = tempfile::tempdir().expect("temp dir");
    let env = SkillEnv {
        cwd: Some(dir.path().to_path_buf()),
        ..SkillEnv::default()
    };
    assert!(Layout::user(&env).is_err());
    assert!(Layout::project(&env, &RealFileSource).is_ok());
}

fn agents(agents: &[Agent]) -> BTreeSet<Agent> {
    agents.iter().copied().collect()
}

#[test]
fn claude_code_is_set_up_by_default_only_where_it_already_is() {
    let home = tempfile::tempdir().expect("temp dir");
    let shared = agents(&[Agent::Codex, Agent::Opencode, Agent::Pi]);
    let without = Layout::user(&env(home.path())).expect("layout");
    assert_eq!(without.setup_agents(&[]), shared);
    assert_eq!(
        without.setup_agents(&[Agent::Claude]),
        agents(&[Agent::Claude]),
        "naming it installs it anyway"
    );

    fs::create_dir(home.path().join(".claude")).expect("create");
    let with = Layout::user(&env(home.path())).expect("layout");
    assert_eq!(with.setup_agents(&[]), agents(&Agent::ALL));

    let project = Layout::project(
        &SkillEnv {
            cwd: Some(home.path().to_path_buf()),
            ..SkillEnv::default()
        },
        &RealFileSource,
    )
    .expect("layout");
    assert_eq!(project.scope, Scope::Project);
    assert_eq!(
        project.setup_agents(&[]),
        agents(&Agent::ALL),
        "a project is set up for whoever works in it"
    );
}

#[test]
fn a_project_is_the_repository_root_or_the_working_directory() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().canonicalize().expect("canonical");
    let nested = root.join("packages").join("app");
    fs::create_dir_all(&nested).expect("create");
    let found = |cwd: &Path| slots::project_root(cwd, &RealFileSource).expect("root");

    assert_eq!(found(&nested), nested, "outside a repository");
    fs::create_dir(root.join(".jj")).expect("create");
    assert_eq!(found(&nested), root, "a jj repository");
    fs::remove_dir(root.join(".jj")).expect("remove");
    fs::write(root.join(".git"), "gitdir: elsewhere").expect("write");
    assert_eq!(found(&nested), root, "a git worktree's .git file");
}

/// A filesystem on which nothing can be inspected.
struct Unreadable;

impl FileSource for Unreadable {
    fn kind(&self, _: &Path) -> io::Result<Option<FileKind>> {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }

    fn read_bounded(&self, _: &Path, _: u64) -> io::Result<Vec<u8>> {
        Err(io::Error::from(io::ErrorKind::PermissionDenied))
    }
}

#[test]
fn a_repository_marker_that_cannot_be_inspected_is_an_error_not_a_guess() {
    let error = slots::project_root(Path::new("/work/app"), &Unreadable).expect_err("unreadable");
    assert!(error.to_string().contains("/work/app"), "{error}");
}

#[test]
fn directories_between_the_working_directory_and_the_root_are_watched() {
    let dir = tempfile::tempdir().expect("temp dir");
    let root = dir.path().canonicalize().expect("canonical");
    fs::create_dir(root.join(".git")).expect("create");
    let cwd = root.join("packages").join("app");
    fs::create_dir_all(&cwd).expect("create");
    let layout = Layout::project(
        &SkillEnv {
            cwd: Some(cwd.clone()),
            ..SkillEnv::default()
        },
        &RealFileSource,
    )
    .expect("layout");

    assert_eq!(layout.root, root);
    assert_eq!(
        paths(&layout, Location::Canonical),
        [
            root.join(".agents/skills/linear-cli"),
            root.join(".claude/skills/linear-cli"),
        ]
    );
    let nested = paths(&layout, Location::Nested);
    assert_eq!(nested.len(), 10, "five directories at each of two levels");
    assert!(nested.contains(&root.join("packages/.agents/skills/linear-cli")));
    assert!(nested.contains(&cwd.join(".opencode/skills/linear-cli")));
}

#[cfg(unix)]
#[test]
fn skills_directories_that_are_one_directory_are_one_slot() {
    let home = tempfile::tempdir().expect("temp dir");
    let shared = home.path().join(".agents").join("skills");
    fs::create_dir_all(&shared).expect("create");
    fs::create_dir(home.path().join(".claude")).expect("create");
    std::os::unix::fs::symlink(&shared, home.path().join(".claude").join("skills"))
        .expect("symlink");

    let layout = Layout::user(&env(home.path())).expect("layout");
    let canonical: Vec<_> = layout
        .slots
        .iter()
        .filter(|slot| slot.location == Location::Canonical)
        .collect();
    assert_eq!(canonical.len(), 1);
    assert_eq!(
        canonical.first().map(|slot| &*slot.agents),
        Some(&Agent::ALL[..])
    );
}

/// The layout of a home whose agents all read one skills repository: two
/// agents' skills directories are symlinks to it, and a third agent has a
/// symlink to the skill inside it.
#[cfg(unix)]
#[test]
fn an_agent_s_symlink_into_an_install_directory_is_that_directory() {
    use std::os::unix::fs::symlink;
    let home = tempfile::tempdir().expect("temp dir");
    let repo = home.path().join("dotfiles/skills");
    fs::create_dir_all(repo.join("linear-cli")).expect("create");
    fs::write(repo.join("linear-cli/SKILL.md"), OLD_SKILL).expect("write");
    for directory in [".claude", ".pi/agent", ".codex/skills"] {
        fs::create_dir_all(home.path().join(directory)).expect("create");
    }
    symlink(&repo, home.path().join(".claude/skills")).expect("symlink");
    symlink(&repo, home.path().join(".pi/agent/skills")).expect("symlink");
    symlink(
        repo.join("linear-cli"),
        home.path().join(".codex/skills/linear-cli"),
    )
    .expect("symlink");

    let layout = Layout::user(&env(home.path())).expect("layout");
    let slots: Vec<_> = layout
        .slots
        .iter()
        .map(|slot| (slot.path.clone(), slot.agents.clone()))
        .collect();
    assert_eq!(
        slots,
        [
            (
                home.path().join(".agents/skills/linear-cli"),
                vec![Agent::Codex, Agent::Opencode, Agent::Pi]
            ),
            (
                home.path().join(".claude/skills/linear-cli"),
                vec![Agent::Claude, Agent::Codex, Agent::Pi]
            ),
            (
                home.path().join(".config/opencode/skills/linear-cli"),
                vec![Agent::Opencode]
            ),
        ],
        "one skill in the repository, read by three agents"
    );

    // Replacing it once is the whole job: no second step trips over the first.
    let mut steps = plan::setup(&layout, &agents(&[Agent::Claude]), true, STUBS, "STAMP");
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(changes(&steps), [Change::Replaced]);
    assert_eq!(
        fs::read_to_string(repo.join("linear-cli/SKILL.md")).expect("read"),
        STUB,
        "written through the symlinked skills directory, into the repository"
    );
    assert_eq!(
        fs::read_to_string(home.path().join(".codex/skills/linear-cli/SKILL.md")).expect("read"),
        STUB,
        "the third agent's link now reaches the stub"
    );
    assert!(
        home.path()
            .join(".claude/linear-cli-skill-backups/STAMP/linear-cli/SKILL.md")
            .exists(),
        "the old skill is backed up outside the repository"
    );
}

// What a directory holds.

const OLD_SKILL: &str = "---\nname: linear-cli\ndescription: Manage Linear issues from the \
                         command line using the linear cli. This skill allows automating \
                         linear management.\nallowed-tools: Bash(linear:*), Bash(curl:*)\n---\n\n\
                         # Linear CLI\n";
const EARLIER_STUB: &str = "---\nname: linear-cli\ndescription: An earlier stub.\nmetadata:\n  \
                            linear-cli-stub: \"0\"\n---\n";

/// The shipped stub, as if one release had already replaced `EARLIER_STUB`.
const STUBS: Stubs = Stubs {
    current: STUB,
    previous: &[EARLIER_STUB],
};

/// A home directory with `contents` as the skill in the shared directory.
fn home_with(contents: Option<&str>) -> (tempfile::TempDir, PathBuf) {
    let home = tempfile::tempdir().expect("temp dir");
    let slot = home.path().join(".agents/skills/linear-cli");
    if let Some(contents) = contents {
        fs::create_dir_all(&slot).expect("create");
        fs::write(slot.join("SKILL.md"), contents).expect("write");
    }
    (home, slot)
}

fn state_of(contents: &str) -> (State, Option<Reason>) {
    let (_home, slot) = home_with(Some(contents));
    let seen = inspect(&slot, STUBS);
    (seen.state, seen.reason)
}

#[test]
fn only_the_exact_bytes_of_a_stub_are_ours() {
    assert_eq!(state_of(STUB), (State::Current, None));
    assert_eq!(state_of(EARLIER_STUB), (State::Outdated, None));
    assert_eq!(
        state_of(&STUB.replace("Run `linear", "Always run `linear")),
        (State::Foreign, Some(Reason::EditedStub)),
        "an edit is the user's, and kept"
    );
    assert_eq!(
        state_of(OLD_SKILL),
        (State::Foreign, Some(Reason::OldSkill))
    );
    assert_eq!(
        state_of("---\nname: linear-cli\ndescription: Mine.\n---\n"),
        (State::Foreign, Some(Reason::Other))
    );
}

#[test]
fn a_slot_without_a_skill_file_is_missing_only_when_it_is_empty() {
    let (home, slot) = home_with(None);
    assert_eq!(inspect(&slot, STUBS).state, State::Missing);
    assert!(!inspect(&slot, STUBS).directory);

    fs::create_dir_all(&slot).expect("create");
    let empty = inspect(&slot, STUBS);
    assert_eq!((empty.state, empty.directory), (State::Missing, true));

    fs::write(slot.join("notes.md"), "mine").expect("write");
    assert_eq!(
        inspect(&slot, STUBS).reason,
        Some(Reason::NoSkillFile),
        "someone's files are not ours to write beside"
    );

    let file = home.path().join(".agents/skills/a-file");
    fs::write(&file, "").expect("write");
    assert_eq!(inspect(&file, STUBS).reason, Some(Reason::NotADirectory));
}

#[cfg(unix)]
#[test]
fn a_symlinked_slot_is_read_and_never_ours() {
    use std::os::unix::fs::symlink;
    let (home, slot) = home_with(None);
    let elsewhere = home.path().join("dotfiles/linear-cli");
    fs::create_dir_all(&elsewhere).expect("create");
    fs::create_dir_all(slot.parent().expect("skills")).expect("create");
    symlink(&elsewhere, &slot).expect("symlink");

    assert_eq!(
        inspect(&slot, STUBS).reason,
        Some(Reason::Symlink),
        "a dangling or empty link"
    );
    fs::write(elsewhere.join("SKILL.md"), STUB).expect("write");
    let linked = inspect(&slot, STUBS);
    assert_eq!((linked.state, linked.symlink), (State::Current, true));

    let layout = Layout::user(&env(home.path())).expect("layout");
    let steps = plan::remove(&layout, &agents(&Agent::ALL), STUBS);
    let shared = steps.first().expect("the shared slot");
    assert_eq!(shared.change, Change::Kept, "the link is someone else's");
}

// Planning and applying.

fn setup_steps(home: &Path, named: &[Agent], force: bool) -> (Layout, Vec<plan::Step>) {
    let layout = Layout::user(&env(home)).expect("layout");
    let steps = plan::setup(&layout, &layout.setup_agents(named), force, STUBS, "STAMP");
    (layout, steps)
}

fn changes(steps: &[plan::Step]) -> Vec<Change> {
    steps.iter().map(|step| step.change).collect()
}

#[test]
fn setup_creates_updates_and_then_leaves_alone() {
    let (home, slot) = home_with(None);
    let (_, mut steps) = setup_steps(home.path(), &[], false);
    assert_eq!(changes(&steps), [Change::Created]);
    assert!(!slot.exists(), "planning writes nothing");
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(changes(&steps), [Change::Created]);
    assert_eq!(
        fs::read_to_string(slot.join("SKILL.md")).expect("read"),
        STUB
    );

    let (_, steps) = setup_steps(home.path(), &[], false);
    assert_eq!(changes(&steps), [Change::Unchanged]);

    fs::write(slot.join("SKILL.md"), EARLIER_STUB).expect("write");
    let (_, mut steps) = setup_steps(home.path(), &[], false);
    assert_eq!(changes(&steps), [Change::Updated]);
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(
        fs::read_to_string(slot.join("SKILL.md")).expect("read"),
        STUB
    );
}

#[test]
fn a_foreign_skill_is_a_conflict_until_forced_into_a_backup() {
    let (home, slot) = home_with(Some(OLD_SKILL));
    fs::write(slot.join("mine.sh"), "echo mine").expect("write");

    let (_, mut steps) = setup_steps(home.path(), &[], false);
    assert_eq!(changes(&steps), [Change::Conflict]);
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(
        fs::read_to_string(slot.join("SKILL.md")).expect("read"),
        OLD_SKILL
    );

    let (_, mut steps) = setup_steps(home.path(), &[], true);
    assert_eq!(changes(&steps), [Change::Replaced]);
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(changes(&steps), [Change::Replaced]);
    let backup = home
        .path()
        .join(".agents/linear-cli-skill-backups/STAMP/linear-cli");
    assert_eq!(
        steps.first().and_then(|step| step.backup.clone()),
        Some(backup.clone())
    );
    assert_eq!(
        fs::read_to_string(backup.join("SKILL.md")).expect("read"),
        OLD_SKILL
    );
    assert_eq!(
        fs::read_to_string(backup.join("mine.sh")).expect("read"),
        "echo mine",
        "everything that was there is kept"
    );
    assert_eq!(
        fs::read_to_string(slot.join("SKILL.md")).expect("read"),
        STUB
    );
    let left: Vec<_> = fs::read_dir(backup.parent().expect("stamp"))
        .expect("list")
        .map(|entry| entry.expect("entry").file_name())
        .collect();
    assert_eq!(left, ["linear-cli"], "no staging directory is left behind");
}

#[test]
fn a_copy_in_an_agent_s_own_directory_is_a_conflict_and_force_only_moves_it() {
    let (home, _) = home_with(Some(STUB));
    let own = home.path().join(".pi/agent/skills/linear-cli");
    fs::create_dir_all(&own).expect("create");
    fs::write(own.join("SKILL.md"), OLD_SKILL).expect("write");

    let (_, steps) = setup_steps(home.path(), &[], false);
    assert_eq!(changes(&steps), [Change::Unchanged, Change::Conflict]);
    let (_, steps) = setup_steps(home.path(), &[Agent::Codex], false);
    assert_eq!(
        changes(&steps),
        [Change::Unchanged],
        "Pi's own directory does not matter to Codex"
    );

    let (_, mut steps) = setup_steps(home.path(), &[], true);
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(changes(&steps), [Change::Unchanged, Change::MovedAside]);
    assert!(!own.exists(), "nothing is installed in its place");
    assert!(
        home.path()
            .join(".pi/agent/linear-cli-skill-backups/STAMP/linear-cli/SKILL.md")
            .exists()
    );
}

#[test]
fn a_slot_that_changed_after_planning_is_left_alone() {
    // Someone installs a skill where there was none.
    let (home, slot) = home_with(None);
    let (_, mut steps) = setup_steps(home.path(), &[], false);
    fs::create_dir_all(&slot).expect("create");
    fs::write(slot.join("SKILL.md"), "theirs").expect("write");
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(changes(&steps), [Change::Conflict]);
    assert_eq!(
        fs::read_to_string(slot.join("SKILL.md")).expect("read"),
        "theirs"
    );

    // Someone edits a stub that was about to be updated.
    let (home, slot) = home_with(Some(EARLIER_STUB));
    let (_, mut steps) = setup_steps(home.path(), &[], false);
    fs::write(slot.join("SKILL.md"), "edited").expect("write");
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(changes(&steps), [Change::Conflict]);
    assert_eq!(
        fs::read_to_string(slot.join("SKILL.md")).expect("read"),
        "edited"
    );
}

#[test]
fn an_earlier_backup_is_never_reused() {
    let (home, _) = home_with(Some(OLD_SKILL));
    let earlier = home
        .path()
        .join(".agents/linear-cli-skill-backups/STAMP/linear-cli");
    fs::create_dir_all(&earlier).expect("create");
    fs::write(earlier.join("SKILL.md"), "an earlier backup").expect("write");

    let (_, mut steps) = setup_steps(home.path(), &[], true);
    apply::apply(&mut steps, STUBS, &RealFiles);
    let backup = home
        .path()
        .join(".agents/linear-cli-skill-backups/STAMP-2/linear-cli");
    assert_eq!(
        steps.first().and_then(|step| step.backup.clone()),
        Some(backup.clone()),
        "the report names where it really went"
    );
    assert_eq!(
        fs::read_to_string(backup.join("SKILL.md")).expect("read"),
        OLD_SKILL
    );
    assert_eq!(
        fs::read_to_string(earlier.join("SKILL.md")).expect("read"),
        "an earlier backup"
    );
}

/// The real filesystem, except that the rename numbered `fail_at` fails.
struct FailingRename {
    fail_at: usize,
    renames: Cell<usize>,
}

impl Files for FailingRename {
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        RealFiles.create_dir_all(path)
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        RealFiles.create_dir(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        RealFiles.write(path, contents)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        self.renames.set(self.renames.get() + 1);
        if self.renames.get() == self.fail_at {
            Err(io::Error::other("the disk said no"))
        } else {
            RealFiles.rename(from, to)
        }
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        RealFiles.remove_file(path)
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        RealFiles.remove_dir(path)
    }
}

/// A home whose shared and Claude Code slots both hold the old skill.
fn two_old_skills() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let (home, shared) = home_with(Some(OLD_SKILL));
    let claude = home.path().join(".claude/skills/linear-cli");
    fs::create_dir_all(&claude).expect("create");
    fs::write(claude.join("SKILL.md"), OLD_SKILL).expect("write");
    (home, shared, claude)
}

#[test]
fn a_failed_move_leaves_the_old_skill_in_place_and_stops() {
    let (home, shared, claude) = two_old_skills();
    let (_, mut steps) = setup_steps(home.path(), &[], true);
    let files = FailingRename {
        fail_at: 1,
        renames: Cell::new(0),
    };
    apply::apply(&mut steps, STUBS, &files);

    assert_eq!(changes(&steps), [Change::Failed, Change::NotRun]);
    assert!(
        steps.iter().all(|step| step.backup.is_none()),
        "nothing was moved, so no backup is reported"
    );
    for slot in [&shared, &claude] {
        assert_eq!(
            fs::read_to_string(slot.join("SKILL.md")).expect("read"),
            OLD_SKILL
        );
    }
    assert!(
        !home
            .path()
            .join(".agents/linear-cli-skill-backups/STAMP")
            .exists(),
        "an unused backup directory is not left behind"
    );
}

#[test]
fn a_failed_install_after_the_move_keeps_the_backup_and_says_where() {
    let (home, shared, _claude) = two_old_skills();
    let (_, mut steps) = setup_steps(home.path(), &[], true);
    let files = FailingRename {
        fail_at: 2,
        renames: Cell::new(0),
    };
    apply::apply(&mut steps, STUBS, &files);

    assert_eq!(changes(&steps), [Change::Failed, Change::NotRun]);
    let backup = home
        .path()
        .join(".agents/linear-cli-skill-backups/STAMP/linear-cli");
    assert_eq!(
        fs::read_to_string(backup.join("SKILL.md")).expect("read"),
        OLD_SKILL
    );
    let backups: Vec<_> = steps.iter().map(|step| step.backup.clone()).collect();
    assert_eq!(
        backups,
        [Some(backup.clone()), None],
        "only the move that happened is reported"
    );
    // The note names the backup as the step reports it, whatever separators
    // the platform prints.
    let (note, reported) = steps
        .first()
        .and_then(|step| step.note.clone().zip(step.backup.clone()))
        .expect("a note and a backup");
    assert!(note.contains(&reported.display().to_string()), "{note}");

    // Running it again finishes: the slot is now empty, so it is created.
    assert!(!shared.exists());
    let (_, mut steps) = setup_steps(home.path(), &[], true);
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(changes(&steps), [Change::Created, Change::Replaced]);
}

#[test]
fn remove_deletes_only_the_stub() {
    let (home, shared) = home_with(Some(STUB));
    fs::write(shared.join("notes.md"), "mine").expect("write");
    let claude = home.path().join(".claude/skills/linear-cli");
    fs::create_dir_all(&claude).expect("create");
    fs::write(claude.join("SKILL.md"), STUB).expect("write");
    let own = home.path().join(".codex/skills/linear-cli");
    fs::create_dir_all(&own).expect("create");
    fs::write(own.join("SKILL.md"), OLD_SKILL).expect("write");

    let layout = Layout::user(&env(home.path())).expect("layout");
    let mut steps = plan::remove(&layout, &agents(&Agent::ALL), STUBS);
    apply::apply(&mut steps, STUBS, &RealFiles);
    assert_eq!(
        changes(&steps),
        [Change::Removed, Change::Removed, Change::Kept]
    );
    assert!(!shared.join("SKILL.md").exists());
    assert!(shared.join("notes.md").exists(), "other files are kept");
    assert!(!claude.exists(), "an emptied directory goes too");
    assert!(own.join("SKILL.md").exists(), "a foreign skill is kept");

    let steps = plan::remove(&layout, &agents(&[Agent::Claude]), STUBS);
    assert_eq!(changes(&steps), [Change::Absent]);
}
