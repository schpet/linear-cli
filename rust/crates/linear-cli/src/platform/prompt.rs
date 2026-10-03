//! Interactive questions on the terminal, drawn on stderr.
//!
//! Every prompt needs a terminal. Commands get a [`Prompter`] from
//! `Ctx::prompter`, which refuses when stdin is not one, and offer flags for
//! scripted use instead. Ctrl-C or Esc at any prompt cancels the command.
use std::fmt;

use inquire::InquireError;
use inquire::ui::RenderConfig;
use inquire::validator::Validation;

use crate::error::{Error, Result};
use crate::platform::output::Stdout;

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
        let message = escape(text.message);
        let placeholder = text.default.map(escape);
        let validator = |raw: &str| {
            Ok(match text.answer(raw) {
                Ok(_) => Validation::Valid,
                Err(reason) => Validation::Invalid(reason.into()),
            })
        };
        let formatter = |raw: &str| escape(&text.answer(raw).unwrap_or_default());
        let mut prompt = inquire::Text::new(&message)
            .with_render_config(self.render_config())
            .with_validator(validator)
            .with_formatter(&formatter);
        if let Some(placeholder) = &placeholder {
            prompt = prompt.with_placeholder(placeholder);
        }
        let raw = finish(prompt.prompt())?;
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
        let message = escape(message);
        let answer = finish(
            inquire::Password::new(&message)
                .with_render_config(self.render_config())
                .with_display_mode(inquire::PasswordDisplayMode::Masked)
                .without_confirmation()
                .with_help_message(help)
                .prompt(),
        )?;
        Ok(answer.trim().to_owned())
    }

    pub fn confirm(&self, message: &str, default: bool) -> Result<bool> {
        self.stdout.flush()?;
        let message = escape(message);
        finish(
            inquire::Confirm::new(&message)
                .with_render_config(self.render_config())
                .with_default(default)
                .prompt(),
        )
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
        self.stdout.flush()?;
        let message = escape(message);
        let choice = finish(
            inquire::Select::new(&message, choices)
                .with_render_config(self.render_config())
                .with_scorer(&score)
                .with_starting_cursor(start)
                .prompt(),
        )?;
        Ok(choice.value)
    }

    /// Picks any number of choices, returned in list order; typing filters
    /// the list.
    pub fn multi_select<T>(&self, message: &str, choices: Vec<Choice<T>>) -> Result<Vec<T>> {
        if choices.is_empty() {
            return Ok(Vec::new());
        }
        self.stdout.flush()?;
        let message = escape(message);
        let picked = finish(
            inquire::MultiSelect::new(&message, choices)
                .with_render_config(self.render_config())
                .with_scorer(&score)
                .prompt(),
        )?;
        Ok(picked.into_iter().map(|choice| choice.value).collect())
    }

    fn render_config(&self) -> RenderConfig<'static> {
        if self.color {
            RenderConfig::default_colored()
        } else {
            RenderConfig::empty()
        }
    }
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

/// Whether a label survives the typed filter: every word of the filter
/// appears in it, ignoring case.
pub fn matches(filter: &str, label: &str) -> bool {
    let label = label.to_lowercase();
    filter
        .split_whitespace()
        .all(|word| label.contains(&word.to_lowercase()))
}

/// Filtering keeps the list's own order: earlier entries score higher.
fn score<T>(filter: &str, _: &Choice<T>, label: &str, index: usize) -> Option<i64> {
    let index = i64::try_from(index).expect("a select list fits in i64");
    matches(filter, label).then_some(-index)
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
    fn the_filter_needs_every_word_ignoring_case() {
        assert!(matches("", "Engineering (ENG)"));
        assert!(matches("eng", "Engineering (ENG)"));
        assert!(matches("ENG ring", "Engineering (ENG)"));
        assert!(!matches("eng ops", "Engineering (ENG)"));
    }

    #[test]
    fn filtering_keeps_list_order() {
        let first = Choice::new("Design", ());
        let second = Choice::new("Design ops", ());
        assert!(score("design", &first, "Design", 0) > score("design", &second, "Design ops", 1));
        assert_eq!(score("ops", &first, "Design", 0), None);
    }

    #[test]
    fn labels_show_control_characters_escaped() {
        assert_eq!(escape("a\u{1b}[31mb\nc"), "a\\u{1b}[31mb\\nc");
        assert_eq!(Choice::new("x\ty", 1).to_string(), "x\\ty");
    }
}
