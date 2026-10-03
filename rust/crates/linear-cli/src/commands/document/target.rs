//! A document's attachment: exactly one project, issue, initiative, team,
//! cycle or release. Clap rejects more than one; `--team` with `--cycle`
//! names the team to look the cycle up in.
use crate::client::LinearClient;
use crate::commands::team_key::configured_team_key;
use crate::ctx::Ctx;
use crate::error::{Error, Result};
use crate::graphql::operations::documents::*;
use crate::graphql::operations::initiatives::IDComparator;
use crate::refs::{
    self, InitiativeReference, LinearUrlKind, LinearUrlRef, PreparedTeamLookup, ProjectReference,
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
        return Ok(Some(PreparedTarget::Project {
            original: original.to_owned(),
            reference: refs::prepare_project_lookup(original, &scope)?,
        }));
    }
    if let Some(original) = target.issue {
        let url = refs::expect_url_kind(
            original,
            LinearUrlKind::Issue,
            "an issue URL, identifier like ENG-123, or UUID",
            &scope,
        )?;
        let id = match url {
            Some(LinearUrlRef::Issue { identifier, .. }) => identifier,
            Some(_) => unreachable!("expect_url_kind only returns issue URLs here"),
            None if refs::is_linear_uuid(original) => original.to_owned(),
            None => original.to_uppercase(),
        };
        return Ok(Some(PreparedTarget::Issue {
            original: original.to_owned(),
            id,
        }));
    }
    if let Some(original) = target.initiative {
        return Ok(Some(PreparedTarget::Initiative {
            original: original.to_owned(),
            reference: refs::prepare_initiative_lookup(original, &scope)?,
        }));
    }
    if let Some(reference) = target.cycle {
        let configured = configured_team_key(ctx.options());
        let team = target.team.or(configured.as_deref()).ok_or_else(|| {
            Error::new("--cycle requires a team to look the cycle up in")
                .with_hint("Pass --team <key, name, or ID> or configure a default team.")
        })?;
        return Ok(Some(PreparedTarget::Cycle {
            team: refs::prepare_team_lookup(team, &scope)?,
            reference: reference.to_owned(),
            url: refs::expect_url_kind(
                reference,
                LinearUrlKind::Cycle,
                "a cycle URL, number, or name",
                &scope,
            )?,
        }));
    }
    if let Some(team) = target.team {
        return Ok(Some(PreparedTarget::Team(refs::prepare_team_lookup(
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
        PreparedTarget::Project {
            original,
            reference,
        } => Ok((
            Kind::Project,
            refs::resolve_project_with_transport(reference, original, client).await?,
        )),
        PreparedTarget::Initiative {
            original,
            reference,
        } => Ok((
            Kind::Initiative,
            refs::resolve_initiative_with_transport(reference, original, client).await?,
        )),
        PreparedTarget::Team(reference) => Ok((
            Kind::Team,
            refs::resolve_team_with_transport(reference, client)
                .await?
                .id,
        )),
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
        PreparedTarget::Cycle {
            team,
            reference,
            url,
        } => {
            let team = refs::resolve_team_with_transport(team, client).await?;
            let id =
                crate::commands::cycle::view::resolve_id(client, &team.id, reference, url.as_ref())
                    .await?;
            Ok((Kind::Cycle, id))
        }
        PreparedTarget::Release(original) => Ok((
            Kind::Release,
            crate::commands::release_lookup::resolve(client, original).await?,
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
