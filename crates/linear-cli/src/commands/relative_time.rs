//! How every command shows a timestamp: "5 minutes ago" for the last week,
//! the local date after that. The clock and display zone are injected.

use chrono::{DateTime, TimeZone, Utc};

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

#[cfg(test)]
mod tests {
    use chrono::{DateTime, FixedOffset, Utc};

    use super::ago;

    fn instant(value: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(value)
            .expect("test timestamp")
            .to_utc()
    }

    #[test]
    fn thresholds_use_the_injected_clock() {
        let now = instant("2026-09-25T12:00:00.999999Z");
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
            ("2026-09-19T00:00:00Z", "6 days ago"),
            ("2026-09-18T12:00:00Z", "2026-09-18"),
            ("2026-09-25T17:30:00+05:30", "just now"),
        ] {
            assert_eq!(ago(instant(value), now, &Utc), expected, "{value}");
        }
    }

    #[test]
    fn absolute_date_uses_the_display_zone() {
        let now = instant("2026-09-25T12:00:00Z");
        let then = instant("2026-09-18T00:00:00Z");
        let west = FixedOffset::west_opt(7 * 3600).expect("west");
        let east = FixedOffset::east_opt(9 * 3600).expect("east");
        assert_eq!(ago(then, now, &west), "2026-09-17");
        assert_eq!(ago(then, now, &east), "2026-09-18");
    }
}
