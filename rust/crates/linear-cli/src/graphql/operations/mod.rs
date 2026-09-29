//! Built-in operations derived against the full Linear schema.
//!
//! Each module mirrors one Deno document field-for-field and in selection
//! order, so the query text and the `--json` output shape both follow the
//! oracle. Rust operation structs carry the Deno operation names.

pub mod agent_session;
pub mod auth_list;
pub mod auth_whoami;
pub mod cycle_view;
pub mod cycles;
pub mod issue_labels;
pub mod issue_update;
pub mod milestones;
pub mod organization_members;
pub mod project_view;
pub mod projects;
pub mod team_members;
pub mod team_resolver;
pub mod teams;
pub mod templates;
pub mod workflow_states;
