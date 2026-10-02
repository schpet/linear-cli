//! `milestone update`: direct milestone id, optional project resolution, one write.
use cynic::MutationBuilder;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::milestone_update::{
    ProjectMilestoneUpdateInput, UpdateProjectMilestone, UpdateProjectMilestoneVariables,
    UpdatedMilestone,
};
use crate::graphql::scalars::TimelessDate;
use crate::graphql::transport::GraphQlTransport;

pub const CONTEXT: &str = "Failed to update milestone";

/// Empty strings mean "not given"; a sort order is sent whenever present.
/// `project_id` contains the resolved UUID when building the request.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Options {
    pub name: Option<String>,
    pub description: Option<String>,
    pub target_date: Option<String>,
    pub sort_order: Option<f64>,
    pub project_id: Option<String>,
}

fn truthy(value: &Option<String>) -> Option<String> {
    value.as_ref().filter(|value| !value.is_empty()).cloned()
}

impl Options {
    /// Check before starting the spinner or preparing transport. Typed callers
    /// also cannot send NaN/infinity and accidentally serialize JSON null.
    pub fn require_update(&self) -> Result<(), AppError> {
        if self.sort_order.is_some_and(|value| !value.is_finite()) {
            return Err(AppError::new(
                AppErrorKind::Validation,
                "Sort order must be a finite number",
            ));
        }
        if truthy(&self.name).is_none()
            && truthy(&self.description).is_none()
            && truthy(&self.target_date).is_none()
            && self.sort_order.is_none()
            && truthy(&self.project_id).is_none()
        {
            return Err(AppError::new(
                AppErrorKind::Validation,
                "At least one update option must be provided",
            )
            .with_suggestion(
                "Use --name, --description, --target-date, --sort-order, or --project",
            ));
        }
        Ok(())
    }
}

/// No date validation or milestone lookup. Normalize signed zero like
/// JSON.stringify; absent and empty strings never clear existing values.
pub fn request(
    id: &str,
    options: &Options,
) -> Result<GraphQlRequest<UpdateProjectMilestoneVariables>, AppError> {
    options.require_update()?;
    Ok(GraphQlRequest::with_variables(
        UpdateProjectMilestone::build(UpdateProjectMilestoneVariables {
            id: id.to_owned(),
            input: ProjectMilestoneUpdateInput {
                name: truthy(&options.name),
                description: truthy(&options.description),
                target_date: truthy(&options.target_date).map(TimelessDate),
                sort_order: options
                    .sort_order
                    .map(|value| if value == 0.0 { 0.0 } else { value }),
                project_id: truthy(&options.project_id),
            },
        }),
    ))
}

/// Sends the update once and reports server and network errors as they are.
pub async fn submit(
    transport: &GraphQlTransport,
    id: &str,
    options: &Options,
) -> Result<Vec<u8>, AppError> {
    let result: UpdateProjectMilestone = transport.execute(&request(id, options)?).await?;
    let payload = result.project_milestone_update;
    if !payload.success {
        return Err(AppError::new(AppErrorKind::GraphQl, CONTEXT));
    }
    render(&payload.project_milestone)
}

pub fn render(milestone: &UpdatedMilestone) -> Result<Vec<u8>, AppError> {
    let mut output = format!(
        "✓ Updated milestone: {}\n  ID: {}\n",
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
    output.push_str(&format!(
        "  Sort Order: {}\n  Project: {}\n",
        milestone.sort_order, milestone.project.name
    ));
    Ok(output.into_bytes())
}
