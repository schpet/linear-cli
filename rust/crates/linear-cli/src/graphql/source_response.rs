//! One source-compatible MIME/text/syntax boundary for captured GraphQL responses.
//! Raw API Fetch responses, assets and signed uploads do not use this classifier.
use super::transport::RawHttpResponse;
use serde_json::Value;

pub(crate) enum SourceBody {
    Text,
    Json(Result<Value, serde_json::Error>),
}
pub(crate) struct SourceResponse {
    pub(crate) text: String,
    pub(crate) body: SourceBody,
}
impl SourceResponse {
    pub(crate) fn classify(response: &RawHttpResponse) -> Self {
        // Fetch.text decodes lossily and drops its leading BOM. Header suffixes
        // may contain non-ASCII bytes: admission is the source's ASCII substring
        // rule, not HeaderValue::to_str or a stricter media-type grammar.
        let decoded = String::from_utf8_lossy(&response.body);
        let text = decoded
            .strip_prefix('\u{feff}')
            .unwrap_or(&decoded)
            .to_owned();
        // Fetch Headers joins repeated Content-Type values. Either substring
        // must lie entirely in a value (the join separator contains a comma).
        let json = has_json_mime(&response.headers);
        let body = if json {
            SourceBody::Json(serde_json::from_str(&text))
        } else {
            SourceBody::Text
        };
        Self { text, body }
    }
    pub(crate) fn is_json(&self) -> bool {
        matches!(&self.body, SourceBody::Json(_))
    }
    pub(crate) fn parsed(&self) -> Option<&Value> {
        match &self.body {
            SourceBody::Json(Ok(value)) => Some(value),
            SourceBody::Text | SourceBody::Json(Err(_)) => None,
        }
    }
    pub(crate) fn invalid_execution_message(&self) -> String {
        format!(
            "Invalid execution result: result is not object or array. \nGot:\n{}",
            self.text
        )
    }
}

/// Whether any Content-Type header names a JSON media type.
fn has_json_mime(headers: &reqwest::header::HeaderMap) -> bool {
    headers
        .get_all(reqwest::header::CONTENT_TYPE)
        .iter()
        .any(|header| {
            let mime = String::from_utf8_lossy(header.as_bytes()).to_ascii_lowercase();
            mime.contains("application/json") || mime.contains("application/graphql-response+json")
        })
}
