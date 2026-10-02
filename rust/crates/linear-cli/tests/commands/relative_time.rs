use chrono::{DateTime, FixedOffset, Utc};
use linear_cli::commands::relative_time::format_relative_time;

#[test]
fn thresholds_use_the_injected_clock() {
    let now = DateTime::parse_from_rfc3339("2026-09-25T12:00:00.999999Z")
        .expect("now")
        .with_timezone(&Utc);
    for (value, expected) in [
        ("2026-09-25T12:00:01Z", "just now"),
        ("2026-09-25T12:00:00Z", "just now"),
        ("2026-09-25T11:59:00Z", "1 minute ago"),
        ("2026-09-25T11:58:01Z", "1 minute ago"),
        ("2026-09-25T11:58:00Z", "2 minutes ago"),
        ("2026-09-25T11:01:00Z", "59 minutes ago"),
        ("2026-09-25T11:00:00Z", "1 hour ago"),
        ("2026-09-24T13:00:00Z", "23 hours ago"),
        ("2026-09-24T12:00:00Z", "1 day ago"),
        ("2026-09-19T12:00:00Z", "6 days ago"),
        ("2026-09-18T12:00:00Z", "9/18/2026"),
        ("2026-09-25T17:30:00+05:30", "just now"),
        ("2026-09-25T11:58:01.999999Z", "1 minute ago"),
        ("not a date", "not a date"),
    ] {
        assert_eq!(format_relative_time(value, now, &Utc), expected, "{value}");
    }
    assert_eq!(format_relative_time("2026-9-5", now, &Utc), "2026-9-5");
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

#[test]
fn units_are_pluralized_and_old_dates_are_shown_as_dates() {
    let now: DateTime<Utc> = "2026-01-10T12:00:00Z".parse().expect("now");
    let ago = |value: &str| format_relative_time(value, now, &Utc);
    assert_eq!(ago("2026-01-10T13:00:00Z"), "just now");
    assert_eq!(ago("2026-01-10T11:59:00Z"), "1 minute ago");
    assert_eq!(ago("2026-01-10T10:00:00Z"), "2 hours ago");
    assert_eq!(ago("2026-01-09T12:00:00Z"), "1 day ago");
    assert_eq!(ago("2026-01-04"), "6 days ago");
    assert_eq!(ago("2026-01-03T12:00:00Z"), "1/3/2026");
}
