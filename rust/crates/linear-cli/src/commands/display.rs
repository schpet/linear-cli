//! Terminal column widths, padding and truncation for table cells.
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

/// Shorten `text` to at most `max_width` columns, ending in `...` when there
/// is room for it. Characters are never split. The result is not padded.
pub fn truncate_text(text: &str, max_width: usize) -> String {
    if display_width(text) <= max_width {
        return text.to_owned();
    }
    let ellipsis = if max_width >= 3 { "..." } else { "" };
    let content_width = max_width - ellipsis.len();
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
    result.push_str(ellipsis);
    result
}

/// The width of a column that shrinks to fit the terminal: its widest
/// content, but never narrower than 20 columns so names stay recognizable on
/// narrow terminals.
pub fn flexible_width(content: usize, available: usize) -> usize {
    content.min(available.max(20))
}

/// A cell exactly `width` columns wide: truncated when too long, padded when short.
pub fn fit(text: &str, width: usize) -> String {
    pad(&truncate_text(text, width), width)
}

#[cfg(test)]
mod tests {
    use super::{display_width, fit, truncate_text};

    #[test]
    fn widths_count_terminal_columns() {
        assert_eq!(display_width("ab"), 2);
        assert_eq!(display_width("界"), 2);
        assert_eq!(display_width("e\u{301}"), 1);
    }

    #[test]
    fn truncation_keeps_whole_characters() {
        assert_eq!(truncate_text("short", 10), "short");
        assert_eq!(truncate_text("abcdefgh", 6), "abc...");
        assert_eq!(truncate_text("界界界界", 7), "界界...");
        assert_eq!(truncate_text("界界", 3), "...");
        assert_eq!(truncate_text("abc", 2), "ab");
        assert_eq!(truncate_text("😀x", 1), "");
    }

    #[test]
    fn fit_pads_or_truncates_to_the_width() {
        assert_eq!(fit("ab", 4), "ab  ");
        assert_eq!(fit("abcdef", 5), "ab...");
        assert_eq!(fit("界界界", 5), "界...");
    }
}
