//! Runtime schema introspection; JSON capability is independent of SDL meta-shape.
use crate::{
    error::{AppError, AppErrorKind},
    graphql::{
        bulk_error,
        envelope::GraphQlRequest,
        schema_introspection::{Model, QUERY},
        transport::{GraphQlTransport, classify_typed},
    },
};
pub const CONTEXT: &str = "Failed to fetch schema";
pub async fn fetch(transport: &GraphQlTransport) -> Result<serde_json::Value, AppError> {
    let request: GraphQlRequest<()> = GraphQlRequest {
        query: QUERY.to_owned(),
        variables: None,
        operation_name: Some("IntrospectionQuery".into()),
    };
    let response = transport.send_request(&request).await?;
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
pub fn content(value: &serde_json::Value, json: bool) -> Result<String, AppError> {
    if json {
        serde_json::to_string_pretty(value).map_err(|error| {
            AppError::new(AppErrorKind::Invariant, "could not serialize schema").with_source(error)
        })
    } else {
        Model::parse(value)?.print()
    }
}
