//! Locale-aware string ordering (root locale), so accented names sort next to
//! their unaccented forms.
use std::cmp::Ordering;
use std::sync::LazyLock;

use icu_collator::{CollatorBorrowed, CollatorPreferences, options::CollatorOptions};

static ROOT: LazyLock<CollatorBorrowed<'static>> = LazyLock::new(|| {
    CollatorBorrowed::try_new(CollatorPreferences::default(), CollatorOptions::default())
        .unwrap_or_else(|error| unreachable!("root collation data is compiled in: {error}"))
});

pub fn compare(left: &str, right: &str) -> Ordering {
    ROOT.compare(left, right)
}
