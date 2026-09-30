//! Static fish completion script over the completion view of the clap tree.
//!
//! `clap_complete`'s fish generator stops before the third command level and
//! recognizes a nested command with `__fish_seen_subcommand_from`, which
//! matches a word anywhere on the line. This generator instead emits a
//! table-driven helper that walks the words before the cursor through the
//! same tree the parser uses, so every completion is conditioned on one exact
//! command path at any depth.

use clap::{Arg, Command};

use crate::error::{AppError, AppErrorKind};

fn invariant(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Invariant, message)
}

/// Command names, aliases, flags and enum values are embedded in `switch`
/// patterns, `-a` word lists and conditions, so only plain words are accepted.
fn plain_word<'a>(kind: &str, word: &'a str) -> Result<&'a str, AppError> {
    let mut chars = word.chars();
    let plain = chars
        .next()
        .is_some_and(|first| first.is_ascii_alphanumeric())
        && chars.all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '_' | '-' | '.'));
    if plain {
        Ok(word)
    } else {
        Err(invariant(format!(
            "fish completion {kind} is not a plain word: {word:?}"
        )))
    }
}

/// Text inside fish single quotes, on one line.
fn quoted(text: &str) -> String {
    format!(
        "'{}'",
        text.replace('\n', " ")
            .replace('\\', "\\\\")
            .replace('\'', "\\'")
    )
}

/// One visible command and its full path of canonical names from the root.
struct State<'a> {
    path: String,
    command: &'a Command,
}

fn collect_states<'a>(
    command: &'a Command,
    path: String,
    states: &mut Vec<State<'a>>,
) -> Result<(), AppError> {
    if command.is_hide_set() {
        return Err(invariant("fish completion does not accept hidden commands"));
    }
    let children = command.get_subcommands().collect::<Vec<_>>();
    states.push(State {
        path: path.clone(),
        command,
    });
    for child in children {
        let name = plain_word("command", child.get_name())?;
        collect_states(child, format!("{path} {name}"), states)?;
    }
    Ok(())
}

fn options(command: &Command) -> impl Iterator<Item = &Arg> {
    command.get_arguments().filter(|arg| !arg.is_positional())
}

fn takes_value(arg: &Arg) -> Result<bool, AppError> {
    arg.get_num_args()
        .map(|range| range.takes_values())
        .ok_or_else(|| invariant(format!("unbuilt fish completion argument {}", arg.get_id())))
}

/// One spelling of an option on a command line.
enum Spelling<'a> {
    Short(char),
    Long(&'a str),
}

impl Spelling<'_> {
    fn flag(&self) -> String {
        match self {
            Self::Short(short) => format!("-{short}"),
            Self::Long(long) => format!("--{long}"),
        }
    }

    fn fish_option(&self) -> String {
        match self {
            Self::Short(short) => format!(" -s {short}"),
            Self::Long(long) => format!(" -l {long}"),
        }
    }
}

/// Every spelling of an option, shorts first.
fn spellings(arg: &Arg) -> Result<Vec<Spelling<'_>>, AppError> {
    let mut spellings = Vec::new();
    for short in arg.get_short_and_visible_aliases().unwrap_or_default() {
        if !short.is_ascii_alphanumeric() {
            return Err(invariant(format!("fish completion short flag -{short}")));
        }
        spellings.push(Spelling::Short(short));
    }
    for long in arg.get_long_and_visible_aliases().unwrap_or_default() {
        spellings.push(Spelling::Long(plain_word("long flag", long)?));
    }
    if spellings.is_empty() {
        return Err(invariant(format!(
            "fish completion option {} has no flag",
            arg.get_id()
        )));
    }
    Ok(spellings)
}

/// A parent's words are option values or command names, so its valued options
/// must consume exactly one word. Leaf option spellings also belong in the
/// table: a short option's attached value ends its cluster, even though a leaf
/// has no further command words to select.
fn value_spellings(state: &State<'_>) -> Result<Vec<String>, AppError> {
    let parent = state.command.has_subcommands();
    if parent && state.command.get_positionals().next().is_some() {
        return Err(invariant(format!(
            "fish completion parent {} has positionals",
            state.path
        )));
    }
    let mut result = Vec::new();
    for arg in options(state.command) {
        if !takes_value(arg)? {
            continue;
        }
        let range = arg
            .get_num_args()
            .ok_or_else(|| invariant("unbuilt fish completion argument"))?;
        if parent && (range.min_values() != 1 || range.max_values() != 1) {
            return Err(invariant(format!(
                "fish completion parent {} has a multi-valued option {}",
                state.path,
                arg.get_id()
            )));
        }
        result.extend(spellings(arg)?.iter().map(Spelling::flag));
    }
    Ok(result)
}

fn helpers(function: &str, root: &str, states: &[State<'_>]) -> Result<String, AppError> {
    let mut parents = Vec::new();
    let mut children = String::new();
    let mut values = Vec::new();
    let mut known_options = Vec::new();
    for state in states {
        for arg in options(state.command) {
            for spelling in spellings(arg)? {
                known_options.push(quoted(&format!("{}:{}", state.path, spelling.flag())));
            }
        }
        for spelling in value_spellings(state)? {
            values.push(quoted(&format!("{}:{spelling}", state.path)));
        }
        if !state.command.has_subcommands() {
            continue;
        }
        parents.push(quoted(&state.path));
        for child in state.command.get_subcommands() {
            let child_path = format!("{} {}", state.path, child.get_name());
            let mut patterns = Vec::new();
            for word in child.get_name_and_visible_aliases() {
                patterns.push(quoted(&format!(
                    "{}:{}",
                    state.path,
                    plain_word("command", word)?
                )));
            }
            children.push_str(&format!(
                "        case {}\n            echo {}\n",
                patterns.join(" "),
                quoted(&child_path)
            ));
        }
    }
    let value_cases = if values.is_empty() {
        String::new()
    } else {
        format!(
            "    switch \"$argv[1]:$argv[2]\"\n        case {}\n            return 0\n    end\n",
            values.join(" ")
        )
    };
    let known_option_cases = if known_options.is_empty() {
        String::new()
    } else {
        format!(
            "    switch \"$argv[1]:$argv[2]\"\n        case {}\n            return 0\n    end\n",
            known_options.join(" ")
        )
    };
    Ok(format!(
        r#"# Succeed when the option spelling $argv[2] takes a value under the parent
# command path $argv[1].
function __fish_{function}_takes_value
{value_cases}    return 1
end

# Succeed only for an option spelling registered on the selected command.
function __fish_{function}_known_option
{known_option_cases}    return 1
end

# Print the command path that the word $argv[2] selects under $argv[1], a
# canonical name or visible alias; fail for any other word.
function __fish_{function}_subcommand
    switch "$argv[1]:$argv[2]"
{children}        case '*'
            return 1
    end
end

# Walk the words before the cursor through the command tree and print the
# selected command path. Option values are skipped, `--` ends command
# selection, and an unknown word under a parent selects nothing.
function __fish_{function}_command_path
    set -l words (commandline -opc)
    set -e words[1]
    set -l path {root}
    set -l value_next 0
    for word in $words
        if test $value_next -eq 1
            set value_next 0
            continue
        end
        switch $word
            case '--'
                return 1
            case '--*=*'
                set -l option (string replace -r '=.*$' '' -- $word)
                __fish_{function}_known_option $path $option
                or return 1
            case '--*'
                __fish_{function}_known_option $path $word
                or return 1
                __fish_{function}_takes_value $path $word
                and set value_next 1
            case '-?*'
                # A short cluster: a valued letter takes the rest of the word,
                # or the next word when it is last.
                set -l letters (string sub --start 2 -- $word | string split '')
                while set -q letters[1]
                    set -l letter $letters[1]
                    set -e letters[1]
                    __fish_{function}_known_option $path -$letter
                    or return 1
                    if __fish_{function}_takes_value $path -$letter
                        set -q letters[1]
                        or set value_next 1
                        break
                    end
                end
            case '*'
                contains -- $path {parents}
                or continue
                set path (__fish_{function}_subcommand $path $word)
                or return 1
        end
    end
    echo $path
end

# Succeed when the words before the cursor select exactly the path $argv[1].
# Every entry asks, so the walk is cached per escaped word list.
function __fish_{function}_using_command
    set -l key (commandline -opc | string escape | string join ' ')
    if not set -q __fish_{function}_command_key; or test "$key" != "$__fish_{function}_command_key"
        set -g __fish_{function}_command_key $key
        set -g __fish_{function}_command (__fish_{function}_command_path)
    end
    test "$__fish_{function}_command" = "$argv[1]"
end

"#,
        root = quoted(root),
        parents = parents.join(" "),
        known_option_cases = known_option_cases,
    ))
}

fn value_completion(arg: &Arg) -> Result<String, AppError> {
    if !takes_value(arg)? {
        return Ok(String::new());
    }
    let mut words = Vec::new();
    for value in arg.get_possible_values() {
        if value.is_hide_set() {
            return Err(invariant(format!(
                "fish completion option {} has a hidden value",
                arg.get_id()
            )));
        }
        words.push(plain_word("value", value.get_name())?.to_owned());
    }
    Ok(if words.is_empty() {
        " -r".to_owned()
    } else {
        // An empty `\t''` description keeps fish from repeating the option's
        // description beside every value.
        format!(
            " -r -f -a \"{}\"",
            words
                .iter()
                .map(|word| format!("{word}\\t''"))
                .collect::<Vec<_>>()
                .join(" ")
        )
    })
}

fn state_lines(bin_name: &str, function: &str, state: &State<'_>) -> Result<String, AppError> {
    let base = format!(
        "complete -c {bin_name} -n \"__fish_{function}_using_command {}\"",
        quoted(&state.path)
    );
    let mut lines = String::new();
    let mut push = |line: String| {
        lines.push_str(&line);
        lines.push('\n');
    };
    if state.command.get_positionals().next().is_none() {
        push(format!("{base} -f"));
    }
    for arg in options(state.command) {
        if arg.is_hide_set() {
            return Err(invariant("fish completion does not accept hidden options"));
        }
        let mut line = base.clone();
        for spelling in spellings(arg)? {
            line.push_str(&spelling.fish_option());
        }
        let help = arg.get_help().ok_or_else(|| {
            invariant(format!(
                "fish completion option {} on {} has no description",
                arg.get_id(),
                state.path
            ))
        })?;
        line.push_str(&format!(" -d {}", quoted(&help.to_string())));
        line.push_str(&value_completion(arg)?);
        push(line);
    }
    for child in state.command.get_subcommands() {
        let about = child.get_about().ok_or_else(|| {
            invariant(format!(
                "fish completion command {} {} has no description",
                state.path,
                child.get_name()
            ))
        })?;
        for word in child.get_name_and_visible_aliases() {
            push(format!(
                "{base} -a {} -d {}",
                plain_word("command", word)?,
                quoted(&about.to_string())
            ));
        }
    }
    Ok(lines)
}

/// Generate the fish script registering completions for `bin_name`.
pub fn script(mut command: Command, bin_name: &str) -> Result<Vec<u8>, AppError> {
    let valid_name = !bin_name.starts_with(['-', '.'])
        && bin_name
            .chars()
            .all(|letter| letter.is_ascii_alphanumeric() || matches!(letter, '_' | '-' | '.'));
    if bin_name.is_empty() || !valid_name {
        return Err(invariant(format!(
            "fish completion bin name is not a plain word: {bin_name:?}"
        )));
    }
    command.build();
    let function = bin_name.replace(['-', '.'], "_");
    let root = plain_word("command", command.get_name())?.to_owned();
    let mut states = Vec::new();
    collect_states(&command, root.clone(), &mut states)?;
    let mut script = format!(
        "# fish completion for {bin_name}, generated from its command tree.\n\n{}",
        helpers(&function, &root, &states)?
    );
    for state in &states {
        script.push_str(&state_lines(bin_name, &function, state)?);
    }
    Ok(script.into_bytes())
}
