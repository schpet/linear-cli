//! Flag value types and parsers shared across commands.
use chrono::{DateTime, NaiveDate, Utc};
use clap::ValueEnum;

use crate::graphql::operations::number::Float;

/// A `YYYY-MM-DD` calendar date.
pub fn date(value: &str) -> Result<NaiveDate, String> {
    NaiveDate::parse_from_str(value, "%Y-%m-%d")
        .map_err(|_| format!("expected a YYYY-MM-DD date, got {value:?}"))
}

/// A `YYYY-MM-DD` date (midnight UTC) or an RFC 3339 date-time such as
/// `2026-01-31T09:00:00Z`.
pub fn date_or_datetime(value: &str) -> Result<DateTime<Utc>, String> {
    if let Ok(date) = NaiveDate::parse_from_str(value, "%Y-%m-%d") {
        return Ok(date.and_time(chrono::NaiveTime::MIN).and_utc());
    }
    DateTime::parse_from_rfc3339(value)
        .map(|datetime| datetime.with_timezone(&Utc))
        .map_err(|_| format!("expected a YYYY-MM-DD date or an RFC 3339 date-time, got {value:?}"))
}

/// A `#RRGGBB` color.
pub fn hex_color(value: &str) -> Result<String, String> {
    let digits = value
        .strip_prefix('#')
        .filter(|digits| digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()));
    match digits {
        Some(_) => Ok(value.to_owned()),
        None => Err(format!("expected a hex color like #5E6AD2, got {value:?}")),
    }
}

/// A finite number for a `sortOrder`; whole values are sent as integers.
pub fn sort_order(value: &str) -> Result<Float, String> {
    serde_json::from_str(value).map_err(|_| format!("expected a number, got {value:?}"))
}

/// An issue estimate in points.
pub fn estimate(value: &str) -> Result<i32, String> {
    value
        .parse::<i32>()
        .ok()
        .filter(|points| *points >= 0)
        .ok_or_else(|| format!("expected a whole number of points, got {value:?}"))
}

/// An issue or project priority, by name or by Linear's number (0-4).
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum Priority {
    #[value(name = "none", alias = "0")]
    None,
    #[value(name = "urgent", alias = "1")]
    Urgent,
    #[value(name = "high", alias = "2")]
    High,
    #[value(name = "medium", alias = "3")]
    Medium,
    #[value(name = "low", alias = "4")]
    Low,
}

impl Priority {
    /// Linear's number for the priority.
    pub fn number(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Urgent => 1,
            Self::High => 2,
            Self::Medium => 3,
            Self::Low => 4,
        }
    }
}

/// An initiative status.
#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub enum InitiativeStatus {
    #[value(name = "planned")]
    Planned,
    #[value(name = "active")]
    Active,
    #[value(name = "completed")]
    Completed,
}

impl From<InitiativeStatus> for crate::graphql::operations::initiatives::InitiativeStatus {
    fn from(status: InitiativeStatus) -> Self {
        match status {
            InitiativeStatus::Planned => Self::Planned,
            InitiativeStatus::Active => Self::Active,
            InitiativeStatus::Completed => Self::Completed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn date_or_datetime_normalizes_to_utc() {
        let parse = |value| date_or_datetime(value).map(|datetime| datetime.to_rfc3339());
        assert_eq!(
            parse("2024-02-29"),
            Ok("2024-02-29T00:00:00+00:00".to_owned())
        );
        assert_eq!(
            parse("2026-01-02T03:04:05+02:30"),
            Ok("2026-01-02T00:34:05+00:00".to_owned())
        );
        for invalid in ["2025-02-29", "yesterday", "2026-01-02T03:04:05", "2026-01"] {
            assert!(parse(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn sort_order_keeps_whole_numbers_integral() {
        let json = |value| {
            sort_order(value)
                .map(|order| serde_json::to_string(&order).expect("a Float serializes"))
        };
        assert_eq!(json("2"), Ok("2".to_owned()));
        assert_eq!(json("2.0"), Ok("2".to_owned()));
        assert_eq!(json("-1.5"), Ok("-1.5".to_owned()));
        for invalid in ["", "abc", "NaN", "inf", "1e999"] {
            assert!(sort_order(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn hex_color_needs_six_digits() {
        assert_eq!(hex_color("#5e6AD2"), Ok("#5e6AD2".to_owned()));
        for invalid in ["5E6AD2", "#5E6AD", "#5E6AD2F", "#GGGGGG"] {
            assert!(hex_color(invalid).is_err(), "{invalid}");
        }
    }
}
