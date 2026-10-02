//! Searchable selection state over display labels and stable values.
//!
//! The command owns data fetching, ordering, and label construction; the
//! prompt feeds key presses into a [`Selector`] and renders it.

use std::io;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SelectOption {
    pub label: String,
    pub value: String,
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
