use std::io;

use linear_cli::commands::display::display_width;
use linear_cli::platform::selector::{
    Key, PromptLabels, SelectOption, Selection, Selector, interactive_allowed, run_with,
};

fn labels() -> PromptLabels<'static> {
    PromptLabels {
        message: "Select a project",
        search_label: "Search projects",
        max_rows: 8,
    }
}

fn options() -> Vec<SelectOption> {
    [
        (
            "Alpha  ·  Planned  ·  ARC  ·  alpha",
            "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa",
        ),
        (
            "Zeta  ·  Active  ·  ARC  ·  zeta",
            "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb",
        ),
        (
            "Zebra  ·  Planned  ·  ZEN  ·  zebra",
            "cccccccc-cccc-4ccc-8ccc-cccccccccccc",
        ),
    ]
    .into_iter()
    .map(|(label, value)| SelectOption {
        label: label.to_owned(),
        value: value.to_owned(),
    })
    .collect()
}

#[test]
fn ci_gate_requires_both_ttys_and_literal_false_or_empty() {
    for (ci, allowed) in [
        (None, true),
        (Some(""), true),
        (Some("false"), true),
        (Some("False"), false),
        (Some("0"), false),
        (Some("true"), false),
    ] {
        assert_eq!(interactive_allowed(true, true, ci), allowed);
        assert!(!interactive_allowed(false, true, ci));
        assert!(!interactive_allowed(true, false, ci));
    }
}

#[test]
fn search_finds_a_nondefault_value_and_backspace_recovers() {
    let rows = options();
    let mut selector = Selector::new(&rows).unwrap();
    assert_eq!(selector.active_value(), Some(rows[0].value.as_str()));
    for character in "Zeta".chars() {
        assert_eq!(selector.on_key(Key::Character(character)), None);
    }
    assert_eq!(selector.active_value(), Some(rows[1].value.as_str()));
    assert_eq!(
        selector.on_key(Key::Enter),
        Some(Selection::Selected(rows[1].value.clone()))
    );
    selector.on_key(Key::Character('X'));
    assert_eq!(selector.active_value(), None);
    selector.on_key(Key::Backspace);
    assert_eq!(selector.active_value(), Some(rows[1].value.as_str()));
}

#[test]
fn search_can_match_value_and_strips_ansi_from_label() {
    let rows = vec![SelectOption {
        label: "\x1b[32mZeta\x1b[0m".to_owned(),
        value: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".to_owned(),
    }];
    let mut selector = Selector::new(&rows).unwrap();
    selector.on_key(Key::Character('z'));
    assert_eq!(selector.visible().count(), 1);
    selector.on_key(Key::Backspace);
    for character in "bbbb".chars() {
        selector.on_key(Key::Character(character));
    }
    assert_eq!(selector.active_value(), Some(rows[0].value.as_str()));
}

#[test]
fn arrow_navigation_wraps_and_filter_clamps_index() {
    let rows = options();
    let mut selector = Selector::new(&rows).unwrap();
    selector.on_key(Key::Up);
    assert_eq!(selector.active_value(), Some(rows[2].value.as_str()));
    selector.on_key(Key::Character('Z'));
    assert_eq!(selector.visible().count(), 2);
    assert_eq!(selector.active_value(), Some(rows[2].value.as_str()));
    selector.on_key(Key::Down);
    assert!(selector.active_value().is_some());
}

#[test]
fn ctrl_c_and_eof_are_distinct_and_prompt_uses_stdout() {
    let rows = options();
    let mut output = Vec::new();
    let mut keys = [
        Key::Character('Z'),
        Key::Character('e'),
        Key::Character('b'),
        Key::Enter,
    ]
    .into_iter();
    let chosen = run_with(
        &rows,
        &labels(),
        80,
        || Ok(keys.next().unwrap_or(Key::EndOfInput)),
        &mut output,
    )
    .unwrap();
    assert_eq!(chosen, Selection::Selected(rows[2].value.clone()));
    let rendered = String::from_utf8(output).unwrap();
    assert!(rendered.contains("Select a project"));
    assert!(rendered.contains("Search projects"));
    assert!(rendered.contains("? Select a project › Zebra"));
    let mut sink = Vec::new();
    assert_eq!(
        run_with(&rows, &labels(), 80, || Ok(Key::Interrupt), &mut sink).unwrap(),
        Selection::Interrupted
    );
    assert_eq!(
        run_with(&rows, &labels(), 80, || Ok(Key::EndOfInput), &mut sink).unwrap(),
        Selection::EndOfInput
    );
    assert_eq!(
        run_with(
            &rows,
            &labels(),
            80,
            || Ok(Key::Character('\u{4}')),
            &mut sink
        )
        .unwrap(),
        Selection::EndOfInput
    );
}

#[test]
fn empty_options_and_writer_failure_are_explicit() {
    let mut sink = Vec::new();
    assert_eq!(
        run_with(&[], &labels(), 80, || Ok(Key::Enter), &mut sink)
            .unwrap_err()
            .to_string(),
        "selector requires at least one option"
    );
    let rows = options();
    struct FailWriter;
    impl io::Write for FailWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::other("unavailable"))
        }
        fn flush(&mut self) -> io::Result<()> {
            Err(io::Error::other("unavailable"))
        }
    }
    let mut writer = FailWriter;
    assert_eq!(
        run_with(&rows, &labels(), 80, || Ok(Key::Enter), &mut writer)
            .unwrap_err()
            .to_string(),
        "failed to write to stdout: unavailable"
    );
}

#[test]
fn multiple_search_matches_rank_by_distance_and_keep_distinct_values() {
    let rows = vec![
        SelectOption {
            label: "Zed".to_owned(),
            value: "first".to_owned(),
        },
        SelectOption {
            label: "Ze".to_owned(),
            value: "second".to_owned(),
        },
        SelectOption {
            label: "Ze".to_owned(),
            value: "third".to_owned(),
        },
    ];
    let mut selector = Selector::new(&rows).unwrap();
    selector.on_key(Key::Character('z'));
    selector.on_key(Key::Character('e'));
    assert_eq!(selector.active_value(), Some("second"));
    selector.on_key(Key::Down);
    assert_eq!(selector.active_value(), Some("third"));
    assert_eq!(
        selector.on_key(Key::Enter),
        Some(Selection::Selected("third".to_owned()))
    );
}

#[test]
fn unicode_search_is_case_insensitive_without_changing_selection_value() {
    let rows = vec![SelectOption {
        label: "Éclair  ·  Active".to_owned(),
        value: "uuid-opaque".to_owned(),
    }];
    let mut selector = Selector::new(&rows).unwrap();
    for character in "écl".chars() {
        selector.on_key(Key::Character(character));
    }
    assert_eq!(selector.active_value(), Some("uuid-opaque"));
    assert_eq!(
        selector.on_key(Key::Enter),
        Some(Selection::Selected("uuid-opaque".to_owned()))
    );
}

#[test]
fn prompt_configuration_rejects_zero_rows_before_writing() {
    let rows = options();
    let zero_rows = PromptLabels {
        message: "Select a project",
        search_label: "Search projects",
        max_rows: 0,
    };
    let mut output = Vec::new();
    let error = run_with(&rows, &zero_rows, 80, || Ok(Key::Enter), &mut output).unwrap_err();
    assert_eq!(
        error.to_string(),
        "selector prompt labels, row limit, and width must be valid"
    );
    assert!(output.is_empty());

    let invalid_labels = PromptLabels {
        message: "Select\nproject",
        search_label: "Search projects",
        max_rows: 8,
    };
    assert!(run_with(&rows, &invalid_labels, 80, || Ok(Key::Enter), &mut output).is_err());
    assert!(run_with(&rows, &labels(), 3, || Ok(Key::Enter), &mut output).is_err());
    assert!(output.is_empty());
}

#[test]
fn broken_stdout_is_reported_before_waiting_for_a_key() {
    struct BrokenWriter;
    impl io::Write for BrokenWriter {
        fn write(&mut self, _: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut writer = BrokenWriter;
    let error = run_with(
        &options(),
        &labels(),
        80,
        || panic!("must not wait"),
        &mut writer,
    )
    .unwrap_err();
    assert_eq!(error.kind(), linear_cli::error::ErrorKind::BrokenPipe);
}

#[test]
fn invalid_options_are_rejected_before_drawing() {
    for (label, value, expected) in [
        ("", "id", "invalid label"),
        ("\x1b[32m\x1b[0m", "id", "invalid label"),
        ("\x0b\x0c", "id", "invalid label"),
        ("row", "", "invalid value"),
        ("row", "id\nnext", "invalid value"),
    ] {
        let options = [SelectOption {
            label: label.to_owned(),
            value: value.to_owned(),
        }];
        let error = Selector::new(&options).unwrap_err();
        assert!(error.to_string().contains(expected), "{error}");
    }
}

#[test]
fn control_characters_in_a_name_do_not_add_prompt_rows() {
    let options = [SelectOption {
        label: "Row\x0bNext\x0cActive\nDone".to_owned(),
        value: "project-id".to_owned(),
    }];
    let mut output = Vec::new();
    assert_eq!(
        run_with(&options, &labels(), 80, || Ok(Key::Enter), &mut output).unwrap(),
        Selection::Selected("project-id".to_owned())
    );
    assert!(!output.contains(&0x0b));
    assert!(!output.contains(&0x0c));
    let text = String::from_utf8(output).unwrap();
    assert!(text.contains("Row Next Active Done"));
}

#[test]
fn narrow_terminal_frames_do_not_wrap_long_or_wide_labels() {
    let options = [SelectOption {
        label: "Long  ·  漢字  ·  project-name-with-many-characters".to_owned(),
        value: "project-id".to_owned(),
    }];
    for columns in [4, 12] {
        let mut output = Vec::new();
        assert_eq!(
            run_with(
                &options,
                &labels(),
                columns,
                || Ok(Key::EndOfInput),
                &mut output,
            )
            .unwrap(),
            Selection::EndOfInput
        );
        let frame = output.split(|byte| *byte == 0x1b).next().unwrap();
        let text = String::from_utf8(frame.to_vec()).unwrap();
        for line in text.lines() {
            assert!(display_width(line) < columns, "{columns}: {line:?}");
        }
    }
}
