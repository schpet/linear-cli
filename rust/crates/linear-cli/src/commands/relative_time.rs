//! How every command shows a timestamp: "5 minutes ago" for the last week,
//! the local date after that. The clock and display zone are injected.

use chrono::{DateTime, NaiveDate, TimeZone, Utc};

/// Parses a Linear timestamp: RFC 3339, or a bare `YYYY-MM-DD` as UTC midnight.
pub fn parse_timestamp(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .map(|date| date.to_utc())
        .ok()
        .or_else(|| {
            NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .filter(|_| value.len() == 10)
                .and_then(|date| date.and_hms_opt(0, 0, 0))
                .map(|date| date.and_utc())
        })
}

/// How long ago `then` was: "just now" under a minute (or in the future),
/// then minutes, hours and days, and the `YYYY-MM-DD` date in `zone` after a week.
pub fn ago<Tz: TimeZone>(then: DateTime<Utc>, now: DateTime<Utc>, zone: &Tz) -> String {
    let elapsed = now.signed_duration_since(then);
    let count =
        |value: i64, unit: &str| format!("{value} {unit}{} ago", if value == 1 { "" } else { "s" });
    if elapsed.num_minutes() < 1 {
        "just now".to_owned()
    } else if elapsed.num_hours() < 1 {
        count(elapsed.num_minutes(), "minute")
    } else if elapsed.num_days() < 1 {
        count(elapsed.num_hours(), "hour")
    } else if elapsed.num_days() < 7 {
        count(elapsed.num_days(), "day")
    } else {
        then.with_timezone(zone).date_naive().to_string()
    }
}

/// [`ago`] for a timestamp as Linear sends it; text that does not parse is shown as-is.
pub fn format_relative_time<Tz: TimeZone>(value: &str, now: DateTime<Utc>, zone: &Tz) -> String {
    parse_timestamp(value).map_or_else(|| value.to_owned(), |then| ago(then, now, zone))
}
