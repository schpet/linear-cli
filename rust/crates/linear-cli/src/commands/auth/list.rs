//! `auth list`: every stored workspace with the organization and user its
//! key belongs to, checked with one request per key, all at once.
use futures_util::future::join_all;
use reqwest::StatusCode;

use crate::auth::{self, CredentialStore};
use crate::client::{LinearClient, RequestError};
use crate::commands::table::{Cell, Column, Table};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::envelope::graphql_message;
use crate::graphql::operations::auth_list::AuthListViewer;
use crate::platform::style;

const EMPTY: &str = "No workspaces configured\nRun `linear auth login` to add a workspace\n";

struct Row {
    workspace: String,
    is_default: bool,
    outcome: Outcome,
}

/// What to do for one workspace.
enum Check {
    Request(LinearClient),
    /// The key cannot be used; the reason is shown instead of an organization.
    Skip(&'static str),
}

enum Outcome {
    Viewer {
        organization: String,
        name: String,
        email: String,
    },
    Failed(String),
}

pub fn run(ctx: &Ctx) -> Result<()> {
    list(ctx).context("Failed to list workspaces")
}

fn list(ctx: &Ctx) -> Result<()> {
    let store = ctx.credentials()?;
    if store.workspaces().is_empty() {
        return ctx.print(EMPTY);
    }
    let clients = clients(ctx, store)?;
    ctx.report_credential_warnings()?;
    let outcomes = ctx.spin(true, join_all(clients.iter().map(check)));
    let rows: Vec<Row> = store
        .workspaces()
        .iter()
        .zip(outcomes)
        .map(|(workspace, outcome)| Row {
            workspace: workspace.clone(),
            is_default: store.default() == Some(workspace.as_str()),
            outcome,
        })
        .collect();
    ctx.print(render(rows).render_for(ctx))
}

/// A client per stored workspace, or why its key cannot be used.
fn clients(ctx: &Ctx, store: &CredentialStore) -> Result<Vec<Check>> {
    let endpoint = ctx.options().endpoint().value();
    let transport = ctx.config().transport_env.production();
    store
        .workspaces()
        .iter()
        .map(|workspace| {
            let Some(secret) = store.key(workspace) else {
                return Ok(Check::Skip("missing credentials"));
            };
            let Ok(key) = auth::header::to_api_key(secret) else {
                return Ok(Check::Skip("invalid API key"));
            };
            let client = LinearClient::new(endpoint.clone(), key, transport.clone())?;
            Ok(Check::Request(client))
        })
        .collect()
}

async fn check(check: &Check) -> Outcome {
    let client = match check {
        Check::Request(client) => client,
        Check::Skip(reason) => return Outcome::Failed((*reason).to_owned()),
    };
    match client.query::<AuthListViewer, _>(()).await {
        Ok(data) => Outcome::Viewer {
            organization: data.viewer.organization.name,
            name: data.viewer.name,
            email: data.viewer.email,
        },
        Err(failure) => Outcome::Failed(failure_cell(&failure)),
    }
}

fn rejected(status: StatusCode) -> bool {
    status == StatusCode::UNAUTHORIZED || status == StatusCode::FORBIDDEN
}

/// A short cell for a failed check. A 401 or 403 means the key was refused.
fn failure_cell(failure: &RequestError) -> String {
    match failure {
        RequestError::GraphQl { status, .. } | RequestError::ResponseTooLarge { status, .. }
            if rejected(*status) =>
        {
            "invalid credentials".to_owned()
        }
        RequestError::Http { response, .. } if rejected(response.status) => {
            "invalid credentials".to_owned()
        }
        RequestError::GraphQl { errors, .. } => {
            graphql_message(errors).unwrap_or_else(|| failure.to_string())
        }
        RequestError::Http { .. }
        | RequestError::ResponseTooLarge { .. }
        | RequestError::Response(_)
        | RequestError::Timeout { .. }
        | RequestError::Network { .. }
        | RequestError::RequestBody(_) => failure.to_string(),
    }
}

/// One row per workspace; `*` marks the default.
fn render(rows: Vec<Row>) -> Table {
    let mut table = Table::new([
        Column::fixed(""),
        Column::fixed("WORKSPACE"),
        Column::flexible("ORGANIZATION"),
        Column::flexible("USER"),
    ]);
    for row in rows {
        let marker = Cell::from(if row.is_default { "*" } else { "" });
        let (organization, user) = match row.outcome {
            Outcome::Viewer {
                organization,
                name,
                email,
            } => (
                Cell::from(organization),
                Cell::from(format!("{name} <{email}>")),
            ),
            Outcome::Failed(reason) => (Cell::styled(reason, style::red), Cell::from("")),
        };
        table.row([marker, Cell::from(row.workspace), organization, user]);
    }
    table
}
