//! A one-line text prompt with the usual readline editing keys (Ctrl-A,
//! Ctrl-E, Ctrl-U, Ctrl-K, Ctrl-W, Alt-B, Alt-F, …), drawn on stderr in the
//! style of the other prompts. Control keys it does not know are ignored
//! rather than typed as letters.
use std::io::{self, Write};

use console::Style;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use crossterm::{cursor, execute, queue, terminal};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::error::{Error, Result};

/// Width assumed when the terminal reports none.
const FALLBACK_COLUMNS: usize = 80;

pub struct LinePrompt<'a> {
    pub message: &'a str,
    /// Follows the message while the prompt is answered, such as `(y/N)`.
    pub hint: Option<&'a str>,
    /// Shown dimmed while the input is empty.
    pub placeholder: Option<&'a str>,
    pub help: Option<&'a str>,
    /// Show `*` for each character typed, and no answer afterwards.
    pub masked: bool,
    pub color: bool,
    /// Why the raw input cannot be submitted, if it cannot.
    pub check: &'a dyn Fn(&str) -> std::result::Result<(), String>,
    /// The answer as shown once submitted.
    pub format: &'a dyn Fn(&str) -> String,
    /// An earlier answer to this question that was submitted and then
    /// refused, with why. The prompt replaces the line that answer left,
    /// starts with it as the input, and shows the reason until it is edited.
    pub retry: Option<(&'a str, &'a str)>,
}

/// Asks `prompt` on the terminal: the raw input submitted, or `None` when
/// it was cancelled (Esc, Ctrl-C, or Ctrl-D on an empty line).
pub fn ask(prompt: &LinePrompt<'_>) -> Result<Option<String>> {
    run(prompt)
        .map_err(|error| Error::new(format!("The prompt failed: {error}")).with_source(error))
}

fn run(prompt: &LinePrompt<'_>) -> io::Result<Option<String>> {
    let _raw = RawMode::enable()?;
    let mut screen = Screen {
        out: io::stderr(),
        cursor_row: 0,
    };
    let mut buffer = LineBuffer::default();
    let mut error: Option<String> = None;
    if let Some((input, reason)) = prompt.retry {
        let answered = 2 + prompt.message.width() + 1 + (prompt.format)(input).width();
        screen.cursor_row = answered.div_ceil(Screen::columns()).max(1);
        for ch in input.chars() {
            buffer.apply(Edit::Insert(ch));
        }
        error = Some(reason.to_owned());
    }
    loop {
        screen.draw(&prompt.frame(&buffer, error.as_deref()))?;
        let command = match event::read()? {
            Event::Key(key) if key.kind != KeyEventKind::Release => command(key),
            Event::Paste(text) => {
                for ch in pasted(&text) {
                    buffer.apply(Edit::Insert(ch));
                }
                continue;
            }
            _ => continue,
        };
        match command {
            Command::Edit(edit) => {
                buffer.apply(edit);
                error = None;
            }
            Command::DeleteOrCancel if !buffer.is_empty() => buffer.apply(Edit::Delete),
            Command::Cancel | Command::DeleteOrCancel => {
                screen.finish(&prompt.done(None))?;
                return Ok(None);
            }
            Command::Submit => {
                let input = buffer.text();
                match (prompt.check)(&input) {
                    Ok(()) => {
                        screen.finish(&prompt.done(Some(&input)))?;
                        return Ok(Some(input));
                    }
                    Err(reason) => error = Some(reason),
                }
            }
            Command::Ignore => {}
        }
    }
}

/// The characters pasting `text` types. Line breaks and tabs become spaces
/// (a line break would otherwise submit half an answer); other control
/// characters are dropped.
fn pasted(text: &str) -> impl Iterator<Item = char> + '_ {
    text.chars()
        .map(|ch| {
            if matches!(ch, '\n' | '\r' | '\t') {
                ' '
            } else {
                ch
            }
        })
        .filter(|ch| !ch.is_control())
}

impl LinePrompt<'_> {
    fn paint(&self, text: &str, style: Style) -> String {
        if self.color {
            style.force_styling(true).apply_to(text).to_string()
        } else {
            text.to_owned()
        }
    }

    /// The question as asked, and its display width.
    fn question(&self) -> (String, usize) {
        let text = match self.hint {
            Some(hint) => format!("{} {hint} ", self.message),
            None => format!("{} ", self.message),
        };
        let width = 2 + text.width();
        let painted = format!("{} {text}", self.paint("?", Style::new().green().bright()));
        (painted, width)
    }

    /// The lines of the prompt while it is being answered, and where the
    /// cursor goes on the first one (in columns).
    fn frame(&self, buffer: &LineBuffer, error: Option<&str>) -> Frame {
        let (question, question_width) = self.question();
        let shown = if self.masked {
            "*".repeat(buffer.chars.len())
        } else {
            buffer.text()
        };
        let input = match self.placeholder {
            Some(placeholder) if buffer.is_empty() => {
                self.paint(placeholder, Style::new().black().bright())
            }
            _ => shown.clone(),
        };
        let input_width = if buffer.is_empty() {
            self.placeholder.map_or(0, UnicodeWidthStr::width)
        } else {
            shown.width()
        };
        let cursor = if self.masked {
            buffer.cursor
        } else {
            buffer.pre_cursor_width()
        };
        // The trailing space gives a cursor at the end of the input a cell
        // to sit on.
        let mut lines = vec![(
            format!("{question}{input} "),
            question_width + input_width + 1,
        )];
        if let Some(error) = error {
            // Parsers shared with clap word their errors in lowercase, as
            // clap's own do; under a prompt they read as sentences.
            let mut chars = error.chars();
            let sentence: String = chars
                .next()
                .map(|first| first.to_uppercase().chain(chars).collect())
                .unwrap_or_default();
            let text = format!("✗ {sentence}");
            let width = text.width();
            lines.push((self.paint(&text, Style::new().red().bright()), width));
        }
        if let Some(help) = self.help {
            let text = format!("[{help}]");
            let width = text.width();
            lines.push((self.paint(&text, Style::new().cyan().bright()), width));
        }
        Frame {
            lines,
            cursor: question_width + cursor,
        }
    }

    /// The line left behind: the question with its answer, or alone when
    /// cancelled.
    fn done(&self, answer: Option<&str>) -> String {
        let Some(answer) = answer else {
            return format!(
                "{} {}",
                self.paint("?", Style::new().green().bright()),
                self.message
            );
        };
        let shown = if self.masked {
            "********".to_owned()
        } else {
            (self.format)(answer)
        };
        format!(
            "{} {} {}",
            self.paint(">", Style::new().green().bright()),
            self.message,
            self.paint(&shown, Style::new().cyan().bright())
        )
    }
}

struct Frame {
    /// Painted lines with their display widths.
    lines: Vec<(String, usize)>,
    cursor: usize,
}

/// Redraws the prompt in place, tracking which of its rows the cursor is on.
struct Screen {
    out: io::Stderr,
    cursor_row: usize,
}

impl Screen {
    fn columns() -> usize {
        terminal::size()
            .ok()
            .map(|(columns, _)| usize::from(columns))
            .filter(|&columns| columns > 0)
            .unwrap_or(FALLBACK_COLUMNS)
    }

    /// Moves to the start of the prompt and clears it and everything below.
    fn rewind(&mut self) -> io::Result<()> {
        if let Some(rows) = u16::try_from(self.cursor_row).ok().filter(|&rows| rows > 0) {
            queue!(self.out, cursor::MoveUp(rows))?;
        }
        queue!(
            self.out,
            cursor::MoveToColumn(0),
            terminal::Clear(terminal::ClearType::FromCursorDown)
        )?;
        self.cursor_row = 0;
        Ok(())
    }

    fn draw(&mut self, frame: &Frame) -> io::Result<()> {
        let columns = Self::columns();
        self.rewind()?;
        let mut rows = 0;
        for (index, (line, width)) in frame.lines.iter().enumerate() {
            if index > 0 {
                self.out.write_all(b"\r\n")?;
            }
            self.out.write_all(line.as_bytes())?;
            rows += width.div_ceil(columns).max(1);
        }
        let row = frame.cursor / columns;
        let column = frame.cursor % columns;
        let up = rows.saturating_sub(1).saturating_sub(row);
        if let Some(up) = u16::try_from(up).ok().filter(|&up| up > 0) {
            queue!(self.out, cursor::MoveUp(up))?;
        }
        queue!(
            self.out,
            cursor::MoveToColumn(u16::try_from(column).unwrap_or(u16::MAX))
        )?;
        self.cursor_row = row;
        self.out.flush()
    }

    fn finish(&mut self, line: &str) -> io::Result<()> {
        self.rewind()?;
        self.out.write_all(line.as_bytes())?;
        self.out.write_all(b"\r\n")?;
        self.out.flush()
    }
}

/// Raw terminal input for as long as it lives.
struct RawMode;

impl RawMode {
    fn enable() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        // Without bracketed paste a pasted line break would submit.
        if let Err(error) = execute!(io::stderr(), event::EnableBracketedPaste) {
            terminal::disable_raw_mode()?;
            return Err(error);
        }
        Ok(Self)
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        // Restoring the terminal is best effort once the prompt is over.
        let _ignored = execute!(io::stderr(), event::DisableBracketedPaste);
        let _ignored = terminal::disable_raw_mode();
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Command {
    Edit(Edit),
    Submit,
    Cancel,
    /// Ctrl-D: deletes forward, or cancels on an empty line.
    DeleteOrCancel,
    Ignore,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Edit {
    Insert(char),
    Left,
    Right,
    WordLeft,
    WordRight,
    Start,
    End,
    Backspace,
    Delete,
    KillToStart,
    KillToEnd,
    KillWordLeft,
    KillWordRight,
}

fn command(key: KeyEvent) -> Command {
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    let edit = match key.code {
        KeyCode::Enter => return Command::Submit,
        KeyCode::Esc => return Command::Cancel,
        KeyCode::Char('c' | 'g') if control => return Command::Cancel,
        KeyCode::Char('d') if control => return Command::DeleteOrCancel,
        KeyCode::Char('j' | 'm') if control => return Command::Submit,
        KeyCode::Char('a') if control => Edit::Start,
        KeyCode::Char('e') if control => Edit::End,
        KeyCode::Char('b') if control => Edit::Left,
        KeyCode::Char('f') if control => Edit::Right,
        KeyCode::Char('h') if control => Edit::Backspace,
        KeyCode::Char('u') if control => Edit::KillToStart,
        KeyCode::Char('k') if control => Edit::KillToEnd,
        KeyCode::Char('w') if control => Edit::KillWordLeft,
        KeyCode::Char('b') if alt => Edit::WordLeft,
        KeyCode::Char('f') if alt => Edit::WordRight,
        KeyCode::Char('d') if alt => Edit::KillWordRight,
        KeyCode::Char(_) if control || alt => return Command::Ignore,
        KeyCode::Char(ch) if !ch.is_control() => Edit::Insert(ch),
        KeyCode::Backspace if alt || control => Edit::KillWordLeft,
        KeyCode::Backspace => Edit::Backspace,
        KeyCode::Delete if alt || control => Edit::KillWordRight,
        KeyCode::Delete => Edit::Delete,
        KeyCode::Left if alt || control => Edit::WordLeft,
        KeyCode::Left => Edit::Left,
        KeyCode::Right if alt || control => Edit::WordRight,
        KeyCode::Right => Edit::Right,
        KeyCode::Home => Edit::Start,
        KeyCode::End => Edit::End,
        _ => return Command::Ignore,
    };
    Command::Edit(edit)
}

/// The text being typed and the cursor in it, in characters.
#[derive(Default)]
struct LineBuffer {
    chars: Vec<char>,
    cursor: usize,
}

impl LineBuffer {
    fn text(&self) -> String {
        self.chars.iter().collect()
    }

    fn is_empty(&self) -> bool {
        self.chars.is_empty()
    }

    fn pre_cursor_width(&self) -> usize {
        self.chars
            .iter()
            .take(self.cursor)
            .map(|ch| ch.width().unwrap_or(0))
            .sum()
    }

    fn apply(&mut self, edit: Edit) {
        let len = self.chars.len();
        match edit {
            Edit::Insert(ch) => {
                self.chars.insert(self.cursor, ch);
                self.cursor += 1;
            }
            Edit::Left => self.cursor = self.cursor.saturating_sub(1),
            Edit::Right => self.cursor = (self.cursor + 1).min(len),
            Edit::WordLeft => self.cursor = self.word_start(),
            Edit::WordRight => self.cursor = self.word_end(),
            Edit::Start => self.cursor = 0,
            Edit::End => self.cursor = len,
            Edit::Backspace => {
                if self.cursor > 0 {
                    self.cursor -= 1;
                    self.chars.remove(self.cursor);
                }
            }
            Edit::Delete => {
                if self.cursor < len {
                    self.chars.remove(self.cursor);
                }
            }
            Edit::KillToStart => {
                self.chars.drain(..self.cursor);
                self.cursor = 0;
            }
            Edit::KillToEnd => {
                self.chars.truncate(self.cursor);
            }
            Edit::KillWordLeft => {
                let start = self.word_start();
                self.chars.drain(start..self.cursor);
                self.cursor = start;
            }
            Edit::KillWordRight => {
                let end = self.word_end();
                self.chars.drain(self.cursor..end);
            }
        }
    }

    /// Where the word before the cursor starts, skipping spaces first.
    fn word_start(&self) -> usize {
        let before = self.chars.get(..self.cursor).unwrap_or_default();
        let word_end = before
            .iter()
            .rposition(|ch| !ch.is_whitespace())
            .map_or(0, |index| index + 1);
        before
            .get(..word_end)
            .unwrap_or_default()
            .iter()
            .rposition(|ch| ch.is_whitespace())
            .map_or(0, |index| index + 1)
    }

    /// Where the word after the cursor ends, skipping spaces first.
    fn word_end(&self) -> usize {
        let after = self.chars.get(self.cursor..).unwrap_or_default();
        let word_start = after
            .iter()
            .position(|ch| !ch.is_whitespace())
            .unwrap_or(after.len());
        let word_len = after
            .get(word_start..)
            .unwrap_or_default()
            .iter()
            .position(|ch| ch.is_whitespace())
            .unwrap_or(after.len() - word_start);
        self.cursor + word_start + word_len
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn typed(text: &str) -> LineBuffer {
        let mut buffer = LineBuffer::default();
        for ch in text.chars() {
            buffer.apply(Edit::Insert(ch));
        }
        buffer
    }

    fn key(code: KeyCode, modifiers: KeyModifiers) -> Command {
        command(KeyEvent::new(code, modifiers))
    }

    #[test]
    fn errors_under_the_input_read_as_sentences() {
        let prompt = LinePrompt {
            message: "Due date:",
            hint: None,
            placeholder: None,
            help: None,
            masked: false,
            color: false,
            check: &|_| Ok(()),
            format: &str::to_owned,
            retry: None,
        };
        let frame = prompt.frame(&typed("soon"), Some("expected a YYYY-MM-DD date"));
        assert_eq!(frame.lines[0].0, "? Due date: soon ");
        assert_eq!(frame.lines[1].0, "✗ Expected a YYYY-MM-DD date");
    }

    #[test]
    fn readline_keys_edit_instead_of_typing_letters() {
        let ctrl = KeyModifiers::CONTROL;
        let alt = KeyModifiers::ALT;
        let edits = [
            ('a', ctrl, Edit::Start),
            ('e', ctrl, Edit::End),
            ('u', ctrl, Edit::KillToStart),
            ('k', ctrl, Edit::KillToEnd),
            ('w', ctrl, Edit::KillWordLeft),
            ('b', alt, Edit::WordLeft),
            ('f', alt, Edit::WordRight),
        ];
        for (ch, modifiers, edit) in edits {
            assert_eq!(
                key(KeyCode::Char(ch), modifiers),
                Command::Edit(edit),
                "{ch}"
            );
        }
        assert_eq!(key(KeyCode::Char('t'), ctrl), Command::Ignore);
        assert_eq!(key(KeyCode::Char('x'), alt), Command::Ignore);
        assert_eq!(
            key(KeyCode::Char('A'), KeyModifiers::SHIFT),
            Command::Edit(Edit::Insert('A'))
        );
        assert_eq!(key(KeyCode::Char('c'), ctrl), Command::Cancel);
        assert_eq!(key(KeyCode::Esc, KeyModifiers::NONE), Command::Cancel);
        assert_eq!(key(KeyCode::Char('d'), ctrl), Command::DeleteOrCancel);
        assert_eq!(key(KeyCode::Enter, KeyModifiers::NONE), Command::Submit);
    }

    #[test]
    fn kills_and_moves_work_on_words_and_line_ends() {
        let mut buffer = typed("fix the  login bug");
        buffer.apply(Edit::KillWordLeft);
        assert_eq!(buffer.text(), "fix the  login ");
        buffer.apply(Edit::KillWordLeft);
        assert_eq!(buffer.text(), "fix the  ");
        buffer.apply(Edit::WordLeft);
        assert_eq!(buffer.cursor, 4);
        buffer.apply(Edit::KillToEnd);
        assert_eq!(buffer.text(), "fix ");
        buffer.apply(Edit::Start);
        buffer.apply(Edit::WordRight);
        assert_eq!(buffer.cursor, 3);
        buffer.apply(Edit::KillToStart);
        assert_eq!((buffer.text().as_str(), buffer.cursor), (" ", 0));
        let mut buffer = typed("one two");
        buffer.apply(Edit::Start);
        buffer.apply(Edit::KillWordRight);
        assert_eq!(buffer.text(), " two");
    }

    #[test]
    fn edits_at_the_edges_do_nothing() {
        let mut buffer = LineBuffer::default();
        for edit in [
            Edit::Backspace,
            Edit::Delete,
            Edit::Left,
            Edit::KillWordLeft,
            Edit::KillWordRight,
            Edit::KillToStart,
        ] {
            buffer.apply(edit);
        }
        assert!(buffer.is_empty());
        let mut buffer = typed("界a");
        buffer.apply(Edit::Left);
        assert_eq!(buffer.pre_cursor_width(), 2);
        buffer.apply(Edit::Right);
        buffer.apply(Edit::Right);
        assert_eq!(buffer.cursor, 2);
    }
}
