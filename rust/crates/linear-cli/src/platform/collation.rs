//! Root-locale collation for commands that mirror JavaScript `localeCompare`.
use icu_collator::{CollatorBorrowed, CollatorPreferences, options::CollatorOptions};

use crate::error::{AppError, AppErrorKind};

pub fn root() -> Result<CollatorBorrowed<'static>, AppError> {
    CollatorBorrowed::try_new(CollatorPreferences::default(), CollatorOptions::default()).map_err(
        |error| {
            AppError::new(
                AppErrorKind::Invariant,
                "could not load root collation data",
            )
            .with_source(error)
        },
    )
}
