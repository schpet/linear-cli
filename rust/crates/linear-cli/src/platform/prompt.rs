//! Reusable text, confirmation, and plain-list prompts.
//!
//! The caller decides whether to prompt. A terminal stdin uses individual keys;
//! a non-terminal stdin uses a deterministic, line-oriented script protocol.
//! One session retains unread script bytes and owns terminal restoration across
//! every prompt in a command.

use std::io::{self, BufReader, IsTerminal, Read, Write};

use unicode_width::UnicodeWidthChar;

use crate::error::Error;

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

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScriptFraming {
    LfOnly,
    CrOrLf,
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
    framing: ScriptFraming,
    pending_optional_lf: bool,
}

impl<R: Read, W: Write> PromptSession<R, W> {
    pub fn checkbox(
        &mut self,
        message: &str,
        options: &[PlainOption],
        searchable: bool,
    ) -> Result<PromptOutcome<Vec<String>>, Error> {
        self.check_ready(message)?;
        // Existing strict member/token domain, no implicit sanitizer of raw values.
        if options.is_empty() {
            return Ok(PromptOutcome::Submitted(Vec::new()));
        }
        validate_select(&PlainSelect {
            message,
            options,
            default_index: 0,
            default_hint: None,
        })?;
        let mut selected = vec![false; options.len()];
        if matches!(&self.input, InputSource::Script(_)) {
            self.write(format!("? {message}\n").as_bytes())?;
            self.flush()?;
            let Some(line) = self.read_script_line()? else {
                return Ok(PromptOutcome::EndOfInput);
            };
            // Native declared comma-separated tokens. Exactly empty means none;
            // whitespace or unknown tokens refuse rather than select a default.
            if !line.is_empty() {
                for token in line.split(',') {
                    let index = options
                        .iter()
                        .position(|o| o.script_token == token)
                        .ok_or_else(|| {
                            Error::new(format!(
                                "Unknown checkbox member: {}",
                                escaped_display(token)
                            ))
                        })?;
                    let member = selected
                        .get_mut(index)
                        .unwrap_or_else(|| unreachable!("option index is within selection"));
                    if *member {
                        return Err(Error::new("Checkbox member was submitted more than once"));
                    }
                    *member = true;
                }
            }
        } else {
            let mut search = String::new();
            let mut cursor = 0_usize;
            let mut old_lines = 0_usize;
            loop {
                let visible: Vec<usize> = options
                    .iter()
                    .enumerate()
                    .filter(|(_, o)| o.label.to_lowercase().contains(&search.to_lowercase()))
                    .map(|(index, _)| index)
                    .collect();
                cursor = cursor.min(visible.len().saturating_sub(1));
                self.clear_frame(old_lines)?;
                let count = MAX_OPTIONS_VISIBLE.min(self.rows.saturating_sub(3)).max(1);
                let start = cursor.saturating_sub(count.saturating_sub(1));
                let rows: Vec<_> = visible.iter().skip(start).take(count).copied().collect();
                self.write(format!("? {}\n", escaped_display(message)).as_bytes())?;
                old_lines = 1;
                if searchable {
                    self.write(format!("Search: {}\n", escaped_display(&search)).as_bytes())?;
                    old_lines += 1;
                }
                if rows.is_empty() {
                    self.write(b"No matching options\n")?;
                    old_lines += 1;
                }
                for (offset, index) in rows.iter().enumerate() {
                    self.write(
                        format!(
                            "{} [{}] {}\n",
                            if start + offset == cursor { "›" } else { " " },
                            if *selected.get(*index).unwrap_or_else(|| unreachable!(
                                "visible option is within selection"
                            )) {
                                "x"
                            } else {
                                " "
                            },
                            escaped_display(
                                &options
                                    .get(*index)
                                    .unwrap_or_else(|| unreachable!(
                                        "visible option is within options"
                                    ))
                                    .label
                            )
                        )
                        .as_bytes(),
                    )?;
                    old_lines += 1;
                }
                match self.next_key()? {
                    PromptKey::Interrupt => {
                        self.clear_frame(old_lines)?;
                        return Ok(PromptOutcome::Interrupted);
                    }
                    PromptKey::EndOfInput => {
                        self.clear_frame(old_lines)?;
                        return Ok(PromptOutcome::EndOfInput);
                    }
                    PromptKey::Enter => {
                        self.clear_frame(old_lines)?;
                        break;
                    }
                    PromptKey::Character(' ') => {
                        if let Some(index) = visible.get(cursor) {
                            let member = selected.get_mut(*index).unwrap_or_else(|| {
                                unreachable!("visible option is within selection")
                            });
                            *member = !*member;
                        }
                    }
                    PromptKey::Up => cursor = cursor.saturating_sub(1),
                    PromptKey::Down => cursor = (cursor + 1).min(visible.len().saturating_sub(1)),
                    PromptKey::Home => cursor = 0,
                    PromptKey::End => cursor = visible.len().saturating_sub(1),
                    PromptKey::Backspace if searchable => {
                        search.pop();
                        cursor = 0
                    }
                    PromptKey::Character(character) if searchable && !character.is_control() => {
                        search.push(character);
                        cursor = 0
                    }
                    PromptKey::Character(_)
                    | PromptKey::Left
                    | PromptKey::Right
                    | PromptKey::PageUp
                    | PromptKey::PageDown
                    | PromptKey::Backspace
                    | PromptKey::Delete
                    | PromptKey::Other => (),
                }
            }
        }
        let labels = options
            .iter()
            .zip(&selected)
            .filter(|(_, on)| **on)
            .map(|(option, _)| escaped_display(&option.label))
            .collect::<Vec<_>>();
        self.write(format!("? {message} › {}\n", labels.join(", ")).as_bytes())?;
        self.flush()?;
        Ok(PromptOutcome::Submitted(
            options
                .iter()
                .zip(selected)
                .filter(|(_, on)| *on)
                .map(|(option, _)| option.value.clone())
                .collect(),
        ))
    }

    /// A searchable team picker; see [`Self::searchable_select_with_no_match`].
    pub fn searchable_select(
        &mut self,
        message: &str,
        search_label: &str,
        options: &[crate::platform::selector::SelectOption],
    ) -> Result<PromptOutcome<String>, Error> {
        self.searchable_select_with_no_match(
            message,
            search_label,
            options,
            "no teams match submitted search query",
        )
    }

    /// A select list filtered by typed search text. `no_match` is the error
    /// when a submitted search matches nothing.
    pub fn searchable_select_with_no_match(
        &mut self,
        message: &str,
        search_label: &str,
        options: &[crate::platform::selector::SelectOption],
        no_match: &str,
    ) -> Result<PromptOutcome<String>, Error> {
        use crate::platform::selector::{Key, Selection, Selector};
        self.check_ready(message)?;
        self.check_ready(search_label)?;
        let mut selector = Selector::new(options)
            .map_err(|error| Error::new(error.to_string()).with_source(error))?;
        match &mut self.input {
            InputSource::Script(_) => {
                self.write(format!("? {message}\n{search_label}:\n").as_bytes())?;
                self.flush()?;
                let Some(query) = self.read_script_line()? else {
                    return Ok(PromptOutcome::EndOfInput);
                };
                for character in query.chars() {
                    selector.on_key(Key::Character(character));
                }
                match selector.on_key(Key::Enter) {
                    Some(Selection::Selected(value)) => {
                        let label = selector
                            .active_label()
                            .ok_or_else(|| invariant("selected searchable label vanished"))?;
                        self.write(
                            format!("? {message} › {}\n", escaped_display(label)).as_bytes(),
                        )?;
                        Ok(PromptOutcome::Submitted(value))
                    }
                    None => Err(Error::new(no_match)),
                    Some(Selection::Interrupted | Selection::EndOfInput) => {
                        Err(invariant("script Enter produced non-selection control"))
                    }
                }
            }
            InputSource::Keys(_) => {
                let mut previous_lines = 0_usize;
                let visible = MAX_OPTIONS_VISIBLE.min(self.rows.saturating_sub(3)).max(1);
                loop {
                    self.clear_frame(previous_lines)?;
                    let header = format!(
                        "? {message}  {search_label}: {}",
                        escaped_display(selector.query())
                    );
                    self.write(format!("{}\n", clip(&header, self.columns - 1)).as_bytes())?;
                    previous_lines = 1;
                    let active = selector.active_index();
                    let start = active.saturating_sub(visible - 1);
                    let mut shown = 0;
                    for (offset, option) in selector.visible().skip(start).take(visible).enumerate()
                    {
                        let marker = if start + offset == active { '❯' } else { ' ' };
                        self.write(
                            format!(
                                "{}\n",
                                clip(
                                    &format!("{marker} {}", escaped_display(&option.label)),
                                    self.columns - 1
                                )
                            )
                            .as_bytes(),
                        )?;
                        previous_lines += 1;
                        shown += 1;
                    }
                    if shown == 0 {
                        self.write(b"  No matches\n")?;
                        previous_lines += 1;
                    }
                    let key = match self.next_key()? {
                        PromptKey::Character(c) => Key::Character(c),
                        PromptKey::Backspace => Key::Backspace,
                        PromptKey::Up => Key::Up,
                        PromptKey::Down => Key::Down,
                        PromptKey::Enter => Key::Enter,
                        PromptKey::Interrupt => Key::Interrupt,
                        PromptKey::EndOfInput => Key::EndOfInput,
                        PromptKey::Left
                        | PromptKey::Right
                        | PromptKey::PageUp
                        | PromptKey::PageDown
                        | PromptKey::Home
                        | PromptKey::End
                        | PromptKey::Delete
                        | PromptKey::Other => Key::Other,
                    };
                    match selector.on_key(key) {
                        Some(Selection::Selected(value)) => {
                            let label = selector
                                .active_label()
                                .ok_or_else(|| invariant("selected searchable label vanished"))?;
                            self.clear_frame(previous_lines)?;
                            self.write(
                                format!("? {message} › {}\n", escaped_display(label)).as_bytes(),
                            )?;
                            return Ok(PromptOutcome::Submitted(value));
                        }
                        Some(Selection::Interrupted) => {
                            self.clear_frame(previous_lines)?;
                            return Ok(PromptOutcome::Interrupted);
                        }
                        Some(Selection::EndOfInput) => {
                            self.clear_frame(previous_lines)?;
                            return Ok(PromptOutcome::EndOfInput);
                        }
                        None => {} // Enter with no match cannot finish; Backspace can recover.
                    }
                }
            }
        }
    }

    pub fn script(reader: R, writer: W) -> Self {
        Self {
            input: InputSource::Script(BufReader::new(reader)),
            output: writer,
            columns: 80,
            rows: 24,
            raw: None,
            state: SessionState::Active,
            framing: ScriptFraming::LfOnly,
            pending_optional_lf: false,
        }
    }

    /// A script session where CR submits immediately and a following LF is skipped.
    pub fn script_cr_or_lf(reader: R, writer: W) -> Self {
        let mut session = Self::script(reader, writer);
        session.framing = ScriptFraming::CrOrLf;
        session
    }

    /// A session driven by injected keys, for tests.
    pub fn keys(
        writer: W,
        columns: usize,
        rows: usize,
        next_key: impl FnMut() -> io::Result<PromptKey> + 'static,
    ) -> Result<Self, Error> {
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
            framing: ScriptFraming::LfOnly,
            pending_optional_lf: false,
        })
    }

    /// Read a text answer. Validation examines raw input; accepted answers are trimmed.
    pub fn text(
        &mut self,
        message: &str,
        min_length: usize,
        validate: impl Fn(&str) -> Result<(), String>,
    ) -> Result<PromptOutcome<String>, Error> {
        self.check_ready(message)?;
        match &mut self.input {
            InputSource::Script(_) => {
                self.write(format!("? {message}\n").as_bytes())?;
                self.flush()?;
                let Some(raw) = self.read_script_line()? else {
                    return Ok(PromptOutcome::EndOfInput);
                };
                validate_text(&raw, min_length, &validate).map_err(Error::new)?;
                self.write(format!("? {message} › {}\n", raw.trim()).as_bytes())?;
                Ok(PromptOutcome::Submitted(raw.trim().to_owned()))
            }
            InputSource::Keys(_) => self.text_keys(message, min_length, validate),
        }
    }

    /// A text prompt with [`crate::platform::prompt_text::TextOptions`]: an optional default and a required flag.
    pub fn text_with_options(
        &mut self,
        message: &str,
        options: crate::platform::prompt_text::TextOptions<'_>,
    ) -> Result<PromptOutcome<String>, Error> {
        self.check_ready(message)?;
        options.preflight().map_err(Error::new)?;
        let header = match options.default {
            Some(value) => format!("{message} ({value})"),
            None => message.to_owned(),
        };
        match &mut self.input {
            InputSource::Script(_) => {
                self.write(format!("? {header}\n").as_bytes())?;
                self.flush()?;
                let Some(raw) = self.read_script_line()? else {
                    return Ok(PromptOutcome::EndOfInput);
                };
                let answer = options.answer(&raw).map_err(Error::new)?;
                self.write(format!("? {message} › {answer}\n").as_bytes())?;
                Ok(PromptOutcome::Submitted(answer))
            }
            InputSource::Keys(_) => self.edit_keys(&header, message, |raw| {
                let answer = options.answer(raw)?;
                Ok((answer.clone(), answer))
            }),
        }
    }

    /// Like [`Self::text_with_options`], but control characters in the default
    /// and answer are escaped when displayed.
    pub fn text_with_display_default(
        &mut self,
        message: &str,
        options: crate::platform::prompt_text::TextOptions<'_>,
    ) -> Result<PromptOutcome<String>, Error> {
        self.check_ready(message)?;
        let header = match options.default {
            Some(value) => format!("{message} ({})", escaped_display(value)),
            None => message.to_owned(),
        };
        match &mut self.input {
            InputSource::Script(_) => {
                self.write(format!("? {header}\n").as_bytes())?;
                self.flush()?;
                let Some(raw) = self.read_script_line()? else {
                    return Ok(PromptOutcome::EndOfInput);
                };
                let answer = options.answer(&raw).map_err(Error::new)?;
                self.write(format!("? {message} › {}\n", escaped_display(&answer)).as_bytes())?;
                Ok(PromptOutcome::Submitted(answer))
            }
            InputSource::Keys(_) => self.edit_keys(&header, message, |raw| {
                let answer = options.answer(raw)?;
                let display = escaped_display(&answer);
                Ok((answer, display))
            }),
        }
    }

    pub fn secret(
        &mut self,
        message: &str,
        hint: &str,
    ) -> Result<PromptOutcome<crate::config::ConfigSecret>, Error> {
        self.check_ready(message)?;
        let parse = |raw: &str| {
            let value = raw.trim().to_owned();
            let mask = "*".repeat(value.chars().count());
            Ok((crate::config::ConfigSecret::new(value), mask))
        };
        let header = format!("{message} ({hint})");
        match &mut self.input {
            InputSource::Script(_) => {
                self.write(format!("? {header}\n").as_bytes())?;
                self.flush()?;
                let Some(raw) = self.read_script_line()? else {
                    return Ok(PromptOutcome::EndOfInput);
                };
                let (answer, mask) = parse(&raw).map_err(|reason: String| Error::new(reason))?;
                self.write(format!("? {message} › {mask}\n").as_bytes())?;
                Ok(PromptOutcome::Submitted(answer))
            }
            InputSource::Keys(_) => {
                self.edit_keys_with_display(&header, message, parse, |value, cursor| {
                    (vec!['*'; value.len()], cursor)
                })
            }
        }
    }

    /// Confirm a destructive action. Only an exactly empty answer takes the
    /// default; explicit whitespace and padded answers are invalid.
    pub fn confirm(&mut self, message: &str, default: bool) -> Result<PromptOutcome<bool>, Error> {
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
                let (answer, label) = parse(&raw).map_err(Error::new)?;
                self.write(format!("? {message} › {label}\n").as_bytes())?;
                Ok(PromptOutcome::Submitted(answer))
            }
            InputSource::Keys(_) => self.edit_keys(&header, message, parse),
        }
    }

    pub fn select(&mut self, select: &PlainSelect<'_>) -> Result<PromptOutcome<String>, Error> {
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
    pub fn suspend(&mut self) -> Result<(), Error> {
        if self.state != SessionState::Active {
            return Err(invariant("only an active prompt session can suspend"));
        }
        self.state = SessionState::Suspended;
        self.restore_terminal()
    }

    /// Re-enter prompt mode after the network call, without replacing input.
    pub fn resume(&mut self) -> Result<(), Error> {
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
    pub fn close(&mut self) -> Result<(), Error> {
        if self.state == SessionState::Closed {
            return Ok(());
        }
        self.state = SessionState::Closed;
        self.restore_terminal()
    }

    fn restore_terminal(&mut self) -> Result<(), Error> {
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
            primary.push_message(&format!("; cleanup also failed: {}", secondary));
        }
        Err(primary)
    }

    /// Cleanup must succeed before an interrupt, EOF, or answer is returned to
    /// the command. This makes a restoration failure take precedence.
    pub fn finish<T>(&mut self, outcome: PromptOutcome<T>) -> Result<PromptOutcome<T>, Error> {
        self.close()?;
        Ok(outcome)
    }

    /// Always attempt cleanup, preserving both errors if the prompt and
    /// restoration or final flush fail. Cleanup is the primary error.
    pub fn finish_result<T>(
        &mut self,
        result: Result<PromptOutcome<T>, Error>,
    ) -> Result<PromptOutcome<T>, Error> {
        match (result, self.close()) {
            (Ok(outcome), Ok(())) => Ok(outcome),
            (Err(error), Ok(())) => Err(error),
            (Ok(_), Err(error)) => Err(error),
            (Err(prompt), Err(mut cleanup)) => {
                cleanup.push_message(&format!("; prompt also failed: {}", prompt));
                Err(cleanup)
            }
        }
    }

    pub fn into_output(mut self) -> Result<W, Error> {
        self.close()?;
        Ok(self.output)
    }

    /// Write a command's ordinary status line while retaining the session input.
    pub fn print_line(&mut self, line: &str) -> Result<(), Error> {
        self.write(format!("{line}\n").as_bytes())?;
        self.flush()
    }

    fn check_ready(&self, message: &str) -> Result<(), Error> {
        if self.state != SessionState::Active
            || message.trim().is_empty()
            || message.chars().any(char::is_control)
        {
            return Err(invariant("prompt session and message must be valid"));
        }
        Ok(())
    }

    fn write(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.output
            .write_all(bytes)
            .map_err(|error| Error::new("failed to write prompt stdout").with_source(error))
    }

    fn flush(&mut self) -> Result<(), Error> {
        self.output
            .flush()
            .map_err(|error| Error::new("failed to flush prompt stdout").with_source(error))
    }

    fn read_script_line(&mut self) -> Result<Option<String>, Error> {
        if self.framing == ScriptFraming::CrOrLf {
            return self.read_script_cr_or_lf();
        }
        let InputSource::Script(reader) = &mut self.input else {
            return Err(invariant("script read requires script input"));
        };
        let mut bytes = Vec::new();
        loop {
            let mut one = [0_u8; 1];
            let count = reader
                .read(&mut one)
                .map_err(|error| Error::new("failed to read prompt stdin").with_source(error))?;
            if count == 0 {
                if bytes.is_empty() {
                    return Ok(None);
                }
                return Err(Error::new("incomplete prompt script line at EOF"));
            }
            bytes.push(one[0]);
            if bytes.len() > MAX_LINE_BYTES {
                return Err(Error::new("prompt script line exceeds 65536 bytes"));
            }
            if one[0] == b'\n' {
                break;
            }
        }
        bytes.pop();
        if bytes.last() == Some(&b'\r') {
            bytes.pop();
        }
        let value = String::from_utf8(bytes)
            .map_err(|error| Error::new("prompt script line is not UTF-8").with_source(error))?;
        if value
            .chars()
            .any(|character| character <= '\u{1f}' || character == '\u{7f}')
        {
            return Err(Error::new(
                "prompt script line contains a control character",
            ));
        }
        Ok(Some(value))
    }

    fn read_script_cr_or_lf(&mut self) -> Result<Option<String>, Error> {
        let InputSource::Script(reader) = &mut self.input else {
            return Err(invariant("script read requires script input"));
        };
        let mut bytes = Vec::new();
        loop {
            let mut byte = [0];
            let count = reader
                .read(&mut byte)
                .map_err(|error| Error::new("failed to read prompt stdin").with_source(error))?;
            if count == 0 {
                return if bytes.is_empty() {
                    Ok(None)
                } else {
                    Err(Error::new("incomplete prompt script line at EOF"))
                };
            }
            if self.pending_optional_lf {
                self.pending_optional_lf = false;
                if byte[0] == b'\n' {
                    continue;
                }
            }
            if bytes.len() + 1 > MAX_LINE_BYTES {
                return Err(Error::new("prompt script line exceeds 65536 bytes"));
            }
            match byte[0] {
                b'\r' => {
                    self.pending_optional_lf = true;
                    break;
                }
                b'\n' => break,
                value => bytes.push(value),
            }
        }
        let value = String::from_utf8(bytes)
            .map_err(|error| Error::new("prompt script line is not UTF-8").with_source(error))?;
        if value
            .chars()
            .any(|character| character <= '\u{1f}' || character == '\u{7f}')
        {
            return Err(Error::new(
                "prompt script line contains a control character",
            ));
        }
        Ok(Some(value))
    }

    fn next_key(&mut self) -> Result<PromptKey, Error> {
        self.flush()?;
        let InputSource::Keys(next) = &mut self.input else {
            return Err(invariant("key read requires terminal input"));
        };
        next().map_err(|error| Error::new("failed to read prompt key").with_source(error))
    }

    fn clear_frame(&mut self, lines: usize) -> Result<(), Error> {
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
    ) -> Result<PromptOutcome<String>, Error> {
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
    ) -> Result<PromptOutcome<T>, Error> {
        self.edit_keys_with_display(header, message, parse, |value, cursor| {
            (value.to_vec(), cursor)
        })
    }

    fn edit_keys_with_display<T>(
        &mut self,
        header: &str,
        message: &str,
        parse: impl Fn(&str) -> Result<(T, String), String>,
        display: impl Fn(&[char], usize) -> (Vec<char>, usize),
    ) -> Result<PromptOutcome<T>, Error> {
        let mut value = Vec::<char>::new();
        let mut cursor = 0_usize;
        let mut drawn = false;
        let mut error_message: Option<String> = None;
        loop {
            if drawn {
                self.write(b"\r\x1b[J")?;
            }
            let raw: String = value.iter().collect();
            let (shown, shown_cursor) = display(&value, cursor);
            let (line, cursor_column) = text_line(header, &shown, shown_cursor, self.columns - 1);
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

    fn select_keys(&mut self, select: &PlainSelect<'_>) -> Result<PromptOutcome<String>, Error> {
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
    /// Choose attended keys or the script protocol from stdin alone. The caller
    /// controls prompt eligibility; stdout and CI do not affect this constructor.
    pub fn stdin_stdio(writer: W) -> Result<Self, Error> {
        if io::stdin().is_terminal() {
            Self::attended(writer, attended_key)
        } else {
            Ok(Self::script(io::stdin(), writer))
        }
    }

    /// Like [`Self::stdin_stdio`], but a scripted CR submits immediately and a
    /// following LF is skipped.
    pub fn stdin_stdio_cr_or_lf(writer: W) -> Result<Self, Error> {
        let mut session = Self::stdin_stdio(writer)?;
        session.framing = ScriptFraming::CrOrLf;
        Ok(session)
    }

    fn attended(
        writer: W,
        next_key: impl FnMut() -> io::Result<PromptKey> + 'static,
    ) -> Result<Self, Error> {
        let (columns, rows) = terminal_size::terminal_size_of(io::stdin())
            .map(|(terminal_size::Width(w), terminal_size::Height(h))| {
                (usize::from(w), usize::from(h))
            })
            .unwrap_or((80, 24));
        let mut session = Self::keys(writer, columns, rows, next_key)?;
        session.raw = Some(RawPrompt::enter()?);
        Ok(session)
    }
}

/// Read input independently of output using the maintained platform decoder.
fn attended_key() -> io::Result<PromptKey> {
    use crossterm::event::Event;
    loop {
        if attended_input_ended()? {
            return Ok(PromptKey::EndOfInput);
        }
        if crossterm::event::poll(std::time::Duration::from_millis(100))? {
            break;
        }
    }
    match crossterm::event::read() {
        Ok(Event::Key(key)) => Ok(attended_key_event(key)),
        Ok(_) => Ok(PromptKey::Other),
        Err(error) if error.kind() == io::ErrorKind::UnexpectedEof => Ok(PromptKey::EndOfInput),
        Err(error) => Err(error),
    }
}

fn attended_key_event(key: crossterm::event::KeyEvent) -> PromptKey {
    use crossterm::event::{KeyCode, KeyEventKind, KeyModifiers};
    if key.kind == KeyEventKind::Release {
        return PromptKey::Other;
    }
    let control = key.modifiers.contains(KeyModifiers::CONTROL);
    let alt = key.modifiers.contains(KeyModifiers::ALT);
    match key.code {
        KeyCode::Char('c') if control && !alt => PromptKey::Interrupt,
        KeyCode::Char('h') if control && !alt => PromptKey::Backspace,
        // Ctrl-D is a Select navigation key, never terminal EOF.
        KeyCode::Char('d') if control && !alt => PromptKey::Character('\u{4}'),
        KeyCode::Char(character)
            if control && alt && !character.is_ascii_alphabetic() && !character.is_control() =>
        {
            PromptKey::Character(character)
        }
        KeyCode::Char(_) if control || alt => PromptKey::Other,
        KeyCode::Char(character) => PromptKey::Character(character),
        KeyCode::Backspace => PromptKey::Backspace,
        KeyCode::Delete => PromptKey::Delete,
        KeyCode::Left => PromptKey::Left,
        KeyCode::Right => PromptKey::Right,
        KeyCode::Up => PromptKey::Up,
        KeyCode::Down => PromptKey::Down,
        KeyCode::PageUp => PromptKey::PageUp,
        KeyCode::PageDown => PromptKey::PageDown,
        KeyCode::Home => PromptKey::Home,
        KeyCode::End => PromptKey::End,
        KeyCode::Enter => PromptKey::Enter,
        _ => PromptKey::Other,
    }
}

// A disconnected terminal must not enter the decoder's EOF polling loop.
// The bounded library poll also lets us check a hangup arriving while waiting.
#[cfg(all(unix, not(target_vendor = "apple")))]
fn attended_input_ended() -> io::Result<bool> {
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
fn attended_input_ended() -> io::Result<bool> {
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

fn validate_select(select: &PlainSelect<'_>) -> Result<(), Error> {
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

fn script_selection(line: &str, select: &PlainSelect<'_>) -> Result<usize, Error> {
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
        (Some(left), Some(right)) if left != right => {
            Err(Error::new("ambiguous numeric prompt selection"))
        }
        (Some(index), _) | (_, Some(index)) => Ok(index),
        (None, None) => Err(Error::new(
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

fn invariant(message: &str) -> Error {
    Error::new(message)
}

#[cfg(unix)]
struct RawPrompt {
    input: io::Stdin,
    original: rustix::termios::Termios,
    restore_attempted: bool,
}

#[cfg(unix)]
impl RawPrompt {
    fn enter() -> Result<Self, Error> {
        use rustix::termios::{OptionalActions, tcgetattr, tcsetattr};
        let input = io::stdin();
        let original = tcgetattr(&input)
            .map_err(|error| Error::new("failed to read terminal mode").with_source(error))?;
        let mut raw = original.clone();
        raw.make_raw();
        raw.output_modes = original.output_modes;
        tcsetattr(&input, OptionalActions::Now, &raw)
            .map_err(|error| Error::new("failed to enable terminal input").with_source(error))?;
        Ok(Self {
            input,
            original,
            restore_attempted: false,
        })
    }

    fn resume(&mut self) -> Result<(), Error> {
        let mut raw = self.original.clone();
        raw.make_raw();
        raw.output_modes = self.original.output_modes;
        rustix::termios::tcsetattr(&self.input, rustix::termios::OptionalActions::Now, &raw)
            .map_err(|error| Error::new("failed to enable terminal input").with_source(error))?;
        self.restore_attempted = false;
        Ok(())
    }

    fn restore(&mut self) -> Result<(), Error> {
        self.restore_attempted = true;
        rustix::termios::tcsetattr(
            &self.input,
            rustix::termios::OptionalActions::Now,
            &self.original,
        )
        .map_err(|error| Error::new("failed to restore terminal input").with_source(error))?;
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

/// Escape control characters for human prompt display only, preserving raw values.
pub fn escaped_display(value: &str) -> String {
    value
        .chars()
        .map(|c| {
            if c.is_control() {
                c.escape_default().collect::<String>()
            } else {
                c.to_string()
            }
        })
        .collect()
}

#[cfg(not(unix))]
struct RawPrompt {
    active: bool,
}

#[cfg(not(unix))]
impl RawPrompt {
    fn enter() -> Result<Self, Error> {
        crossterm::terminal::enable_raw_mode()
            .map_err(|error| Error::new("failed to enable terminal input").with_source(error))?;
        Ok(Self { active: true })
    }
    fn resume(&mut self) -> Result<(), Error> {
        crossterm::terminal::enable_raw_mode()
            .map_err(|error| Error::new("failed to enable terminal input").with_source(error))?;
        self.active = true;
        Ok(())
    }
    fn restore(&mut self) -> Result<(), Error> {
        if self.active {
            self.active = false;
            crossterm::terminal::disable_raw_mode().map_err(|error| {
                Error::new("failed to restore terminal input").with_source(error)
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

#[cfg(test)]
mod tests {
    use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};

    use super::{PromptKey, attended_key_event};

    #[test]
    fn attended_event_mapping_preserves_navigation_controls_and_altgr() {
        let control_alt = KeyModifiers::CONTROL | KeyModifiers::ALT;
        for (code, modifiers, expected) in [
            (
                KeyCode::Char('c'),
                KeyModifiers::CONTROL,
                PromptKey::Interrupt,
            ),
            (
                KeyCode::Char('h'),
                KeyModifiers::CONTROL,
                PromptKey::Backspace,
            ),
            (
                KeyCode::Char('d'),
                KeyModifiers::CONTROL,
                PromptKey::Character('\u{4}'),
            ),
            (KeyCode::Char('c'), control_alt, PromptKey::Other),
            (KeyCode::Char('h'), control_alt, PromptKey::Other),
            (KeyCode::Char('d'), control_alt, PromptKey::Other),
            (
                KeyCode::Char('J'),
                KeyModifiers::SHIFT,
                PromptKey::Character('J'),
            ),
            (KeyCode::Char('@'), control_alt, PromptKey::Character('@')),
            (KeyCode::Char('é'), control_alt, PromptKey::Character('é')),
            (KeyCode::Char('\u{4}'), control_alt, PromptKey::Other),
            (KeyCode::Char('x'), KeyModifiers::ALT, PromptKey::Other),
            (KeyCode::Char('x'), KeyModifiers::CONTROL, PromptKey::Other),
            (KeyCode::Up, KeyModifiers::NONE, PromptKey::Up),
            (KeyCode::Down, KeyModifiers::NONE, PromptKey::Down),
            (KeyCode::PageUp, KeyModifiers::NONE, PromptKey::PageUp),
            (KeyCode::PageDown, KeyModifiers::NONE, PromptKey::PageDown),
            (KeyCode::Null, KeyModifiers::NONE, PromptKey::Other),
        ] {
            for kind in [KeyEventKind::Press, KeyEventKind::Repeat] {
                assert_eq!(
                    attended_key_event(KeyEvent::new_with_kind(code, modifiers, kind)),
                    expected,
                    "{code:?} {modifiers:?} {kind:?}"
                );
            }
            assert_eq!(
                attended_key_event(KeyEvent::new_with_kind(
                    code,
                    modifiers,
                    KeyEventKind::Release
                )),
                PromptKey::Other,
                "release {code:?} {modifiers:?}"
            );
        }
    }
}
