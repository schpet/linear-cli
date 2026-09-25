//! The template/comment relative-time text, with clock and display zone injected.

use chrono::{DateTime, Datelike, NaiveDate, TimeZone, Utc};

/// Format the frozen CLI thresholds using millisecond precision and en-US M/D/Y.
/// Linear timestamps are RFC3339; an exact YYYY-MM-DD is UTC midnight.
pub fn format_relative_time<Tz: TimeZone>(value: &str, now: DateTime<Utc>, zone: &Tz) -> String {
    let parsed = DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&Utc))
        .ok()
        .or_else(|| {
            if value.len() != 10
                || !value.as_bytes().iter().enumerate().all(|(index, byte)| {
                    if matches!(index, 4 | 7) {
                        *byte == b'-'
                    } else {
                        byte.is_ascii_digit()
                    }
                })
            {
                return None;
            }
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .map(|date| date.and_utc())
        });
    let Some(parsed) = parsed else {
        return "Invalid Date".to_owned();
    };
    let diff_ms = now.timestamp_millis() - parsed.timestamp_millis();
    let minutes = diff_ms.div_euclid(60_000);
    let hours = diff_ms.div_euclid(3_600_000);
    let days = diff_ms.div_euclid(86_400_000);
    if minutes < 60 {
        if minutes <= 1 {
            "1 minute ago".to_owned()
        } else {
            format!("{minutes} minutes ago")
        }
    } else if hours < 24 {
        if hours == 1 {
            "1 hour ago".to_owned()
        } else {
            format!("{hours} hours ago")
        }
    } else if days < 7 {
        if days == 1 {
            "1 day ago".to_owned()
        } else {
            format!("{days} days ago")
        }
    } else {
        let local = parsed.with_timezone(zone);
        format!("{}/{}/{}", local.month(), local.day(), local.year())
    }
}
