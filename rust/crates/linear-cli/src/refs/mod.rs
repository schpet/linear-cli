//! Linear URL classification, local workspace checks and typed team lookup.
mod project;
mod team;
mod url;
mod uuid;
mod workspace;

pub use project::{ProjectReference, prepare_project_lookup, resolve_project_with_transport};
pub use team::{
    PreparedTeamLookup, ResolvedTeam, find_team, prepare_team_lookup, resolve_team,
    resolve_team_with_transport,
};
pub use url::{CycleSelector, LinearUrlKind, LinearUrlParse, LinearUrlRef, parse_linear_url};
pub use uuid::is_linear_uuid;
pub use workspace::{
    WorkspaceScope, expect_team_url, expect_url_kind, reject_comment_url, reject_linear_url,
};
mod document;
mod initiative;
pub use document::resolve_document_reference;
pub use initiative::{
    InitiativeReference, prepare_initiative_lookup, resolve_initiative_with,
    resolve_initiative_with_transport,
};
