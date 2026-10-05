//! Text from Linear made safe to print on a terminal.
//!
//! Titles, names and comment bodies are written by other people. Control
//! characters in them (escape, CSI, bell, backspace, a lone carriage
//! return…) could move the cursor, restyle or retitle the terminal, or make
//! output look like something it is not. These functions run on remote text
//! before any styling of our own is added. [`wrap`] fits plain text to the
//! terminal's width.
use std::borrow::Cow;

use unicode_width::UnicodeWidthStr;

/// Shown in place of a removed control character.
const REPLACEMENT: char = '\u{FFFD}';

/// `text` with line breaks kept (a carriage return becomes a newline) and
/// tabs kept; every other control character is replaced with U+FFFD.
pub fn multiline(text: &str) -> Cow<'_, str> {
    if !text
        .chars()
        .any(|ch| ch.is_control() && ch != '\n' && ch != '\t')
    {
        return Cow::Borrowed(text);
    }
    let text = text.replace("\r\n", "\n");
    Cow::Owned(
        text.chars()
            .map(|ch| match ch {
                '\n' | '\t' => ch,
                '\r' => '\n',
                ch if ch.is_control() => REPLACEMENT,
                ch => ch,
            })
            .collect(),
    )
}

/// `text` on one line: newlines, carriage returns and tabs become spaces,
/// and every other control character is replaced with U+FFFD.
pub fn single_line(text: &str) -> Cow<'_, str> {
    if !text.chars().any(char::is_control) {
        return Cow::Borrowed(text);
    }
    let text = text.replace("\r\n", " ");
    Cow::Owned(
        text.chars()
            .map(|ch| match ch {
                '\n' | '\r' | '\t' => ' ',
                ch if ch.is_control() => REPLACEMENT,
                ch => ch,
            })
            .collect(),
    )
}

/// `text` wrapped at spaces to lines at most `columns` wide, the first line
/// starting with `first` and the rest with `rest`. A word wider than a line
/// gets a line of its own, and line breaks in `text` are kept.
pub fn wrap(text: &str, columns: usize, first: &str, rest: &str) -> String {
    let mut out = String::new();
    for (index, line) in text.split('\n').enumerate() {
        let prefix = if index == 0 { first } else { rest };
        if index > 0 {
            out.push('\n');
        }
        out.push_str(prefix);
        let start = prefix.width();
        let mut width = start;
        for word in line.split(' ').filter(|word| !word.is_empty()) {
            let word_width = word.width();
            if width > start && width + 1 + word_width > columns {
                out.push('\n');
                out.push_str(rest);
                width = rest.width();
            } else if width > start {
                out.push(' ');
                width += 1;
            }
            out.push_str(word);
            width += word_width;
        }
    }
    out
}

/// Joins `parts` (text, possibly styled, with its display width) with spaces
/// into lines that start with `indent`, starting another line before a part
/// that would reach past `columns`. Parts are never split.
pub fn wrap_parts(parts: &[(String, usize)], indent: &str, columns: usize) -> String {
    let mut out = indent.to_owned();
    let mut width = indent.width();
    for (index, (text, part_width)) in parts.iter().enumerate() {
        if index > 0 {
            if width + 1 + part_width > columns {
                out.push('\n');
                out.push_str(indent);
                width = indent.width();
            } else {
                out.push(' ');
                width += 1;
            }
        }
        out.push_str(text);
        width += part_width;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrapping_breaks_at_spaces_and_indents_later_lines() {
        let hint = "Valid states: backlog, unstarted, started, completed";
        assert_eq!(
            wrap(hint, 24, "  ", "  "),
            "  Valid states: backlog,\n  unstarted, started,\n  completed"
        );
        assert_eq!(wrap("short", 30, "✗ ", "  "), "✗ short");
        assert_eq!(
            wrap("a https://linear.app/a/very/long/url b", 10, "", ""),
            "a\nhttps://linear.app/a/very/long/url\nb"
        );
        assert_eq!(wrap("one\ntwo three", 7, "", "> "), "one\n> two\n> three");
    }

    #[test]
    fn plain_text_is_borrowed_unchanged() {
        assert!(matches!(multiline("a\n\tb ✓"), Cow::Borrowed("a\n\tb ✓")));
        assert!(matches!(single_line("a b ✓"), Cow::Borrowed("a b ✓")));
    }

    #[test]
    fn escape_sequences_cannot_reach_the_terminal() {
        let hostile = "ok\u{1b}]0;pwned\u{7}\u{1b}[2J\u{9b}31mred\u{8}\u{7f}";
        assert_eq!(
            multiline(hostile),
            "ok\u{FFFD}]0;pwned\u{FFFD}\u{FFFD}[2J\u{FFFD}31mred\u{FFFD}\u{FFFD}"
        );
        assert_eq!(single_line(hostile), multiline(hostile));
    }

    #[test]
    fn line_breaks_are_kept_in_bodies_and_flattened_in_cells() {
        assert_eq!(multiline("a\r\nb\rc\n\td"), "a\nb\nc\n\td");
        assert_eq!(single_line("a\r\nb\rc\n\td"), "a b c  d");
    }

    #[test]
    fn comment_headers_wrap_between_parts_and_keep_their_indent() {
        let part = |text: &str| (text.to_owned(), text.len());
        let parts = [
            part("@alice"),
            part("replied 3 days ago"),
            part("[0a1b2c3d-0000-4000-8000-000000000000]"),
        ];
        assert_eq!(
            wrap_parts(&parts, "  ", 80),
            "  @alice replied 3 days ago [0a1b2c3d-0000-4000-8000-000000000000]"
        );
        assert_eq!(
            wrap_parts(&parts, "  ", 60),
            "  @alice replied 3 days ago\n  [0a1b2c3d-0000-4000-8000-000000000000]"
        );
        // A part wider than the terminal gets a line of its own.
        assert_eq!(
            wrap_parts(&parts, "", 10),
            "@alice\nreplied 3 days ago\n[0a1b2c3d-0000-4000-8000-000000000000]"
        );
    }
}
