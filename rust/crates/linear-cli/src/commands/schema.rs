//! Runtime schema introspection; JSON capability is independent of SDL meta-shape.
use crate::{
    error::{AppError, AppErrorKind},
    graphql::{
        bulk_error,
        envelope::GraphQlRequest,
        schema_introspection::{Model, QUERY},
        transport::{GraphQlTransport, classify_typed},
    },
    js_value::{JsValue, js_pretty},
};
pub const CONTEXT: &str = "Failed to fetch schema";
pub async fn fetch(transport: &GraphQlTransport) -> Result<JsValue, AppError> {
    let request: GraphQlRequest<()> = GraphQlRequest {
        query: QUERY.to_owned(),
        variables: None,
        operation_name: Some("IntrospectionQuery".into()),
    };
    let response = transport.send_request(&request).await?;
    // SDK JSON is decoded before its execution result is handled. Preserve its
    // malformed-error stage, but classify source-valid unsupported codec input
    // using the same whole-text RawValue boundary as the dynamic API command.
    let mime = response
        .headers
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_ascii_lowercase();
    if mime.contains("application/json") || mime.contains("application/graphql-response+json") {
        let text = String::from_utf8_lossy(&response.body);
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let _ = crate::commands::api::decode(text, true)?;
    }
    if let Some(error) =
        bulk_error::observe_source_error(&response, &request).map_err(|e| e.into_error())?
    {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            error.preferred_message.unwrap_or(error.message),
        ));
    }
    classify_typed(response).map_err(AppError::from)
}
pub fn content(value: &JsValue, json: bool) -> Result<String, AppError> {
    if json {
        Ok(js_pretty(value))
    } else {
        Model::parse(value)?.print()
    }
}
