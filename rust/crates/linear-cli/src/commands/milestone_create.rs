//! `milestone create`: one typed mutation after shared project resolution.
use cynic::MutationBuilder;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::milestone_create::{
    CreateProjectMilestone, CreateProjectMilestoneVariables, CreatedMilestone,
    ProjectMilestoneCreateInput,
};
use crate::graphql::scalars::TimelessDate;
use crate::graphql::transport::{GraphQlTransport, NetworkPhase, TransportFailure};

/// The source's single `handleError` prefix for every action failure.
pub const CONTEXT: &str = "Failed to create milestone";

/// Parsed flag values. The parser rejects empty values, but the request
/// builder does not rely on that: every supplied value is sent verbatim.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Options {
    pub name: String,
    pub description: Option<String>,
    pub target_date: Option<String>,
}

/// The create mutation for an already resolved project. The target date is
/// not validated locally; Linear owns `TimelessDate` parsing.
pub fn request(
    project_id: &str,
    options: &Options,
) -> GraphQlRequest<CreateProjectMilestoneVariables> {
    GraphQlRequest::with_variables(CreateProjectMilestone::build(
        CreateProjectMilestoneVariables {
            input: ProjectMilestoneCreateInput {
                project_id: project_id.to_owned(),
                name: options.name.clone(),
                description: options.description.clone(),
                target_date: options.target_date.clone().map(TimelessDate),
            },
        },
    ))
}

/// Sends the mutation once and renders the created milestone. Failures after
/// the request may have reached Linear say the milestone may already exist,
/// so a user checks before creating it again; nothing is retried.
pub async fn submit(
    transport: &GraphQlTransport,
    project_id: &str,
    options: &Options,
) -> Result<Vec<u8>, AppError> {
    let result: CreateProjectMilestone = transport
        .execute(&request(project_id, options))
        .await
        .map_err(|failure| {
            let uncertain = outcome_unknown(&failure);
            let mut error = AppError::from(failure);
            if uncertain {
                error.message.push_str("; milestone may already exist");
            }
            error
        })?;
    let payload = result.project_milestone_create;
    if !payload.success {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            "Failed to create milestone",
        ));
    }
    Ok(render(&payload.project_milestone))
}

/// Only a failed connection proves nothing was sent. A timeout, any later
/// network failure (a reset after the request was written surfaces as a
/// request-phase error), or an undecodable success response leaves the
/// create's outcome unknown; errors Linear reported do not.
pub(crate) fn outcome_unknown(failure: &TransportFailure) -> bool {
    match failure {
        TransportFailure::Timeout { .. } | TransportFailure::Response(_) => true,
        TransportFailure::Network { phase, .. } => !matches!(phase, NetworkPhase::Connect),
        TransportFailure::ResponseTooLarge { status, .. } => status.is_success(),
        TransportFailure::RequestBody(_)
        | TransportFailure::GraphQl { .. }
        | TransportFailure::Http { .. } => false,
    }
}

/// The source's `console.log` lines; an empty target date is skipped like
/// a null one.
pub fn render(milestone: &CreatedMilestone) -> Vec<u8> {
    let mut output = format!(
        "✓ Created milestone: {}\n  ID: {}\n",
        milestone.name,
        milestone.id.inner()
    );
    if let Some(date) = milestone
        .target_date
        .as_ref()
        .filter(|date| !date.0.is_empty())
    {
        output.push_str(&format!("  Target Date: {}\n", date.0));
    }
    output.push_str(&format!("  Project: {}\n", milestone.project.name));
    output.into_bytes()
}
