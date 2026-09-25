use chrono::{DateTime, FixedOffset, Utc};
use linear_cli::commands::relative_time::format_relative_time;

#[test]
fn thresholds_use_floored_milliseconds_and_injected_clock() {
    let now = DateTime::parse_from_rfc3339("2026-09-25T12:00:00.999999Z")
        .expect("now")
        .with_timezone(&Utc);
    for (value, expected) in [
        ("2026-09-25T12:00:01Z", "1 minute ago"),
        ("2026-09-25T12:00:00Z", "1 minute ago"),
        ("2026-09-25T11:59:00Z", "1 minute ago"),
        ("2026-09-25T11:58:01Z", "1 minute ago"),
        ("2026-09-25T11:58:00Z", "2 minutes ago"),
        ("2026-09-25T11:01:00Z", "59 minutes ago"),
        ("2026-09-25T11:00:00Z", "1 hour ago"),
        ("2026-09-24T13:00:00Z", "23 hours ago"),
        ("2026-09-24T12:00:00Z", "1 day ago"),
        ("2026-09-19T12:00:00Z", "6 days ago"),
        ("2026-09-18T12:00:00Z", "9/18/2026"),
        ("2026-09-25T17:30:00+05:30", "1 minute ago"),
        ("2026-09-25T11:58:01.999999Z", "1 minute ago"),
        ("not a date", "Invalid Date"),
    ] {
        assert_eq!(format_relative_time(value, now, &Utc), expected, "{value}");
    }
    // Rust's strict date-only contract deliberately excludes V8 legacy forms.
    assert_eq!(format_relative_time("2026-9-5", now, &Utc), "Invalid Date");
}

#[test]
fn date_only_uses_utc_midnight_and_absolute_date_uses_display_zone() {
    let now = DateTime::parse_from_rfc3339("2026-09-25T12:00:00Z")
        .expect("now")
        .with_timezone(&Utc);
    let west = FixedOffset::west_opt(7 * 3600).expect("west");
    let east = FixedOffset::east_opt(9 * 3600).expect("east");
    assert_eq!(format_relative_time("2026-09-18", now, &west), "9/17/2026");
    assert_eq!(format_relative_time("2026-09-18", now, &east), "9/18/2026");
}
