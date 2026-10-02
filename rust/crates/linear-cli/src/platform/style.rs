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

pub fn warning(text: &str, enabled: bool) -> String {
    yellow(text, enabled)
}
