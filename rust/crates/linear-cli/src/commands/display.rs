//! The reviewed Rust v3 per-code-point width, padding, and truncation rules.
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

/// Truncate by display columns, preserving Deno's short-width UTF-16 slice.
///
/// The returned text is not padded. Callers that render a table cell should
/// apply [`pad`] separately. A split surrogate in the short-width branch is
/// replaced by U+FFFD, matching the source CLI's UTF-8 terminal bytes.
pub fn truncate_text(text: &str, max_width: usize) -> String {
    if display_width(text) <= max_width {
        return text.to_owned();
    }

    if max_width < 3 {
        let code_units: Vec<u16> = text.encode_utf16().take(max_width).collect();
        return String::from_utf16_lossy(&code_units);
    }

    let content_width = max_width - 3;
    let mut result = String::new();
    let mut width = 0;
    for ch in text.chars() {
        let next_width = width + ch.width().unwrap_or(0);
        if next_width > content_width {
            break;
        }
        result.push(ch);
        width = next_width;
    }
    result.push_str("...");
    result
}

/// Format a name cell using the frozen `team list`/`label list` rule.
///
/// These commands compare JavaScript UTF-16 string length to a width measured
/// in terminal columns. A name that fits by code-unit length is padded by
/// display width, even if its display width exceeds the requested width.
/// Otherwise JavaScript `slice(0, width - 3)` takes code units, including the
/// negative-end behavior when `width < 3`, and appends unpadded `...`. A split
/// surrogate becomes U+FFFD in UTF-8 output. [`truncate_text`] instead walks
/// code points by display width.
pub fn truncate_js(text: &str, name_width: usize) -> String {
    let code_units: Vec<u16> = text.encode_utf16().collect();
    if code_units.len() <= name_width {
        return pad(text, name_width);
    }
    let end = if name_width >= 3 {
        name_width - 3
    } else {
        code_units.len().saturating_sub(3 - name_width)
    };
    let prefix: Vec<u16> = code_units.into_iter().take(end).collect();
    format!("{}...", String::from_utf16_lossy(&prefix))
}
