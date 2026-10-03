use super::{
    create::{Input, Templates},
    write::{self as domain, Backend, Created, Label, Named, Parent, State, Team, Updated},
};
use crate::{
    config::ConfigOptions,
    error::Error,
    graphql::{
        envelope::GraphQlRequest, operations::issue_write as ops, transport::GraphQlTransport,
    },
    refs,
};
use cynic::{MutationBuilder, QueryBuilder};
use serde::{Deserialize, Serialize, de::DeserializeOwned};

#[derive(Clone)]
pub struct NetworkBackend {
    pub transport: GraphQlTransport,
    pub options: ConfigOptions,
    pub cli_workspace: Option<String>,
    pub default_workspace: Option<String>,
}
fn request<F, V: Serialize>(operation: cynic::Operation<F, V>) -> GraphQlRequest<V> {
    GraphQlRequest::with_variables(operation)
}
async fn fetch<T: DeserializeOwned, V: Serialize>(
    transport: &GraphQlTransport,
    request: &GraphQlRequest<V>,
) -> Result<T, Error> {
    Ok(transport.execute(request).await?)
}
fn sorted_names(mut rows: Vec<Named>) -> Vec<Named> {
    rows.sort_by(|left, right| {
        crate::platform::collation::compare(&left.name.to_lowercase(), &right.name.to_lowercase())
    });
    rows
}
impl NetworkBackend {
    fn scope<'a>(&'a self, key: &'a crate::auth::ApiKeyInput<'a>) -> refs::WorkspaceScope<'a> {
        refs::WorkspaceScope {
            cli_workspace: self.cli_workspace.as_deref(),
            sourced_workspace: self.options.workspace().map(|v| v.value().as_str()),
            default_workspace: self.default_workspace.as_deref(),
            api_key: key.clone(),
        }
    }
    async fn parent_reference(&self, reference: &str) -> Result<String, Error> {
        let key = crate::auth::ApiKeyInput::from_options(&self.options);
        let team = crate::commands::team_key::configured_team_key(&self.options);
        match refs::prepare_issue_reference(Some(reference), team.as_deref(), &self.scope(&key))? {
            refs::IssueReference::Identifier(id) => Ok(id),
            refs::IssueReference::Unresolved | refs::IssueReference::Inferred => {
                Err(domain::validation(format!(
                    "Could not resolve parent issue identifier: {reference}"
                )))
            }
        }
    }
}
impl Backend for NetworkBackend {
    async fn team(&self, reference: String) -> Result<Team, Error> {
        let key = crate::auth::ApiKeyInput::from_options(&self.options);
        let prepared = refs::prepare_team_lookup(&reference, &self.scope(&key))?;
        let team = refs::resolve_team(
            &prepared,
            |req| async move { fetch(&self.transport, &req).await },
            |req| async move { fetch(&self.transport, &req).await },
        )
        .await?;
        Ok(Team {
            id: team.id,
            key: team.key,
            name: team.name,
        })
    }
    async fn find_team(&self, reference: String) -> Result<Option<Team>, Error> {
        let key = crate::auth::ApiKeyInput::from_options(&self.options);
        let prepared = refs::prepare_team_lookup(&reference, &self.scope(&key))?;
        Ok(refs::find_team(&prepared, |req| async move {
            fetch(&self.transport, &req).await
        })
        .await?
        .map(|team| Team {
            id: team.id,
            key: team.key,
            name: team.name,
        }))
    }
    async fn teams(&self) -> Result<Vec<Team>, Error> {
        Ok(
            refs::fetch_all_teams(|req| async move { fetch(&self.transport, &req).await })
                .await?
                .into_iter()
                .map(|team| Team {
                    id: team.id,
                    key: team.key,
                    name: team.name,
                })
                .collect(),
        )
    }
    async fn team_options(&self, reference: String) -> Result<Vec<Named>, Error> {
        let data: ops::GetTeamIdOptionsByKey = fetch(
            &self.transport,
            &request(ops::GetTeamIdOptionsByKey::build(ops::TeamSubstring {
                team: reference,
            })),
        )
        .await?;
        let mut teams = data.teams.nodes;
        teams.sort_by(|a, b| {
            crate::platform::collation::compare(&a.key.to_lowercase(), &b.key.to_lowercase())
        });
        Ok(teams
            .into_iter()
            .map(|team| Named {
                id: team.id.into_inner(),
                name: format!("{} ({})", team.name, team.key),
            })
            .collect())
    }
    async fn viewer(&self) -> Result<String, Error> {
        use crate::graphql::operations::initiatives::{GetViewerId, GetViewerIdVariables};
        let data: GetViewerId = fetch(
            &self.transport,
            &request(GetViewerId::build(GetViewerIdVariables {})),
        )
        .await?;
        Ok(data.viewer.id.into_inner())
    }
    async fn auto_assign(&self) -> Result<bool, Error> {
        let data: ops::GetUserSettings = fetch(
            &self.transport,
            &GraphQlRequest::without_variables(ops::GetUserSettings::build(())),
        )
        .await?;
        Ok(data.user_settings.auto_assign_to_self)
    }
    async fn user(&self, reference: String) -> Result<String, Error> {
        refs::reject_linear_url(&reference, "an email, username, display name, or @me")?;
        if reference == "self" || reference == "@me" {
            return self.viewer().await;
        }
        crate::commands::user::resolve(&self.transport, &reference, "User").await
    }
    async fn states(&self, team_key: String) -> Result<Vec<State>, Error> {
        use crate::graphql::operations::workflow_states::{
            GetWorkflowStates, GetWorkflowStatesVariables,
        };
        let data: GetWorkflowStates = fetch(
            &self.transport,
            &request(GetWorkflowStates::build(GetWorkflowStatesVariables {
                team_key,
            })),
        )
        .await?;
        let mut states = data.team.states.nodes;
        crate::workflow_states::sort(&mut states);
        Ok(states
            .into_iter()
            .map(|s| State {
                id: s.id.into_inner(),
                name: s.name,
                kind: s.state_type,
                position: s.position.get(),
            })
            .collect())
    }
    async fn state(&self, team_key: String, reference: String) -> Result<String, Error> {
        let states = self.states(team_key.clone()).await?;
        refs::reject_linear_url(&reference, "a workflow state name or type")?;
        if let Some(state) = states
            .iter()
            .find(|s| s.name.to_lowercase() == reference.to_lowercase())
        {
            return Ok(state.id.clone());
        }
        let mut lowest: Option<&State> = None;
        for state in states.iter().filter(|s| s.kind == reference.to_lowercase()) {
            if lowest.is_none_or(|old| state.position < old.position) {
                lowest = Some(state)
            }
        }
        if let Some(state) = lowest {
            return Ok(state.id.clone());
        }
        let suggestion = if states.is_empty() {
            format!("Team {team_key} has no workflow states. Run `linear team states {team_key}`.")
        } else {
            format!(
                "Valid states: {}. Run `linear team states {team_key}` to list them.",
                states
                    .iter()
                    .map(|s| format!(
                        "{} ({})",
                        serde_json::to_string(&s.name)
                            .unwrap_or_else(|_| unreachable!("string serialization cannot fail")),
                        s.kind
                    ))
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        };
        Err(Error::not_found(
            "Workflow state",
            &format!("'{reference}' for team {team_key}"),
        )
        .with_hint(suggestion))
    }
    async fn label(&self, team_key: String, reference: String) -> Result<Option<String>, Error> {
        refs::reject_linear_url(&reference, "a label name")?;
        let data: ops::GetIssueLabelIdByNameForTeam = fetch(
            &self.transport,
            &request(ops::GetIssueLabelIdByNameForTeam::build(
                ops::LabelVariables {
                    name: reference,
                    team_key,
                },
            )),
        )
        .await?;
        Ok(data
            .issue_labels
            .nodes
            .into_iter()
            .next()
            .map(|l| l.id.into_inner()))
    }
    async fn label_options(
        &self,
        team_key: String,
        reference: String,
    ) -> Result<Vec<Named>, Error> {
        let data: ops::GetIssueLabelIdOptionsByNameForTeam = fetch(
            &self.transport,
            &request(ops::GetIssueLabelIdOptionsByNameForTeam::build(
                ops::LabelVariables {
                    name: reference,
                    team_key,
                },
            )),
        )
        .await?;
        Ok(sorted_names(
            data.issue_labels
                .nodes
                .into_iter()
                .map(|l| Named {
                    id: l.id.into_inner(),
                    name: l.name,
                })
                .collect(),
        ))
    }
    async fn labels(&self, team_key: String) -> Result<Vec<Label>, Error> {
        let data: ops::GetLabelsForTeam = fetch(
            &self.transport,
            &request(ops::GetLabelsForTeam::build(ops::TeamKey { team_key })),
        )
        .await?;
        let mut labels = data.team.map(|t| t.labels.nodes).unwrap_or_default();
        labels.sort_by(|a, b| {
            crate::platform::collation::compare(&a.name.to_lowercase(), &b.name.to_lowercase())
        });
        Ok(labels
            .into_iter()
            .map(|l| Label {
                id: l.id.into_inner(),
                name: l.name,
                color: l.color,
            })
            .collect())
    }
    async fn project(&self, reference: String) -> Result<Option<String>, Error> {
        let key = crate::auth::ApiKeyInput::from_options(&self.options);
        let prepared = refs::prepare_project_lookup(&reference, &self.scope(&key))?;
        crate::commands::issue::read::project_id(&self.transport, &prepared).await
    }
    async fn project_options(&self, reference: String) -> Result<Vec<Named>, Error> {
        use crate::graphql::operations::issue_read::{
            GetProjectIdOptionsByName, GetProjectIdOptionsByNameVariables,
        };
        let data: GetProjectIdOptionsByName = fetch(
            &self.transport,
            &request(GetProjectIdOptionsByName::build(
                GetProjectIdOptionsByNameVariables { name: reference },
            )),
        )
        .await?;
        Ok(data
            .projects
            .nodes
            .into_iter()
            .map(|p| Named {
                id: p.id.into_inner(),
                name: p.name,
            })
            .collect())
    }
    async fn projects(&self, team_key: String) -> Result<Vec<Named>, Error> {
        use crate::graphql::operations::{
            projects::{ProjectFilter, TeamCollectionFilter},
            teams::{StringComparator, TeamFilter},
        };
        let filter = ProjectFilter {
            accessible_teams: Some(TeamCollectionFilter {
                some: Some(TeamFilter {
                    key: Some(StringComparator {
                        eq: Some(team_key),
                        ..Default::default()
                    }),
                    ..Default::default()
                }),
            }),
            ..Default::default()
        };
        let mut after = crate::graphql::edit::Edit::Unchanged;
        let mut seen = std::collections::HashSet::new();
        let mut rows = Vec::new();
        loop {
            let data: ops::GetProjectsForTeam = fetch(
                &self.transport,
                &request(ops::GetProjectsForTeam::build(ops::ProjectsVariables {
                    filter: Some(filter.clone()),
                    first: Some(100),
                    after,
                })),
            )
            .await?;
            rows.extend(data.projects.nodes.into_iter().map(|p| Named {
                id: p.id.into_inner(),
                name: p.name,
            }));
            if !data.projects.page_info.has_next_page {
                break;
            }
            let cursor = data.projects.page_info.end_cursor.ok_or_else(|| {
                domain::validation(
                    "Linear reported more projects but returned no pagination cursor",
                )
            })?;
            if !seen.insert(cursor.clone()) {
                return Err(domain::validation(
                    "Linear repeated a project pagination cursor",
                ));
            }
            after = crate::graphql::edit::Edit::Set(cursor);
        }
        Ok(sorted_names(rows))
    }
    async fn milestone(&self, project_id: String, reference: String) -> Result<String, Error> {
        crate::commands::issue::read::milestone_id(
            &self.transport,
            &reference,
            Some(project_id.as_str()).filter(|project| !project.is_empty()),
        )
        .await
    }
    async fn cycle(&self, team_id: String, reference: String) -> Result<String, Error> {
        let key = crate::auth::ApiKeyInput::from_options(&self.options);
        let url = refs::expect_url_kind(
            &reference,
            refs::LinearUrlKind::Cycle,
            "a cycle URL, number, or name",
            &self.scope(&key),
        )?;
        crate::commands::cycle::view::resolve_id_with(
            &team_id,
            &reference,
            url.as_ref(),
            |req| async move { fetch(&self.transport, &req).await },
        )
        .await
    }
    async fn parent_id(&self, reference: String) -> Result<String, Error> {
        let identifier = self.parent_reference(&reference).await?;
        // Object-only optional selected shape: reuse the already approved pattern,
        // but return its ID rather than committing the old strict GetIssueId model.
        let request = crate::commands::issue::id::request(&identifier);
        let data: OptionalIssue = fetch(&self.transport, &request).await?;
        data.issue
            .and_then(|i| i.id)
            .filter(|id| !id.is_empty())
            .ok_or_else(|| Error::not_found("Parent issue", &identifier))
    }
    async fn parent_metadata(&self, id: String) -> Result<Option<Parent>, Error> {
        let req = request(ops::GetParentIssueData::build(ops::IssueVariables { id }));
        // Only request and GraphQL errors make the parent optional; a malformed
        // response is still an error.
        let data: OptionalParent = match self.transport.execute(&req).await {
            Ok(data) => data,
            Err(crate::graphql::transport::TransportFailure::Response(error)) => {
                return Err(Error::new(
                    "Linear returned parent issue metadata with an unexpected shape",
                )
                .with_source(error));
            }
            Err(_) => return Ok(None),
        };
        let Some(data) = data.issue else {
            return Ok(None);
        };
        Ok(Some(Parent {
            title: data.title,
            identifier: data.identifier,
            project_id: data
                .project
                .map(|p| p.id.into_inner())
                .filter(|id| !id.is_empty()),
        }))
    }
    async fn issue_project(&self, id: String) -> Result<Option<String>, Error> {
        let data: ops::GetIssueProjectId = fetch(
            &self.transport,
            &request(ops::GetIssueProjectId::build(ops::IssueVariables { id })),
        )
        .await?;
        Ok(data
            .issue
            .and_then(|i| i.project)
            .map(|p| p.id.into_inner()))
    }
    async fn create(&self, input: Input) -> Result<Created, Error> {
        use crate::graphql::operations::issue_create::{CreateIssue, CreateIssueVariables};
        let data: CreateIssue = fetch(
            &self.transport,
            &request(CreateIssue::build(CreateIssueVariables { input })),
        )
        .await?;
        if !data.issue_create.success {
            return Err(Error::new("Issue creation failed"));
        }
        let issue = data
            .issue_create
            .issue
            .ok_or_else(|| Error::new("Issue creation failed - no issue returned"))?;
        Ok(Created {
            id: issue.id.into_inner(),
            identifier: issue.identifier,
            url: issue.url,
            team_key: issue.team.key,
        })
    }
    async fn update(
        &self,
        id: String,
        input: crate::graphql::operations::issue_update::IssueUpdateInput,
    ) -> Result<Updated, Error> {
        use crate::graphql::operations::issue_update::{UpdateIssue, UpdateIssueVariables};
        let data: UpdateIssue = fetch(
            &self.transport,
            &request(UpdateIssue::build(UpdateIssueVariables { id, input })),
        )
        .await?;
        if !data.issue_update.success {
            return Err(Error::new("Issue update failed"));
        }
        let issue = data
            .issue_update
            .issue
            .ok_or_else(|| Error::new("Issue update failed - no issue returned"))?;
        Ok(Updated {
            identifier: issue.identifier,
            title: issue.title,
            url: issue.url,
        })
    }
}
#[derive(serde::Deserialize)]
struct OptionalParent {
    #[serde(default)]
    issue: Option<ops::ParentIssue>,
}
// An issue that may be null or missing in the response.
#[derive(serde::Deserialize)]
struct OptionalIssue {
    #[serde(default)]
    issue: Option<OptionalId>,
}
struct OptionalId {
    id: Option<String>,
}
impl<'de> serde::Deserialize<'de> for OptionalId {
    fn deserialize<D: serde::Deserializer<'de>>(de: D) -> Result<Self, D::Error> {
        struct Object;
        impl<'de> serde::de::Visitor<'de> for Object {
            type Value = OptionalId;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("an issue object")
            }
            fn visit_map<M: serde::de::MapAccess<'de>>(
                self,
                map: M,
            ) -> Result<Self::Value, M::Error> {
                #[derive(serde::Deserialize)]
                struct Fields {
                    #[serde(default)]
                    id: Option<String>,
                }
                let fields =
                    Fields::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(OptionalId { id: fields.id })
            }
        }
        de.deserialize_map(Object)
    }
}
impl Templates for NetworkBackend {
    async fn issue_template(&self, reference: String, team_id: String) -> Result<String, Error> {
        use super::template_scope::{self, TemplateScope};
        use crate::graphql::operations::templates::GetTemplates;
        refs::reject_linear_url(&reference, "a template name or UUID")?;
        let team_ids = [team_id];
        let template = if refs::is_linear_uuid(&reference) {
            let template =
                crate::commands::template::template_by_id(&self.transport, &reference).await?;
            template_scope::assert_scope(&template, &team_ids, TemplateScope::Issue)?;
            template
        } else {
            let req = GraphQlRequest::without_variables(GetTemplates::build(()));
            let data: GetTemplates = fetch(&self.transport, &req).await?;
            template_scope::select(&reference, data.templates, &team_ids, TemplateScope::Issue)?
        };
        Ok(template.id.into_inner())
    }
}
