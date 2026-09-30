//! Source-specific archived resolution: first match, with exchange-error fallback.
use crate::commands::initiative_view::{FETCH_CONTEXT, RESOLVE_CONTEXT, Reference};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::{GraphQlRequest, ResponseError};
use crate::graphql::operations::initiative_unarchive::{
    DetailVariables, GetInitiativeByNameIncludeArchived, GetInitiativeBySlugIncludeArchived,
    GetInitiativeForUnarchive, NameVariables, SlugVariables, UnarchiveDetail, UnarchiveInitiative,
    UnarchiveVariables,
};
use crate::graphql::operations::initiative_view::{ResolveInitiativeBySlug, UrlSlugVariables};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use crate::refs::is_linear_uuid;
use cynic::{MutationBuilder, QueryBuilder};
pub const CONTEXT: &str = "Failed to unarchive initiative";

// Only the two source text-query catches swallow these failures. Strict shape
// errors and impossible query states retain their error rather than resolving
// another entity after a corrupted response.
fn text_result<T>(result: Result<T, TransportFailure>) -> Result<Option<T>, AppError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(
            TransportFailure::GraphQl { .. }
            | TransportFailure::Http { .. }
            | TransportFailure::Network { .. }
            | TransportFailure::Timeout { .. }
            | TransportFailure::ResponseTooLarge { .. }
            | TransportFailure::Response(
                ResponseError::MalformedJson(_)
                | ResponseError::MissingData
                | ResponseError::GraphQl { .. },
            ),
        ) => Ok(None),
        Err(
            error @ (TransportFailure::RequestBody(_)
            | TransportFailure::Response(
                ResponseError::UnexpectedShape(_)
                | ResponseError::MutationRejected
                | ResponseError::MissingPayloadEntity,
            )),
        ) => Err(AppError::from(error)),
    }
}
fn missing(original: &str) -> AppError {
    AppError::not_found("Initiative", original)
}
async fn resolve_text(
    transport: &GraphQlTransport,
    token: &str,
    original: &str,
) -> Result<String, AppError> {
    let request =
        GraphQlRequest::with_variables(GetInitiativeBySlugIncludeArchived::build(SlugVariables {
            slug_id: token.to_owned(),
        }));
    let result: Option<GetInitiativeBySlugIncludeArchived> =
        text_result(transport.execute(&request).await)?;
    if let Some(node) = result.and_then(|data| data.initiatives.nodes.into_iter().next()) {
        return if node.id.inner().is_empty() {
            Err(missing(original))
        } else {
            Ok(node.id.into_inner())
        };
    }
    let request =
        GraphQlRequest::with_variables(GetInitiativeByNameIncludeArchived::build(NameVariables {
            name: token.to_owned(),
        }));
    let result: Option<GetInitiativeByNameIncludeArchived> =
        text_result(transport.execute(&request).await)?;
    result
        .and_then(|data| data.initiatives.nodes.into_iter().next())
        .map(|node| node.id.into_inner())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| missing(original))
}
pub async fn resolve_reference(
    transport: &GraphQlTransport,
    reference: &Reference,
    original: &str,
) -> Result<String, AppError> {
    let result = async {
        match reference {
            Reference::Id(id) => Ok(id.clone()),
            Reference::NameOrSlug(token) => resolve_text(transport, token, original).await,
            Reference::UrlSlug(slug_id) => {
                let request = GraphQlRequest::with_variables(ResolveInitiativeBySlug::build(
                    UrlSlugVariables {
                        slug_id: slug_id.clone(),
                        include_archived: Some(true),
                    },
                ));
                let data: ResolveInitiativeBySlug =
                    transport.execute(&request).await.map_err(AppError::from)?;
                let id = data
                    .initiatives
                    .nodes
                    .into_iter()
                    .next()
                    .map(|node| node.id.into_inner())
                    .ok_or_else(|| missing(original))?;
                if is_linear_uuid(&id) {
                    Ok(id)
                } else {
                    resolve_text(transport, &id, original).await
                }
            }
        }
    }
    .await;
    result.map_err(|error| error.with_context(RESOLVE_CONTEXT))
}
pub async fn fetch_details(
    transport: &GraphQlTransport,
    id: &str,
    original: &str,
) -> Result<UnarchiveDetail, AppError> {
    let request =
        GraphQlRequest::with_variables(GetInitiativeForUnarchive::build(DetailVariables {
            id: cynic::Id::new(id),
        }));
    let data: GetInitiativeForUnarchive = transport
        .execute(&request)
        .await
        .map_err(AppError::from)
        .map_err(|error| error.with_context(FETCH_CONTEXT))?;
    data.initiatives
        .nodes
        .into_iter()
        .next()
        .ok_or_else(|| missing(original).with_context(RESOLVE_CONTEXT))
}
pub fn active_output(detail: &UnarchiveDetail) -> Option<Vec<u8>> {
    if detail
        .archived_at
        .as_ref()
        .is_none_or(|date| date.0.is_empty())
    {
        Some(format!("Initiative \"{}\" is not archived.\n", detail.name).into_bytes())
    } else {
        None
    }
}
pub async fn submit(transport: &GraphQlTransport, id: &str) -> Result<Vec<u8>, AppError> {
    let request = GraphQlRequest::with_variables(UnarchiveInitiative::build(UnarchiveVariables {
        id: id.to_owned(),
    }));
    let data: UnarchiveInitiative = transport
        .execute(&request)
        .await
        .map_err(AppError::from)
        .map_err(|error| error.with_context(CONTEXT))?;
    if !data.initiative_unarchive.success {
        return Err(AppError::new(AppErrorKind::GraphQl, CONTEXT).with_context(CONTEXT));
    }
    let entity = data.initiative_unarchive.entity;
    let mut output = format!(
        "✓ Unarchived initiative: {}\n",
        entity
            .as_ref()
            .map_or("undefined", |node| node.name.as_str())
    );
    if let Some(entity) = entity.filter(|node| !node.url.is_empty()) {
        output.push_str(&entity.url);
        output.push('\n');
    }
    Ok(output.into_bytes())
}
