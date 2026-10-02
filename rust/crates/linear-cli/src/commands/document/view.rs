//! Document detail: JSON-only complete comment pagination, raw body and metadata.
use crate::{
    error::Error,
    graphql::{
        envelope::{GraphQlRequest, is_not_found},
        operations::documents::*,
        transport::{GraphQlTransport, TransportFailure},
    },
};
use chrono::{DateTime, TimeZone, Utc};
use cynic::QueryBuilder;
use std::{collections::HashSet, future::Future};
pub const CONTEXT: &str = "Failed to view document";
pub enum DocumentResult {
    Body(DocumentBody),
    WithComments(DocumentWithComments),
}
pub fn body_request(id: &str) -> GraphQlRequest<GetDocumentVariables> {
    GraphQlRequest::with_variables(GetDocument::build(GetDocumentVariables {
        id: id.to_owned(),
    }))
}
pub fn comments_request(
    id: &str,
    after: Option<String>,
) -> GraphQlRequest<GetDocumentCommentsVariables> {
    GraphQlRequest::with_variables(GetDocumentWithComments::build(
        GetDocumentCommentsVariables {
            id: id.to_owned(),
            comments_after: after,
        },
    ))
}
fn translate(failure: TransportFailure, original: &str) -> Error {
    if let TransportFailure::GraphQl { errors, .. } = &failure
        && is_not_found(errors)
    {
        return Error::not_found("Document", original);
    }
    Error::from(failure)
}
pub async fn fetch(
    transport: &GraphQlTransport,
    original: &str,
    id: &str,
    json: bool,
) -> Result<DocumentResult, Error> {
    if json {
        return all_comments_with(id, |query| async move {
            transport
                .execute(&query)
                .await
                .map_err(|failure| translate(failure, original))
        })
        .await
        .map(DocumentResult::WithComments);
    }
    let data: GetDocument = transport
        .execute(&body_request(id))
        .await
        .map_err(|failure| translate(failure, original))?;
    data.document
        .ok_or_else(|| Error::not_found("Document", id))
        .map(DocumentResult::Body)
}
pub async fn all_comments_with<F, Fut>(
    id: &str,
    mut fetch: F,
) -> Result<DocumentWithComments, Error>
where
    F: FnMut(GraphQlRequest<GetDocumentCommentsVariables>) -> Fut,
    Fut: Future<Output = Result<GetDocumentWithComments, Error>>,
{
    let mut document = fetch(comments_request(id, None))
        .await?
        .document
        .ok_or_else(|| Error::not_found("Document", id))?;
    let mut seen = HashSet::new();
    while document.comments.page_info.has_next_page {
        let after = document
            .comments
            .page_info
            .end_cursor
            .clone()
            .ok_or_else(|| {
                Error::new(
                    "Linear reported more document comments but returned no pagination cursor",
                )
            })?;
        if !seen.insert(after.clone()) {
            return Err(Error::new(
                "Linear repeated a document comment pagination cursor",
            ));
        }
        let next = fetch(comments_request(id, Some(after)))
            .await?
            .document
            .ok_or_else(|| Error::not_found("Document", id))?;
        document.comments.nodes.extend(next.comments.nodes);
        document.comments.page_info = next.comments.page_info;
    }
    Ok(document)
}
impl DocumentResult {
    pub fn url(&self) -> &str {
        match self {
            Self::Body(document) => &document.url,
            Self::WithComments(document) => &document.url,
        }
    }
    pub fn json(&self) -> Result<Vec<u8>, Error> {
        let mut bytes = match self {
            Self::Body(document) => serde_json::to_vec_pretty(document),
            Self::WithComments(document) => serde_json::to_vec_pretty(document),
        }
        .map_err(|error| Error::new("could not serialize document").with_source(error))?;
        bytes.push(b'\n');
        Ok(bytes)
    }
}
pub fn raw(content: Option<&str>) -> Vec<u8> {
    content
        .filter(|content| !content.is_empty())
        .map_or(Vec::new(), |content| format!("{content}\n").into_bytes())
}
pub fn markdown<Tz: TimeZone>(
    document: &DocumentBody,
    content: Option<&str>,
    now: DateTime<Utc>,
    zone: &Tz,
) -> String {
    let mut lines = vec![
        format!("# {}", document.title),
        String::new(),
        format!("**Slug:** {}", document.slug_id),
        format!("**URL:** {}", document.url),
    ];
    if let Some(creator) = &document.creator {
        lines.push(format!("**Creator:** {}", creator.name));
    }
    if let Some(project) = &document.project {
        lines.push(format!("**Project:** {}", project.name));
    }
    if let Some(issue) = &document.issue {
        lines.push(format!("**Issue:** {} - {}", issue.identifier, issue.title));
    }
    if let Some(initiative) = &document.initiative {
        lines.push(format!("**Initiative:** {}", initiative.name));
    }
    if let Some(team) = &document.team {
        lines.push(format!("**Team:** {} ({})", team.name, team.key));
    }
    if let Some(cycle) = &document.cycle {
        let name = cycle
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
            .map_or(String::new(), |name| format!(" - {name}"));
        lines.push(format!(
            "**Cycle:** {} #{}{name}",
            cycle.team.key, cycle.number
        ));
    }
    if let Some(release) = &document.release {
        let version = release
            .version
            .as_deref()
            .filter(|version| !version.is_empty())
            .map_or(String::new(), |version| format!(" ({version})"));
        lines.push(format!("**Release:** {}{version}", release.name));
    }
    lines.push(format!(
        "**Created:** {}",
        crate::commands::relative_time::format_relative_time(&document.created_at.0, now, zone)
    ));
    lines.push(format!(
        "**Updated:** {}",
        crate::commands::relative_time::format_relative_time(&document.updated_at.0, now, zone)
    ));
    if let Some(content) = content.filter(|content| !content.is_empty()) {
        lines.extend([
            String::new(),
            "---".to_owned(),
            String::new(),
            content.to_owned(),
        ]);
    }
    lines.join("\n")
}
