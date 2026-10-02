//! `project create`: fields from flags or prompts, then one mutation.
use crate::{
    commands::project::write as shared,
    error::Error,
    graphql::{
        bulk_error,
        envelope::GraphQlRequest,
        operations::project_write::*,
        transport::{GraphQlTransport, classify_typed},
    },
    platform::{
        prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession},
        prompt_text::TextOptions,
    },
    refs::{self, InitiativeReference, WorkspaceScope},
};
use cynic::{MutationBuilder, QueryBuilder};
use std::io::{Read, Write};

#[derive(Clone, Debug, Default)]
pub struct Fields {
    pub name: Option<String>,
    pub description: Option<String>,
    pub description_file: Option<String>,
    pub content: Option<String>,
    pub teams: Vec<String>,
    pub lead: Option<String>,
    pub status: Option<String>,
    pub start_date: Option<String>,
    pub target_date: Option<String>,
    pub priority: Option<i32>,
    pub labels: Vec<String>,
    pub members: Vec<String>,
    pub icon: Option<String>,
    pub color: Option<String>,
    pub initiative: Option<String>,
    pub template: Option<String>,
}
pub fn local(action: &crate::cli::project::ProjectCreate) -> Result<Fields, Error> {
    let content = shared::content(action.content.as_deref(), action.content_file.as_deref())?;
    let priority = action
        .priority
        .as_deref()
        .map(shared::priority)
        .transpose()?;
    Ok(Fields {
        name: action.name.clone(),
        description: action.description.clone(),
        description_file: action.description_file.clone(),
        content,
        teams: action.team.clone(),
        lead: action.lead.clone(),
        status: action.status.clone(),
        start_date: action.start_date.clone(),
        target_date: action.target_date.clone(),
        priority,
        labels: action.label.clone(),
        members: action.member.clone(),
        icon: action.icon.clone(),
        color: action.color.clone(),
        initiative: action.initiative.clone(),
        template: action.template.clone(),
    })
}
pub fn interactive(fields: &Fields, explicit: bool, stdout_tty: bool) -> bool {
    stdout_tty
        && (explicit
            || (shared::truthy(fields.name.as_deref()).is_none() && fields.teams.is_empty()))
}
pub async fn prompt<R: Read, W: Write>(
    session: &mut PromptSession<R, W>,
    transport: &GraphQlTransport,
    mut fields: Fields,
    default_team: Option<&str>,
) -> Result<PromptOutcome<Fields>, Error> {
    macro_rules! answer {
        ($call:expr) => {
            match $call? {
                PromptOutcome::Submitted(value) => value,
                PromptOutcome::Interrupted => return Ok(PromptOutcome::Interrupted),
                PromptOutcome::EndOfInput => return Ok(PromptOutcome::EndOfInput),
            }
        };
    }
    let text = |required| TextOptions {
        required,
        default: None,
    };
    if shared::truthy(fields.name.as_deref()).is_none() {
        fields.name = Some(answer!(
            session.text_with_options("Project name:", text(true))
        ));
    }
    if shared::truthy(fields.description.as_deref()).is_none() && fields.description_file.is_none()
    {
        let value = answer!(session.text_with_options("Description (optional):", text(false)));
        fields.description = (!value.is_empty()).then_some(value);
    }
    if fields.teams.is_empty() {
        session.suspend()?;
        let teams = refs::fetch_all_teams_with_transport(transport).await?;
        let options: Vec<_> = teams
            .into_iter()
            .map(|t| PlainOption {
                label: format!("{} ({})", t.name, t.key),
                value: t.key.clone(),
                script_token: t.key,
            })
            .collect();
        let default_index = default_team
            .and_then(|key| options.iter().position(|o| o.value == key))
            .unwrap_or(0);
        session.resume()?;
        fields.teams = vec![answer!(session.select(&PlainSelect {
            message: "Team:",
            options: &options,
            default_index,
            default_hint: None
        }))];
    }
    if shared::truthy(fields.status.as_deref()).is_none() {
        session.suspend()?;
        let statuses = shared::statuses(transport).await?;
        if !statuses.is_empty() {
            // Different named statuses can share a legal type. Keep every row,
            // give the menu unique tokens, then return the selected type so the
            // normal input phase still refetches its first matching status ID.
            let options: Vec<_> = statuses
                .iter()
                .enumerate()
                .map(|(index, status)| {
                    let token = format!("project-status-{index}");
                    PlainOption {
                        label: status.name.clone(),
                        value: token.clone(),
                        script_token: token,
                    }
                })
                .collect();
            let default_index = statuses
                .iter()
                .position(|status| status.status_type.as_str() == "planned")
                .unwrap_or(0);
            session.resume()?;
            let selected = answer!(session.select(&PlainSelect {
                message: "Status:",
                options: &options,
                default_index,
                default_hint: None
            }));
            let status = statuses
                .iter()
                .zip(&options)
                .find_map(|(status, option)| (option.value == selected).then_some(status))
                .ok_or_else(|| Error::new("selected project status token is absent from menu"))?;
            fields.status = Some(status.status_type.as_str().to_owned());
        } else {
            session.resume()?;
        }
    }
    for (field, message) in [
        (
            &mut fields.lead,
            "Lead (username, email, or @me - press Enter to skip):",
        ),
        (
            &mut fields.start_date,
            "Start date (YYYY-MM-DD - press Enter to skip):",
        ),
        (
            &mut fields.target_date,
            "Target date (YYYY-MM-DD - press Enter to skip):",
        ),
    ] {
        if shared::truthy(field.as_deref()).is_none() {
            let value = answer!(session.text_with_options(message, text(false)));
            *field = (!value.is_empty()).then_some(value);
        }
    }
    Ok(PromptOutcome::Submitted(fields))
}
pub async fn input(
    transport: &GraphQlTransport,
    scope: &WorkspaceScope<'_>,
    fields: &Fields,
    default_team: Option<&str>,
) -> Result<ProjectCreateInput, Error> {
    let description = shared::description(
        fields.description.as_deref(),
        fields.description_file.as_deref(),
    )?;
    let name = shared::truthy(fields.name.as_deref()).ok_or_else(|| {
        shared::validation("Project name is required")
            .with_hint("Use --name or -n flag to specify a project name.")
    })?;
    let teams = if fields.teams.is_empty() {
        vec![
            default_team
                .filter(|v| !v.is_empty())
                .ok_or_else(|| {
                    shared::validation("At least one team is required")
                        .with_hint("Use --team or -t flag to specify a team.")
                })?
                .to_owned(),
        ]
    } else {
        fields.teams.clone()
    };
    let team_ids: Vec<_> = shared::teams(transport, scope, &teams)
        .await?
        .into_iter()
        .map(|t| t.id)
        .collect();
    let template_id = match &fields.template {
        Some(value) => Some(shared::template(transport, value, &team_ids).await?),
        None => None,
    };
    let lead_id = match shared::truthy(fields.lead.as_deref()) {
        Some(value) => Some(shared::user(transport, value, "Lead").await?),
        None => None,
    };
    let status_id = match shared::truthy(fields.status.as_deref()) {
        Some(value) => Some(shared::status(transport, value).await?),
        None => None,
    };
    let mut label_ids = Vec::new();
    for value in &fields.labels {
        label_ids.push(shared::label(transport, value).await?);
    }
    let mut member_ids = Vec::new();
    for value in &fields.members {
        member_ids.push(shared::user(transport, value, "User").await?);
    }
    shared::date(fields.start_date.as_deref(), "Start")?;
    shared::date(fields.target_date.as_deref(), "Target")?;
    Ok(ProjectCreateInput {
        name: name.to_owned(),
        team_ids,
        description,
        content: fields.content.clone(),
        lead_id: lead_id.filter(|v| !v.is_empty()),
        status_id: status_id.filter(|v| !v.is_empty()),
        start_date: shared::truthy(fields.start_date.as_deref())
            .map(|v| crate::graphql::scalars::TimelessDate(v.to_owned())),
        target_date: shared::truthy(fields.target_date.as_deref())
            .map(|v| crate::graphql::scalars::TimelessDate(v.to_owned())),
        priority: fields.priority,
        label_ids: (!label_ids.is_empty()).then_some(label_ids),
        member_ids: (!member_ids.is_empty()).then_some(member_ids),
        icon: fields.icon.clone(),
        color: fields.color.clone(),
        template_id,
    })
}
pub async fn submit(
    transport: &GraphQlTransport,
    input: ProjectCreateInput,
) -> Result<CreatedProjectPayload, Error> {
    let query =
        GraphQlRequest::with_variables(CreateProject::build(CreateProjectVariables { input }));
    let result: CreateProject = transport.execute(&query).await.map_err(Error::from)?;
    if !result.project_create.success {
        return Err(Error::new("Failed to create project"));
    }
    if result.project_create.project.is_none() {
        return Err(Error::new("Failed to create project: no project returned"));
    }
    Ok(result.project_create)
}
pub async fn initiative_for_create(
    transport: &GraphQlTransport,
    scope: &WorkspaceScope<'_>,
    value: &str,
) -> Result<Option<String>, Error> {
    let prepared = refs::prepare_initiative_lookup(value, scope)?;
    match prepared {
        InitiativeReference::Id(id) => return Ok(Some(id)),
        InitiativeReference::UrlSlug(slug) => {
            let query = GraphQlRequest::with_variables(
                crate::graphql::operations::initiative_reference::ResolveInitiativeBySlug::build(
                    crate::graphql::operations::initiative_reference::UrlSlugVariables {
                        slug_id: slug,
                        include_archived: Some(false),
                    },
                ),
            );
            let result: crate::graphql::operations::initiative_reference::ResolveInitiativeBySlug =
                transport.execute(&query).await.map_err(Error::from)?;
            return Ok(result
                .initiatives
                .nodes
                .into_iter()
                .next()
                .map(|i| i.id.into_inner()));
        }
        InitiativeReference::NameOrSlug(_) => (),
    }
    let slug = GraphQlRequest::with_variables(GetInitiativeBySlugForCreate::build(SlugVariables {
        slug_id: value.to_owned(),
    }));
    let result: Result<GetInitiativeBySlugForCreate, _> = transport.execute(&slug).await;
    if let Ok(data) = result
        && let Some(found) = data.initiatives.nodes.into_iter().next()
    {
        return Ok(Some(found.id.into_inner()));
    }
    let name = GraphQlRequest::with_variables(GetInitiativeByNameForCreate::build(NameVariables {
        name: value.to_owned(),
    }));
    let result: Result<GetInitiativeByNameForCreate, _> = transport.execute(&name).await;
    Ok(match result {
        Ok(data) => data
            .initiatives
            .nodes
            .into_iter()
            .next()
            .map(|i| i.id.into_inner()),
        Err(_) => None,
    })
}
#[derive(Debug, PartialEq, Eq)]
pub enum JoinOutcome {
    Added,
    Rejected,
    Warning(String),
}
pub fn warning(message: &str, client_error: bool) -> String {
    format!(
        "\nWarning: Failed to add project to initiative: {}{message}\n",
        if client_error { "ClientError: " } else { "" }
    )
}
/// Preserve captured status/MIME so the raw observer's Some classes aren't conflated.
pub fn client_error_branch(response: &crate::graphql::transport::RawHttpResponse) -> bool {
    if !response.status.is_success() {
        return true;
    }
    crate::graphql::source_response::has_json_mime(&response.headers)
}
pub async fn join(transport: &GraphQlTransport, input: InitiativeLinkInput) -> JoinOutcome {
    let request =
        GraphQlRequest::with_variables(AddProjectToInitiativeForCreate::build(LinkVariables {
            input,
        }));
    let response = match transport.send_request(&request).await {
        Ok(response) => response,
        Err(error) => {
            return JoinOutcome::Warning(warning(&Error::from(error).to_string(), false));
        }
    };
    let client_class = client_error_branch(&response);
    let message = match bulk_error::source_error(&response, &request) {
        Ok(message) => message,
        Err(error) => return JoinOutcome::Warning(warning(&error.into_error().to_string(), false)),
    };
    if let Some(message) = message {
        return JoinOutcome::Warning(warning(&message, client_class));
    }
    let data: Result<AddProjectToInitiativeForCreate, _> = classify_typed(response);
    match data {
        Ok(data) if data.initiative_to_project_create.success => JoinOutcome::Added,
        Ok(_) => JoinOutcome::Rejected,
        Err(error) => JoinOutcome::Warning(warning(&Error::from(error).to_string(), false)),
    }
}
#[derive(Debug, Default)]
pub struct Output {
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
}
pub async fn followup_and_output(
    transport: &GraphQlTransport,
    scope: &WorkspaceScope<'_>,
    payload: &CreatedProjectPayload,
    initiative: Option<&str>,
    json: bool,
) -> Result<Output, Error> {
    let project = payload
        .project
        .as_ref()
        .ok_or_else(|| Error::new("project creation output has no project"))?;
    let mut output = Output::default();
    if let Some(value) = shared::truthy(initiative) {
        match initiative_for_create(transport, scope, value).await? {
            None => output.stderr.extend_from_slice(
                format!("\nWarning: Initiative not found: {value}\nProject was created but not added to initiative.\n").as_bytes(),
            ),
            Some(id) => match join(transport, InitiativeLinkInput {
                initiative_id: id,
                project_id: project.id.clone().into_inner(),
            }).await {
                JoinOutcome::Added if !json => output.stdout.extend_from_slice(
                    format!("✓ Added to initiative: {value}\n").as_bytes(),
                ),
                JoinOutcome::Added => (),
                JoinOutcome::Rejected => output.stderr.extend_from_slice(
                    b"\nWarning: Failed to add project to initiative\n",
                ),
                JoinOutcome::Warning(text) => output.stderr.extend_from_slice(text.as_bytes()),
            },
        }
    }
    if json {
        let mut bytes = serde_json::to_vec_pretty(payload).map_err(|error| {
            Error::new("could not serialize project creation").with_source(error)
        })?;
        bytes.push(b'\n');
        output.stdout.extend(bytes);
    } else {
        output.stdout.extend_from_slice(
            format!(
                "✓ Created project: {}\n  Slug: {}\n",
                project.name, project.slug_id
            )
            .as_bytes(),
        );
        if !project.url.is_empty() {
            output
                .stdout
                .extend_from_slice(format!("  URL: {}\n", project.url).as_bytes());
        }
    }
    Ok(output)
}
