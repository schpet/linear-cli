use linear_cli::platform::selector::{Key, SelectOption, Selection, Selector, interactive_allowed};

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
