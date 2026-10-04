//! Flag value types and parsers shared across commands.
use std::ffi::OsStr;
use std::fmt;
use std::path::PathBuf;
use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use clap::ValueEnum;
use clap::builder::{NonEmptyStringValueParser, TypedValueParser};

use crate::graphql::scalars::Float;
use crate::refs::reject_linear_url;

/// A user: yourself, or someone to look up.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UserRef {
    /// `@me`, also spelled `self`.
    Me,
    /// An email, a username (display name), or a name or part of one.
    Query(String),
}

impl FromStr for UserRef {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "" => Err("expected a user".to_owned()),
            "@me" | "self" => Ok(Self::Me),
            _ => {
                reject_linear_url(value, "an email, username, name, or @me")
                    .map_err(|error| error.message().to_owned())?;
                Ok(Self::Query(value.to_owned()))
            }
        }
    }
}

impl fmt::Display for UserRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Me => f.write_str("@me"),
            Self::Query(query) => f.write_str(query),
        }
    }
}

/// Text that must not be empty or only whitespace, such as a title, name, or
/// reference. An empty value gets clap's "a value is required" error.
#[derive(Clone, Copy, Debug)]
pub struct NonBlank;

impl TypedValueParser for NonBlank {
    type Value = String;

    fn parse_ref(
        &self,
        cmd: &clap::Command,
        arg: Option<&clap::Arg>,
        value: &OsStr,
    ) -> Result<String, clap::Error> {
        NonEmptyStringValueParser::new().parse_ref(cmd, arg, value)?;
        let not_blank = |text: &str| {
            if text.trim().is_empty() {
                Err("the value is only whitespace")
            } else {
                Ok(text.to_owned())
            }
        };
        not_blank.parse_ref(cmd, arg, value)
    }
}

/// Where to read a body or description from: a file, or `-` for stdin.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TextSource {
    Stdin,
    File(PathBuf),
}

impl FromStr for TextSource {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "" => Err("expected a file path, or - for stdin".to_owned()),
            "-" => Ok(Self::Stdin),
            path => Ok(Self::File(PathBuf::from(path))),
        }
    }
}

impl fmt::Display for TextSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stdin => f.write_str("stdin"),
            Self::File(path) => write!(f, "{}", path.display()),
        }
    }
}

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

/// A `#RRGGBB` color, as written.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HexColor(String);

impl FromStr for HexColor {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, String> {
        let digits = value
            .strip_prefix('#')
            .filter(|digits| digits.len() == 6 && digits.bytes().all(|b| b.is_ascii_hexdigit()));
        match digits {
            Some(_) => Ok(Self(value.to_owned())),
            None => Err(format!("expected a hex color like #5E6AD2, got {value:?}")),
        }
    }
}

impl From<HexColor> for String {
    fn from(color: HexColor) -> Self {
        color.0
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

impl From<InitiativeStatus> for crate::graphql::operations::initiative::InitiativeStatus {
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
    fn user_refs_spell_yourself_two_ways_and_refuse_urls() {
        assert_eq!("@me".parse(), Ok(UserRef::Me));
        assert_eq!("self".parse(), Ok(UserRef::Me));
        assert_eq!(
            "ada@example.com".parse(),
            Ok(UserRef::Query("ada@example.com".to_owned()))
        );
        for invalid in ["", "https://linear.app/acme/profiles/ada"] {
            assert!(invalid.parse::<UserRef>().is_err(), "{invalid}");
        }
    }

    #[test]
    fn text_sources_read_stdin_for_a_dash() {
        assert_eq!("-".parse(), Ok(TextSource::Stdin));
        assert_eq!(
            "notes.md".parse(),
            Ok(TextSource::File(PathBuf::from("notes.md")))
        );
        assert!("".parse::<TextSource>().is_err());
    }

    #[test]
    fn hex_color_needs_six_digits() {
        assert_eq!("#5e6AD2".parse(), Ok(HexColor("#5e6AD2".to_owned())));
        for invalid in ["5E6AD2", "#5E6AD", "#5E6AD2F", "#GGGGGG"] {
            assert!(invalid.parse::<HexColor>().is_err(), "{invalid}");
        }
    }
}
