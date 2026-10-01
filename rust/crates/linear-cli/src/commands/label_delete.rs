//! Source lookup order, client-side scope and attended duplicate selection.
use std::io::{Read, Write};

use cynic::{MutationBuilder, QueryBuilder};

use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::{GraphQlRequest, ResponseError};
use crate::graphql::operations::label_delete::{
    DeleteIssueLabel, GetLabelById, GetLabelByName, IdVariables, Label, NameVariables,
};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use crate::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};
use crate::refs::{is_linear_uuid, reject_linear_url};

pub const CONTEXT: &str = "Failed to delete label";

pub fn id_request(id: &str) -> GraphQlRequest<IdVariables> {
    GraphQlRequest::with_variables(GetLabelById::build(IdVariables { id: id.to_owned() }))
}

pub fn name_request(name: &str) -> GraphQlRequest<NameVariables> {
    GraphQlRequest::with_variables(GetLabelByName::build(NameVariables {
        name: name.to_owned(),
    }))
}

pub fn delete_request(id: &str) -> GraphQlRequest<IdVariables> {
    GraphQlRequest::with_variables(DeleteIssueLabel::build(IdVariables { id: id.to_owned() }))
}

// Source lookup catches swallow ordinary exchange failures. Broken typed
// boundaries and impossible request/payload states stop instead of deleting
// another label after a corrupted response.
fn lookup_result<T>(result: Result<T, TransportFailure>) -> Result<Option<T>, AppError> {
    match result {
        Ok(value) => Ok(Some(value)),
        Err(
            TransportFailure::GraphQl { .. }
            | TransportFailure::Http { .. }
            | TransportFailure::Network { .. }
            | TransportFailure::Timeout { .. }
            | TransportFailure::ResponseTooLarge { .. }
            | TransportFailure::Response(
                ResponseError::NonJsonExecution(_)
                | ResponseError::MalformedJson(_)
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

pub enum Lookup {
    Direct(Label),
    Named(Vec<Label>),
}

pub async fn lookup(transport: &GraphQlTransport, original: &str) -> Result<Lookup, AppError> {
    reject_linear_url(original, "a label name or UUID")?;
    if is_linear_uuid(original) {
        let data: Option<GetLabelById> =
            lookup_result(transport.execute(&id_request(original)).await)?;
        if let Some(data) = data {
            return Ok(Lookup::Direct(data.issue_label));
        }
    }
    let data: Option<GetLabelByName> =
        lookup_result(transport.execute(&name_request(original)).await)?;
    Ok(Lookup::Named(
        data.map_or_else(Vec::new, |data| data.issue_labels.nodes),
    ))
}

/// UUID successes bypass team filtering; all name results preserve server order.
pub fn scoped(lookup: Lookup, team: Option<&str>) -> Vec<Label> {
    let labels = match lookup {
        Lookup::Direct(label) => return vec![label],
        Lookup::Named(labels) => labels,
    };
    match team {
        Some(key) if !key.is_empty() => {
            let chosen = labels
                .iter()
                .position(|label| {
                    label
                        .team
                        .as_ref()
                        .is_some_and(|team| team.key.to_lowercase() == key.to_lowercase())
                })
                .or_else(|| labels.iter().position(|label| label.team.is_none()));
            labels
                .into_iter()
                .enumerate()
                .filter_map(|(index, label)| (Some(index) == chosen).then_some(label))
                .collect()
        }
        Some(_) | None => labels,
    }
}

pub fn missing(original: &str, team: Option<&str>) -> AppError {
    let error = AppError::not_found("Label", original);
    match team.filter(|key| !key.is_empty()) {
        Some(key) => error.with_suggestion(format!("Searched in team {key} and workspace.")),
        None => error,
    }
}

pub fn display(label: &Label) -> String {
    let team = label
        .team
        .as_ref()
        .map(|team| team.key.as_str())
        .filter(|key| !key.is_empty())
        .unwrap_or("Workspace");
    format!("{} ({team})", label.name)
}

pub fn choose<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    original: &str,
    labels: &[Label],
) -> Result<PromptOutcome<Label>, AppError> {
    let options: Vec<_> = labels
        .iter()
        .map(|label| PlainOption {
            label: format!("{} - {}", display(label), label.color),
            value: label.id.inner().to_owned(),
            script_token: label.id.inner().to_owned(),
        })
        .collect();
    match session.select(&PlainSelect {
        message: &format!("Multiple labels named \"{original}\" found. Which one?"),
        options: &options,
        default_index: 0,
        default_hint: None,
    })? {
        PromptOutcome::Submitted(id) => labels
            .iter()
            .find(|label| label.id.inner() == id)
            .cloned()
            .map(PromptOutcome::Submitted)
            .ok_or_else(|| AppError::new(AppErrorKind::Invariant, "selected label ID is missing")),
        PromptOutcome::Interrupted => Ok(PromptOutcome::Interrupted),
        PromptOutcome::EndOfInput => Ok(PromptOutcome::EndOfInput),
    }
}

pub async fn submit(transport: &GraphQlTransport, label: &Label) -> Result<Vec<u8>, AppError> {
    let data: DeleteIssueLabel = transport.execute(&delete_request(label.id.inner())).await?;
    if !data.issue_label_delete.success {
        return Err(AppError::new(AppErrorKind::GraphQl, CONTEXT));
    }
    Ok(format!("✓ Deleted label: {}\n", display(label)).into_bytes())
}
