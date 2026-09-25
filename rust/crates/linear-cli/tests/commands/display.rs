use linear_cli::commands::display::{display_width, truncate_js, truncate_text};

#[test]
fn short_widths_slice_utf16_code_units_without_dots() {
    assert_eq!(truncate_text("abcd", 0), "");
    assert_eq!(truncate_text("abcd", 1), "a");
    assert_eq!(truncate_text("日本語", 2), "日本");
    assert_eq!(display_width(&truncate_text("日本語", 2)), 4);

    // A split surrogate is replaced when Deno's string is encoded to UTF-8.
    assert_eq!(truncate_text("😀abc", 1), "\u{fffd}");
    assert_eq!(truncate_text("a😀bc", 2), "a\u{fffd}");
}

#[test]
fn display_width_boundary_reserves_three_ascii_dots() {
    for (text, width, expected) in [
        ("abcd", 3, "..."),
        ("abcd", 4, "abcd"),
        ("abcde", 4, "a..."),
        ("😀abcd", 5, "😀..."),
        ("日本語xy", 5, "日..."),
        ("e\u{301}abcd", 4, "e\u{301}..."),
        ("👨‍👩‍👧x", 5, "👨\u{200d}..."),
        ("ab\u{301}cd", 3, "..."),
    ] {
        assert_eq!(truncate_text(text, width), expected, "{text:?} at {width}");
    }
}

#[test]
fn zero_width_and_control_characters_follow_display_width() {
    assert_eq!(truncate_text("\u{301}", 0), "\u{301}");
    assert_eq!(truncate_text("a\tb", 2), "a\tb");
}

#[test]
fn reviewed_v3_width_table_changes_hexagram_truncation() {
    // C002-WIDTH-TABLE: frozen Deno counts each U+4DC0 as one column; the
    // reviewed Rust unicode-width 0.2.2 table counts each as two.
    assert_eq!(display_width("䷀䷀䷀"), 6);
    assert_eq!(truncate_text("䷀䷀䷀", 3), "...");
}

#[test]
fn js_name_cell_keeps_the_frozen_utf16_slice_and_padding() {
    assert_eq!(truncate_js("A", 4), "A   ");
    assert_eq!(truncate_js("日本語", 3), "日本語");
    assert_eq!(truncate_js("e\u{301}", 3), "e\u{301}  ");
    assert_eq!(truncate_js("䷀", 3), "䷀ ");
    assert_eq!(truncate_js("日本語", 2), "日本...");
    assert_eq!(truncate_js("😀abc", 4), "\u{fffd}...");
    assert_eq!(truncate_js("🇺🇸", 2), "🇺\u{fffd}...");
    assert_eq!(truncate_js("e\u{301}", 1), "...");
    assert_eq!(truncate_js("\u{301}", 0), "...");
    assert_eq!(truncate_js("", 0), "");
    assert_eq!(truncate_js("abcdef", 4), "a...");
    assert_eq!(truncate_js("abcdef", 1), "abcd...");
}
