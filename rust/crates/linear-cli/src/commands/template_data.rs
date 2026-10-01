//! A template's inner `templateData` as the JavaScript value `JSON.parse`
//! builds, for `template view`'s text output only; `--json` never parses it.
//!
//! Numbers are IEEE doubles, as in JavaScript: integer literals are rounded
//! through `f64` and printed with the ECMAScript number spelling. Object keys
//! follow `Object.entries` order: canonical array-index keys ascending, then
//! the other keys in insertion order. A duplicate key keeps its first position
//! and its last value. Unlike `JSON.parse`, an out-of-range number such as
//! `1e400` or an escaped lone surrogate is rejected as invalid JSON.

use crate::error::{AppError, AppErrorKind};
use crate::graphql::operations::templates::Template;
pub use crate::js_value::{JsObject, JsValue, js_number, js_stringify};

/// Decode a template's stringified `templateData` into its top-level object.
pub fn parse_template_data(template: &Template) -> Result<JsObject, AppError> {
    let subject = format!(
        "Template data for \"{}\" ({})",
        template.name,
        template.id.inner()
    );
    let decoded: JsValue = serde_json::from_str(&template.template_data.0).map_err(|error| {
        AppError::new(
            AppErrorKind::Validation,
            format!("{subject} is not valid JSON"),
        )
        .with_source(error)
    })?;
    match decoded {
        JsValue::Object(object) => Ok(object),
        _ => Err(AppError::new(
            AppErrorKind::Validation,
            format!("{subject} is not a JSON object"),
        )),
    }
}
