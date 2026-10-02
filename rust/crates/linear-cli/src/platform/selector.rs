//! Searchable terminal selection over display labels and stable values.
//!
//! The command owns data fetching, ordering, and label construction. This
//! module owns only terminal interaction. It reads stdin and writes stdout;
//! the caller must check both TTYs and CI before fetching picker data.

use std::io::{self, IsTerminal, Write};
use unicode_width::UnicodeWidthChar;

use crate::error::{AppError, AppErrorKind};
use crate::platform::output::{Output, Stream};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectOption {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PromptLabels<'a> {
    pub message: &'a str,
    pub search_label: &'a str,
    pub max_rows: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Selection {
    Selected(String),
    Interrupted,
    EndOfInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Character(char),
    Backspace,
    Up,
    Down,
    Enter,
    Interrupt,
    EndOfInput,
    Other,
}

/// CI is active for every nonempty value other than the literal `false`.
pub fn interactive_allowed(stdin_tty: bool, stdout_tty: bool, ci: Option<&str>) -> bool {
    stdin_tty && stdout_tty && !ci.is_some_and(|value| !value.is_empty() && value != "false")
}

#[derive(Debug)]
pub struct Selector<'a> {
    options: &'a [SelectOption],
    query: String,
    filtered: Vec<&'a SelectOption>,
    active: usize,
}

impl<'a> Selector<'a> {
    pub fn new(options: &'a [SelectOption]) -> io::Result<Self> {
        if options.is_empty() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "selector requires at least one option",
            ));
        }
        for (index, option) in options.iter().enumerate() {
            if display_label(&option.label).trim().is_empty() {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("selection option {index} has an invalid label"),
                ));
            }
            if option.value.trim().is_empty() || option.value.chars().any(char::is_control) {
                return Err(io::Error::new(
                    io::ErrorKind::InvalidInput,
                    format!("selection option {index} has an invalid value"),
                ));
            }
        }
        Ok(Self {
            options,
            query: String::new(),
            filtered: options.iter().collect(),
            active: 0,
        })
    }

    pub fn query(&self) -> &str {
        &self.query
    }

    pub fn visible(&self) -> impl Iterator<Item = &SelectOption> {
        self.filtered.iter().copied()
    }

    pub fn active_value(&self) -> Option<&str> {
        self.filtered
            .get(self.active)
            .map(|option| option.value.as_str())
    }

    /// Expose the owned selector cursor for PromptSession's escaped renderer.
    /// Matching, ranking, option validation and legacy standalone callers stay unchanged.
    pub fn active_index(&self) -> usize {
        self.active
    }

    pub fn active_label(&self) -> Option<&str> {
        self.filtered
            .get(self.active)
            .map(|option| option.label.as_str())
    }

    pub fn on_key(&mut self, key: Key) -> Option<Selection> {
        match key {
            Key::Character(character) if !character.is_control() => {
                self.query.push(character);
                self.filter();
            }
            Key::Backspace => {
                self.query.pop();
                self.filter();
            }
            Key::Up => {
                if !self.filtered.is_empty() {
                    self.active = self
                        .active
                        .checked_sub(1)
                        .unwrap_or(self.filtered.len() - 1);
                }
            }
            Key::Down => {
                if !self.filtered.is_empty() {
                    self.active = (self.active + 1) % self.filtered.len();
                }
            }
            Key::Enter => {
                if let Some(value) = self.active_value() {
                    return Some(Selection::Selected(value.to_owned()));
                }
            }
            Key::Character('\u{4}') => return Some(Selection::EndOfInput),
            Key::Interrupt => return Some(Selection::Interrupted),
            Key::EndOfInput => return Some(Selection::EndOfInput),
            Key::Character(_) | Key::Other => {}
        }
        None
    }

    fn filter(&mut self) {
        let needle = self.query.to_lowercase();
        if needle.is_empty() {
            self.filtered = self.options.iter().collect();
            self.active = self.active.min(self.filtered.len().saturating_sub(1));
            return;
        }
        let mut matched: Vec<(&SelectOption, usize)> = self
            .options
            .iter()
            .filter_map(|option| {
                let label = display_label(&option.label);
                if label.to_lowercase().contains(&needle)
                    || (option.label != option.value
                        && option.value.to_lowercase().contains(&needle))
                {
                    Some((option, levenshtein(&label, &needle)))
                } else {
                    None
                }
            })
            .collect();
        matched.sort_by_key(|(_, distance)| *distance);
        self.filtered = matched.into_iter().map(|(option, _)| option).collect();
        self.active = self.active.min(self.filtered.len().saturating_sub(1));
    }
}

/// Strip CSI and OSC terminal controls from labels before matching.
fn strip_ansi(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();
    while let Some(character) = chars.next() {
        if character != '\u{1b}' {
            result.push(character);
            continue;
        }
        match chars.next() {
            Some('[') => {
                for part in chars.by_ref() {
                    if ('@'..='~').contains(&part) {
                        break;
                    }
                }
            }
            Some(']') => {
                let mut escaped = false;
                for part in chars.by_ref() {
                    if part == '\u{7}' || (escaped && part == '\\') {
                        break;
                    }
                    escaped = part == '\u{1b}';
                }
            }
            Some(_) | None => {}
        }
    }
    result
}

fn display_label(input: &str) -> String {
    strip_ansi(input)
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect()
}

fn levenshtein(left: &str, right: &str) -> usize {
    let right: Vec<char> = right.chars().collect();
    let mut previous: Vec<usize> = (0..=right.len()).collect();
    let mut last_cell = right.len();
    for (row, character) in left.chars().enumerate() {
        let mut current = Vec::with_capacity(right.len() + 1);
        last_cell = row + 1;
        current.push(last_cell);
        for ((diagonal, above), other) in previous
            .iter()
            .zip(previous.iter().skip(1))
            .zip(right.iter())
        {
            let insertion = last_cell + 1;
            let deletion = *above + 1;
            let substitution = *diagonal + usize::from(character != *other);
            last_cell = insertion.min(deletion).min(substitution);
            current.push(last_cell);
        }
        previous = current;
    }
    last_cell
}

fn clip_line(text: &str, limit: usize) -> String {
    let width: usize = text
        .chars()
        .map(|character| character.width().unwrap_or(0))
        .sum();
    if width <= limit {
        return text.to_owned();
    }
    let suffix = if limit >= 4 { "..." } else { "" };
    let available = limit - suffix.len();
    let mut result = String::new();
    let mut used = 0;
    for character in text.chars() {
        let next = character.width().unwrap_or(0);
        if used + next > available {
            break;
        }
        result.push(character);
        used += next;
    }
    result.push_str(suffix);
    result
}

/// Render the current frame without taking over the alternate screen.
fn frame(selector: &Selector<'_>, labels: &PromptLabels<'_>, columns: usize) -> String {
    // Leave the final terminal column unused so every explicit newline is one
    // physical row. Then the redraw count cannot be upset by implicit wraps.
    let header = format!(
        "? {}  {}: {}\n",
        labels.message,
        labels.search_label,
        selector.query()
    );
    let mut rendered = format!(
        "{}\n",
        clip_line(header.trim_end_matches('\n'), columns - 1)
    );
    if selector.filtered.is_empty() {
        rendered.push_str(&format!("{}\n", clip_line("  No matches", columns - 1)));
    } else {
        let visible_rows = labels.max_rows;
        let start = selector.active.saturating_sub(visible_rows - 1);
        for (offset, option) in selector
            .visible()
            .skip(start)
            .take(visible_rows)
            .enumerate()
        {
            let marker = if start + offset == selector.active {
                '❯'
            } else {
                ' '
            };
            rendered.push_str(&format!(
                "{marker} {}\n",
                clip_line(&display_label(&option.label), columns - 3)
            ));
        }
    }
    rendered
}

/// Drive one selection with an injectable key source and stdout writer.
/// EOF is an explicit outcome rather than a re-prompt.
pub fn run_with(
    options: &[SelectOption],
    labels: &PromptLabels<'_>,
    columns: usize,
    mut next_key: impl FnMut() -> io::Result<Key>,
    stdout: &mut dyn Write,
) -> Result<Selection, AppError> {
    if labels.message.trim().is_empty()
        || labels.search_label.trim().is_empty()
        || labels.message.chars().any(char::is_control)
        || labels.search_label.chars().any(char::is_control)
        || labels.max_rows == 0
        || columns < 4
    {
        return Err(AppError::new(
            AppErrorKind::Invariant,
            "selector prompt labels, row limit, and width must be valid",
        ));
    }
    let mut selector = Selector::new(options).map_err(|error| {
        AppError::new(AppErrorKind::Invariant, error.to_string()).with_source(error)
    })?;
    let mut output = Output::new(stdout, Stream::Stdout);
    let mut previous_lines = 0;
    loop {
        if previous_lines > 0 {
            output.write(format!("\x1b[{previous_lines}A\r\x1b[J").as_bytes())?;
        }
        let rendered = frame(&selector, labels, columns);
        output.write(rendered.as_bytes())?;
        previous_lines = rendered.lines().count();
        let key = next_key().map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "failed to read selector input")
                .with_source(error)
        })?;
        let outcome = selector.on_key(key);
        if let Some(outcome) = outcome {
            output.write(format!("\x1b[{previous_lines}A\r\x1b[J").as_bytes())?;
            if let Selection::Selected(_) = &outcome
                && let Some(label) = selector.active_label()
            {
                output.write(
                    format!("? {} › {}\n", labels.message, display_label(label)).as_bytes(),
                )?;
            }
            return Ok(outcome);
        }
    }
}

#[cfg(unix)]
struct RawPrompt {
    input: io::Stdin,
    original: rustix::termios::Termios,
    restore_attempted: bool,
}

#[cfg(unix)]
impl RawPrompt {
    fn enter() -> Result<Self, AppError> {
        use rustix::termios::{OptionalActions, tcgetattr, tcsetattr};

        let input = io::stdin();
        let original = tcgetattr(&input).map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "failed to read terminal mode")
                .with_source(error)
        })?;
        let mut raw = original.clone();
        raw.make_raw();
        // Keep the terminal's newline translation while keys stay raw.
        raw.output_modes = original.output_modes;
        tcsetattr(&input, OptionalActions::Now, &raw).map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "failed to enable terminal input")
                .with_source(error)
        })?;
        Ok(Self {
            input,
            original,
            restore_attempted: false,
        })
    }

    fn restore(&mut self) -> Result<(), AppError> {
        // A failed explicit restore is already returned to the caller. Do not
        // retry in Drop and report a second, unrelated error to stderr.
        self.restore_attempted = true;
        rustix::termios::tcsetattr(
            &self.input,
            rustix::termios::OptionalActions::Now,
            &self.original,
        )
        .map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "failed to restore terminal input")
                .with_source(error)
        })?;
        Ok(())
    }
}

#[cfg(unix)]
impl Drop for RawPrompt {
    fn drop(&mut self) {
        if !self.restore_attempted
            && let Err(error) = self.restore()
        {
            let _ = writeln!(io::stderr(), "failed to restore terminal input: {error}");
        }
    }
}

/// Read keys from stdin while the caller supplies the stdout writer.
/// On Unix the prompt holds raw input mode throughout every redraw; the guard
/// restores the original mode even when reading or writing fails.
pub fn run(
    options: &[SelectOption],
    labels: &PromptLabels<'_>,
    ci: Option<&str>,
    stdout: &mut dyn Write,
) -> Result<Selection, AppError> {
    if !interactive_allowed(io::stdin().is_terminal(), io::stdout().is_terminal(), ci) {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "interactive selection requires a terminal outside CI",
        ));
    }
    let terminal = console::Term::stdout();
    let (columns, rows) = terminal_size::terminal_size_of(io::stdout())
        .map(
            |(terminal_size::Width(width), terminal_size::Height(height))| {
                (usize::from(width), usize::from(height))
            },
        )
        .unwrap_or((80, 24));
    if columns < 4 || rows < 3 {
        return Err(AppError::new(
            AppErrorKind::Validation,
            "interactive selection requires a terminal at least 4 columns wide and 3 rows high",
        ));
    }
    let mut labels = labels.clone();
    labels.max_rows = labels.max_rows.min(rows - 2);
    #[cfg(unix)]
    let mut raw_prompt = RawPrompt::enter()?;
    let selection = run_with(
        options,
        &labels,
        columns,
        || match terminal.read_key_raw() {
            Ok(console::Key::Char(character)) => Ok(Key::Character(character)),
            Ok(console::Key::Backspace) => Ok(Key::Backspace),
            Ok(console::Key::ArrowUp) => Ok(Key::Up),
            Ok(console::Key::ArrowDown) => Ok(Key::Down),
            Ok(console::Key::Enter) => Ok(Key::Enter),
            Ok(console::Key::CtrlC) => Ok(Key::Interrupt),
            Ok(_) => Ok(Key::Other),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(Key::EndOfInput),
            Err(error) => Err(error),
        },
        stdout,
    );
    #[cfg(unix)]
    {
        let cleanup = raw_prompt.restore();
        match (selection, cleanup) {
            (Ok(value), Ok(())) => Ok(value),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(mut error), Err(cleanup)) => {
                error.message.push_str(&format!(
                    "; terminal cleanup also failed: {}",
                    cleanup.display_message()
                ));
                Err(error)
            }
        }
    }
    #[cfg(not(unix))]
    selection
}
