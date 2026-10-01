//! Built-in operations derived against the full Linear schema.
//!
//! Each module mirrors one Deno document field-for-field and in selection
//! order, so the query text and the `--json` output shape both follow the
//! oracle. Rust operation structs carry the Deno operation names.

pub mod agent_session;
pub mod auth_list;
pub mod auth_whoami;
pub mod comment_create;
pub mod comment_delete;
pub mod comments;
pub mod cycle_view;
pub mod cycles;
pub mod document_comments;
pub mod initiative_comments;
pub mod initiative_create;
pub mod initiative_reference;
pub mod initiative_updates;
pub mod initiative_view;
pub mod initiatives;
pub mod issue_labels;
pub mod issue_update;
pub mod milestone_create;
pub mod milestone_delete;
pub mod milestone_update;
pub mod milestone_view;
pub mod milestones;
pub mod organization_members;
pub mod project_comments;
pub mod project_delete;
pub mod project_updates;
pub mod project_view;
pub mod projects;
pub mod team_create;
pub mod team_members;
pub mod team_resolver;
pub mod teams;
pub mod templates;
pub mod workflow_states;

pub mod issue_details;

pub mod label_create;
pub mod label_delete;

pub mod initiative_unarchive;

pub mod issue_relations;

pub mod issue_link;

pub mod issue_id;

pub mod initiative_projects;

pub mod initiative_bulk;

pub mod issue_comments;

pub mod upload;

pub mod documents;
pub mod releases;

pub mod document_delete;
pub mod team_delete;

pub mod document_write;

pub mod project_write;

pub mod issue_archive_delete;

pub mod update_create;

pub mod initiative_update;
