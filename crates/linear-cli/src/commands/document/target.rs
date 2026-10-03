//! A document's attachment: exactly one project, issue, initiative, team,
//! cycle or release. Clap rejects more than one; `--team` with `--cycle`
//! names the team to look the cycle up in.
use crate::client::LinearClient;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::document::*;
use crate::graphql::operations::initiative::IDComparator;
use crate::refs::{
    self, LinearUrlKind, LinearUrlRef, cycle::CycleReference, initiative::Archived,
    initiative::InitiativeReference, project::ProjectReference, team::TeamReference,
};

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Project,
    Issue,
    Initiative,
    Team,
    Cycle,
    Release,
}

/// A target checked locally, ready to look up.
pub enum PreparedTarget {
    Project(ProjectReference),
    Issue {
        original: String,
        id: String,
    },
    Initiative(InitiativeReference),
    Team(TeamReference),
    Cycle {
        team: TeamReference,
        cycle: CycleReference,
    },
    Release(String),
}

/// The target flags as given.
#[derive(Clone, Copy, Debug, Default)]
pub struct TargetOptions<'a> {
    pub project: Option<&'a str>,
    pub issue: Option<&'a str>,
    pub initiative: Option<&'a str>,
    pub team: Option<&'a str>,
    pub cycle: Option<&'a str>,
    pub release: Option<&'a str>,
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
}

pub fn prepare(ctx: &Ctx, target: TargetOptions<'_>) -> Result<Option<PreparedTarget>> {
    let scope = ctx.scope()?;
    if let Some(original) = target.project {
        return Ok(Some(PreparedTarget::Project(ProjectReference::parse(
            original, &scope,
        )?)));
    }
    if let Some(original) = target.issue {
        let url = refs::expect_url_kind(
            original,
            LinearUrlKind::Issue,
            "an issue URL, identifier like ENG-123, or UUID",
            &scope,
            |url| match url {
                LinearUrlRef::Issue { identifier, .. } => Some(identifier),
                _ => None,
            },
        )?;
        let id = match url {
            Some(identifier) => identifier,
            None if refs::is_linear_uuid(original) => original.to_owned(),
            None => original.to_uppercase(),
        };
        return Ok(Some(PreparedTarget::Issue {
            original: original.to_owned(),
            id,
        }));
    }
    if let Some(original) = target.initiative {
        return Ok(Some(PreparedTarget::Initiative(
            InitiativeReference::parse(original, &scope)?,
        )));
    }
    if let Some(reference) = target.cycle {
        let configured = configured_team_key(ctx.options());
        let team = target.team.or(configured.as_deref()).ok_or_else(|| {
            Error::new("--cycle requires a team to look the cycle up in")
                .with_hint("Pass --team <key, name, or ID> or configure a default team.")
        })?;
        return Ok(Some(PreparedTarget::Cycle {
            team: TeamReference::parse(team, &scope)?,
            cycle: CycleReference::parse(reference, &scope)?,
        }));
    }
    if let Some(team) = target.team {
        return Ok(Some(PreparedTarget::Team(TeamReference::parse(
            team, &scope,
        )?)));
    }
    if let Some(original) = target.release {
        refs::reject_linear_url(original, "a release name, version, or UUID")?;
        return Ok(Some(PreparedTarget::Release(original.to_owned())));
    }
    Ok(None)
}

/// The target's kind and Linear ID.
pub async fn resolve(target: &PreparedTarget, client: &LinearClient) -> Result<(Kind, String)> {
    match target {
        PreparedTarget::Project(reference) => Ok((
            Kind::Project,
            refs::project::resolve(client, reference).await?,
        )),
        PreparedTarget::Initiative(reference) => Ok((
            Kind::Initiative,
            refs::initiative::resolve(client, reference, Archived::Exclude).await?,
        )),
        PreparedTarget::Team(reference) => {
            Ok((Kind::Team, refs::team::resolve(client, reference).await?.id))
        }
        PreparedTarget::Issue { original, id } => {
            let not_found = || {
                Error::not_found("Issue", original)
                    .with_hint("Provide a valid issue identifier (e.g., TC-123) or UUID.")
            };
            let data: GetIssueForDocumentTarget = client
                .query(GetDocumentVariables { id: id.clone() })
                .await
                .map_err(|failure| {
                    if failure.is_not_found() {
                        not_found()
                    } else {
                        Error::from(failure)
                    }
                })?;
            Ok((
                Kind::Issue,
                data.issue.ok_or_else(not_found)?.id.into_inner(),
            ))
        }
        PreparedTarget::Cycle { team, cycle } => {
            let team = refs::team::resolve(client, team).await?;
            Ok((
                Kind::Cycle,
                refs::cycle::resolve(client, &team.id, cycle).await?,
            ))
        }
        PreparedTarget::Release(original) => Ok((
            Kind::Release,
            refs::release::resolve(client, original).await?,
        )),
    }
}

/// The `document list` filter for one target.
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
