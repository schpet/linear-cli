//! `--limit`, shared by every list command: a positive number of rows, or
//! `all`.
use std::fmt;
use std::num::NonZeroU32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Limit {
    All,
    At(NonZeroU32),
}

impl Limit {
    /// The most rows to show; `None` for no limit.
    pub fn max(self) -> Option<NonZeroU32> {
        match self {
            Self::All => None,
            Self::At(count) => Some(count),
        }
    }

    /// Keeps the first rows of an already fetched, sorted list.
    pub fn apply<T>(self, rows: &mut Vec<T>) {
        if let Self::At(count) = self {
            rows.truncate(usize::try_from(count.get()).unwrap_or(usize::MAX));
        }
    }
}

impl fmt::Display for Limit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::All => f.write_str("all"),
            Self::At(count) => write!(f, "{count}"),
        }
    }
}

pub(crate) fn parse(value: &str) -> Result<Limit, String> {
    if value == "all" {
        return Ok(Limit::All);
    }
    if !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && let Ok(count) = value.parse::<NonZeroU32>()
    {
        return Ok(Limit::At(count));
    }
    Err(format!(
        "expected a positive number or `all`, got {value:?}"
    ))
}

#[cfg(test)]
mod tests {
    use super::{Limit, parse};
    use std::num::NonZeroU32;

    #[test]
    fn accepts_positive_numbers_and_all() {
        assert_eq!(parse("all"), Ok(Limit::All));
        assert_eq!(
            parse("25"),
            Ok(Limit::At(NonZeroU32::new(25).expect("nonzero")))
        );
        for bad in ["0", "-1", "+5", "1.5", "", "ALL", "99999999999"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn apply_keeps_the_first_rows() {
        let mut rows = vec![1, 2, 3];
        Limit::At(NonZeroU32::new(2).expect("nonzero")).apply(&mut rows);
        assert_eq!(rows, [1, 2]);
        Limit::All.apply(&mut rows);
        assert_eq!(rows, [1, 2]);
    }
}
