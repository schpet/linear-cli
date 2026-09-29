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

pub(crate) fn positive_limit(route: &str, option: &str) -> bool {
    option == "limit"
        && matches!(
            route,
            "linear document list" | "linear project-update list" | "linear initiative-update list"
        )
}
