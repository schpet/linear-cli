//! Interactive questions on the terminal, drawn on stderr.
//!
//! Every prompt needs a terminal. Commands get a [`Prompter`] from
//! `Ctx::prompter`, which refuses when stdin is not one, and offer flags for
//! scripted use instead. Esc, Ctrl-C or Ctrl-G at any prompt cancels the
//! command, leaving the question on screen without the rest of the prompt.
//! A spinner left running hides while a prompt is open.
//!
//! Text answers and yes/no questions take the usual readline editing keys
//! (see [`line_input`]). Select lists come from inquire, whose filter knows
//! only the arrow keys, Home, End, Backspace and Delete: other control keys
//! there type their letter.
use std::fmt;
use std::io::{self, Write};

use console::Style;
use crossterm::{cursor, queue, terminal};
use inquire::InquireError;
use inquire::ui::{RenderConfig, Styled};
use unicode_width::UnicodeWidthStr;

use crate::error::{Error, Result};
use crate::platform::line_input::{self, LinePrompt};
use crate::platform::output::Stdout;
use crate::platform::spinner;

/// The rows a select list shows at once (inquire's default).
const PAGE_SIZE: usize = 7;

const SELECT_HELP: &str = "↑↓ to move, enter to select, type to filter";
const MULTI_SELECT_HELP: &str = "↑↓ to move, space to toggle, → all, ← none, type to filter";

pub struct Prompter<'a> {
    stdout: &'a Stdout,
    color: bool,
}

impl<'a> Prompter<'a> {
    /// `stdout` is flushed before each question so earlier output shows
    /// above it; `color` is whether stderr may be colored.
    pub fn new(stdout: &'a Stdout, color: bool) -> Self {
        Self { stdout, color }
    }

    pub fn text(&self, text: Text<'_>) -> Result<String> {
        self.stdout.flush()?;
        let _hold = spinner::hold();
        let message = escape(text.message);
        let placeholder = text.default.map(escape);
        let check = |raw: &str| text.answer(raw).map(drop);
        let format = |raw: &str| escape(&text.answer(raw).unwrap_or_default());
        let raw = line_input::ask(&LinePrompt {
            message: &message,
            hint: None,
            placeholder: placeholder.as_deref(),
            help: None,
            masked: false,
            color: self.color,
            check: &check,
            format: &format,
        })?
        .ok_or_else(Error::cancelled)?;
        Ok(text
            .answer(&raw)
            .expect("the prompt only accepts valid answers"))
    }

    /// The value `parse` makes of a nonblank answer to `text`, asking again
    /// with the parse error until one parses; `None` for a blank answer
    /// (possible only when `text` is neither required nor defaulted).
    pub fn parsed<T>(
        &self,
        text: Text<'_>,
        parse: &dyn Fn(&str) -> std::result::Result<T, String>,
    ) -> Result<Option<T>> {
        let check = |raw: &str| parse(raw).map(drop);
        let answer = self.text(text.with_check(&check))?;
        if answer.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            parse(&answer).expect("the prompt only accepts answers that parse"),
        ))
    }

    /// A masked answer, such as an API key. It is trimmed.
    pub fn secret(&self, message: &str, help: &str) -> Result<String> {
        self.stdout.flush()?;
        let _hold = spinner::hold();
        let message = escape(message);
        let answer = line_input::ask(&LinePrompt {
            message: &message,
            hint: None,
            placeholder: None,
            help: Some(help),
            masked: true,
            color: self.color,
            check: &|_| Ok(()),
            format: &str::to_owned,
        })?
        .ok_or_else(Error::cancelled)?;
        Ok(answer.trim().to_owned())
    }

    /// A yes/no question; a blank answer takes `default`.
    pub fn confirm(&self, message: &str, default: bool) -> Result<bool> {
        self.stdout.flush()?;
        let _hold = spinner::hold();
        let message = escape(message);
        let check = |raw: &str| yes_no(raw, default).map(drop);
        let format = |raw: &str| {
            let yes = yes_no(raw, default).expect("only accepted answers are shown");
            if yes { "Yes" } else { "No" }.to_owned()
        };
        let raw = line_input::ask(&LinePrompt {
            message: &message,
            hint: Some(if default { "(Y/n)" } else { "(y/N)" }),
            placeholder: None,
            help: None,
            masked: false,
            color: self.color,
            check: &check,
            format: &format,
        })?
        .ok_or_else(Error::cancelled)?;
        Ok(yes_no(&raw, default).expect("the prompt only accepts y or n"))
    }

    /// Picks one choice; typing filters the list.
    pub fn select<T>(&self, message: &str, choices: Vec<Choice<T>>) -> Result<T> {
        self.select_from(message, choices, 0)
    }

    /// Like [`Prompter::select`], starting on the choice at `start`.
    pub fn select_from<T>(
        &self,
        message: &str,
        choices: Vec<Choice<T>>,
        start: usize,
    ) -> Result<T> {
        assert!(!choices.is_empty(), "a select prompt needs choices");
        assert!(start < choices.len(), "the starting choice is in the list");
        let message = escape(message);
        let rows = list_rows(choices.len(), SELECT_HELP);
        let choice = self.ask(&message, rows, || {
            inquire::Select::new(&message, choices)
                .with_render_config(self.render_config())
                .with_scorer(&score)
                .with_page_size(PAGE_SIZE)
                .with_starting_cursor(start)
                .with_help_message(SELECT_HELP)
                .prompt()
        })?;
        Ok(choice.value)
    }

    /// Picks any number of choices, returned in list order; typing filters
    /// the list. Space toggles a choice, so the filter ignores the spaces in
    /// labels: `qafeedback` finds "QA feedback".
    pub fn multi_select<T>(&self, message: &str, choices: Vec<Choice<T>>) -> Result<Vec<T>> {
        if choices.is_empty() {
            return Ok(Vec::new());
        }
        let message = escape(message);
        let rows = list_rows(choices.len(), MULTI_SELECT_HELP);
        let picked = self.ask(&message, rows, || {
            inquire::MultiSelect::new(&message, choices)
                .with_render_config(self.render_config())
                .with_scorer(&score)
                .with_page_size(PAGE_SIZE)
                .with_help_message(MULTI_SELECT_HELP)
                .prompt()
        })?;
        Ok(picked.into_iter().map(|choice| choice.value).collect())
    }

    /// Runs an inquire prompt that draws `message` and at most `below` rows
    /// under it. Space for them is made first, so the prompt never scrolls
    /// the screen and can be erased after Ctrl-C (which inquire leaves on
    /// screen, unlike Esc).
    fn ask<T>(
        &self,
        message: &str,
        below: usize,
        prompt: impl FnOnce() -> inquire::error::InquireResult<T>,
    ) -> Result<T> {
        self.stdout.flush()?;
        let _hold = spinner::hold();
        // Room for the `? ` prefix and a short typed filter or answer.
        let rows = screen_rows(message.width() + 12) + below;
        reserve(rows).map_err(prompt_failed)?;
        match prompt() {
            Err(InquireError::OperationInterrupted) => {
                self.erase_canceled(message).map_err(prompt_failed)?;
                Err(Error::cancelled())
            }
            result => finish(result),
        }
    }

    /// Replaces the prompt left by Ctrl-C with its question, as Esc does.
    fn erase_canceled(&self, message: &str) -> io::Result<()> {
        let mut stderr = io::stderr();
        queue!(
            stderr,
            cursor::RestorePosition,
            terminal::Clear(terminal::ClearType::FromCursorDown)
        )?;
        let prefix = if self.color {
            Style::new()
                .green()
                .bright()
                .force_styling(true)
                .apply_to("?")
                .to_string()
        } else {
            "?".to_owned()
        };
        writeln!(stderr, "{prefix} {message}")?;
        stderr.flush()
    }

    fn render_config(&self) -> RenderConfig<'static> {
        let config = if self.color {
            RenderConfig::default_colored()
        } else {
            RenderConfig::empty()
        };
        // A cancelled prompt keeps only its question; `Canceled.` follows.
        config.with_canceled_prompt_indicator(Styled::new(""))
    }
}

/// The answer `raw` input gives a yes/no question, or why it is refused.
fn yes_no(raw: &str, default: bool) -> std::result::Result<bool, String> {
    match raw.trim().to_lowercase().as_str() {
        "" => Ok(default),
        "y" | "yes" => Ok(true),
        "n" | "no" => Ok(false),
        _ => Err("Type y for yes or n for no".to_owned()),
    }
}

/// Rows a select list of `choices` takes below its question: a page of
/// choices and the `help` line.
fn list_rows(choices: usize, help: &str) -> usize {
    choices.min(PAGE_SIZE) + screen_rows(help.width() + 2)
}

/// Rows a line `width` columns wide takes on stderr's terminal.
fn screen_rows(width: usize) -> usize {
    let columns = terminal::size()
        .ok()
        .map(|(columns, _)| usize::from(columns))
        .filter(|&columns| columns > 0)
        .unwrap_or(80);
    width.div_ceil(columns).max(1)
}

/// Makes room for `rows` rows below the cursor, scrolling the screen if it
/// must, and saves the position where they start.
fn reserve(rows: usize) -> io::Result<()> {
    let mut stderr = io::stderr();
    let below = rows.saturating_sub(1);
    stderr.write_all("\n".repeat(below).as_bytes())?;
    if let Some(below) = u16::try_from(below).ok().filter(|&below| below > 0) {
        queue!(stderr, cursor::MoveUp(below))?;
    }
    queue!(stderr, cursor::MoveToColumn(0), cursor::SavePosition)?;
    stderr.flush()
}

fn prompt_failed(error: io::Error) -> Error {
    Error::new(format!("The prompt failed: {error}")).with_source(error)
}

/// Checks a nonempty answer: trimmed input, or the default as supplied.
pub type Check<'a> = &'a dyn Fn(&str) -> std::result::Result<(), String>;

/// A free-text question. Answers are trimmed, and a blank answer takes the
/// default when there is one.
#[derive(Clone, Copy)]
pub struct Text<'a> {
    message: &'a str,
    default: Option<&'a str>,
    required: bool,
    check: Option<Check<'a>>,
}

impl<'a> Text<'a> {
    pub fn new(message: &'a str) -> Self {
        Self {
            message,
            default: None,
            required: false,
            check: None,
        }
    }

    #[cfg(test)]
    pub fn message(&self) -> &'a str {
        self.message
    }

    /// Blank answers are refused (unless there is a default).
    pub fn required(self) -> Self {
        Self {
            required: true,
            ..self
        }
    }

    /// The answer a blank answer stands for. An empty default is no default.
    pub fn with_default(self, default: &'a str) -> Self {
        Self {
            default: Some(default).filter(|default| !default.is_empty()),
            ..self
        }
    }

    /// Checks nonempty answers, including defaults; errors repeat the question.
    pub fn with_check(self, check: Check<'a>) -> Self {
        Self {
            check: Some(check),
            ..self
        }
    }

    /// The answer `raw` input gives, or why it is refused.
    pub fn answer(&self, raw: &str) -> std::result::Result<String, String> {
        let answer = raw.trim();
        let answer = if answer.is_empty() {
            match self.default {
                Some(default) => default,
                None if self.required => return Err("An answer is required".to_owned()),
                None => return Ok(String::new()),
            }
        } else {
            answer
        };
        if let Some(check) = self.check {
            check(answer)?;
        }
        Ok(answer.to_owned())
    }
}

/// One entry in a select list: the label shown and the value picking it gives.
pub struct Choice<T> {
    label: String,
    pub value: T,
}

impl<T> Choice<T> {
    pub fn new(label: impl AsRef<str>, value: T) -> Self {
        Self {
            label: escape(label.as_ref()),
            value,
        }
    }
}

impl<T> fmt::Display for Choice<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.label)
    }
}

/// Choices from `(label, detail, value)` rows, where a label several rows
/// share gets its row's detail in parentheses so the rows can be told apart.
pub fn distinct_choices<T>(rows: Vec<(String, Option<String>, T)>) -> Vec<Choice<T>> {
    let mut counts: std::collections::HashMap<String, usize> = std::collections::HashMap::new();
    for (label, _, _) in &rows {
        *counts.entry(label.to_lowercase()).or_default() += 1;
    }
    rows.into_iter()
        .map(|(label, detail, value)| {
            let shared = counts
                .get(&label.to_lowercase())
                .is_some_and(|count| *count > 1);
            match detail {
                Some(detail) if shared => Choice::new(format!("{label} ({detail})"), value),
                Some(_) | None => Choice::new(label, value),
            }
        })
        .collect()
}

/// Whether a label survives the typed filter: every word of the filter
/// appears in it, ignoring case and the label's own spaces (which a
/// multi-select cannot type, since space toggles a choice there).
pub fn matches(filter: &str, label: &str) -> bool {
    let label: String = label
        .to_lowercase()
        .chars()
        .filter(|ch| !ch.is_whitespace())
        .collect();
    filter
        .split_whitespace()
        .all(|word| label.contains(&word.to_lowercase()))
}

/// How closely a label that survives the filter matches it.
#[derive(Clone, Copy)]
enum Rank {
    /// The filter appears only inside a word.
    MidWord,
    /// Every filter word starts a word of the label.
    WordStart,
    /// The filter starts the label or a key in parentheses: `sc` for
    /// "Scholie (SCH)".
    KeyStart,
    /// The filter is the whole label or a key in parentheses: `sch` or
    /// `(SCH)` for "Scholie (SCH)".
    Exact,
}

fn rank(filter: &str, label: &str) -> Option<Rank> {
    if !matches(filter, label) {
        return None;
    }
    let filter = filter.trim().to_lowercase();
    if filter.is_empty() {
        return Some(Rank::MidWord);
    }
    let label = label.trim().to_lowercase();
    let bare =
        |word: &str| -> String { word.trim_matches(|c: char| !c.is_alphanumeric()).to_owned() };
    let keys: Vec<&str> = label
        .split_whitespace()
        .filter_map(|word| word.strip_prefix('(')?.strip_suffix(')'))
        .collect();
    let key = Some(bare(&filter)).filter(|key| !key.is_empty());
    let is_key = |key: &str| keys.contains(&key);
    if label == filter || key.as_deref().is_some_and(is_key) {
        return Some(Rank::Exact);
    }
    let starts_key = |key: &str| keys.iter().any(|candidate| candidate.starts_with(key));
    if label.starts_with(&filter) || key.as_deref().is_some_and(starts_key) {
        return Some(Rank::KeyStart);
    }
    let words: Vec<String> = label.split_whitespace().map(bare).collect();
    let word_start = filter
        .split_whitespace()
        .map(bare)
        .all(|part| words.iter().any(|word| word.starts_with(&part)));
    Some(if word_start {
        Rank::WordStart
    } else {
        Rank::MidWord
    })
}

/// Closer matches come first (see [`Rank`]), and equal ones keep the list's
/// own order.
fn score<T>(filter: &str, _: &Choice<T>, label: &str, index: usize) -> Option<i64> {
    let index = i64::try_from(index).expect("a select list fits in i64");
    let tier = match rank(filter, label)? {
        Rank::MidWord => 0,
        Rank::WordStart => 1,
        Rank::KeyStart => 2,
        Rank::Exact => 3,
    };
    Some(tier * (i64::from(u32::MAX) + 1) - index)
}

/// Shows control characters (from names Linear returned, say) as escapes
/// instead of letting them act on the terminal.
pub fn escape(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().collect()
            } else {
                c.to_string()
            }
        })
        .collect()
}

fn finish<T>(result: inquire::error::InquireResult<T>) -> Result<T> {
    match result {
        Ok(answer) => Ok(answer),
        Err(InquireError::OperationCanceled | InquireError::OperationInterrupted) => {
            Err(Error::cancelled())
        }
        Err(InquireError::NotTTY) => Err(Error::new("Prompts need a terminal")),
        Err(InquireError::IO(error)) => {
            Err(Error::new(format!("The prompt failed: {error}")).with_source(error))
        }
        Err(InquireError::InvalidConfiguration(reason)) => {
            unreachable!("prompts are configured correctly: {reason}")
        }
        Err(InquireError::Custom(error)) => {
            unreachable!("prompt validators never fail: {error}")
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ok(answer: &str) -> std::result::Result<String, String> {
        Ok(answer.to_owned())
    }

    #[test]
    fn answers_are_trimmed() {
        assert_eq!(Text::new("Title").answer("  Title 界 \t"), ok("Title 界"));
        assert_eq!(Text::new("Title").answer(" \t "), ok(""));
    }

    #[test]
    fn required_answers_refuse_blank_input() {
        let text = Text::new("Title").required();
        assert!(text.answer("").is_err());
        assert!(text.answer(" \t ").is_err());
        assert_eq!(text.answer(" x "), ok("x"));
    }

    #[test]
    fn blank_answers_take_the_default() {
        let text = Text::new("Name").required().with_default("Current");
        assert_eq!(text.answer(""), ok("Current"));
        assert_eq!(text.answer("   "), ok("Current"));
        assert_eq!(text.answer(" New "), ok("New"));
        let empty_default = Text::new("Name").required().with_default("");
        assert!(empty_default.answer("").is_err());
    }

    #[test]
    fn checks_see_trimmed_nonblank_answers() {
        let hex = |value: &str| {
            if value.starts_with('#') {
                Ok(())
            } else {
                Err("Enter a hex color".to_owned())
            }
        };
        let text = Text::new("Color").with_check(&hex);
        assert_eq!(text.answer(" #fff "), ok("#fff"));
        assert_eq!(text.answer("red"), Err("Enter a hex color".to_owned()));
        assert_eq!(text.answer(""), ok(""));
    }

    #[test]
    fn checked_defaults_must_parse_before_they_can_be_accepted() {
        let hex = |value: &str| value.parse::<crate::cli::values::HexColor>().map(drop);
        for default in ["red", "#fff"] {
            for required in [false, true] {
                let text = Text::new("Color").with_default(default).with_check(&hex);
                let text = if required { text.required() } else { text };
                for answer in ["", " \t "] {
                    assert_eq!(
                        text.answer(answer),
                        hex(default).map(|()| default.to_owned())
                    );
                    assert!(text.answer(answer).is_err());
                }
                assert_eq!(text.answer(" #123456 "), ok("#123456"));
            }
        }
        let text = Text::new("Color").with_default("#5E6AD2").with_check(&hex);
        assert_eq!(text.answer(""), ok("#5E6AD2"));
    }

    #[test]
    fn optional_blank_answers_bypass_checks_but_defaults_keep_their_whitespace() {
        let reject = |_: &str| Err("checked".to_owned());
        let text = Text::new("Optional").with_default("").with_check(&reject);
        assert_eq!(text.answer(" \t "), ok(""));
        assert_eq!(
            text.required().answer(""),
            Err("An answer is required".to_owned())
        );
        let exact = |value: &str| {
            assert_eq!(value, " Current ");
            Ok(())
        };
        let text = Text::new("Name")
            .with_default(" Current ")
            .with_check(&exact);
        assert_eq!(text.answer(""), ok(" Current "));
        let text = Text::new("Name").with_default("   ").with_check(&reject);
        assert_eq!(text.answer(""), Err("checked".to_owned()));
    }
    #[test]
    fn yes_no_answers_take_y_n_or_the_default() {
        for (raw, default, answer) in [
            ("", true, Ok(true)),
            (" ", false, Ok(false)),
            ("y", false, Ok(true)),
            (" YES ", false, Ok(true)),
            ("n", true, Ok(false)),
            ("No", true, Ok(false)),
        ] {
            assert_eq!(yes_no(raw, default), answer, "{raw:?}");
        }
        assert!(yes_no("u", true).is_err());
        assert!(yes_no("yep", false).is_err());
    }

    #[test]
    fn the_filter_needs_every_word_ignoring_case() {
        assert!(matches("", "Engineering (ENG)"));
        assert!(matches("eng", "Engineering (ENG)"));
        assert!(matches("ENG ring", "Engineering (ENG)"));
        assert!(!matches("eng ops", "Engineering (ENG)"));
        assert!(matches("qafeed", "QA feedback"));
        assert!(matches("qa feedback", "QA feedback"));
    }

    #[test]
    fn filtering_keeps_list_order() {
        let first = Choice::new("Design", ());
        let second = Choice::new("Design ops", ());
        assert!(score("design", &first, "Design", 0) > score("design", &second, "Design ops", 1));
        assert_eq!(score("ops", &first, "Design", 0), None);
    }

    /// The labels left by `filter`, in the order the list shows them.
    fn ranked<'a>(filter: &str, labels: &[&'a str]) -> Vec<&'a str> {
        let mut scored: Vec<(i64, &str)> = labels
            .iter()
            .enumerate()
            .filter_map(|(index, &label)| {
                score(filter, &Choice::new(label, ()), label, index).map(|score| (score, label))
            })
            .collect();
        scored.sort_by_key(|&(score, _)| std::cmp::Reverse(score));
        scored.into_iter().map(|(_, label)| label).collect()
    }

    #[test]
    fn keys_and_word_starts_rank_above_mid_word_matches() {
        let teams = [
            "Peter Schilling's Team (PST)",
            "Eschaton (ESC)",
            "Scholie (SCH)",
            "Schedules (SCD)",
        ];
        assert_eq!(
            ranked("sch", &teams),
            [
                "Scholie (SCH)",
                "Schedules (SCD)",
                "Peter Schilling's Team (PST)",
                "Eschaton (ESC)",
            ]
        );
        assert_eq!(ranked("(SCH)", &teams), ["Scholie (SCH)"]);
        assert_eq!(ranked("(", &["Ops", "A (B)"]), ["A (B)"]);
        assert_eq!(
            ranked("sc", &teams),
            [
                "Scholie (SCH)",
                "Schedules (SCD)",
                "Peter Schilling's Team (PST)",
                "Eschaton (ESC)",
            ]
        );
        assert_eq!(
            ranked("team", &["Steam", "Platform team", "Team"]),
            ["Team", "Platform team", "Steam"]
        );
    }

    #[test]
    fn an_empty_filter_keeps_list_order() {
        assert_eq!(
            ranked("", &["Zeta (Z)", "Alpha (A)"]),
            ["Zeta (Z)", "Alpha (A)"]
        );
    }

    #[test]
    fn labels_show_control_characters_escaped() {
        assert_eq!(escape("a\u{1b}[31mb\nc"), "a\\u{1b}[31mb\\nc");
        assert_eq!(Choice::new("x\ty", 1).to_string(), "x\\ty");
    }
}
