//! Document target cardinality, strict local preparation and six typed resolvers.
use crate::cli::document::DocumentList;
use crate::error::Error;
use crate::graphql::envelope::{GraphQlRequest, is_not_found};
use crate::graphql::operations::documents::*;
use crate::graphql::operations::initiatives::IDComparator;
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use crate::refs::{
    self, InitiativeReference, LinearUrlKind, LinearUrlRef, PreparedTeamLookup, ProjectReference,
    WorkspaceScope,
};
use cynic::QueryBuilder;

pub const TARGET_SUGGESTION: &str = "Pass exactly one of --project, --issue, --initiative, --team, --cycle, or --release. (--team combined with --cycle scopes the cycle lookup and does not count as a second target.)";
#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Project,
    Issue,
    Initiative,
    Team,
    Cycle,
    Release,
}
pub enum PreparedTarget {
    Project {
        original: String,
        reference: ProjectReference,
    },
    Issue {
        original: String,
        id: String,
    },
    Initiative {
        original: String,
        reference: InitiativeReference,
    },
    Team(PreparedTeamLookup),
    Cycle {
        team: PreparedTeamLookup,
        reference: String,
        url: Option<LinearUrlRef>,
    },
    Release(String),
}
#[derive(Clone, Copy, Debug, Default)]
pub struct TargetOptions<'a> {
    pub project: Option<&'a str>,
    pub issue: Option<&'a str>,
    pub initiative: Option<&'a str>,
    pub team: Option<&'a str>,
    pub cycle: Option<&'a str>,
    pub release: Option<&'a str>,
}
impl<'a> From<&'a DocumentList> for TargetOptions<'a> {
    fn from(action: &'a DocumentList) -> Self {
        Self {
            project: action.project.as_deref(),
            issue: action.issue.as_deref(),
            initiative: action.initiative.as_deref(),
            team: action.team.as_deref(),
            cycle: action.cycle.as_deref(),
            release: action.release.as_deref(),
        }
    }
}
impl TargetOptions<'_> {
    pub fn any(self) -> bool {
        self.project.is_some()
            || self.issue.is_some()
            || self.initiative.is_some()
            || self.team.is_some()
            || self.cycle.is_some()
            || self.release.is_some()
    }
    pub fn cardinality(self, required: bool) -> Result<(), Error> {
        let mut flags = Vec::new();
        if self.project.is_some() {
            flags.push("--project");
        }
        if self.issue.is_some() {
            flags.push("--issue");
        }
        if self.initiative.is_some() {
            flags.push("--initiative");
        }
        if self.cycle.is_some() {
            flags.push("--cycle");
        } else if self.team.is_some() {
            flags.push("--team");
        }
        if self.release.is_some() {
            flags.push("--release");
        }
        if flags.len() > 1 {
            return Err(Error::new(format!(
                "Only one attachment target may be set (got {})",
                flags.join(", ")
            ))
            .with_hint(TARGET_SUGGESTION));
        }
        if required && flags.is_empty() {
            return Err(
                Error::new("A document attachment target is required").with_hint(TARGET_SUGGESTION)
            );
        }
        Ok(())
    }
}
pub fn prepare(
    action: &DocumentList,
    scope: &WorkspaceScope<'_>,
    configured_team: Option<&str>,
) -> Result<Option<PreparedTarget>, Error> {
    prepare_options(action.into(), scope, configured_team)
}
pub fn prepare_options(
    action: TargetOptions<'_>,
    scope: &WorkspaceScope<'_>,
    configured_team: Option<&str>,
) -> Result<Option<PreparedTarget>, Error> {
    action.cardinality(false)?;
    if let Some(original) = &action.project {
        return Ok(Some(PreparedTarget::Project {
            original: (*original).to_owned(),
            reference: refs::prepare_project_lookup(original, scope)?,
        }));
    }
    if let Some(original) = &action.issue {
        let url = refs::expect_url_kind(
            original,
            LinearUrlKind::Issue,
            "an issue URL, identifier like ENG-123, or UUID",
            scope,
        )?;
        let id = match url {
            Some(LinearUrlRef::Issue { identifier, .. }) => identifier,
            Some(_) => {
                return Err(Error::new("issue URL preparation returned wrong kind"));
            }
            None if refs::is_linear_uuid(original) => (*original).to_owned(),
            None => original.to_uppercase(),
        };
        return Ok(Some(PreparedTarget::Issue {
            original: (*original).to_owned(),
            id,
        }));
    }
    if let Some(original) = &action.initiative {
        return Ok(Some(PreparedTarget::Initiative {
            original: (*original).to_owned(),
            reference: refs::prepare_initiative_lookup(original, scope)?,
        }));
    }
    if let Some(reference) = &action.cycle {
        let configured = configured_team
            .filter(|team| !team.is_empty())
            .map(str::to_uppercase);
        let team = action.team.or(configured.as_deref()).ok_or_else(|| {
            Error::new("--cycle requires a team to look the cycle up in")
                .with_hint("Pass --team <key, name, or ID> or configure a default team.")
        })?;
        return Ok(Some(PreparedTarget::Cycle {
            team: refs::prepare_team_lookup(team, scope)?,
            reference: (*reference).to_owned(),
            url: refs::expect_url_kind(
                reference,
                LinearUrlKind::Cycle,
                "a cycle URL, number, or name",
                scope,
            )?,
        }));
    }
    if let Some(team) = &action.team {
        return Ok(Some(PreparedTarget::Team(refs::prepare_team_lookup(
            team, scope,
        )?)));
    }
    if let Some(original) = &action.release {
        refs::reject_linear_url(original, "a release name, version, or UUID")?;
        return Ok(Some(PreparedTarget::Release((*original).to_owned())));
    }
    Ok(None)
}
pub async fn resolve(
    target: &PreparedTarget,
    transport: &GraphQlTransport,
) -> Result<(Kind, String), Error> {
    match target {
        PreparedTarget::Project {
            original,
            reference,
        } => Ok((
            Kind::Project,
            refs::resolve_project_with_transport(reference, original, transport).await?,
        )),
        PreparedTarget::Initiative {
            original,
            reference,
        } => Ok((
            Kind::Initiative,
            refs::resolve_initiative_with_transport(reference, original, transport).await?,
        )),
        PreparedTarget::Team(reference) => Ok((
            Kind::Team,
            refs::resolve_team_with_transport(reference, transport)
                .await?
                .id,
        )),
        PreparedTarget::Issue { original, id } => {
            let query = GraphQlRequest::with_variables(GetIssueForDocumentTarget::build(
                GetDocumentVariables { id: id.clone() },
            ));
            let not_found = || {
                Error::not_found("Issue", original)
                    .with_hint("Provide a valid issue identifier (e.g., TC-123) or UUID.")
            };
            let data: GetIssueForDocumentTarget =
                transport
                    .execute(&query)
                    .await
                    .map_err(|failure| match &failure {
                        TransportFailure::GraphQl { errors, .. } if is_not_found(errors) => {
                            not_found()
                        }
                        _ => Error::from(failure),
                    })?;
            Ok((
                Kind::Issue,
                data.issue.ok_or_else(not_found)?.id.into_inner(),
            ))
        }
        PreparedTarget::Cycle {
            team,
            reference,
            url,
        } => {
            let team = refs::resolve_team_with_transport(team, transport).await?;
            let id = crate::commands::cycle::view::resolve_id_with(
                &team.id,
                reference,
                url.as_ref(),
                |query| async move { transport.execute(&query).await.map_err(Error::from) },
            )
            .await?;
            Ok((Kind::Cycle, id))
        }
        PreparedTarget::Release(original) => Ok((
            Kind::Release,
            crate::commands::release_lookup::resolve(transport, original).await?,
        )),
    }
}
pub fn filter(kind: Kind, id: String) -> DocumentFilter {
    let id = IDComparator {
        eq: Some(cynic::Id::new(id)),
    };
    let mut filter = DocumentFilter::default();
    match kind {
        Kind::Project => {
            filter.project = Some(DocumentProjectFilter {
                id: EntityIdentifierIDComparator { eq: id.eq },
            })
        }
        Kind::Issue => {
            filter.issue = Some(DocumentIssueFilter {
                id: IssueIDComparator { eq: id.eq },
            })
        }
        Kind::Initiative => {
            filter.initiative = Some(DocumentInitiativeFilter {
                id: EntityIdentifierIDComparator { eq: id.eq },
            })
        }
        Kind::Team => filter.team = Some(DocumentTeamFilter { id }),
        Kind::Cycle => filter.cycle = Some(DocumentCycleFilter { id }),
        Kind::Release => filter.release = Some(DocumentReleaseFilter { id }),
    }
    filter
}
