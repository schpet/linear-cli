//! `project update`: resolve every reference first, then apply the writes in order.
use crate::{
    commands::{
        project_collections::{self, FailedWrite, InitiativeChange, InitiativeLink, ResolvedRef},
        project_write as shared,
    },
    error::{AppError, AppErrorKind},
    graphql::{
        edit::Edit,
        envelope::GraphQlRequest,
        operations::{project_write::*, teams::PageInfo},
        transport::GraphQlTransport,
    },
    refs::{self, WorkspaceScope},
};
use cynic::{MutationBuilder, QueryBuilder};
use std::collections::HashSet;
#[derive(Clone, Debug, Default)]
pub struct Options {
    pub name: Option<String>,
    pub description: Option<String>,
    pub description_file: Option<String>,
    pub content: Option<String>,
    pub content_file: Option<String>,
    pub status: Option<String>,
    pub lead: Option<String>,
    pub clear_lead: bool,
    pub start_date: Option<String>,
    pub clear_start_date: bool,
    pub target_date: Option<String>,
    pub clear_target_date: bool,
    pub teams: Option<Vec<String>>,
    pub add_team: Option<Vec<String>>,
    pub remove_team: Option<Vec<String>>,
    pub labels: Option<Vec<String>>,
    pub add_label: Option<Vec<String>>,
    pub remove_label: Option<Vec<String>>,
    pub initiatives: Option<Vec<String>>,
    pub add_initiative: Option<Vec<String>>,
    pub remove_initiative: Option<Vec<String>>,
}
impl Options {
    pub fn from_cli(action: &crate::cli::project::ProjectUpdate) -> Self {
        let list = |values: &Vec<String>| (!values.is_empty()).then(|| values.clone());
        Self {
            name: action.name.clone(),
            description: action.description.clone(),
            description_file: action.description_file.clone(),
            content: action.content.clone(),
            content_file: action.content_file.clone(),
            status: action.status.clone(),
            lead: action.lead.clone(),
            clear_lead: action.clear_lead,
            start_date: action.start_date.clone(),
            clear_start_date: action.clear_start_date,
            target_date: action.target_date.clone(),
            clear_target_date: action.clear_target_date,
            teams: list(&action.team),
            add_team: list(&action.add_team),
            remove_team: list(&action.remove_team),
            labels: list(&action.label),
            add_label: list(&action.add_label),
            remove_label: list(&action.remove_label),
            initiatives: list(&action.initiative),
            add_initiative: list(&action.add_initiative),
            remove_initiative: list(&action.remove_initiative),
        }
    }
}
pub fn replace_conflict(
    kind: &str,
    replace: bool,
    add: bool,
    remove: bool,
) -> Result<(), AppError> {
    if replace && (add || remove) {
        return Err(shared::validation(format!("Cannot combine --{kind} with --add-{kind} or --remove-{kind}")).with_suggestion(format!("--{kind} replaces the project's entire {kind} set. Use it alone to set the exact set, or use --add-{kind}/--remove-{kind} alone to change it incrementally.")));
    }
    Ok(())
}
pub fn overlap(kind: &str, add: &[ResolvedRef], remove: &[ResolvedRef]) -> Result<(), AppError> {
    if project_collections::has_add_remove_overlap(add, remove) {
        return Err(shared::validation(format!(
            "Cannot add and remove the same {kind} in one update"
        ))
        .with_suggestion(format!(
            "Remove the duplicate {kind} from either --add-{kind} or --remove-{kind}."
        )));
    }
    Ok(())
}
pub fn local(options: &Options) -> Result<ProjectUpdateInput, AppError> {
    let has_option = shared::truthy(options.name.as_deref()).is_some()
        || options.description.is_some()
        || options.description_file.is_some()
        || options.content.is_some()
        || options.content_file.is_some()
        || shared::truthy(options.status.as_deref()).is_some()
        || shared::truthy(options.lead.as_deref()).is_some()
        || options.clear_lead
        || shared::truthy(options.start_date.as_deref()).is_some()
        || options.clear_start_date
        || shared::truthy(options.target_date.as_deref()).is_some()
        || options.clear_target_date
        || options.teams.is_some()
        || options.add_team.is_some()
        || options.remove_team.is_some()
        || options.labels.is_some()
        || options.add_label.is_some()
        || options.remove_label.is_some()
        || options.initiatives.is_some()
        || options.add_initiative.is_some()
        || options.remove_initiative.is_some();
    if !has_option {
        return Err(shared::validation("At least one update option must be provided").with_suggestion("Use --name, --description, --description-file, --content, --content-file, --status, --lead, --clear-lead, --start-date, --clear-start-date, --target-date, --clear-target-date, --team, --add-team, --remove-team, --label, --add-label, --remove-label, --initiative, --add-initiative, or --remove-initiative"));
    }
    for (name, clear, value, noun) in [
        ("lead", options.clear_lead, options.lead.as_ref(), "user"),
        (
            "start-date",
            options.clear_start_date,
            options.start_date.as_ref(),
            "date",
        ),
        (
            "target-date",
            options.clear_target_date,
            options.target_date.as_ref(),
            "date",
        ),
    ] {
        if clear && value.is_some() {
            return Err(shared::validation(format!(
                "Cannot specify both --{name} and --clear-{name}"
            ))
            .with_suggestion(format!(
                "Use --{name} <{noun}> to set a {}, or --clear-{name} on its own to remove it.",
                if name == "lead" {
                    "lead"
                } else if name == "start-date" {
                    "start date"
                } else {
                    "target date"
                }
            )));
        }
    }
    replace_conflict(
        "team",
        options.teams.is_some(),
        options.add_team.is_some(),
        options.remove_team.is_some(),
    )?;
    replace_conflict(
        "label",
        options.labels.is_some(),
        options.add_label.is_some(),
        options.remove_label.is_some(),
    )?;
    replace_conflict(
        "initiative",
        options.initiatives.is_some(),
        options.add_initiative.is_some(),
        options.remove_initiative.is_some(),
    )?;
    for value in options
        .labels
        .iter()
        .flatten()
        .chain(options.add_label.iter().flatten())
        .chain(options.remove_label.iter().flatten())
    {
        if value.trim().is_empty() {
            return Err(shared::validation("Project label cannot be empty")
                .with_suggestion("Provide a label name, e.g. --label \"My Label\"."));
        }
    }
    let description = shared::description(
        options.description.as_deref(),
        options.description_file.as_deref(),
    )?;
    let content = shared::content(options.content.as_deref(), options.content_file.as_deref())?;
    shared::date(options.start_date.as_deref(), "Start")?;
    shared::date(options.target_date.as_deref(), "Target")?;
    Ok(ProjectUpdateInput {
        name: Edit::set_or_unchanged(shared::truthy(options.name.as_deref()).map(str::to_owned)),
        description: Edit::set_or_unchanged(description),
        content: Edit::set_or_unchanged(content),
        start_date: if options.clear_start_date {
            Edit::Clear
        } else {
            Edit::set_or_unchanged(
                shared::truthy(options.start_date.as_deref())
                    .map(|v| crate::graphql::scalars::TimelessDate(v.to_owned())),
            )
        },
        target_date: if options.clear_target_date {
            Edit::Clear
        } else {
            Edit::set_or_unchanged(
                shared::truthy(options.target_date.as_deref())
                    .map(|v| crate::graphql::scalars::TimelessDate(v.to_owned())),
            )
        },
        ..Default::default()
    })
}
pub fn has_fields(input: &ProjectUpdateInput) -> bool {
    !input.name.is_unchanged()
        || !input.description.is_unchanged()
        || !input.content.is_unchanged()
        || !input.status_id.is_unchanged()
        || !input.lead_id.is_unchanged()
        || !input.start_date.is_unchanged()
        || !input.target_date.is_unchanged()
        || input.team_ids.is_some()
        || input.label_ids.is_some()
}
fn next_cursor(
    after: Option<&str>,
    page: &PageInfo,
    seen: &mut HashSet<String>,
) -> Result<Option<String>, AppError> {
    if !page.has_next_page {
        return Ok(None);
    }
    let cursor = page.end_cursor.as_ref().ok_or_else(|| {
        AppError::new(
            AppErrorKind::GraphQl,
            "Linear reported another page of results but returned no cursor to fetch it",
        )
    })?;
    if Some(cursor.as_str()) == after {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            "Linear reported another page of results but returned the same cursor again",
        ));
    }
    if !seen.insert(cursor.clone()) {
        return Err(AppError::new(
            AppErrorKind::GraphQl,
            "Linear returned a pagination cursor seen earlier in this pagination walk",
        ));
    }
    Ok(Some(cursor.clone()))
}
pub async fn current_teams(
    transport: &GraphQlTransport,
    id: &str,
) -> Result<Vec<ProjectTeam>, AppError> {
    let mut result: Vec<ProjectTeam> = Vec::new();
    let mut after = None;
    let mut seen = HashSet::new();
    loop {
        let query =
            GraphQlRequest::with_variables(GetProjectTeamsForUpdate::build(PageVariables {
                id: id.to_owned(),
                after: after.clone(),
            }));
        let data: GetProjectTeamsForUpdate =
            transport.execute(&query).await.map_err(AppError::from)?;
        for node in data.project.teams.nodes {
            if !result.iter().any(|kept| kept.id == node.id) {
                result.push(node);
            }
        }
        match next_cursor(after.as_deref(), &data.project.teams.page_info, &mut seen)? {
            Some(cursor) => after = Some(cursor),
            None => return Ok(result),
        }
    }
}
pub async fn current_labels(
    transport: &GraphQlTransport,
    id: &str,
) -> Result<Vec<ProjectLabel>, AppError> {
    let mut result: Vec<ProjectLabel> = Vec::new();
    let mut after = None;
    let mut seen = HashSet::new();
    loop {
        let query =
            GraphQlRequest::with_variables(GetProjectLabelsForUpdate::build(PageVariables {
                id: id.to_owned(),
                after: after.clone(),
            }));
        let data: GetProjectLabelsForUpdate =
            transport.execute(&query).await.map_err(AppError::from)?;
        for node in data.project.labels.nodes {
            if !result.iter().any(|kept| kept.id == node.id) {
                result.push(node);
            }
        }
        match next_cursor(after.as_deref(), &data.project.labels.page_info, &mut seen)? {
            Some(cursor) => after = Some(cursor),
            None => return Ok(result),
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub struct DisplayProject {
    pub name: String,
    pub url: String,
}
pub async fn current_links(
    transport: &GraphQlTransport,
    id: &str,
) -> Result<(Vec<InitiativeLink>, DisplayProject), AppError> {
    let mut result: Vec<InitiativeLink> = Vec::new();
    let mut after = None;
    let mut seen = HashSet::new();
    loop {
        let query = GraphQlRequest::with_variables(GetProjectInitiativeLinksForUpdate::build(
            PageVariables {
                id: id.to_owned(),
                after: after.clone(),
            },
        ));
        let data: GetProjectInitiativeLinksForUpdate =
            transport.execute(&query).await.map_err(AppError::from)?;
        let display = DisplayProject {
            name: data.project.name,
            url: data.project.url,
        };
        for node in data.project.initiative_to_projects.nodes {
            if !result.iter().any(|kept| kept.id == node.id.inner()) {
                result.push(InitiativeLink {
                    id: node.id.into_inner(),
                    initiative_id: node.initiative.id.into_inner(),
                    initiative_name: node.initiative.name,
                });
            }
        }
        match next_cursor(
            after.as_deref(),
            &data.project.initiative_to_projects.page_info,
            &mut seen,
        )? {
            Some(cursor) => after = Some(cursor),
            None => return Ok((result, display)),
        }
    }
}
fn ids(values: &[ResolvedRef]) -> Vec<String> {
    values.iter().map(|v| v.id.clone()).collect()
}
fn slices(value: &Option<Vec<String>>) -> &[String] {
    value.as_deref().unwrap_or(&[])
}
pub struct Plan {
    pub project_id: String,
    pub input: ProjectUpdateInput,
    pub changes: Vec<InitiativeChange>,
    pub initiative_only_display: Option<DisplayProject>,
}
pub async fn plan(
    transport: &GraphQlTransport,
    scope: &WorkspaceScope<'_>,
    original: &str,
    options: &Options,
    mut input: ProjectUpdateInput,
) -> Result<Plan, AppError> {
    let reference = refs::prepare_project_lookup(original, scope)?;
    let id = refs::resolve_project_with_transport(&reference, original, transport).await?;
    if let Some(value) = shared::truthy(options.status.as_deref()) {
        input.status_id = Edit::Set(shared::status(transport, value).await?);
    }
    input.lead_id = if options.clear_lead {
        Edit::Clear
    } else {
        match shared::truthy(options.lead.as_deref()) {
            Some(value) => Edit::Set(shared::user(transport, value, "Lead").await?),
            None => Edit::Unchanged,
        }
    };
    if let Some(values) = &options.teams {
        input.team_ids = Some(
            shared::teams(transport, scope, values)
                .await?
                .into_iter()
                .map(|t| t.id)
                .collect(),
        );
    } else if options.add_team.is_some() || options.remove_team.is_some() {
        let to_refs = |teams: Vec<crate::refs::ResolvedTeam>| {
            teams
                .into_iter()
                .map(|t| ResolvedRef {
                    id: t.id,
                    label: t.key,
                })
                .collect::<Vec<_>>()
        };
        let added = to_refs(shared::teams(transport, scope, slices(&options.add_team)).await?);
        let removed = to_refs(shared::teams(transport, scope, slices(&options.remove_team)).await?);
        overlap("team", &added, &removed)?;
        let current = current_teams(transport, &id).await?;
        let current_ids: Vec<_> = current.iter().map(|t| t.id.clone().into_inner()).collect();
        let result = project_collections::apply_collection_edit(&current_ids, &added, &removed)
            .map_err(|missing| {
                shared::validation(format!(
                    "Cannot remove team \"{}\": it is not on this project",
                    missing.0.label
                ))
                .with_suggestion(format!(
                    "Current teams: {}. Use --add-team to add one.",
                    current
                        .iter()
                        .map(|t| format!("{} ({})", t.key, t.name))
                        .collect::<Vec<_>>()
                        .join(", ")
                ))
            })?;
        if result.is_empty() {
            return Err(shared::validation("Removing these teams would leave the project with no teams; Linear requires at least one").with_suggestion("Keep at least one team, or use --team to replace the set."));
        }
        input.team_ids = Some(result);
    }
    if let Some(values) = &options.labels {
        input.label_ids = Some(ids(&shared::labels(transport, values).await?));
    } else if options.add_label.is_some() || options.remove_label.is_some() {
        let added = shared::labels(transport, slices(&options.add_label)).await?;
        let removed = shared::labels(transport, slices(&options.remove_label)).await?;
        overlap("label", &added, &removed)?;
        let current = current_labels(transport, &id).await?;
        let current_ids: Vec<_> = current.iter().map(|l| l.id.clone().into_inner()).collect();
        input.label_ids = Some(
            project_collections::apply_collection_edit(&current_ids, &added, &removed).map_err(
                |missing| {
                    shared::validation(format!(
                        "Cannot remove label \"{}\": it is not on this project",
                        missing.0.label
                    ))
                    .with_suggestion(if current.is_empty() {
                        "The project has no labels. Use --add-label to add one.".to_owned()
                    } else {
                        format!(
                            "Current labels: {}. Use --add-label to add one.",
                            current
                                .iter()
                                .map(|l| l.name.clone())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })
                },
            )?,
        );
    }
    let mut changes = Vec::new();
    let mut display = None;
    if options.initiatives.is_some()
        || options.add_initiative.is_some()
        || options.remove_initiative.is_some()
    {
        let replacement = match &options.initiatives {
            Some(values) => Some(shared::initiatives(transport, scope, values).await?),
            None => None,
        };
        let added = shared::initiatives(transport, scope, slices(&options.add_initiative)).await?;
        let removed =
            shared::initiatives(transport, scope, slices(&options.remove_initiative)).await?;
        overlap("initiative", &added, &removed)?;
        let (links, last_page) = current_links(transport, &id).await?;
        let current_ids: Vec<_> = links.iter().map(|l| l.initiative_id.clone()).collect();
        let desired=match &replacement{Some(refs)=>ids(refs),None=>project_collections::apply_collection_edit(&current_ids,&added,&removed).map_err(|missing|shared::validation(format!("Cannot remove initiative \"{}\": it is not linked to this project",missing.0.label)).with_suggestion(if links.is_empty(){"The project is not linked to any initiative. Use --add-initiative to link one.".to_owned()}else{format!("Current initiatives: {}. Use --add-initiative to link one.",links.iter().map(|l|l.initiative_name.clone()).collect::<Vec<_>>().join(", "))}))?};
        let mut labels = replacement.unwrap_or_default();
        labels.extend(added);
        changes = project_collections::plan_initiative_changes(&links, &desired, &labels);
        display = Some(last_page);
    }
    Ok(Plan {
        project_id: id,
        input,
        changes,
        initiative_only_display: display,
    })
}
pub async fn apply(
    transport: &GraphQlTransport,
    id: &str,
    changes: &[InitiativeChange],
    prior_fields: bool,
) -> Result<(), AppError> {
    for (applied, change) in changes.iter().enumerate() {
        let result = match change {
            InitiativeChange::Add { initiative_id, .. } => {
                let query = GraphQlRequest::with_variables(AddProjectToInitiativeForUpdate::build(
                    LinkVariables {
                        input: InitiativeLinkInput {
                            initiative_id: initiative_id.clone(),
                            project_id: id.to_owned(),
                        },
                    },
                ));
                let result: Result<AddProjectToInitiativeForUpdate, _> =
                    transport.execute(&query).await;
                result.map(|r| r.initiative_to_project_create.success)
            }
            InitiativeChange::Remove { link_id, .. } => {
                let query = GraphQlRequest::with_variables(
                    RemoveProjectFromInitiativeForUpdate::build(IdVariables {
                        id: link_id.clone(),
                    }),
                );
                let result: Result<RemoveProjectFromInitiativeForUpdate, _> =
                    transport.execute(&query).await;
                result.map(|r| r.initiative_to_project_delete.success)
            }
        };
        let (outcome, cause) = match result {
            Ok(true) => continue,
            Ok(false) => (
                FailedWrite::Rejected,
                AppError::new(
                    AppErrorKind::GraphQl,
                    format!(
                        "Linear reported failure for initiative \"{}\"",
                        match change {
                            InitiativeChange::Add { label, .. }
                            | InitiativeChange::Remove { label, .. } => label,
                        }
                    ),
                ),
            ),
            Err(error) => (FailedWrite::Unknown, AppError::from(error)),
        };
        let diagnostic =
            project_collections::partial_diagnostic(changes, applied, outcome, prior_fields)?;
        return Err(AppError::new(AppErrorKind::GraphQl, diagnostic.message)
            .with_suggestion(diagnostic.suggestion)
            .with_source(cause));
    }
    Ok(())
}
pub async fn submit(
    transport: &GraphQlTransport,
    plan: Plan,
) -> Result<Option<DisplayProject>, AppError> {
    let prior_fields = has_fields(&plan.input);
    let display = if prior_fields {
        let query = GraphQlRequest::with_variables(UpdateProject::build(UpdateProjectVariables {
            id: plan.project_id.clone(),
            input: plan.input,
        }));
        let result: UpdateProject = transport.execute(&query).await.map_err(AppError::from)?;
        if !result.project_update.success {
            return Err(AppError::new(
                AppErrorKind::GraphQl,
                "Failed to update project",
            ));
        }
        result.project_update.project.map(|p| DisplayProject {
            name: p.name,
            url: p.url,
        })
    } else {
        plan.initiative_only_display
    };
    apply(transport, &plan.project_id, &plan.changes, prior_fields).await?;
    Ok(display)
}
pub fn output(project: Option<&DisplayProject>) -> Vec<u8> {
    match project {
        None => Vec::new(),
        Some(project) => {
            let mut text = format!("✓ Updated project: {}\n", project.name);
            if !project.url.is_empty() {
                text.push_str(&project.url);
                text.push('\n');
            }
            text.into_bytes()
        }
    }
}
