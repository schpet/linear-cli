//! Shared table presentation primitives for list commands.

use std::time::SystemTime;

use chrono::{DateTime, NaiveDate, Utc};

pub fn utf16_len(text: &str) -> usize {
    text.encode_utf16().count()
}

/// The source `team list` relative-time wording, distinct from template dates.
pub fn time_ago(value: &str, now: SystemTime) -> String {
    let updated = DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|date| date.to_utc())
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .map(|date| DateTime::<Utc>::from_naive_utc_and_offset(date, Utc))
        });
    let Some(updated) = updated else {
        return "NaN days ago".to_owned();
    };
    let now: DateTime<Utc> = now.into();
    let diff = now.signed_duration_since(updated);
    let minutes = diff.num_milliseconds().div_euclid(60_000);
    if minutes < 1 {
        return "just now".to_owned();
    }
    if minutes < 60 {
        return format!("{minutes} minutes ago");
    }
    let hours = minutes.div_euclid(60);
    if hours < 24 {
        return format!("{hours} hour{} ago", if hours == 1 { "" } else { "s" });
    }
    let days = hours.div_euclid(24);
    format!("{days} day{} ago", if days == 1 { "" } else { "s" })
}

pub fn terminal_color(color: &str) -> Option<String> {
    let hex = color.strip_prefix('#')?;
    let rgb = match hex.len() {
        6 => {
            let red = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
            let green = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
            let blue = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
            (red, green, blue)
        }
        3 => {
            let red = u8::from_str_radix(hex.get(0..1)?, 16).ok()? * 17;
            let green = u8::from_str_radix(hex.get(1..2)?, 16).ok()? * 17;
            let blue = u8::from_str_radix(hex.get(2..3)?, 16).ok()? * 17;
            (red, green, blue)
        }
        _ => return None,
    };
    Some(format!("\x1b[38;2;{};{};{}m", rgb.0, rgb.1, rgb.2))
}

/// Render already-padded header cells with the frozen per-cell underline codes.
pub fn underlined_header(cells: &[String], color: bool) -> String {
    if !color {
        return format!("{}\n", cells.join(" "));
    }
    let mut line = String::new();
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            line.push(' ');
        }
        line.push_str("\x1b[4m");
        line.push_str(cell);
        line.push_str(if index + 1 == cells.len() {
            "\x1b[0m"
        } else {
            "\x1b[24m"
        });
    }
    line.push('\n');
    line
}

pub fn stdout_columns(is_terminal: bool) -> usize {
    if !is_terminal {
        return 120;
    }
    if let Some((terminal_size::Width(width), _)) =
        terminal_size::terminal_size_of(std::io::stdout())
    {
        return usize::from(width);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{stdout_columns, terminal_color, time_ago, underlined_header, utf16_len};
    use std::time::{Duration, UNIX_EPOCH};

    #[test]
    fn utf16_counts_code_units() {
        assert_eq!(utf16_len("A👩‍💻"), 6);
    }

    #[test]
    fn source_relative_time_thresholds_and_invalid_input() {
        let now = UNIX_EPOCH + Duration::from_secs(86_400);
        assert_eq!(time_ago("1970-01-02T00:00:00Z", now), "just now");
        assert_eq!(time_ago("1970-01-03T00:00:00Z", now), "just now");
        assert_eq!(time_ago("1970-01-01T23:59:00Z", now), "1 minutes ago");
        assert_eq!(time_ago("1970-01-01T23:01:00Z", now), "59 minutes ago");
        assert_eq!(time_ago("1970-01-01T23:00:00Z", now), "1 hour ago");
        assert_eq!(time_ago("1970-01-01T22:00:00Z", now), "2 hours ago");
        assert_eq!(time_ago("1970-01-01", now), "1 day ago");
        assert_eq!(
            time_ago("1970-01-01", now + Duration::from_secs(86_400)),
            "2 days ago"
        );
        assert_eq!(time_ago("invalid", now), "NaN days ago");
    }

    #[test]
    fn rgb_color_and_underlined_header_preserve_control_bytes() {
        assert_eq!(
            terminal_color("#abc").as_deref(),
            Some("\x1b[38;2;170;187;204m")
        );
        assert_eq!(
            terminal_color("#010203").as_deref(),
            Some("\x1b[38;2;1;2;3m")
        );
        assert_eq!(terminal_color("abc"), None);
        let cells = ["KEY".to_owned(), "NAME ".to_owned()];
        assert_eq!(underlined_header(&cells, false), "KEY NAME \n");
        assert_eq!(
            underlined_header(&cells, true),
            "\x1b[4mKEY\x1b[24m \x1b[4mNAME \x1b[0m\n"
        );
    }

    #[test]
    fn non_terminal_width_is_fixed() {
        assert_eq!(stdout_columns(false), 120);
    }
}
