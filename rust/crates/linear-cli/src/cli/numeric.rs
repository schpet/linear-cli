//! Shared strict numeric input boundaries. API/output numbers are unaffected.
use std::num::NonZeroU32;

pub(crate) fn finite_decimal(value: &str) -> Result<f64, String> {
    if value.is_empty()
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'+' | b'-' | b'.' | b'e' | b'E'))
    {
        return Err(format!("expected a finite decimal number, got {value:?}"));
    }
    match value.parse::<f64>() {
        Ok(number) if number.is_finite() => Ok(number),
        Ok(_) | Err(_) => Err(format!("expected a finite decimal number, got {value:?}")),
    }
}

/// An issue-list `--limit`: a non-negative integer, where 0 means no limit.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct IssueLimit(pub Option<NonZeroU32>);

pub(crate) fn issue_limit(value: &str) -> Result<IssueLimit, String> {
    if !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && let Ok(number) = value.parse::<u32>()
    {
        return Ok(IssueLimit(NonZeroU32::new(number)));
    }
    Err(format!(
        "expected a non-negative decimal integer (0 for no limit), got {value:?}"
    ))
}

pub(crate) fn positive_u32(value: &str) -> Result<NonZeroU32, String> {
    if !value.is_empty()
        && value.bytes().all(|byte| byte.is_ascii_digit())
        && let Ok(number) = value.parse::<NonZeroU32>()
    {
        return Ok(number);
    }
    Err(format!(
        "expected a positive decimal integer between 1 and 4294967295, got {value:?}"
    ))
}
