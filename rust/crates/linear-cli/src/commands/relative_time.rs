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

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, Utc};

    use super::format_relative_time;

    #[test]
    fn thresholds_use_the_injected_clock() {
        let now = DateTime::parse_from_rfc3339("2026-09-25T12:00:00.999999Z")
            .expect("now")
            .with_timezone(&Utc);
        for (value, expected) in [
            ("2026-09-25T12:00:01Z", "just now"),
            ("2026-09-25T12:00:00Z", "just now"),
            ("2026-09-25T11:59:00Z", "1 minute ago"),
            ("2026-09-25T11:58:01.999999Z", "1 minute ago"),
            ("2026-09-25T11:58:00Z", "2 minutes ago"),
            ("2026-09-25T11:01:00Z", "59 minutes ago"),
            ("2026-09-25T11:00:00Z", "1 hour ago"),
            ("2026-09-25T10:00:00Z", "2 hours ago"),
            ("2026-09-24T13:00:00Z", "23 hours ago"),
            ("2026-09-24T12:00:00Z", "1 day ago"),
            ("2026-09-19", "6 days ago"),
            ("2026-09-18T12:00:00Z", "2026-09-18"),
            ("2026-09-25T17:30:00+05:30", "just now"),
            ("not a date", "not a date"),
            ("2026-9-5", "2026-9-5"),
        ] {
            assert_eq!(format_relative_time(value, now, &Utc), expected, "{value}");
        }
    }

    #[test]
    fn date_only_uses_utc_midnight_and_absolute_date_uses_display_zone() {
        let now = DateTime::parse_from_rfc3339("2026-09-25T12:00:00Z")
            .expect("now")
            .with_timezone(&Utc);
        let west = FixedOffset::west_opt(7 * 3600).expect("west");
        let east = FixedOffset::east_opt(9 * 3600).expect("east");
        assert_eq!(format_relative_time("2026-09-18", now, &west), "2026-09-17");
        assert_eq!(format_relative_time("2026-09-18", now, &east), "2026-09-18");
    }
}
