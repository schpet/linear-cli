//! Terminal text styles. Each takes `enabled`, the stream's color decision
//! (see [`crate::ctx::Terminal`]), and returns the text unchanged when it is off.
use console::Style;

fn paint(style: Style, text: &str, enabled: bool) -> String {
    if enabled {
        style.force_styling(true).apply_to(text).to_string()
    } else {
        text.to_owned()
    }
}

pub fn bold(text: &str, enabled: bool) -> String {
    paint(Style::new().bold(), text, enabled)
}

pub fn underline(text: &str, enabled: bool) -> String {
    paint(Style::new().underlined(), text, enabled)
}

/// Table column headings.
pub fn heading(text: &str, enabled: bool) -> String {
    paint(Style::new().bold().underlined(), text, enabled)
}

/// Secondary text such as dates, previews and hints.
pub fn gray(text: &str, enabled: bool) -> String {
    paint(Style::new().black().bright(), text, enabled)
}

pub fn green(text: &str, enabled: bool) -> String {
    paint(Style::new().green(), text, enabled)
}

pub fn yellow(text: &str, enabled: bool) -> String {
    paint(Style::new().yellow(), text, enabled)
}

pub fn red(text: &str, enabled: bool) -> String {
    paint(Style::new().red(), text, enabled)
}

pub fn blue(text: &str, enabled: bool) -> String {
    paint(Style::new().blue(), text, enabled)
}

/// `text` in a Linear color such as `#5e6ad2` or `#abc`. Linear owns these
/// values, so one that is not a hex color leaves the text plain.
pub fn rgb(text: &str, hex: &str, enabled: bool) -> String {
    match parse_hex(hex) {
        Some([red, green, blue]) => paint(Style::new().true_color(red, green, blue), text, enabled),
        None => text.to_owned(),
    }
}

fn parse_hex(hex: &str) -> Option<[u8; 3]> {
    let digits = hex.strip_prefix('#')?;
    if !digits.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let channel = |range| u8::from_str_radix(digits.get(range)?, 16).ok();
    match digits.len() {
        6 => Some([channel(0..2)?, channel(2..4)?, channel(4..6)?]),
        3 => Some([
            channel(0..1)? * 17,
            channel(1..2)? * 17,
            channel(2..3)? * 17,
        ]),
        _ => None,
    }
}

pub fn warning(text: &str, enabled: bool) -> String {
    yellow(text, enabled)
}

#[cfg(test)]
mod tests {
    use super::{parse_hex, rgb};

    #[test]
    fn hex_colors_accept_long_and_short_forms() {
        assert_eq!(parse_hex("#010203"), Some([1, 2, 3]));
        assert_eq!(parse_hex("#abc"), Some([170, 187, 204]));
        assert_eq!(parse_hex("abc"), None);
        assert_eq!(parse_hex("#+1+2+3"), None);
        assert_eq!(parse_hex("#abcd"), None);
    }

    #[test]
    fn rgb_paints_only_when_enabled_and_valid() {
        assert_eq!(rgb("x", "#010203", true), "\x1b[38;2;1;2;3mx\x1b[0m");
        assert_eq!(rgb("x", "#010203", false), "x");
        assert_eq!(rgb("x", "red", true), "x");
    }
}
