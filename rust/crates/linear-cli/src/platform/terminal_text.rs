//! Text from Linear made safe to print on a terminal.
//!
//! Titles, names and comment bodies are written by other people. Control
//! characters in them (escape, CSI, bell, backspace, a lone carriage
//! return…) could move the cursor, restyle or retitle the terminal, or make
//! output look like something it is not. These functions run on remote text
//! before any styling of our own is added.
use std::borrow::Cow;

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

#[cfg(test)]
mod tests {
    use super::*;

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
}
