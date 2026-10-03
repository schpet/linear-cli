//! Cycles, referenced within a team by number, name, keyword (`active`,
//! `next`, `previous`), offset from the active cycle (`+1`, `-2`) or URL.
mod number;

pub use number::{CycleNumber, CycleNumberProblem};

use crate::client::LinearClient;
use crate::error::{Error, Result};
use crate::graphql::operations::cycle::{
    ActiveCycle, GetTeamCyclesForLookup, LookupCycle, LookupVariables,
};
use crate::graphql::pagination::Pages;

use super::url::{CycleSelector, LinearUrlKind, LinearUrlRef};
use super::workspace::{WorkspaceScope, expect_url_kind};

/// A cycle argument, checked locally. A cycle URL also names its team.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CycleReference {
    input: String,
    url: Option<CycleUrl>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CycleUrl {
    team_key: String,
    selector: CycleSelector,
}

impl CycleReference {
    pub fn parse(input: &str, scope: &WorkspaceScope<'_>) -> Result<Self> {
        let url = expect_url_kind(
            input,
            LinearUrlKind::Cycle,
            "a cycle URL, number, or name",
            scope,
            |url| match url {
                LinearUrlRef::Cycle {
                    team_key, cycle, ..
                } => Some(CycleUrl {
                    team_key,
                    selector: cycle,
                }),
                _ => None,
            },
        )?;
        Ok(Self {
            input: input.to_owned(),
            url,
        })
    }

    /// The team key from a cycle URL.
    pub fn url_team_key(&self) -> Option<&str> {
        self.url.as_ref().map(|url| url.team_key.as_str())
    }
}

const SIMPLE_HINT: &str = "Use a cycle number or name instead.";

/// The ID of the cycle `reference` names in team `team_id`. Every cycle page
/// is fetched before choosing; the team itself (URL team, cycles enabled) is
/// checked on the first page.
pub async fn resolve(
    client: &LinearClient,
    team_id: &str,
    reference: &CycleReference,
) -> Result<String> {
    let mut pages = Pages::new(None);
    let mut first: Option<(String, Option<ActiveCycle>)> = None;
    let mut cycles = Vec::new();
    loop {
        let data: GetTeamCyclesForLookup = client
            .query(LookupVariables {
                team_id: team_id.to_owned(),
                after: pages.after(),
            })
            .await?;
        let team = data.team.ok_or_else(|| Error::not_found("Team", team_id))?;
        if first.is_none() {
            check_team(&team.key, team.cycles_enabled, reference)?;
            first = Some((team.key.clone(), team.active_cycle));
        }
        let connection = team.cycles.ok_or_else(|| {
            Error::new(format!(
                "Linear returned no cycle list for team {}",
                team.key
            ))
        })?;
        let more = pages.advance(connection.nodes.len(), &connection.page_info)?;
        cycles.extend(connection.nodes);
        if !more {
            break;
        }
    }
    let (key, active) = first.expect("the walk fetched at least one page");
    select(&cycles, active.as_ref(), &key, reference)
}

fn check_team(key: &str, enabled: bool, reference: &CycleReference) -> Result<()> {
    if let Some(url_key) = reference.url_team_key()
        && url_key.to_uppercase() != key.to_uppercase()
    {
        return Err(Error::new(format!(
            "That cycle URL is for team {url_key}, but this command is working in team {key}."
        ))
        .with_hint(format!("Pass --team {url_key}.")));
    }
    if !enabled {
        return Err(Error::new(format!("Cycles are not enabled for team {key}"))
            .with_hint("Enable cycles for the team in Linear's settings before filtering or assigning by cycle."));
    }
    Ok(())
}

/// The magnitude and sign of `+N`/`-N`, or `None` for anything else.
fn offset(input: &str) -> Option<(bool, &str)> {
    let (negative, digits) = match input.split_at_checked(1)? {
        ("+", digits) => (false, digits),
        ("-", digits) => (true, digits),
        _ => return None,
    };
    (!digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then_some((negative, digits))
}

fn select(
    cycles: &[LookupCycle],
    active: Option<&ActiveCycle>,
    key: &str,
    reference: &CycleReference,
) -> Result<String> {
    let id = |cycle: &LookupCycle| cycle.id.inner().to_owned();
    let input = match &reference.url {
        Some(CycleUrl {
            selector: CycleSelector::Number(number),
            ..
        }) => {
            return cycles
                .iter()
                .find(|cycle| cycle.number.0 == number.get())
                .map(id)
                .ok_or_else(|| Error::not_found("Cycle", &format!("#{number} in team {key}")));
        }
        Some(CycleUrl {
            selector: CycleSelector::Active,
            ..
        }) => "active",
        Some(CycleUrl {
            selector: CycleSelector::Next,
            ..
        }) => "next",
        None => reference.input.as_str(),
    };
    let keyword = input.to_lowercase();
    match keyword.as_str() {
        "active" | "now" => {
            if let Some(active) = active {
                return Ok(active.id.inner().to_owned());
            }
            let hint = match cycles.iter().find(|cycle| cycle.is_next) {
                Some(next) => format!(
                    "The next cycle (#{}) starts {} — use --cycle next, a cycle number, or a name.",
                    next.number,
                    next.starts_at.0.date_naive()
                ),
                None => SIMPLE_HINT.to_owned(),
            };
            return Err(Error::new(format!("Team {key} has no active cycle")).with_hint(hint));
        }
        "next" => {
            return cycles
                .iter()
                .find(|cycle| cycle.is_next)
                .map(id)
                .ok_or_else(|| {
                    Error::new(format!("Team {key} has no upcoming cycle")).with_hint(SIMPLE_HINT)
                });
        }
        "previous" => {
            return cycles
                .iter()
                .find(|cycle| cycle.is_previous)
                .map(id)
                .ok_or_else(|| {
                    Error::new(format!("Team {key} has no previous cycle")).with_hint(SIMPLE_HINT)
                });
        }
        _ => {}
    }
    if let Some((negative, digits)) = offset(input) {
        let magnitude = digits
            .parse::<u32>()
            .map(i64::from)
            .map_err(|_| Error::new(format!("Cycle offset {input} is out of range")))?;
        let active = active.ok_or_else(|| {
            Error::new(format!(
                "Cannot resolve relative cycle {input}: the team has no active cycle"
            ))
            .with_hint("Use 'next', a cycle number, or a cycle name while no cycle is active.")
        })?;
        let signed = if negative { -magnitude } else { magnitude };
        let target = i64::from(active.number.0) + signed;
        return cycles
            .iter()
            .find(|cycle| i64::from(cycle.number.0) == target)
            .map(id)
            .ok_or_else(|| Error::not_found("Cycle", &format!("{input} (cycle {target})")));
    }
    let number = input.parse::<CycleNumber>();
    if let Some(cycle) = cycles.iter().find(|cycle| {
        cycle
            .name
            .as_deref()
            .is_some_and(|name| name.to_lowercase() == keyword)
            || number
                .as_ref()
                .is_ok_and(|number| cycle.number.0 == number.get())
    }) {
        return Ok(id(cycle));
    }
    // A digits-only reference that names no cycle was meant as a number, so
    // say why it cannot be one rather than reporting it as an unknown name.
    match number {
        Ok(_) => Err(Error::not_found("Cycle", input)),
        Err(error) => match error.problem() {
            CycleNumberProblem::NotDigits => Err(Error::not_found("Cycle", input)),
            CycleNumberProblem::Zero
            | CycleNumberProblem::LeadingZero
            | CycleNumberProblem::TooLarge => Err(Error::new(error.to_string())),
        },
    }
}
