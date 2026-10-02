//! `auth list`: every stored workspace with the organization and user its
//! key belongs to, checked with one request per key, all at once.
use cynic::QueryBuilder;
use futures_util::future::join_all;
use reqwest::StatusCode;

use crate::auth::{self, CredentialStore};
use crate::commands::display::{display_width, pad};
use crate::ctx::Ctx;
use crate::error::{Result, ResultExt};
use crate::graphql::envelope::{GraphQlRequest, graphql_message};
use crate::graphql::operations::auth_list::AuthListViewer;
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use crate::platform::style;

const EMPTY: &str = "No workspaces configured\nRun `linear auth login` to add a workspace\n";
const WORKSPACE_HEADER: &str = "WORKSPACE";
const ORG_HEADER: &str = "ORG NAME";

struct Row {
    workspace: String,
    is_default: bool,
    outcome: Outcome,
}

/// What to do for one workspace.
enum Check {
    Request(GraphQlTransport),
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
    ctx.print(render(&rows, ctx.color()))
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
            let client = GraphQlTransport::new(endpoint.clone(), key, transport.clone())?;
            Ok(Check::Request(client))
        })
        .collect()
}

async fn check(check: &Check) -> Outcome {
    let client = match check {
        Check::Request(client) => client,
        Check::Skip(reason) => return Outcome::Failed((*reason).to_owned()),
    };
    let request = GraphQlRequest::without_variables(AuthListViewer::build(()));
    match client.execute::<AuthListViewer, _>(&request).await {
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
fn failure_cell(failure: &TransportFailure) -> String {
    match failure {
        TransportFailure::GraphQl { status, .. }
        | TransportFailure::ResponseTooLarge { status, .. }
            if rejected(*status) =>
        {
            "invalid credentials".to_owned()
        }
        TransportFailure::Http { response, .. } if rejected(response.status) => {
            "invalid credentials".to_owned()
        }
        TransportFailure::GraphQl { errors, .. } => {
            graphql_message(errors).unwrap_or_else(|| failure.to_string())
        }
        TransportFailure::Http { .. }
        | TransportFailure::ResponseTooLarge { .. }
        | TransportFailure::Response(_)
        | TransportFailure::Timeout { .. }
        | TransportFailure::Network { .. }
        | TransportFailure::RequestBody(_) => failure.to_string(),
    }
}

fn org_cell(row: &Row) -> &str {
    match &row.outcome {
        Outcome::Viewer { organization, .. } => organization,
        Outcome::Failed(reason) => reason,
    }
}

fn render(rows: &[Row], color: bool) -> String {
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
    let mut output = format!("{}\n", style::underline(&header, color));
    for row in rows {
        let marker = if row.is_default { "* " } else { "  " };
        let workspace = pad(&row.workspace, workspace_width);
        match &row.outcome {
            Outcome::Viewer {
                organization,
                name,
                email,
            } => output.push_str(&format!(
                "{marker}{workspace} {} {name} <{email}>\n",
                pad(organization, org_width)
            )),
            Outcome::Failed(reason) => output.push_str(&format!(
                "{marker}{workspace} {}\n",
                style::red(&pad(reason, org_width), color)
            )),
        }
    }
    output
}
