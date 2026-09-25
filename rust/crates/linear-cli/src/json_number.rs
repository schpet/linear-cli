//! ECMAScript-compatible JSON number bytes for typed command projections.
//!
//! `RawValue` must be serialized directly inside the final Serde structure;
//! routing it through `serde_json::Value` would parse and format the number again.

use serde_json::value::RawValue;

use crate::error::{AppError, AppErrorKind};

pub fn finite_js_number(value: f64) -> Result<Box<RawValue>, AppError> {
    if !value.is_finite() {
        return Err(AppError::new(
            AppErrorKind::Invariant,
            "could not serialize non-finite JSON number",
        ));
    }
    let mut buffer = ryu_js::Buffer::new();
    let number = buffer.format(value);
    RawValue::from_string(number.to_owned()).map_err(|error| {
        AppError::new(AppErrorKind::Invariant, "could not serialize JSON number").with_source(error)
    })
}
