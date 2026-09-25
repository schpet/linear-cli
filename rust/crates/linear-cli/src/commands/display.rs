//! The reviewed Rust v3 per-code-point width and right-padding rule.
//!
//! `unicode-width 0.2.2` differs from frozen Deno's Unicode 15 table for
//! some code points; the C002-WIDTH-TABLE compatibility row binds that choice.
use unicode_width::UnicodeWidthChar;

pub fn display_width(text: &str) -> usize {
    text.chars().map(|ch| ch.width().unwrap_or(0)).sum()
}

pub fn pad(text: &str, width: usize) -> String {
    let mut padded = text.to_owned();
    padded.extend(std::iter::repeat_n(
        ' ',
        width.saturating_sub(display_width(text)),
    ));
    padded
}
