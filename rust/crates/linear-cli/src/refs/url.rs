use reqwest::Url;

use crate::text::js_space;

const TEAM_SUBPAGES: &[&str] = &[
    "overview",
    "all",
    "active",
    "backlog",
    "triage",
    "cycles",
    "projects",
    "projects/all",
    "views/issues",
    "settings",
];
const PROJECT_SUBPAGES: &[&str] = &["overview", "issues", "updates", "activity"];
const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Eq, PartialEq)]
enum SlugKind {
    Project,
    Document,
    Initiative,
}

impl SlugKind {
    const fn label(self) -> &'static str {
        match self {
            Self::Project => "a project",
            Self::Document => "a document",
            Self::Initiative => "an initiative",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinearUrlKind {
    Issue,
    Project,
    Document,
    Initiative,
    Team,
    Cycle,
}

impl LinearUrlKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Issue => "an issue",
            Self::Project => "a project",
            Self::Document => "a document",
            Self::Initiative => "an initiative",
            Self::Team => "a team",
            Self::Cycle => "a cycle",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CycleSelector {
    Number(u64),
    Active,
    Next,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinearUrlRef {
    Issue {
        workspace: String,
        identifier: String,
        comment_id_prefix: Option<String>,
    },
    Project {
        workspace: String,
        slug_id: String,
    },
    Document {
        workspace: String,
        slug_id: String,
    },
    Initiative {
        workspace: String,
        slug_id: String,
    },
    Team {
        workspace: String,
        team_key: String,
    },
    Cycle {
        workspace: String,
        team_key: String,
        cycle: CycleSelector,
    },
}

impl LinearUrlRef {
    pub const fn kind(&self) -> LinearUrlKind {
        match self {
            Self::Issue { .. } => LinearUrlKind::Issue,
            Self::Project { .. } => LinearUrlKind::Project,
            Self::Document { .. } => LinearUrlKind::Document,
            Self::Initiative { .. } => LinearUrlKind::Initiative,
            Self::Team { .. } => LinearUrlKind::Team,
            Self::Cycle { .. } => LinearUrlKind::Cycle,
        }
    }

    pub fn workspace(&self) -> &str {
        match self {
            Self::Issue { workspace, .. }
            | Self::Project { workspace, .. }
            | Self::Document { workspace, .. }
            | Self::Initiative { workspace, .. }
            | Self::Team { workspace, .. }
            | Self::Cycle { workspace, .. } => workspace,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LinearUrlParse {
    NotLinear,
    Unsupported(String),
    Known(LinearUrlRef),
}

fn unsupported(reason: impl Into<String>) -> LinearUrlParse {
    LinearUrlParse::Unsupported(reason.into())
}

fn hex_value(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// `decodeURIComponent` per path segment: invalid escapes and invalid UTF-8 fail.
fn decode_segment(segment: &str) -> Option<String> {
    let bytes = segment.as_bytes();
    let mut decoded = Vec::with_capacity(bytes.len());
    let mut position = 0;
    while let Some(byte) = bytes.get(position).copied() {
        if byte == b'%' {
            let high = hex_value(*bytes.get(position + 1)?)?;
            let low = hex_value(*bytes.get(position + 2)?)?;
            decoded.push((high << 4) | low);
            position += 3;
        } else {
            decoded.push(byte);
            position += 1;
        }
    }
    String::from_utf8(decoded).ok()
}

fn slug_id(segment: &str) -> Option<String> {
    let lower = segment.to_lowercase();
    let candidate = lower.rsplit_once('-').map_or(lower.as_str(), |(_, id)| id);
    (candidate.len() == 12 && candidate.bytes().all(|b| b.is_ascii_hexdigit()))
        .then(|| candidate.to_owned())
}

fn issue_identifier(segment: &str) -> Option<String> {
    let (team, number) = segment.split_once('-')?;
    if team.is_empty()
        || !team.bytes().all(|b| b.is_ascii_alphanumeric())
        || number.is_empty()
        || !number.starts_with(|ch: char| ('1'..='9').contains(&ch))
        || !number.bytes().all(|b| b.is_ascii_digit())
    {
        return None;
    }
    Some(format!("{}-{number}", team.to_ascii_uppercase()))
}

fn comment_anchor(anchor: &str) -> Option<String> {
    let id = anchor.strip_prefix("comment-")?;
    (id.len() == 8
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
    .then(|| id.to_owned())
}

fn project_update_anchor(anchor: &str) -> bool {
    let Some(id) = anchor.strip_prefix("project-update-") else {
        return false;
    };
    id.len() == 8
        && id
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase())
}

fn cycle_path(workspace: String, team_key: String, rest: &[String]) -> LinearUrlParse {
    let Some(segment) = rest.first() else {
        return unsupported("it does not name a cycle");
    };
    if rest.len() > 1 {
        return unsupported(format!("\"{}\" is not a cycle page", rest.join("/")));
    }
    let cycle = match segment.to_lowercase().as_str() {
        "active" => CycleSelector::Active,
        "upcoming" => CycleSelector::Next,
        _ => {
            let valid_number = segment.starts_with(|ch: char| ('1'..='9').contains(&ch))
                && segment.bytes().all(|b| b.is_ascii_digit());
            match valid_number.then(|| segment.parse::<u64>().ok()).flatten() {
                Some(number) if number <= MAX_SAFE_INTEGER => CycleSelector::Number(number),
                _ => return unsupported(format!("\"{segment}\" is not a cycle number")),
            }
        }
    };
    LinearUrlParse::Known(LinearUrlRef::Cycle {
        workspace,
        team_key: team_key.to_uppercase(),
        cycle,
    })
}

/// Classify a URL without network access. Ordinary text stays unchanged for lookup.
pub fn parse_linear_url(value: &str) -> LinearUrlParse {
    let trimmed = value.trim_matches(js_space);
    if trimmed.is_empty() {
        return LinearUrlParse::NotLinear;
    }
    let lowered = trimmed.to_lowercase();
    let scheme_less = ["linear.app/", "www.linear.app/"]
        .iter()
        .any(|host| lowered.starts_with(host));
    let with_scheme = if scheme_less {
        format!("https://{trimmed}")
    } else {
        trimmed.to_owned()
    };
    let Ok(url) = Url::parse(&with_scheme) else {
        return LinearUrlParse::NotLinear;
    };
    if !matches!(url.scheme(), "http" | "https") {
        return LinearUrlParse::NotLinear;
    }
    let Some(host) = url.host_str() else {
        return LinearUrlParse::NotLinear;
    };
    let host = host.strip_suffix('.').unwrap_or(host);
    if !matches!(
        host.to_ascii_lowercase().as_str(),
        "linear.app" | "www.linear.app"
    ) || url.port().is_some()
        || !url.username().is_empty()
        || url.password().is_some_and(|password| !password.is_empty())
    {
        return LinearUrlParse::NotLinear;
    }
    let mut segments = Vec::new();
    for encoded in url.path().split('/').filter(|part| !part.is_empty()) {
        let Some(decoded) = decode_segment(encoded) else {
            return unsupported("its path could not be decoded");
        };
        segments.push(decoded);
    }
    if segments
        .iter()
        .any(|part| matches!(part.as_str(), "." | ".."))
    {
        return unsupported("its path contains relative segments");
    }
    let mut parts = segments.into_iter();
    let (Some(workspace), Some(entity)) = (parts.next(), parts.next()) else {
        return unsupported("it does not name a workspace and an entity");
    };
    let rest: Vec<String> = parts.collect();
    let anchor = url.fragment().unwrap_or("");
    match entity.as_str() {
        "issue" => {
            let Some(raw) = rest.first() else {
                return unsupported("it does not name an issue");
            };
            let Some(identifier) = issue_identifier(raw) else {
                return unsupported(format!("\"{raw}\" is not an issue identifier"));
            };
            let comment_id_prefix = if anchor.is_empty() {
                None
            } else {
                let Some(comment) = comment_anchor(anchor) else {
                    return unsupported(format!("\"#{anchor}\" is not a comment link"));
                };
                Some(comment)
            };
            LinearUrlParse::Known(LinearUrlRef::Issue {
                workspace,
                identifier,
                comment_id_prefix,
            })
        }
        "project" | "document" | "initiative" => {
            let label = match entity.as_str() {
                "project" => SlugKind::Project,
                "document" => SlugKind::Document,
                "initiative" => SlugKind::Initiative,
                _ => return unsupported("it does not name a supported entity"),
            };
            let Some(raw) = rest.first() else {
                return unsupported(format!("it does not name {}", label.label()));
            };
            let Some(id) = slug_id(raw) else {
                return unsupported(format!("\"{raw}\" does not end in a Linear slug ID"));
            };
            let tail = rest
                .iter()
                .skip(1)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("/");
            if !(tail.is_empty()
                || label == SlugKind::Project && PROJECT_SUBPAGES.contains(&tail.as_str()))
            {
                return unsupported(format!("\"{tail}\" is not a page this command can use"));
            }
            if !anchor.is_empty() && !project_update_anchor(anchor) {
                return unsupported(format!("\"#{anchor}\" is not a link this command can use"));
            }
            match label {
                SlugKind::Project => LinearUrlParse::Known(LinearUrlRef::Project {
                    workspace,
                    slug_id: id,
                }),
                SlugKind::Document => LinearUrlParse::Known(LinearUrlRef::Document {
                    workspace,
                    slug_id: id,
                }),
                SlugKind::Initiative => LinearUrlParse::Known(LinearUrlRef::Initiative {
                    workspace,
                    slug_id: id,
                }),
            }
        }
        "team" => {
            let Some(key) = rest.first() else {
                return unsupported("it does not name a team");
            };
            if rest.get(1).is_some_and(|segment| segment == "cycle") {
                return cycle_path(workspace, key.clone(), rest.get(2..).unwrap_or(&[]));
            }
            let tail = rest
                .iter()
                .skip(1)
                .map(String::as_str)
                .collect::<Vec<_>>()
                .join("/");
            if !tail.is_empty() && !TEAM_SUBPAGES.contains(&tail.as_str()) {
                return unsupported(format!(
                    "\"{tail}\" is not a team page this command can use"
                ));
            }
            LinearUrlParse::Known(LinearUrlRef::Team {
                workspace,
                team_key: key.to_uppercase(),
            })
        }
        _ => unsupported(format!(
            "\"{entity}\" is not an entity this command can use"
        )),
    }
}
