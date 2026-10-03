//! `project update`: resolve every reference first, then apply the field
//! update and the initiative links in order.

use crate::cli::project::ProjectUpdate;
use crate::cli::values::Priority;
use crate::client::LinearClient;
use crate::commands::project::collections::{
    self, FailedWrite, InitiativeChange, InitiativeLink, ResolvedRef,
};
use crate::commands::project::write;
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::edit::Edit;
use crate::graphql::operations::common::IdVariables;
use crate::graphql::operations::project::{
    AddProjectToInitiative, GetProjectInitiativeLinksForUpdate, GetProjectLabelsForUpdate,
    GetProjectTeamsForUpdate, InitiativeLinkInput, LinkVariables, PageVariables, ProjectLabel,
    ProjectUpdateInput, RemoveProjectFromInitiative, UpdateProject, UpdateProjectVariables,
};
use crate::graphql::operations::team::TeamRef;
use crate::graphql::pagination::{self, Page};
use crate::graphql::scalars::TimelessDate;
use crate::refs::{
    self, initiative::InitiativeReference, project::ProjectReference, team::TeamReference,
};

pub fn run(ctx: &Ctx, args: &ProjectUpdate) -> Result<()> {
    update(ctx, args).context("Failed to update project")
}

/// How a flag family (`--team`, `--add-team`, `--remove-team`) changes a set.
enum SetChange<T> {
    Keep,
    Replace(Vec<T>),
    Edit { add: Vec<T>, remove: Vec<T> },
}

impl<T> SetChange<T> {
    /// clap rejects replacing together with adding or removing.
    fn new(replace: Vec<T>, add: Vec<T>, remove: Vec<T>) -> Self {
        if !replace.is_empty() {
            Self::Replace(replace)
        } else if add.is_empty() && remove.is_empty() {
            Self::Keep
        } else {
            Self::Edit { add, remove }
        }
    }

    fn is_keep(&self) -> bool {
        matches!(self, Self::Keep)
    }
}

/// The project as the success line names it.
struct Shown {
    name: String,
    url: String,
}

fn update(ctx: &Ctx, args: &ProjectUpdate) -> Result<()> {
    let fields = &args.fields;
    let scope = ctx.scope()?;
    let teams = SetChange::new(
        write::prepare_teams(&args.team, &scope)?,
        write::prepare_teams(&args.add_team, &scope)?,
        write::prepare_teams(&args.remove_team, &scope)?,
    );
    let labels = SetChange::new(
        args.label.clone(),
        args.add_label.clone(),
        args.remove_label.clone(),
    );
    let initiatives = SetChange::new(
        write::prepare_initiatives(&args.initiative, &scope)?,
        write::prepare_initiatives(&args.add_initiative, &scope)?,
        write::prepare_initiatives(&args.remove_initiative, &scope)?,
    );
    let changes_fields = fields.name.is_some()
        || fields.description.is_some()
        || fields.description_file.is_some()
        || fields.content.is_some()
        || fields.content_file.is_some()
        || fields.status.is_some()
        || fields.lead.is_some()
        || fields.start_date.is_some()
        || fields.target_date.is_some()
        || fields.priority.is_some()
        || args.clear_lead
        || args.clear_start_date
        || args.clear_target_date;
    if !changes_fields && teams.is_keep() && labels.is_keep() && initiatives.is_keep() {
        return Err(Error::new("No changes specified").with_hint(
            "Pass at least one field to change, like --name, --status, --lead, or --add-team.",
        ));
    }
    write::plain_references(&fields.lead, "an email, username, display name, or @me")?;
    write::plain_references(
        args.label
            .iter()
            .chain(&args.add_label)
            .chain(&args.remove_label),
        "a project label name",
    )?;
    let original = &args.project_id;
    let reference = ProjectReference::parse(original, &scope)?;
    let mut input = ProjectUpdateInput {
        name: Edit::set_or_unchanged(fields.name.clone()),
        description: Edit::set_or_unchanged(write::description(fields)?),
        content: Edit::set_or_unchanged(write::content(fields)?),
        start_date: if args.clear_start_date {
            Edit::Clear
        } else {
            Edit::set_or_unchanged(fields.start_date.map(TimelessDate::from))
        },
        target_date: if args.clear_target_date {
            Edit::Clear
        } else {
            Edit::set_or_unchanged(fields.target_date.map(TimelessDate::from))
        },
        priority: fields.priority.map(Priority::number),
        ..Default::default()
    };
    let client = ctx.client()?;
    let shown = ctx.spin(true, async {
        let id = refs::project::resolve(client, &reference).await?;
        if let Some(status) = fields.status {
            input.status_id = Edit::Set(write::status_id(client, status).await?);
        }
        input.lead_id = match &fields.lead {
            Some(lead) => Edit::Set(refs::user::resolve(client, lead, "Lead").await?),
            None if args.clear_lead => Edit::Clear,
            None => Edit::Unchanged,
        };
        input.team_ids = team_ids(client, &id, &teams).await?;
        input.label_ids = label_ids(client, &id, &labels).await?;
        let (changes, linked) = initiative_changes(client, &id, &initiatives).await?;
        let updated = changes_fields || input.team_ids.is_some() || input.label_ids.is_some();
        let shown = if updated {
            submit(client, &id, input).await?
        } else {
            linked
        };
        apply(client, &id, &changes, updated).await?;
        Ok::<_, Error>(shown)
    })?;
    ctx.print(match shown {
        Some(shown) => format!("✓ Updated project: {}\n{}\n", shown.name, shown.url),
        None => format!("✓ Updated project: {original}\n"),
    })
}

async fn team_ids(
    client: &LinearClient,
    project_id: &str,
    change: &SetChange<TeamReference>,
) -> Result<Option<Vec<String>>> {
    let (add, remove) = match change {
        SetChange::Keep => return Ok(None),
        SetChange::Replace(teams) => return Ok(Some(ids(&team_refs(client, teams).await?))),
        SetChange::Edit { add, remove } => (
            team_refs(client, add).await?,
            team_refs(client, remove).await?,
        ),
    };
    no_overlap("team", &add, &remove)?;
    let current = current_teams(client, project_id).await?;
    let current_ids: Vec<_> = current
        .iter()
        .map(|team| team.id.inner().to_owned())
        .collect();
    let result =
        collections::apply_collection_edit(&current_ids, &add, &remove).map_err(|missing| {
            Error::new(format!(
                "Cannot remove team \"{}\": it is not on this project",
                missing.0.label
            ))
            .with_hint(format!(
                "Current teams: {}. Use --add-team to add one.",
                current
                    .iter()
                    .map(|team| format!("{} ({})", team.key, team.name))
                    .collect::<Vec<_>>()
                    .join(", ")
            ))
        })?;
    if result.is_empty() {
        return Err(Error::new(
            "Removing these teams would leave the project with no teams; Linear requires at least one",
        )
        .with_hint("Keep at least one team, or use --team to replace the set."));
    }
    Ok(Some(result))
}

async fn team_refs(client: &LinearClient, teams: &[TeamReference]) -> Result<Vec<ResolvedRef>> {
    Ok(write::teams(client, teams)
        .await?
        .into_iter()
        .map(|team| ResolvedRef {
            id: team.id,
            label: team.key,
        })
        .collect())
}

async fn label_ids(
    client: &LinearClient,
    project_id: &str,
    change: &SetChange<String>,
) -> Result<Option<Vec<String>>> {
    let (add, remove) = match change {
        SetChange::Keep => return Ok(None),
        SetChange::Replace(labels) => return Ok(Some(ids(&write::labels(client, labels).await?))),
        SetChange::Edit { add, remove } => (
            write::labels(client, add).await?,
            write::labels(client, remove).await?,
        ),
    };
    no_overlap("label", &add, &remove)?;
    let current = current_labels(client, project_id).await?;
    let current_ids: Vec<_> = current
        .iter()
        .map(|label| label.id.inner().to_owned())
        .collect();
    let result =
        collections::apply_collection_edit(&current_ids, &add, &remove).map_err(|missing| {
            Error::new(format!(
                "Cannot remove label \"{}\": it is not on this project",
                missing.0.label
            ))
            .with_hint(if current.is_empty() {
                "The project has no labels. Use --add-label to add one.".to_owned()
            } else {
                format!(
                    "Current labels: {}. Use --add-label to add one.",
                    current
                        .iter()
                        .map(|label| label.name.as_str())
                        .collect::<Vec<_>>()
                        .join(", ")
                )
            })
        })?;
    Ok(Some(result))
}

/// The link changes to make, and the project as its links query names it.
async fn initiative_changes(
    client: &LinearClient,
    project_id: &str,
    change: &SetChange<InitiativeReference>,
) -> Result<(Vec<InitiativeChange>, Option<Shown>)> {
    let (desired, labels, links, shown) = match change {
        SetChange::Keep => return Ok((Vec::new(), None)),
        SetChange::Replace(initiatives) => {
            let replacement = write::initiatives(client, initiatives).await?;
            let (links, shown) = current_links(client, project_id).await?;
            (ids(&replacement), replacement, links, shown)
        }
        SetChange::Edit { add, remove } => {
            let add = write::initiatives(client, add).await?;
            let remove = write::initiatives(client, remove).await?;
            no_overlap("initiative", &add, &remove)?;
            let (links, shown) = current_links(client, project_id).await?;
            let current: Vec<_> = links
                .iter()
                .map(|link| link.initiative_id.clone())
                .collect();
            let desired = collections::apply_collection_edit(&current, &add, &remove).map_err(
                |missing| {
                    Error::new(format!(
                        "Cannot remove initiative \"{}\": it is not linked to this project",
                        missing.0.label
                    ))
                    .with_hint(if links.is_empty() {
                        "The project is not linked to any initiative. Use --add-initiative to link one."
                            .to_owned()
                    } else {
                        format!(
                            "Current initiatives: {}. Use --add-initiative to link one.",
                            links
                                .iter()
                                .map(|link| link.initiative_name.as_str())
                                .collect::<Vec<_>>()
                                .join(", ")
                        )
                    })
                },
            )?;
            (desired, add, links, shown)
        }
    };
    let changes = collections::plan_initiative_changes(&links, &desired, &labels);
    Ok((changes, Some(shown)))
}

fn ids(references: &[ResolvedRef]) -> Vec<String> {
    references
        .iter()
        .map(|reference| reference.id.clone())
        .collect()
}

fn no_overlap(kind: &str, add: &[ResolvedRef], remove: &[ResolvedRef]) -> Result<()> {
    if collections::has_add_remove_overlap(add, remove) {
        return Err(Error::new(format!(
            "Cannot add and remove the same {kind} in one update"
        ))
        .with_hint(format!(
            "Remove the duplicate {kind} from either --add-{kind} or --remove-{kind}."
        )));
    }
    Ok(())
}

async fn current_teams(client: &LinearClient, id: &str) -> Result<Vec<TeamRef>> {
    let mut teams = pagination::collect(None, |after, _first| {
        let variables = PageVariables {
            id: id.to_owned(),
            after,
        };
        async move {
            let data: GetProjectTeamsForUpdate = client.query(variables).await?;
            Ok(Page {
                nodes: data.project.teams.nodes,
                page_info: data.project.teams.page_info,
            })
        }
    })
    .await?;
    dedupe(&mut teams, |team| team.id.inner().to_owned());
    Ok(teams)
}

async fn current_labels(client: &LinearClient, id: &str) -> Result<Vec<ProjectLabel>> {
    let mut labels = pagination::collect(None, |after, _first| {
        let variables = PageVariables {
            id: id.to_owned(),
            after,
        };
        async move {
            let data: GetProjectLabelsForUpdate = client.query(variables).await?;
            Ok(Page {
                nodes: data.project.labels.nodes,
                page_info: data.project.labels.page_info,
            })
        }
    })
    .await?;
    dedupe(&mut labels, |label| label.id.inner().to_owned());
    Ok(labels)
}

/// The project's initiative links, and the project as that query names it.
async fn current_links(client: &LinearClient, id: &str) -> Result<(Vec<InitiativeLink>, Shown)> {
    let project = pagination::collect_within(
        None,
        |after, _first| {
            let variables = PageVariables {
                id: id.to_owned(),
                after,
            };
            async move {
                let data: GetProjectInitiativeLinksForUpdate = client.query(variables).await?;
                Ok(data.project)
            }
        },
        |project| Page {
            nodes: std::mem::take(&mut project.initiative_to_projects.nodes),
            page_info: project.initiative_to_projects.page_info.clone(),
        },
        |project, page| project.initiative_to_projects.nodes = page.nodes,
    )
    .await?;
    let mut links: Vec<_> = project
        .initiative_to_projects
        .nodes
        .into_iter()
        .map(|row| InitiativeLink {
            id: row.id.into_inner(),
            initiative_id: row.initiative.id.into_inner(),
            initiative_name: row.initiative.name,
        })
        .collect();
    dedupe(&mut links, |link| link.id.clone());
    let shown = Shown {
        name: project.name,
        url: project.url,
    };
    Ok((links, shown))
}

/// Keeps the first of each item with the same key, in order.
fn dedupe<T>(items: &mut Vec<T>, key: impl Fn(&T) -> String) {
    let mut seen = std::collections::HashSet::new();
    items.retain(|item| seen.insert(key(item)));
}

async fn submit(
    client: &LinearClient,
    id: &str,
    input: ProjectUpdateInput,
) -> Result<Option<Shown>> {
    let result: UpdateProject = client
        .mutate(UpdateProjectVariables {
            id: id.to_owned(),
            input,
        })
        .await?;
    let payload = result.project_update;
    if !payload.success {
        return Err(Error::new("Linear did not update the project"));
    }
    Ok(payload.project.map(|project| Shown {
        name: project.name,
        url: project.url,
    }))
}

/// Makes the link changes one at a time. A failure reports what was and was
/// not applied, since earlier changes are not rolled back.
async fn apply(
    client: &LinearClient,
    project_id: &str,
    changes: &[InitiativeChange],
    updated_fields: bool,
) -> Result<()> {
    for (applied, change) in changes.iter().enumerate() {
        let result = match change {
            InitiativeChange::Add { initiative_id, .. } => client
                .mutate::<AddProjectToInitiative, _>(LinkVariables {
                    input: InitiativeLinkInput {
                        initiative_id: initiative_id.clone(),
                        project_id: project_id.to_owned(),
                    },
                })
                .await
                .map(|data| data.initiative_to_project_create.success),
            InitiativeChange::Remove { link_id, .. } => client
                .mutate::<RemoveProjectFromInitiative, _>(IdVariables {
                    id: link_id.clone(),
                })
                .await
                .map(|data| data.initiative_to_project_delete.success),
        };
        let (outcome, cause) = match result {
            Ok(true) => continue,
            Ok(false) => (
                FailedWrite::Rejected,
                Error::new(format!(
                    "Linear rejected the change: {}",
                    change.description()
                )),
            ),
            Err(error) if error.outcome_unknown() => (FailedWrite::Unknown, Error::from(error)),
            Err(error) => (FailedWrite::Rejected, Error::from(error)),
        };
        let diagnostic = collections::partial_diagnostic(changes, applied, outcome, updated_fields);
        return Err(Error::new(format!("{} Cause: {cause}", diagnostic.message))
            .with_hint(diagnostic.suggestion)
            .with_source(cause));
    }
    Ok(())
}
