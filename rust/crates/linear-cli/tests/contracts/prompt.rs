use std::io::{self, Cursor, Read, Write};

use linear_cli::platform::prompt::{
    PlainOption, PlainSelect, PromptKey, PromptOutcome, PromptSession,
};

fn options() -> Vec<PlainOption> {
    [
        ("Planned", "Planned", "Planned"),
        ("Active", "Active", "Active"),
        ("Skip", "__skip__", "skip"),
        ("Custom", "__custom__", "custom"),
    ]
    .into_iter()
    .map(|(label, value, script_token)| PlainOption {
        label: label.to_owned(),
        value: value.to_owned(),
        script_token: script_token.to_owned(),
    })
    .collect()
}

fn status<'a>(rows: &'a [PlainOption]) -> PlainSelect<'a> {
    PlainSelect {
        message: "Status:",
        options: rows,
        default_index: 0,
        default_hint: Some("planned"),
    }
}

#[test]
fn script_preserves_lines_across_prompts_and_decodes_split_utf8() {
    struct OneByte<R>(R);
    impl<R: Read> Read for OneByte<R> {
        fn read(&mut self, bytes: &mut [u8]) -> io::Result<usize> {
            let Some(first) = bytes.first_mut() else {
                return Ok(0);
            };
            self.0.read(std::slice::from_mut(first))
        }
    }
    for one_byte in [false, true] {
        let raw = "Éclair\r\nsecond\n".as_bytes().to_vec();
        let reader: Box<dyn Read> = if one_byte {
            Box::new(OneByte(Cursor::new(raw)))
        } else {
            Box::new(Cursor::new(raw))
        };
        let mut session = PromptSession::script(reader, Vec::new());
        assert_eq!(
            session.text("Name:", 1, |_| Ok(())).unwrap(),
            PromptOutcome::Submitted("Éclair".to_owned())
        );
        assert_eq!(
            session.text("Description:", 0, |_| Ok(())).unwrap(),
            PromptOutcome::Submitted("second".to_owned())
        );
        assert_eq!(
            session.text("Next:", 0, |_| Ok(())).unwrap(),
            PromptOutcome::EndOfInput
        );
        let output = String::from_utf8(session.into_output().unwrap()).unwrap();
        assert!(output.contains("? Name: › Éclair\n"));
        assert!(output.contains("? Description: › second\n"));
        assert!(!output.contains('\u{1b}'));
    }
}

#[test]
fn script_choice_is_typed_and_numeric_ambiguity_fails() {
    let rows = options();
    let mut session = PromptSession::script(Cursor::new(b"\n4\nskip\n"), Vec::new());
    assert_eq!(
        session.select(&status(&rows)).unwrap(),
        PromptOutcome::Submitted("Planned".to_owned())
    );
    assert_eq!(
        session.select(&status(&rows)).unwrap(),
        PromptOutcome::Submitted("__custom__".to_owned())
    );
    assert_eq!(
        session.select(&status(&rows)).unwrap(),
        PromptOutcome::Submitted("__skip__".to_owned())
    );
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    assert!(output.contains("? Status: (planned)"));
    assert!(output.contains("? Status: › Planned"));
    let numeric = [
        PlainOption {
            label: "One".into(),
            value: "one".into(),
            script_token: "2".into(),
        },
        PlainOption {
            label: "Two".into(),
            value: "two".into(),
            script_token: "two".into(),
        },
    ];
    let mut conflict = PromptSession::script(Cursor::new(b"2\n"), Vec::new());
    let error = conflict
        .select(&PlainSelect {
            message: "Choose:",
            options: &numeric,
            default_index: 0,
            default_hint: None,
        })
        .unwrap_err();
    assert_eq!(
        error.display_message(),
        "ambiguous numeric prompt selection"
    );
    let mut unknown = PromptSession::script(Cursor::new(b"SKIP\n"), Vec::new());
    assert!(unknown.select(&status(&rows)).is_err());
}

#[test]
fn script_invalid_raw_answer_does_not_consume_following_line() {
    let mut session = PromptSession::script(Cursor::new(b"\nGood\n"), Vec::new());
    let error = session.text("Name:", 1, |_| Ok(())).unwrap_err();
    assert!(error.display_message().contains("at least 1"));
    assert_eq!(
        session.text("Name:", 1, |_| Ok(())).unwrap(),
        PromptOutcome::Submitted("Good".into())
    );
    let mut color = PromptSession::script(Cursor::new(b" #ABCDEF\n#ABCDEF\n"), Vec::new());
    let check_color = |raw: &str| {
        if raw.len() == 7
            && raw.starts_with('#')
            && raw.chars().skip(1).all(|ch| ch.is_ascii_hexdigit())
        {
            Ok(())
        } else {
            Err("invalid hex".to_owned())
        }
    };
    assert_eq!(
        color
            .text("Color:", 0, check_color)
            .unwrap_err()
            .display_message(),
        "invalid hex"
    );
    assert_eq!(
        color.text("Color:", 0, check_color).unwrap(),
        PromptOutcome::Submitted("#ABCDEF".into())
    );
}

#[test]
fn script_rejects_incomplete_control_invalid_utf8_and_oversize() {
    for (bytes, expected) in [
        (b"partial".to_vec(), "incomplete prompt script line at EOF"),
        (
            b"a\x1bb\n".to_vec(),
            "prompt script line contains a control character",
        ),
        (b"\xff\n".to_vec(), "prompt script line is not UTF-8"),
        (vec![b'a'; 65_537], "prompt script line exceeds 65536 bytes"),
    ] {
        let mut session = PromptSession::script(Cursor::new(bytes), Vec::new());
        assert_eq!(
            session
                .text("Name:", 0, |_| Ok(()))
                .unwrap_err()
                .display_message(),
            expected
        );
    }
    let mut accepted = vec![b'a'; 65_535];
    accepted.push(b'\n');
    let mut session = PromptSession::script(Cursor::new(accepted), Vec::new());
    assert!(matches!(
        session.text("Name:", 0, |_| Ok(())).unwrap(),
        PromptOutcome::Submitted(_)
    ));
}

#[test]
fn key_input_retries_raw_validation_and_trims_after_accepting() {
    let keys = [
        PromptKey::Enter,
        PromptKey::Character(' '),
        PromptKey::Enter,
    ];
    let mut keys = keys.into_iter();
    let mut session = PromptSession::<io::Empty, _>::keys(Vec::new(), 80, 24, move || {
        Ok(keys.next().unwrap_or(PromptKey::EndOfInput))
    })
    .unwrap();
    assert_eq!(
        session.text("Name:", 1, |_| Ok(())).unwrap(),
        PromptOutcome::Submitted(String::new())
    );
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    assert!(output.contains("at least 1"));

    let mut keys = [
        PromptKey::Character(' '),
        PromptKey::Character('#'),
        PromptKey::Character('A'),
        PromptKey::Enter,
        PromptKey::Home,
        PromptKey::Delete,
        PromptKey::Enter,
    ]
    .into_iter();
    let mut color = PromptSession::<io::Empty, _>::keys(Vec::new(), 80, 24, move || {
        Ok(keys.next().unwrap_or(PromptKey::EndOfInput))
    })
    .unwrap();
    let validate = |raw: &str| {
        if raw == "#A" {
            Ok(())
        } else {
            Err("bad color".into())
        }
    };
    assert_eq!(
        color.text("Color:", 0, validate).unwrap(),
        PromptOutcome::Submitted("#A".into())
    );
    assert!(
        String::from_utf8(color.into_output().unwrap())
            .unwrap()
            .contains("bad color")
    );
}

#[test]
fn key_input_cursor_tracks_insertions_and_long_text_viewport() {
    let mut keys: Vec<PromptKey> = "abcdefghijklmnopqrstuvwxyz0123456789"
        .chars()
        .map(PromptKey::Character)
        .collect();
    keys.extend([
        PromptKey::Left,
        PromptKey::Left,
        PromptKey::Character('X'),
        PromptKey::Enter,
    ]);
    let mut keys = keys.into_iter();
    let mut session = PromptSession::<io::Empty, _>::keys(Vec::new(), 40, 24, move || {
        Ok(keys.next().unwrap_or(PromptKey::EndOfInput))
    })
    .unwrap();
    let result = session.text("Name:", 1, |_| Ok(())).unwrap();
    assert_eq!(
        result,
        PromptOutcome::Submitted("abcdefghijklmnopqrstuvwxyz01234567X89".to_owned())
    );
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    assert!(output.contains("\x1b["));
    assert!(output.contains("G"));
    assert!(output.contains("67X89"));
}

#[test]
fn unicode_editing_and_crlf_key_paste_preserve_prompt_boundaries() {
    let mut keys = [
        PromptKey::Character('É'),
        PromptKey::Character('🙂'),
        PromptKey::Character('x'),
        PromptKey::Left,
        PromptKey::Backspace,
        PromptKey::Enter,
        PromptKey::Enter,
    ]
    .into_iter();
    let mut session = PromptSession::<io::Empty, _>::keys(Vec::new(), 40, 24, move || {
        Ok(keys.next().unwrap_or(PromptKey::EndOfInput))
    })
    .unwrap();
    assert_eq!(
        session.text("Name:", 1, |_| Ok(())).unwrap(),
        PromptOutcome::Submitted("Éx".to_owned())
    );
    assert_eq!(
        session.text("Optional:", 0, |_| Ok(())).unwrap(),
        PromptOutcome::Submitted(String::new())
    );
}

#[test]
fn key_select_uses_plain_navigation_wrap_and_page_end() {
    let rows: Vec<_> = (0..12)
        .map(|index| PlainOption {
            label: format!("Color {index}"),
            value: format!("v{index}"),
            script_token: format!("t{index}"),
        })
        .collect();
    let mut keys = [
        PromptKey::Up,
        PromptKey::PageDown,
        PromptKey::PageUp,
        PromptKey::Character('d'),
        PromptKey::Enter,
    ]
    .into_iter();
    let mut session = PromptSession::<io::Empty, _>::keys(Vec::new(), 40, 24, move || {
        Ok(keys.next().unwrap_or(PromptKey::EndOfInput))
    })
    .unwrap();
    let select = PlainSelect {
        message: "Color:",
        options: &rows,
        default_index: 0,
        default_hint: None,
    };
    assert_eq!(
        session.select(&select).unwrap(),
        PromptOutcome::Submitted("v2".into())
    );
    let output = String::from_utf8(session.into_output().unwrap()).unwrap();
    assert!(output.contains("Color 11"));
}

#[test]
fn interruption_eof_io_and_close_are_explicit() {
    for (key, expected) in [
        (PromptKey::Interrupt, PromptOutcome::Interrupted),
        (PromptKey::EndOfInput, PromptOutcome::EndOfInput),
    ] {
        let mut session =
            PromptSession::<io::Empty, _>::keys(Vec::new(), 80, 24, move || Ok(key)).unwrap();
        assert_eq!(session.text("Name:", 1, |_| Ok(())).unwrap(), expected);
        session.close().unwrap();
        assert!(session.text("Name:", 1, |_| Ok(())).is_err());
    }
    struct FailingWriter;
    impl Write for FailingWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("closed"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut output_failure = PromptSession::script(Cursor::new(b"a\n"), FailingWriter);
    assert_eq!(
        output_failure
            .text("Name:", 1, |_| Ok(()))
            .unwrap_err()
            .display_message(),
        "failed to write prompt stdout"
    );
    struct FailingReader;
    impl Read for FailingReader {
        fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
            Err(io::Error::other("closed"))
        }
    }
    let mut input_failure = PromptSession::script(FailingReader, Vec::new());
    assert_eq!(
        input_failure
            .text("Name:", 1, |_| Ok(()))
            .unwrap_err()
            .display_message(),
        "failed to read prompt stdin"
    );
}

#[test]
fn flush_occurs_before_key_read_and_cleanup_errors_merge() {
    use std::cell::Cell;
    use std::rc::Rc;

    struct FlushFailure;
    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("synthetic flush failure"))
        }
    }
    let read_count = Rc::new(Cell::new(0));
    let observed = Rc::clone(&read_count);
    let mut session = PromptSession::<io::Empty, _>::keys(FlushFailure, 80, 24, move || {
        observed.set(observed.get() + 1);
        Ok(PromptKey::Enter)
    })
    .unwrap();
    let prompt_error = session.text("Name:", 1, |_| Ok(())).unwrap_err();
    assert_eq!(
        prompt_error.display_message(),
        "failed to flush prompt stdout"
    );
    assert_eq!(read_count.get(), 0);
    let combined = session
        .finish_result::<String>(Err(prompt_error))
        .unwrap_err();
    assert!(
        combined
            .display_message()
            .contains("failed to flush prompt stdout")
    );
    assert!(combined.display_message().contains("prompt also failed"));
}

#[test]
fn cleanup_errors_override_interrupt_and_eof() {
    struct FlushFailure;
    impl Write for FlushFailure {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("synthetic flush failure"))
        }
    }

    let mut interrupted =
        PromptSession::<io::Empty, _>::keys(FlushFailure, 80, 24, || Ok(PromptKey::Interrupt))
            .unwrap();
    assert_eq!(
        interrupted
            .finish_result::<String>(Ok(PromptOutcome::Interrupted))
            .unwrap_err()
            .display_message(),
        "failed to flush prompt stdout"
    );

    let mut eof =
        PromptSession::<io::Empty, _>::keys(FlushFailure, 80, 24, || Ok(PromptKey::EndOfInput))
            .unwrap();
    assert_eq!(
        eof.finish::<String>(PromptOutcome::EndOfInput)
            .unwrap_err()
            .display_message(),
        "failed to flush prompt stdout"
    );
}

#[test]
fn invalid_select_configuration_and_key_eof_are_explicit() {
    let mut session = PromptSession::script(Cursor::new(b"\n"), Vec::new());
    let empty = PlainSelect {
        message: "Choose:",
        options: &[],
        default_index: 0,
        default_hint: None,
    };
    assert!(session.select(&empty).is_err());
    let rows = options();
    let invalid_default = PlainSelect {
        message: "Choose:",
        options: &rows,
        default_index: rows.len(),
        default_hint: None,
    };
    assert!(session.select(&invalid_default).is_err());
    let duplicate = [rows[0].clone(), rows[0].clone()];
    let invalid_duplicate = PlainSelect {
        message: "Choose:",
        options: &duplicate,
        default_index: 0,
        default_hint: None,
    };
    assert!(session.select(&invalid_duplicate).is_err());
    let invalid_hint = PlainSelect {
        message: "Choose:",
        options: &rows,
        default_index: 0,
        default_hint: Some("bad\n"),
    };
    assert!(session.select(&invalid_hint).is_err());
    assert!(session.into_output().unwrap().is_empty());

    let mut keys = [PromptKey::Down, PromptKey::EndOfInput].into_iter();
    let mut terminal = PromptSession::<io::Empty, _>::keys(Vec::new(), 80, 24, move || {
        Ok(keys.next().unwrap_or(PromptKey::EndOfInput))
    })
    .unwrap();
    assert_eq!(
        terminal.select(&status(&rows)).unwrap(),
        PromptOutcome::EndOfInput
    );
}
