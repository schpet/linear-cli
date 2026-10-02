//! Concurrent read-only `AuthListViewer` requests, one per stored workspace
//! key, rendered as one table in credential-store order.
use std::future::Future;

use cynic::QueryBuilder;
use reqwest::StatusCode;
use tokio::task::JoinSet;

use crate::auth::{self, CredentialStore};
use crate::commands::display::{display_width, pad};
use crate::config::TransportEnvInputs;
use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::{GraphQlRequest, ResponseError};
use crate::graphql::operations::auth_list::AuthListViewer;
use crate::graphql::transport::{
    ApiKey, EndpointUrl, GraphQlTransport, TransportBuildError, TransportConfig, TransportFailure,
};

pub const CONTEXT: &str = "Failed to list workspaces";
pub const EMPTY_OUTPUT: &str =
    "No workspaces configured\nRun `linear auth login` to add a workspace\n";

const WORKSPACE_HEADER: &str = "WORKSPACE";
const ORG_HEADER: &str = "ORG NAME";

/// One credential-store entry. `state` moves from a classified key, through a
/// prepared request, to a final outcome without reordering rows.
#[derive(Debug)]
pub struct Row<S> {
    pub workspace: String,
    pub is_default: bool,
    pub state: S,
}

#[derive(Debug)]
pub enum StoredKey {
    Missing,
    Unusable,
    Usable(ApiKey),
}

#[derive(Debug)]
pub enum Prepared<T> {
    Done(Outcome),
    Request(T),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Outcome {
    Viewer {
        organization: String,
        name: String,
        email: String,
    },
    Error(RowError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RowError {
    MissingCredentials,
    UnusableKey,
    InvalidCredentials,
    Failure(String),
}

impl RowError {
    fn text(&self) -> &str {
        match self {
            Self::MissingCredentials => "missing credentials",
            Self::UnusableKey => "invalid API key",
            Self::InvalidCredentials => "invalid credentials",
            Self::Failure(message) => message,
        }
    }
}

/// Classify every stored workspace without selection, network policy or I/O.
/// Raw/project API keys and `--workspace` do not participate.
pub fn classify(store: &CredentialStore) -> Vec<Row<StoredKey>> {
    store
        .workspaces()
        .iter()
        .map(|workspace| Row {
            workspace: workspace.clone(),
            is_default: store.default() == Some(workspace.as_str()),
            state: match store.key(workspace) {
                None => StoredKey::Missing,
                Some(secret) => match auth::header::to_api_key(secret) {
                    Ok(key) => StoredKey::Usable(key),
                    Err(_) => StoredKey::Unusable,
                },
            },
        })
        .collect()
}

/// Build every request handle in row order before any request starts, so a
/// later build failure cannot follow an earlier request.
pub fn prepare_with<T>(
    rows: Vec<Row<StoredKey>>,
    mut build: impl FnMut(ApiKey) -> Result<T, AppError>,
) -> Result<Vec<Row<Prepared<T>>>, AppError> {
    rows.into_iter()
        .map(|row| {
            let state = match row.state {
                StoredKey::Missing => Prepared::Done(Outcome::Error(RowError::MissingCredentials)),
                StoredKey::Unusable => Prepared::Done(Outcome::Error(RowError::UnusableKey)),
                StoredKey::Usable(key) => Prepared::Request(build(key)?),
            };
            Ok(Row {
                workspace: row.workspace,
                is_default: row.is_default,
                state,
            })
        })
        .collect()
}

/// Resolve the ambient transport policy once, and only when some key is usable.
pub fn prepare_transports(
    rows: Vec<Row<StoredKey>>,
    endpoint: &EndpointUrl,
    transport_env: &TransportEnvInputs,
) -> Result<Vec<Row<Prepared<GraphQlTransport>>>, AppError> {
    let usable = rows
        .iter()
        .any(|row| matches!(row.state, StoredKey::Usable(_)));
    let config: Option<TransportConfig> = if usable {
        Some(transport_env.production())
    } else {
        None
    };
    prepare_with(rows, |key| {
        let config = config.clone().ok_or_else(|| {
            AppError::new(
                AppErrorKind::Invariant,
                "transport policy was not resolved for a usable key",
            )
        })?;
        GraphQlTransport::new(endpoint.clone(), key, config)
            .map_err(|error: TransportBuildError| AppError::from(error))
    })
}

/// Start every prepared request at once and place each result at its row.
/// Request failures stay row-local; only a failed task is fatal.
pub async fn fetch_with<T, F, Fut>(
    rows: Vec<Row<Prepared<T>>>,
    fetch: F,
) -> Result<Vec<Row<Outcome>>, AppError>
where
    F: Fn(T, GraphQlRequest<()>) -> Fut,
    Fut: Future<Output = Result<AuthListViewer, TransportFailure>> + Send + 'static,
{
    let request = GraphQlRequest::without_variables(AuthListViewer::build(()));
    let mut tasks = JoinSet::new();
    let mut outcomes = Vec::with_capacity(rows.len());
    let mut labels = Vec::with_capacity(rows.len());
    for (index, row) in rows.into_iter().enumerate() {
        labels.push((row.workspace, row.is_default));
        match row.state {
            Prepared::Done(outcome) => outcomes.push(Some(outcome)),
            Prepared::Request(handle) => {
                outcomes.push(None);
                let pending = fetch(handle, request.clone());
                tasks.spawn(async move { (index, pending.await) });
            }
        }
    }
    while let Some(joined) = tasks.join_next().await {
        let (index, result) = joined.map_err(|error| {
            AppError::new(AppErrorKind::Invariant, "a workspace request task failed")
                .with_source(error)
        })?;
        let slot = outcomes.get_mut(index).ok_or_else(|| {
            AppError::new(
                AppErrorKind::Invariant,
                "workspace request index out of range",
            )
        })?;
        if slot.is_some() {
            return Err(AppError::new(
                AppErrorKind::Invariant,
                "workspace request completed twice",
            ));
        }
        *slot = Some(match result {
            Ok(data) => Outcome::Viewer {
                organization: data.viewer.organization.name,
                name: data.viewer.name,
                email: data.viewer.email,
            },
            Err(failure) => Outcome::Error(row_error(&failure)),
        });
    }
    labels
        .into_iter()
        .zip(outcomes)
        .map(|((workspace, is_default), outcome)| {
            let state = outcome.ok_or_else(|| {
                AppError::new(AppErrorKind::Invariant, "workspace request never completed")
            })?;
            Ok(Row {
                workspace,
                is_default,
                state,
            })
        })
        .collect()
}

pub async fn fetch(
    rows: Vec<Row<Prepared<GraphQlTransport>>>,
) -> Result<Vec<Row<Outcome>>, AppError> {
    fetch_with(rows, |transport, request| async move {
        transport.execute::<AuthListViewer, ()>(&request).await
    })
    .await
}

fn rejected(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
}

/// A short, stable cell for one failed request. Any response known to carry
/// HTTP 401 or 403 means the stored key was refused.
pub fn row_error(failure: &TransportFailure) -> RowError {
    match failure {
        TransportFailure::GraphQl { status, .. }
        | TransportFailure::ResponseTooLarge { status, .. }
            if rejected(*status) =>
        {
            RowError::InvalidCredentials
        }
        TransportFailure::Http { response, .. } if rejected(response.status) => {
            RowError::InvalidCredentials
        }
        TransportFailure::Response(ResponseError::MalformedJson(_)) => {
            RowError::Failure("response body is not valid JSON".to_owned())
        }
        TransportFailure::Response(ResponseError::UnexpectedShape(_)) => {
            RowError::Failure("response did not match the expected viewer shape".to_owned())
        }
        TransportFailure::Response(
            error @ (ResponseError::NotJson { .. }
            | ResponseError::GraphQl { .. }
            | ResponseError::MissingData
            | ResponseError::MutationRejected
            | ResponseError::MissingPayloadEntity),
        ) => RowError::Failure(error.to_string()),
        TransportFailure::GraphQl { .. }
        | TransportFailure::Http { .. }
        | TransportFailure::ResponseTooLarge { .. }
        | TransportFailure::Timeout { .. }
        | TransportFailure::Network { .. }
        | TransportFailure::RequestBody(_) => RowError::Failure(failure.to_string()),
    }
}

fn org_cell(row: &Row<Outcome>) -> &str {
    match &row.state {
        Outcome::Viewer { organization, .. } => organization,
        Outcome::Error(error) => error.text(),
    }
}

/// Render the whole table into one buffer. `color` is true only when stdout
/// is a color terminal.
pub fn render(rows: &[Row<Outcome>], color: bool) -> Vec<u8> {
    if rows.is_empty() {
        return EMPTY_OUTPUT.as_bytes().to_vec();
    }
    let workspace_width = rows
        .iter()
        .map(|row| display_width(&row.workspace))
        .fold(display_width(WORKSPACE_HEADER), usize::max);
    let org_width = rows
        .iter()
        .map(|row| display_width(org_cell(row)))
        .fold(display_width(ORG_HEADER), usize::max);
    let header = format!(
        "  {} {} USER",
        pad(WORKSPACE_HEADER, workspace_width),
        pad(ORG_HEADER, org_width)
    );
    let mut output = if color {
        format!("\x1b[4m{header}\x1b[0m\n")
    } else {
        format!("{header}\n")
    };
    for row in rows {
        let prefix = if row.is_default { "* " } else { "  " };
        let workspace = pad(&row.workspace, workspace_width);
        match &row.state {
            Outcome::Viewer {
                organization,
                name,
                email,
            } => output.push_str(&format!(
                "{prefix}{workspace} {} {name} <{email}>\n",
                pad(organization, org_width)
            )),
            Outcome::Error(error) => {
                let cell = pad(error.text(), org_width);
                if color {
                    output.push_str(&format!(
                        "{prefix}{workspace} \x1b[31m{cell}\x1b[39m\x1b[0m\n"
                    ));
                } else {
                    output.push_str(&format!("{prefix}{workspace} {cell}\n"));
                }
            }
        }
    }
    output.into_bytes()
}
