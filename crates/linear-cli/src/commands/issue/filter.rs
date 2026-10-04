//! Issue list filters built from command-line flags, and the lookups they need.
use crate::cli::values::UserRef;
use crate::client::LinearClient;
use crate::error::Error;
use crate::graphql::operations::issue_read::*;
use crate::graphql::pagination::{self, Page};
use crate::graphql::scalars::DateTimeOrDuration;
use crate::refs::workflow_states::{self, STATE_TYPES};
use crate::refs::{self, is_linear_uuid, reject_linear_url};
use chrono::{DateTime, SecondsFormat, Utc};

/// Filters on issues created or updated at or after the given times.
pub fn apply_dates(
    filter: &mut IssueFilter,
    created: Option<DateTime<Utc>>,
    updated: Option<DateTime<Utc>>,
) {
    let at_or_after = |time: DateTime<Utc>| DateComparator {
        gte: Some(DateTimeOrDuration(
            time.to_rfc3339_opts(SecondsFormat::Millis, true),
        )),
    };
    if let Some(time) = created {
        filter.created_at = Some(at_or_after(time));
    }
    if let Some(time) = updated {
        filter.updated_at = Some(at_or_after(time));
    }
}
pub fn team_filter(keys: &[String], mine: bool) -> TeamFilter {
    TeamFilter {
        key: Some(StringComparator {
            eq: if mine { keys.first().cloned() } else { None },
            r#in: (!mine).then(|| keys.to_vec()),
            ..Default::default()
        }),
        ..Default::default()
    }
}
pub fn query_team_filter(keys: &[String]) -> TeamFilter {
    if keys.len() == 1 {
        team_filter(keys, true)
    } else {
        TeamFilter {
            or: Some(
                keys.iter()
                    .map(|k| team_filter(std::slice::from_ref(k), true))
                    .collect(),
            ),
            ..Default::default()
        }
    }
}
pub async fn state_filter(
    client: &LinearClient,
    values: &[String],
    keys: Option<&[String]>,
) -> Result<Option<WorkflowStateFilter>, Error> {
    if values.is_empty() {
        return Ok(None);
    }
    let mut types = vec![];
    let mut lookups = vec![];
    for value in values {
        reject_linear_url(value, "a workflow state name, type, or ID")?;
        if value.chars().all(char::is_whitespace) {
            return Err(Error::new("--state value is empty").with_hint(format!(
                "Pass a state type ({}), name, or ID.",
                STATE_TYPES.join(", ")
            )));
        }
        let selected = if STATE_TYPES.contains(&value.as_str()) {
            &mut types
        } else {
            &mut lookups
        };
        if !selected.contains(value) {
            selected.push(value.clone());
        }
    }
    let states = if lookups.is_empty() {
        vec![]
    } else {
        pagination::collect(None, |after, first| async move {
            let data: GetWorkflowStatesInScope = client
                .query(GetWorkflowStatesInScopeVariables {
                    filter: keys.map(|keys| WorkflowStateFilter {
                        team: Some(team_filter(keys, false)),
                        ..Default::default()
                    }),
                    first: Some(first),
                    after,
                })
                .await?;
            Ok(Page {
                nodes: data.workflow_states.nodes,
                page_info: data.workflow_states.page_info,
            })
        })
        .await?
    };
    let mut ids = vec![];
    for value in lookups {
        let matches = states
            .iter()
            .filter(|s| {
                if is_linear_uuid(&value) {
                    s.id.inner().eq_ignore_ascii_case(&value)
                } else {
                    s.name.to_lowercase() == value.to_lowercase()
                }
            })
            .collect::<Vec<_>>();
        if matches.is_empty() {
            let candidates = states
                .iter()
                .map(|state| workflow_states::Candidate {
                    name: &state.name,
                    state_type: &state.r#type,
                    position: state.position.get(),
                    team_key: &state.team.key,
                })
                .collect();
            return Err(workflow_states::unknown_state(&value, keys, candidates));
        }
        for state in matches {
            if !ids.contains(&state.id) {
                ids.push(state.id.clone());
            }
        }
    }
    let by_type = (!types.is_empty()).then(|| WorkflowStateFilter {
        r#type: Some(StringComparator {
            r#in: Some(types),
            ..Default::default()
        }),
        ..Default::default()
    });
    let by_id = (!ids.is_empty()).then(|| WorkflowStateFilter {
        id: Some(IDComparator {
            r#in: Some(ids),
            ..Default::default()
        }),
        ..Default::default()
    });
    Ok(Some(match (by_type, by_id) {
        (Some(a), Some(b)) => WorkflowStateFilter {
            or: Some(vec![a, b]),
            ..Default::default()
        },
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => return Err(Error::new("--state selection is empty")),
    }))
}
pub fn entity_filters(
    filter: &mut IssueFilter,
    project: Option<String>,
    project_label: Option<&str>,
    cycle: Option<String>,
    milestone: Option<String>,
    labels: &[String],
) {
    filter.project = if let Some(id) = project.filter(|s| !s.is_empty()) {
        Some(NullableProjectFilter {
            id: Some(EntityIdentifierIDComparator {
                eq: Some(cynic::Id::new(id)),
            }),
            ..Default::default()
        })
    } else {
        project_label
            .filter(|s| !s.is_empty())
            .map(|name| NullableProjectFilter {
                labels: Some(ProjectLabelCollectionFilter {
                    name: Some(StringComparator {
                        eq_ignore_case: Some(name.to_owned()),
                        ..Default::default()
                    }),
                }),
                ..Default::default()
            })
    };
    filter.cycle = cycle.map(|id| NullableCycleFilter {
        id: Some(IDComparator {
            eq: Some(cynic::Id::new(id)),
            ..Default::default()
        }),
    });
    filter.project_milestone = milestone.map(|id| NullableProjectMilestoneFilter {
        id: Some(IDComparator {
            eq: Some(cynic::Id::new(id)),
            ..Default::default()
        }),
    });
    let label = |name: &String| IssueLabelCollectionFilter {
        some: Some(IssueLabelFilter {
            name: Some(StringComparator {
                eq_ignore_case: Some(name.clone()),
                ..Default::default()
            }),
        }),
        ..Default::default()
    };
    filter.labels = match labels {
        [] => None,
        [name] => Some(label(name)),
        names => Some(IssueLabelCollectionFilter {
            and: Some(names.iter().map(label).collect()),
            ..Default::default()
        }),
    };
}
/// Fails on the first `--label` name that matches no label the issues could
/// carry: a label of one of `keys`' teams or a workspace label, or any label
/// when `keys` is `None`. Without this an unknown label just finds nothing.
pub async fn check_labels(
    client: &LinearClient,
    names: &[String],
    keys: Option<&[String]>,
) -> Result<(), Error> {
    use crate::graphql::operations::common::NameVariables;
    use crate::graphql::operations::label::GetLabelByName;
    for name in names {
        let data: GetLabelByName = client.query(NameVariables { name: name.clone() }).await?;
        let usable = data
            .issue_labels
            .nodes
            .iter()
            .any(|label| match (&label.team, keys) {
                (None, _) | (Some(_), None) => true,
                (Some(team), Some(keys)) => keys.contains(&team.key),
            });
        if !usable {
            let list = match keys {
                Some([key]) => format!("linear label list --team {key}"),
                _ => "linear label list --all-teams".to_owned(),
            };
            return Err(Error::not_found("Issue label", name)
                .with_hint(format!("Run `{list}` to see the labels.")));
        }
    }
    Ok(())
}
pub async fn assignee_filter(
    client: &LinearClient,
    input: Option<&UserRef>,
    unassigned: bool,
    mine: bool,
) -> Result<Option<NullableUserFilter>, Error> {
    if mine || input == Some(&UserRef::Me) {
        return Ok(Some(NullableUserFilter {
            is_me: Some(BooleanComparator { eq: Some(true) }),
            ..Default::default()
        }));
    }
    if unassigned {
        return Ok(Some(NullableUserFilter {
            r#null: Some(true),
            ..Default::default()
        }));
    }
    let Some(input) = input else {
        return Ok(None);
    };
    let id = cynic::Id::new(refs::user::resolve(client, input, "User").await?);
    Ok(Some(NullableUserFilter {
        id: Some(IDComparator {
            eq: Some(id),
            ..Default::default()
        }),
        ..Default::default()
    }))
}
/// A milestone UUID, or the ID of the named milestone in `project`.
pub async fn milestone_id(
    client: &LinearClient,
    value: &str,
    project: Option<&str>,
) -> Result<String, Error> {
    if is_linear_uuid(value) {
        return Ok(value.to_owned());
    }
    reject_linear_url(value, "a milestone name or UUID")?;
    let project = project.ok_or_else(|| Error::new(format!("Cannot resolve milestone \"{value}\" without --project")).with_hint("Pass a milestone UUID, or specify --project so the milestone name can be looked up within that project."))?;
    crate::commands::milestone::id_by_name(client, project, value).await
}

/// The team `reference` (a key, name, ID or URL) names.
pub(super) fn resolve_team(
    ctx: &crate::ctx::Ctx,
    client: &LinearClient,
    reference: &str,
) -> Result<refs::team::ResolvedTeam, Error> {
    let lookup = refs::team::TeamReference::parse(reference, &ctx.scope()?)?;
    ctx.block_on(refs::team::resolve(client, &lookup))
}

/// The project `--project` names. When no project matches exactly, a terminal
/// user may pick one of the similarly named projects.
pub(super) fn resolve_project(
    ctx: &crate::ctx::Ctx,
    client: &LinearClient,
    value: Option<&str>,
) -> Result<Option<String>, Error> {
    use super::write::Named;
    let Some(value) = value else { return Ok(None) };
    let reference = refs::project::ProjectReference::parse(value, &ctx.scope()?)?;
    if let Some(id) = ctx.block_on(refs::project::find(client, &reference))? {
        return Ok(Some(id));
    }
    let data: GetProjectIdOptionsByName =
        ctx.block_on(client.query(GetProjectIdOptionsByNameVariables {
            name: value.to_owned(),
        }))?;
    let mut rows: Vec<Named> = vec![];
    for row in data.projects.nodes {
        if !rows.iter().any(|named| named.id == row.id.inner()) {
            rows.push(Named {
                id: row.id.into_inner(),
                name: row.name,
                detail: Some(row.slug_id),
            });
        }
    }
    if rows.is_empty() {
        return Err(Error::not_found("Project", value));
    }
    if !ctx.interactive() {
        let names: Vec<&str> = rows.iter().map(|named| named.name.as_str()).collect();
        return Err(Error::new(format!(
            "Project \"{value}\" not found. Similar projects: {}",
            names.join(", ")
        )));
    }
    let candidates: Vec<&Named> = rows.iter().collect();
    let suggestions =
        super::write::suggestions("Project", value, &candidates).expect("there are candidates");
    ctx.eprint(&suggestions.note)?;
    ctx.prompter()?
        .select(&suggestions.question, suggestions.choices)
}

/// The cycle `--cycle` names in the one team in scope.
pub(super) fn resolve_cycle(
    ctx: &crate::ctx::Ctx,
    client: &LinearClient,
    value: Option<&str>,
    team_key: Option<&str>,
    team_id: Option<&str>,
) -> Result<Option<String>, Error> {
    let Some(value) = value else { return Ok(None) };
    let team_id = match team_id {
        Some(id) => id.to_owned(),
        None => {
            let key = team_key.ok_or_else(|| Error::new("--cycle requires a single team scope"))?;
            resolve_team(ctx, client, key)?.id
        }
    };
    let reference = refs::cycle::CycleReference::parse(value, &ctx.scope()?)?;
    ctx.block_on(refs::cycle::resolve(client, &team_id, &reference))
        .map(Some)
}
