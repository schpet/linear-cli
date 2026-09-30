//! Reusable text and plain-list prompts for commands whose stdout is a terminal.
//!
//! The caller decides whether to prompt. A terminal stdin uses individual keys;
//! a non-terminal stdin uses a deterministic, line-oriented script protocol.
//! One session retains unread script bytes and owns terminal restoration across
//! every prompt in a command.

use std::io::{self, BufReader, IsTerminal, Read, Write};

use unicode_width::UnicodeWidthChar;

use crate::error::{AppError, AppErrorKind};

const MAX_LINE_BYTES: usize = 64 * 1024;
const MAX_OPTIONS_VISIBLE: usize = 10;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PromptOutcome<T> {
    Submitted(T),
    Interrupted,
    EndOfInput,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PromptKey {
    Character(char),
    Left,
    Right,
    Up,
    Down,
    PageUp,
    PageDown,
    Home,
    End,
    Backspace,
    Delete,
    Enter,
    Interrupt,
    EndOfInput,
    Other,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlainOption {
    pub label: String,
    pub value: String,
    /// Public line-script spelling. Internal sentinel values do not belong here.
    pub script_token: String,
}

#[derive(Clone, Debug)]
pub struct PlainSelect<'a> {
    pub message: &'a str,
    pub options: &'a [PlainOption],
    pub default_index: usize,
    /// Displayed hint can differ from the highlighted option's value.
    pub default_hint: Option<&'a str>,
}

enum InputSource<R: Read> {
    Script(BufReader<R>),
    Keys(Box<dyn FnMut() -> io::Result<PromptKey>>),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SessionState {
    Active,
    Suspended,
    Closed,
}

/// Close or suspend a prompt session before starting network work.
pub struct PromptSession<R: Read, W: Write> {
    input: InputSource<R>,
    output: W,
    columns: usize,
    rows: usize,
    raw: Option<RawPrompt>,
    state: SessionState,
}

impl<R: Read, W: Write> PromptSession<R, W> {
    pub fn script(reader: R, writer: W) -> Self {
        Self {
            input: InputSource::Script(BufReader::new(reader)),
            output: writer,
            columns: 80,
            rows: 24,
            raw: None,
            state: SessionState::Active,
        }
    }

    /// Injectable key source for public state-machine tests and confined QA.
    pub fn keys(
        writer: W,
        columns: usize,
        rows: usize,
        next_key: impl FnMut() -> io::Result<PromptKey> + 'static,
    ) -> Result<Self, AppError> {
        if columns < 4 || rows < 3 {
            return Err(invariant(
                "prompt terminal must be at least 4 columns and 3 rows",
            ));
        }
        Ok(Self {
            input: InputSource::Keys(Box::new(next_key)),
            output: writer,
            columns,
            rows,
            raw: None,
            state: SessionState::Active,
        })
    }

    /// Read a text answer. Validation examines raw input; accepted answers are trimmed.
    pub fn text(
        &mut self,
        message: &str,
        min_length: usize,
        validate: impl Fn(&str) -> Result<(), String>,
    ) -> Result<PromptOutcome<String>, AppError> {
        self.check_ready(message)?;
        match &mut self.input {
            InputSource::Script(_) => {
                self.write(format!("? {message}\n").as_bytes())?;
                self.flush()?;
                let Some(raw) = self.read_script_line()? else {
                    return Ok(PromptOutcome::EndOfInput);
                };
                validate_text(&raw, min_length, &validate)
                    .map_err(|reason| AppError::new(AppErrorKind::Validation, reason))?;
                self.write(format!("? {message} › {}\n", raw.trim()).as_bytes())?;
                Ok(PromptOutcome::Submitted(raw.trim().to_owned()))
            }
            InputSource::Keys(_) => self.text_keys(message, min_length, validate),
        }
    }

    /// Confirm a destructive action. Only an exactly empty answer takes the
    /// default; explicit whitespace and padded answers are invalid.
    pub fn confirm(
        &mut self,
        message: &str,
        default: bool,
    ) -> Result<PromptOutcome<bool>, AppError> {
        self.check_ready(message)?;
        let header = format!("{message} ({})", if default { "Y/n" } else { "y/N" });
        let parse = |raw: &str| {
            parse_confirmation(raw, default)
                .map(|answer| (answer, if answer { "Yes" } else { "No" }.to_owned()))
        };
        match &mut self.input {
            InputSource::Script(_) => {
                self.write(format!("? {header}\n").as_bytes())?;
                self.flush()?;
                let Some(raw) = self.read_script_line()? else {
                    return Ok(PromptOutcome::EndOfInput);
                };
                let (answer, label) = parse(&raw)
                    .map_err(|reason| AppError::new(AppErrorKind::Validation, reason))?;
                self.write(format!("? {message} › {label}\n").as_bytes())?;
                Ok(PromptOutcome::Submitted(answer))
            }
            InputSource::Keys(_) => self.edit_keys(&header, message, parse),
        }
    }

    pub fn select(&mut self, select: &PlainSelect<'_>) -> Result<PromptOutcome<String>, AppError> {
        self.check_ready(select.message)?;
        validate_select(select)?;
        match &mut self.input {
            InputSource::Script(_) => {
                self.write(format!("{}\n", select_header(select)).as_bytes())?;
                self.flush()?;
                let Some(line) = self.read_script_line()? else {
                    return Ok(PromptOutcome::EndOfInput);
                };
                let index = script_selection(&line, select)?;
                let option = select
                    .options
                    .get(index)
                    .ok_or_else(|| invariant("selection index vanished"))?;
                self.write(format!("? {} › {}\n", select.message, option.label).as_bytes())?;
                Ok(PromptOutcome::Submitted(option.value.clone()))
            }
            InputSource::Keys(_) => self.select_keys(select),
        }
    }

    /// Restore the terminal and cursor before network work, retaining unread
    /// script bytes and the key source. A suspended session cannot prompt.
    pub fn suspend(&mut self) -> Result<(), AppError> {
        if self.state != SessionState::Active {
            return Err(invariant("only an active prompt session can suspend"));
        }
        self.state = SessionState::Suspended;
        self.restore_terminal()
    }

    /// Re-enter prompt mode after the network call, without replacing input.
    pub fn resume(&mut self) -> Result<(), AppError> {
        if self.state != SessionState::Suspended {
            return Err(invariant("only a suspended prompt session can resume"));
        }
        if let Some(raw) = &mut self.raw {
            raw.resume()?;
        }
        self.state = SessionState::Active;
        Ok(())
    }

    /// Explicitly restore terminal state. Cleanup errors override any prompt outcome.
    pub fn close(&mut self) -> Result<(), AppError> {
        if self.state == SessionState::Closed {
            return Ok(());
        }
        self.state = SessionState::Closed;
        self.restore_terminal()
    }

    fn restore_terminal(&mut self) -> Result<(), AppError> {
        let restore_error = self.raw.as_mut().and_then(|raw| raw.restore().err());
        let cursor_error = if self.raw.is_some() {
            self.write(b"\x1b[?25h").err()
        } else {
            None
        };
        let flush_error = self.flush().err();
        let mut errors = [restore_error, cursor_error, flush_error]
            .into_iter()
            .flatten();
        let Some(mut primary) = errors.next() else {
            return Ok(());
        };
        for secondary in errors {
            primary.message.push_str(&format!(
                "; cleanup also failed: {}",
                secondary.display_message()
            ));
        }
        Err(primary)
    }

    /// Cleanup must succeed before an interrupt, EOF, or answer is returned to
    /// the command. This makes a restoration failure take precedence.
    pub fn finish<T>(&mut self, outcome: PromptOutcome<T>) -> Result<PromptOutcome<T>, AppError> {
        self.close()?;
        Ok(outcome)
    }

    /// Always attempt cleanup, preserving both errors if the prompt and
    /// restoration or final flush fail. Cleanup is the primary error.
    pub fn finish_result<T>(
        &mut self,
        result: Result<PromptOutcome<T>, AppError>,
    ) -> Result<PromptOutcome<T>, AppError> {
        match (result, self.close()) {
            (Ok(outcome), Ok(())) => Ok(outcome),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(prompt), Err(mut cleanup)) => {
                cleanup.message.push_str(&format!(
                    "; prompt also failed: {}",
                    prompt.display_message()
                ));
                Err(cleanup)
            }
        }
    }

    pub fn into_output(mut self) -> Result<W, AppError> {
        self.close()?;
        Ok(self.output)
    }

    fn check_ready(&self, message: &str) -> Result<(), AppError> {
        if self.state != SessionState::Active
            || message.trim().is_empty()
            || message.chars().any(char::is_control)
        {
            return Err(invariant("prompt session and message must be valid"));
        }
        Ok(())
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), AppError> {
        self.output.write_all(bytes).map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "failed to write prompt stdout")
                .with_source(error)
        })
    }

    fn flush(&mut self) -> Result<(), AppError> {
        self.output.flush().map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "failed to flush prompt stdout")
                .with_source(error)
        })
    }

    fn read_script_line(&mut self) -> Result<Option<String>, AppError> {
        let InputSource::Script(reader) = &mut self.input else {
            return Err(invariant("script read requires script input"));
        };
        let mut bytes = Vec::new();
        loop {
            let mut one = [0_u8; 1];
            let count = reader.read(&mut one).map_err(|error| {
                AppError::new(AppErrorKind::IoProcess, "failed to read prompt stdin")
                    .with_source(error)
            })?;
            if count == 0 {
                if bytes.is_empty() {
                    return Ok(None);
                }
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "incomplete prompt script line at EOF",
                ));
            }
            bytes.push(one[0]);
            if bytes.len() > MAX_LINE_BYTES {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "prompt script line exceeds 65536 bytes",
                ));
            }
            if one[0] == b'\n' {
                break;
            }
        }
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        let value = String::from_utf8(bytes).map_err(|error| {
            AppError::new(AppErrorKind::Validation, "prompt script line is not UTF-8")
                .with_source(error)
        })?;
        if value
            .chars()
            .any(|character| character <= '\u{1f}' || character == '\u{7f}')
        {
            return Err(AppError::new(
                AppErrorKind::Validation,
                "prompt script line contains a control character",
            ));
        }
        Ok(Some(value))
    }

    fn next_key(&mut self) -> Result<PromptKey, AppError> {
        self.flush()?;
        let InputSource::Keys(next) = &mut self.input else {
            return Err(invariant("key read requires terminal input"));
        };
        next().map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "failed to read prompt key").with_source(error)
        })
    }

    fn clear_frame(&mut self, lines: usize) -> Result<(), AppError> {
        if lines > 0 {
            self.write(format!("\x1b[{lines}A\r\x1b[J").as_bytes())?;
        }
        Ok(())
    }

    fn text_keys(
        &mut self,
        message: &str,
        min_length: usize,
        validate: impl Fn(&str) -> Result<(), String>,
    ) -> Result<PromptOutcome<String>, AppError> {
        self.edit_keys(message, message, |raw| {
            validate_text(raw, min_length, &validate)?;
            Ok((raw.trim().to_owned(), raw.trim().to_owned()))
        })
    }

    fn edit_keys<T>(
        &mut self,
        header: &str,
        message: &str,
        parse: impl Fn(&str) -> Result<(T, String), String>,
    ) -> Result<PromptOutcome<T>, AppError> {
        let mut value = Vec::<char>::new();
        let mut cursor = 0_usize;
        let mut drawn = false;
        let mut error_message: Option<String> = None;
        loop {
            if drawn {
                self.write(b"\r\x1b[J")?;
            }
            let raw: String = value.iter().collect();
            let (line, cursor_column) = text_line(header, &value, cursor, self.columns - 1);
            self.write(format!("{line}\n").as_bytes())?;
            let mut lines = 1_usize;
            if let Some(error) = &error_message {
                self.write(
                    format!("{}\n", clip(&format!("✗ {error}"), self.columns - 1)).as_bytes(),
                )?;
                lines += 1;
            }
            self.write(format!("\x1b[{lines}A\r\x1b[{cursor_column}G").as_bytes())?;
            drawn = true;
            match self.next_key()? {
                PromptKey::Character(character) if !character.is_control() => {
                    if raw.len() + character.len_utf8() > MAX_LINE_BYTES {
                        error_message = Some("prompt answer exceeds 65536 bytes".to_owned());
                    } else {
                        value.insert(cursor, character);
                        cursor += 1;
                        error_message = None;
                    }
                }
                PromptKey::Backspace if cursor > 0 => {
                    cursor -= 1;
                    value.remove(cursor);
                    error_message = None;
                }
                PromptKey::Delete if cursor < value.len() => {
                    value.remove(cursor);
                    error_message = None;
                }
                PromptKey::Left => cursor = cursor.saturating_sub(1),
                PromptKey::Right => cursor = (cursor + 1).min(value.len()),
                PromptKey::Home => cursor = 0,
                PromptKey::End => cursor = value.len(),
                PromptKey::Enter => match parse(&raw) {
                    Ok((answer, label)) => {
                        self.write(b"\r\x1b[J")?;
                        self.write(format!("? {message} › {label}\n").as_bytes())?;
                        return Ok(PromptOutcome::Submitted(answer));
                    }
                    Err(reason) => error_message = Some(reason),
                },
                PromptKey::Interrupt => {
                    self.write(b"\r\x1b[J")?;
                    self.write(b"\x1b[?25h")?;
                    return Ok(PromptOutcome::Interrupted);
                }
                PromptKey::EndOfInput => {
                    self.write(b"\r\x1b[J")?;
                    self.write(b"\x1b[?25h")?;
                    return Ok(PromptOutcome::EndOfInput);
                }
                _ => {}
            }
        }
    }

    fn select_keys(&mut self, select: &PlainSelect<'_>) -> Result<PromptOutcome<String>, AppError> {
        let mut active = select.default_index;
        let mut previous_lines = 0_usize;
        let visible = MAX_OPTIONS_VISIBLE.min(self.rows.saturating_sub(2)).max(1);
        loop {
            self.clear_frame(previous_lines)?;
            self.write(format!("{}\n", clip(&select_header(select), self.columns - 1)).as_bytes())?;
            let start = active
                .saturating_sub(visible - 1)
                .min(select.options.len().saturating_sub(visible));
            previous_lines = 1;
            for (index, option) in select.options.iter().enumerate().skip(start).take(visible) {
                let pointer = if index == active { '❯' } else { ' ' };
                self.write(
                    format!(
                        "{}\n",
                        clip(&format!("{pointer} {}", option.label), self.columns - 1)
                    )
                    .as_bytes(),
                )?;
                previous_lines += 1;
            }
            match self.next_key()? {
                PromptKey::Down | PromptKey::Character('j' | 'd' | 'n' | '2' | '\u{4}') => {
                    active = (active + 1) % select.options.len();
                }
                PromptKey::Up | PromptKey::Character('k' | 'u' | 'p' | '8') => {
                    active = active.checked_sub(1).unwrap_or(select.options.len() - 1);
                }
                PromptKey::PageDown | PromptKey::Right | PromptKey::Character('l') => {
                    active = (active + visible).min(select.options.len() - 1);
                }
                PromptKey::PageUp | PromptKey::Left | PromptKey::Character('h') => {
                    active = active.saturating_sub(visible);
                }
                PromptKey::Enter => {
                    let option = select
                        .options
                        .get(active)
                        .ok_or_else(|| invariant("selection index vanished"))?;
                    let outcome = PromptOutcome::Submitted(option.value.clone());
                    let label = option.label.clone();
                    self.clear_frame(previous_lines)?;
                    self.write(format!("? {} › {label}\n", select.message).as_bytes())?;
                    return Ok(outcome);
                }
                PromptKey::Interrupt => {
                    self.clear_frame(previous_lines)?;
                    self.write(b"\x1b[?25h")?;
                    return Ok(PromptOutcome::Interrupted);
                }
                PromptKey::EndOfInput => {
                    self.clear_frame(previous_lines)?;
                    self.write(b"\x1b[?25h")?;
                    return Ok(PromptOutcome::EndOfInput);
                }
                _ => {}
            }
        }
    }
}

impl<W: Write> PromptSession<io::Stdin, W> {
    /// Confirmation is gated by stdin alone. Use a terminal whose attended
    /// check follows stdin, even when both output streams are redirected.
    pub fn confirmation_stdio(writer: W) -> Result<Self, AppError> {
        if !io::stdin().is_terminal() {
            return Err(AppError::new(
                AppErrorKind::Validation,
                "Interactive confirmation required",
            )
            .with_suggestion("Use --force to skip confirmation."));
        }
        let (columns, rows) = terminal_size::terminal_size_of(io::stdin())
            .map(|(terminal_size::Width(w), terminal_size::Height(h))| {
                (usize::from(w), usize::from(h))
            })
            .unwrap_or((80, 24));
        let mut session = Self::keys(writer, columns, rows, confirmation_key)?;
        session.raw = Some(RawPrompt::enter_confirmation()?);
        Ok(session)
    }

    /// Enter the C039 prompt path without applying the search selector's CI or
    /// stdin-TTY gate. The command must decide whether stdout permits prompts.
    pub fn stdio(writer: W) -> Result<Self, AppError> {
        if !io::stdin().is_terminal() {
            return Ok(Self::script(io::stdin(), writer));
        }
        let term = console::Term::stdout();
        let (columns, rows) = terminal_size::terminal_size_of(io::stdout())
            .map(|(terminal_size::Width(w), terminal_size::Height(h))| {
                (usize::from(w), usize::from(h))
            })
            .unwrap_or((80, 24));
        let mut session = Self::keys(writer, columns, rows, move || match term.read_key_raw() {
            Ok(console::Key::Char(character)) => Ok(PromptKey::Character(character)),
            Ok(console::Key::Backspace) => Ok(PromptKey::Backspace),
            Ok(console::Key::Del) => Ok(PromptKey::Delete),
            Ok(console::Key::ArrowLeft) => Ok(PromptKey::Left),
            Ok(console::Key::ArrowRight) => Ok(PromptKey::Right),
            Ok(console::Key::ArrowUp) => Ok(PromptKey::Up),
            Ok(console::Key::ArrowDown) => Ok(PromptKey::Down),
            Ok(console::Key::PageUp) => Ok(PromptKey::PageUp),
            Ok(console::Key::PageDown) => Ok(PromptKey::PageDown),
            Ok(console::Key::Home) => Ok(PromptKey::Home),
            Ok(console::Key::End) => Ok(PromptKey::End),
            Ok(console::Key::Enter) => Ok(PromptKey::Enter),
            Ok(console::Key::CtrlC) => Ok(PromptKey::Interrupt),
            Ok(_) => Ok(PromptKey::Other),
            Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(PromptKey::EndOfInput),
            Err(error) => Err(error),
        })?;
        session.raw = Some(RawPrompt::enter()?);
        Ok(session)
    }
}

/// Read input independently of output using the maintained platform decoder.
/// Existing text/select keep their console key source.
fn confirmation_key() -> io::Result<PromptKey> {
    use crossterm::event::{Event, KeyCode, KeyEventKind, KeyModifiers};
    loop {
        if confirmation_input_ended()? {
            return Ok(PromptKey::EndOfInput);
        }
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            break;
        }
    }
    match crossterm::event::read() {
        Ok(Event::Key(key)) if key.kind != KeyEventKind::Release => Ok(match key.code {
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                PromptKey::Interrupt
            }
            KeyCode::Char(_)
                if key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                PromptKey::Other
            }
            KeyCode::Char(character) => PromptKey::Character(character),
            KeyCode::Backspace => PromptKey::Backspace,
            KeyCode::Delete => PromptKey::Delete,
            KeyCode::Left => PromptKey::Left,
            KeyCode::Right => PromptKey::Right,
            KeyCode::Home => PromptKey::Home,
            KeyCode::End => PromptKey::End,
            KeyCode::Enter => PromptKey::Enter,
            _ => PromptKey::Other,
        }),
        Ok(_) => Ok(PromptKey::Other),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(PromptKey::EndOfInput),
        Err(error) => Err(error),
    }
}

// A disconnected terminal must not enter the decoder's EOF polling loop.
// The bounded library poll also lets us check a hangup arriving while waiting.
#[cfg(all(unix, not(target_vendor = "apple")))]
fn confirmation_input_ended() -> io::Result<bool> {
    use rustix::event::{PollFd, PollFlags, poll};
    let input = io::stdin();
    let mut descriptors = [PollFd::new(&input, PollFlags::IN)];
    poll(
        &mut descriptors,
        Some(&rustix::event::Timespec {
            tv_sec: 0,
            tv_nsec: 0,
        }),
    )?;
    Ok(descriptors[0]
        .revents()
        .intersects(PollFlags::HUP | PollFlags::ERR | PollFlags::NVAL))
}

// Apple poll does not support some terminal descriptors. Let crossterm's
// select backend read input there; terminal disconnect uses ordinary SIGHUP.
#[cfg(any(not(unix), target_vendor = "apple"))]
fn confirmation_input_ended() -> io::Result<bool> {
    Ok(false)
}

fn parse_confirmation(raw: &str, default: bool) -> Result<bool, String> {
    if raw.is_empty() {
        Ok(default)
    } else if raw.eq_ignore_ascii_case("y") || raw.eq_ignore_ascii_case("yes") {
        Ok(true)
    } else if raw.eq_ignore_ascii_case("n") || raw.eq_ignore_ascii_case("no") {
        Ok(false)
    } else {
        Err("Invalid answer.".to_owned())
    }
}

fn validate_text(
    raw: &str,
    min_length: usize,
    validate: &impl Fn(&str) -> Result<(), String>,
) -> Result<(), String> {
    if raw.chars().count() < min_length {
        return Err(format!(
            "answer must contain at least {min_length} character(s)"
        ));
    }
    validate(raw)
}

fn validate_select(select: &PlainSelect<'_>) -> Result<(), AppError> {
    if select.options.is_empty() || select.default_index >= select.options.len() {
        return Err(invariant(
            "plain selection requires options and a valid default",
        ));
    }
    if select
        .default_hint
        .is_some_and(|hint| hint.chars().any(char::is_control))
    {
        return Err(invariant(
            "plain selection default hint contains a control character",
        ));
    }
    for (index, option) in select.options.iter().enumerate() {
        if option.label.trim().is_empty()
            || option.value.trim().is_empty()
            || option.script_token.trim().is_empty()
            || option.label.chars().any(char::is_control)
            || option.value.chars().any(char::is_control)
            || option.script_token.chars().any(char::is_control)
            || select.options.iter().take(index).any(|previous| {
                previous.value == option.value || previous.script_token == option.script_token
            })
        {
            return Err(invariant(
                "plain selection options must have unique nonempty values and tokens",
            ));
        }
    }
    Ok(())
}

fn select_header(select: &PlainSelect<'_>) -> String {
    match select.default_hint {
        Some(hint) => format!("? {} ({hint})", select.message),
        None => format!("? {}", select.message),
    }
}

fn script_selection(line: &str, select: &PlainSelect<'_>) -> Result<usize, AppError> {
    if line.is_empty() {
        return Ok(select.default_index);
    }
    let by_token = select
        .options
        .iter()
        .position(|option| option.script_token == line);
    let by_index = if !line.is_empty() && line.bytes().all(|byte| byte.is_ascii_digit()) {
        line.parse::<usize>()
            .ok()
            .and_then(|number| number.checked_sub(1))
            .filter(|index| *index < select.options.len())
    } else {
        None
    };
    match (by_token, by_index) {
        (Some(left), Some(right)) if left != right => Err(AppError::new(
            AppErrorKind::Validation,
            "ambiguous numeric prompt selection",
        )),
        (Some(index), _) | (_, Some(index)) => Ok(index),
        (None, None) => Err(AppError::new(
            AppErrorKind::Validation,
            "unknown prompt selection; use a menu number or exact choice token",
        )),
    }
}

fn clip(value: &str, limit: usize) -> String {
    let mut output = String::new();
    let mut width = 0_usize;
    for character in value.chars() {
        let next = character.width().unwrap_or(0);
        if width + next > limit {
            break;
        }
        output.push(character);
        width += next;
    }
    output
}

/// Keep the edit cursor and nearby text visible within one physical row.
fn text_line(message: &str, value: &[char], cursor: usize, limit: usize) -> (String, usize) {
    let prefix = clip(&format!("? {message} › "), limit.saturating_sub(2));
    let prefix_width: usize = prefix
        .chars()
        .map(|character| character.width().unwrap_or(0))
        .sum();
    let available = limit.saturating_sub(prefix_width).max(1);
    let mut start = cursor.min(value.len());
    let mut before_width = 0_usize;
    while start > 0 {
        let Some(character) = value.get(start - 1) else {
            break;
        };
        let next = character.width().unwrap_or(0);
        if before_width + next >= available {
            break;
        }
        before_width += next;
        start -= 1;
    }
    let mut shown = String::new();
    let mut shown_width = 0_usize;
    for character in value.iter().skip(start) {
        let next = character.width().unwrap_or(0);
        if shown_width + next > available {
            break;
        }
        shown.push(*character);
        shown_width += next;
    }
    let cursor_column = (prefix_width + before_width + 1).min(limit + 1);
    (format!("{prefix}{shown}"), cursor_column)
}

fn invariant(message: &str) -> AppError {
    AppError::new(AppErrorKind::Invariant, message)
}

#[cfg(unix)]
struct RawPrompt {
    input: io::Stdin,
    original: rustix::termios::Termios,
    restore_attempted: bool,
}

#[cfg(unix)]
impl RawPrompt {
    fn enter_confirmation() -> Result<Self, AppError> {
        Self::enter()
    }

    fn enter() -> Result<Self, AppError> {
        use rustix::termios::{OptionalActions, tcgetattr, tcsetattr};
        let input = io::stdin();
        let original = tcgetattr(&input).map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "failed to read terminal mode")
                .with_source(error)
        })?;
        let mut raw = original.clone();
        raw.make_raw();
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

    fn resume(&mut self) -> Result<(), AppError> {
        let mut raw = self.original.clone();
        raw.make_raw();
        raw.output_modes = self.original.output_modes;
        rustix::termios::tcsetattr(&self.input, rustix::termios::OptionalActions::Now, &raw)
            .map_err(|error| {
                AppError::new(AppErrorKind::IoProcess, "failed to enable terminal input")
                    .with_source(error)
            })?;
        self.restore_attempted = false;
        Ok(())
    }

    fn restore(&mut self) -> Result<(), AppError> {
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

#[cfg(not(unix))]
struct RawPrompt {
    confirmation: bool,
    active: bool,
}

#[cfg(not(unix))]
impl RawPrompt {
    fn enter() -> Result<Self, AppError> {
        Ok(Self {
            confirmation: false,
            active: false,
        })
    }
    fn enter_confirmation() -> Result<Self, AppError> {
        crossterm::terminal::enable_raw_mode().map_err(|error| {
            AppError::new(AppErrorKind::IoProcess, "failed to enable terminal input")
                .with_source(error)
        })?;
        Ok(Self {
            confirmation: true,
            active: true,
        })
    }
    fn resume(&mut self) -> Result<(), AppError> {
        if self.confirmation {
            crossterm::terminal::enable_raw_mode().map_err(|error| {
                AppError::new(AppErrorKind::IoProcess, "failed to enable terminal input")
                    .with_source(error)
            })?;
            self.active = true;
        }
        Ok(())
    }
    fn restore(&mut self) -> Result<(), AppError> {
        if self.active {
            self.active = false;
            crossterm::terminal::disable_raw_mode().map_err(|error| {
                AppError::new(AppErrorKind::IoProcess, "failed to restore terminal input")
                    .with_source(error)
            })?;
        }
        Ok(())
    }
}

#[cfg(not(unix))]
impl Drop for RawPrompt {
    fn drop(&mut self) {
        if let Err(error) = self.restore() {
            let _ = writeln!(io::stderr(), "failed to restore terminal input: {error}");
        }
    }
}
