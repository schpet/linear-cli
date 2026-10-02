//! Input handling shared by `project create` and `project update`.
use crate::{
    commands::{project_collections::ResolvedRef, text_input},
    error::{AppError, AppErrorKind},
    graphql::{
        envelope::GraphQlRequest,
        operations::{
            initiatives::{GetViewerId, GetViewerIdVariables, LookupUser, LookupUserVariables},
            project_write::*,
            templates::{GetTemplate, GetTemplateVariables, GetTemplates, Template},
        },
        transport::{GraphQlTransport, TransportFailure},
    },
    refs::{self, WorkspaceScope},
};
use cynic::QueryBuilder;
use futures_util::future::try_join_all;

pub fn validation(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Validation, message)
}
pub fn content(inline: Option<&str>, file: Option<&str>) -> Result<Option<String>, AppError> {
    if inline.is_some() && file.is_some() {
        return Err(validation(
            "Cannot specify both --content and --content-file",
        ));
    }
    match file {
        None => Ok(inline.map(str::to_owned)),
        Some(path) => crate::commands::text_input::read_file(path)
            .map(Some)
            .map_err(|error| {
                validation(format!("Failed to read content file: {path}"))
                    .with_suggestion(format!("Error: {error}"))
                    .with_source(error)
            }),
    }
}
pub fn description(inline: Option<&str>, file: Option<&str>) -> Result<Option<String>, AppError> {
    if inline.is_some() && file.is_some() {
        return Err(
            validation("Cannot use --description and --description-file together")
                .with_suggestion("Pass only one of --description or --description-file."),
        );
    }
    let value = match file {
        None => inline.map(str::to_owned),
        Some(path) => Some(match text_input::read_file(path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(AppError::not_found("File", path));
            }
            Err(error) => {
                return Err(AppError::new(
                    AppErrorKind::IoProcess,
                    format!("Failed to read description file: {error}"),
                )
                .with_source(error));
            }
        }),
    };
    if let Some(value) = &value {
        // Linear measures this limit in UTF-16 code units.
        let len = value.encode_utf16().count();
        if len > 255 {
            return Err(validation(format!("Project description is {len} characters, exceeds the 255-character limit enforced by Linear's API")).with_suggestion("Shorten the description to 255 characters or fewer, or move the long content into an attached document via `linear document create --project <slug>`."));
        }
    }
    Ok(value)
}
pub fn truthy(value: Option<&str>) -> Option<&str> {
    value.filter(|v| !v.is_empty())
}
pub fn priority(value: &str) -> Result<i32, AppError> {
    match value.to_lowercase().as_str() {
        "none" => Ok(0),
        "urgent" => Ok(1),
        "high" => Ok(2),
        "medium" => Ok(3),
        "low" => Ok(4),
        _ => Err(validation(format!("Invalid priority: {value}"))
            .with_suggestion("Valid values: none, urgent, high, medium, low")),
    }
}
pub fn status_type(value: &str) -> Result<&'static str, AppError> {
    match value.to_lowercase().as_str() {
        "planned" => Ok("planned"),
        "in progress" | "started" => Ok("started"),
        "paused" => Ok("paused"),
        "completed" => Ok("completed"),
        "canceled" => Ok("canceled"),
        "backlog" => Ok("backlog"),
        _ => Err(
            validation(format!("Invalid status: {value}")).with_suggestion(
                "Valid values: planned, started, paused, completed, canceled, backlog",
            ),
        ),
    }
}
pub fn date(value: Option<&str>, noun: &str) -> Result<(), AppError> {
    if let Some(value) = truthy(value) {
        let bytes = value.as_bytes();
        if bytes.len() != 10
            || bytes.get(4) != Some(&b'-')
            || bytes.get(7) != Some(&b'-')
            || !bytes
                .iter()
                .enumerate()
                .all(|(i, b)| i == 4 || i == 7 || b.is_ascii_digit())
        {
            return Err(validation(format!(
                "{noun} date must be in YYYY-MM-DD format"
            )));
        }
    }
    Ok(())
}
pub async fn statuses(transport: &GraphQlTransport) -> Result<Vec<ProjectStatus>, AppError> {
    let query = GraphQlRequest::without_variables(GetProjectStatuses::build(()));
    let data: GetProjectStatuses = transport.execute(&query).await.map_err(AppError::from)?;
    Ok(data.project_statuses.nodes)
}
pub async fn status(transport: &GraphQlTransport, value: &str) -> Result<String, AppError> {
    let kind = status_type(value)?;
    statuses(transport)
        .await?
        .into_iter()
        .find(|s| s.status_type.as_str() == kind)
        .map(|s| s.id.into_inner())
        .ok_or_else(|| AppError::not_found("Project status", kind))
}
pub async fn user(
    transport: &GraphQlTransport,
    value: &str,
    noun: &str,
) -> Result<String, AppError> {
    refs::reject_linear_url(value, "an email, username, display name, or @me")?;
    let id = if value == "self" || value == "@me" {
        let query = GraphQlRequest::with_variables(GetViewerId::build(GetViewerIdVariables {}));
        let data: GetViewerId = transport.execute(&query).await.map_err(AppError::from)?;
        Some(data.viewer.id.into_inner())
    } else {
        let query = GraphQlRequest::with_variables(LookupUser::build(LookupUserVariables {
            input: value.to_owned(),
        }));
        let data: LookupUser = transport.execute(&query).await.map_err(AppError::from)?;
        let wanted = value.to_lowercase();
        data.users
            .nodes
            .iter()
            .find(|u| u.email.to_lowercase() == wanted)
            .or_else(|| {
                data.users
                    .nodes
                    .iter()
                    .find(|u| u.display_name.to_lowercase() == wanted)
            })
            .or_else(|| data.users.nodes.first())
            .map(|u| u.id.clone().into_inner())
    };
    id.filter(|id| !id.is_empty())
        .ok_or_else(|| AppError::not_found(noun, value))
}
pub async fn label(transport: &GraphQlTransport, value: &str) -> Result<String, AppError> {
    refs::reject_linear_url(value, "a project label name")?;
    let query = GraphQlRequest::with_variables(GetProjectLabelIdByName::build(NameVariables {
        name: value.to_owned(),
    }));
    let data: GetProjectLabelIdByName = transport.execute(&query).await.map_err(AppError::from)?;
    data.project_labels
        .nodes
        .into_iter()
        .next()
        .map(|v| v.id.into_inner())
        .filter(|id| !id.is_empty())
        .ok_or_else(|| AppError::not_found("Project label", value))
}
pub async fn labels(
    transport: &GraphQlTransport,
    values: &[String],
) -> Result<Vec<ResolvedRef>, AppError> {
    let mut result = Vec::new();
    for value in values {
        let id = label(transport, value).await?;
        if !result.iter().any(|r: &ResolvedRef| r.id == id) {
            result.push(ResolvedRef {
                id,
                label: value.clone(),
            });
        }
    }
    Ok(result)
}
pub async fn teams(
    transport: &GraphQlTransport,
    scope: &WorkspaceScope<'_>,
    values: &[String],
) -> Result<Vec<crate::refs::ResolvedTeam>, AppError> {
    let resolved = try_join_all(values.iter().map(|value| async move {
        let prepared = refs::prepare_team_lookup(value, scope)?;
        refs::resolve_team_with_transport(&prepared, transport).await
    }))
    .await?;
    let mut result = Vec::new();
    for team in resolved {
        if !result
            .iter()
            .any(|kept: &crate::refs::ResolvedTeam| kept.id == team.id)
        {
            result.push(team);
        }
    }
    Ok(result)
}
pub async fn initiatives(
    transport: &GraphQlTransport,
    scope: &WorkspaceScope<'_>,
    values: &[String],
) -> Result<Vec<ResolvedRef>, AppError> {
    let mut result = Vec::new();
    for value in values {
        let reference = if refs::is_linear_uuid(value) {
            let query = GraphQlRequest::with_variables(GetInitiativeByIdForUpdate::build(
                InitiativeIdVariables {
                    id: cynic::Id::new(value),
                },
            ));
            let data: GetInitiativeByIdForUpdate =
                transport.execute(&query).await.map_err(AppError::from)?;
            let found = data.initiatives.nodes.into_iter().next().ok_or_else(|| {
                AppError::not_found("Initiative", value)
                    .with_suggestion("Pass an initiative UUID, slug ID, or exact initiative name.")
            })?;
            ResolvedRef {
                id: found.id.into_inner(),
                label: found.name,
            }
        } else {
            let prepared = refs::prepare_initiative_lookup(value, scope)?;
            ResolvedRef {
                id: refs::resolve_initiative_with_transport(&prepared, value, transport).await?,
                label: value.clone(),
            }
        };
        if !result.iter().any(|r: &ResolvedRef| r.id == reference.id) {
            result.push(reference);
        }
    }
    Ok(result)
}
fn template_available(template: &Template, team_ids: &[String]) -> bool {
    template
        .team
        .as_ref()
        .is_none_or(|team| team_ids.iter().any(|id| id == team.id.inner()))
}
fn wrong_type(template: &Template) -> AppError {
    let article = if template
        .template_type
        .chars()
        .next()
        .is_some_and(|c| "aeiouAEIOU".contains(c))
    {
        "an"
    } else {
        "a"
    };
    validation(format!(
        "Template \"{}\" is {article} {} template, not a project template",
        template.name, template.template_type
    ))
    .with_suggestion("Run `linear template list --type project` to see the project templates.")
}
fn wrong_team(name: &str, keys: &[String]) -> AppError {
    let Some(first_key) = keys.first() else {
        return AppError::new(
            AppErrorKind::Invariant,
            "unavailable template has no team key",
        );
    };
    validation(format!("Template \"{name}\" belongs to team{} {} and cannot be applied here", if keys.len() == 1 { "" } else { "s" }, keys.join(", ")))
        .with_suggestion(format!("Pass --team {first_key}, or pick a workspace template or one from the target team with `linear template list --type project --team <team>`."))
}
pub fn select_template(
    reference: &str,
    all: Vec<Template>,
    team_ids: &[String],
) -> Result<Template, AppError> {
    let wanted = reference.to_lowercase();
    let by_name: Vec<_> = all
        .iter()
        .filter(|t| t.name.to_lowercase() == wanted)
        .collect();
    let eligible = |t: &Template| t.template_type == "project" && template_available(t, team_ids);
    let selected: Vec<_> = by_name.iter().copied().filter(|t| eligible(t)).collect();
    if selected.len() > 1 {
        return Err(validation(format!(
            "Template name \"{reference}\" is ambiguous: it matches {} templates",
            selected.len()
        ))
        .with_suggestion(format!(
            "Pass the template ID instead: {}",
            selected
                .iter()
                .map(|t| format!(
                    "{} ({}, {})",
                    t.id.inner(),
                    t.template_type,
                    t.team.as_ref().map_or("Workspace", |v| v.key.as_str())
                ))
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    if let Some(selected) = selected.first() {
        return Ok((*selected).clone());
    }
    if let Some(first_name) = by_name.first() {
        let same_type: Vec<_> = by_name
            .iter()
            .copied()
            .filter(|t| t.template_type == "project")
            .collect();
        let mut keys = Vec::new();
        for t in &same_type {
            if let Some(team) = &t.team
                && !keys.contains(&team.key)
            {
                keys.push(team.key.clone());
            }
        }
        if !keys.is_empty() {
            let first = same_type.first().ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "template team keys have no matching project template",
                )
            })?;
            return Err(wrong_team(&first.name, &keys));
        }
        return Err(wrong_type(first_name));
    }
    let mut names = Vec::new();
    for t in all.iter().filter(|t| eligible(t)) {
        if !names.contains(&t.name) {
            names.push(t.name.clone());
        }
    }
    names.sort_by(|a, b| crate::platform::collation::compare(a, b));
    let suggestion = if names.is_empty() {
        "No project templates are available here. Run `linear template list` to see every template."
            .to_owned()
    } else {
        format!(
            "Available project templates: {}. Run `linear template list` to see every template.",
            names
                .iter()
                .map(|name| format!("\"{name}\""))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    Err(AppError::not_found("Template", reference).with_suggestion(suggestion))
}
pub async fn template(
    transport: &GraphQlTransport,
    reference: &str,
    team_ids: &[String],
) -> Result<String, AppError> {
    refs::reject_linear_url(reference, "a template name or UUID")?;
    let t = if refs::is_linear_uuid(reference) {
        let query = GraphQlRequest::with_variables(GetTemplate::build(GetTemplateVariables {
            id: reference.to_owned(),
        }));
        let data: GetTemplate = match transport.execute(&query).await {
            Ok(data) => data,
            Err(TransportFailure::GraphQl { errors, .. })
                if errors
                    .iter()
                    .any(|e| e.message.to_lowercase().contains("no template found")) =>
            {
                return Err(AppError::not_found("Template", reference)
                    .with_suggestion("Run `linear template list` to see every template."));
            }
            Err(error) => return Err(AppError::from(error)),
        };
        super::issue_template_scope::assert_scope(
            &data.template,
            team_ids,
            super::issue_template_scope::TemplateScope::Project,
        )?;
        data.template
    } else {
        let query = GraphQlRequest::without_variables(GetTemplates::build(()));
        let data: GetTemplates = transport.execute(&query).await.map_err(AppError::from)?;
        select_template(reference, data.templates, team_ids)?
    };
    Ok(t.id.into_inner())
}
