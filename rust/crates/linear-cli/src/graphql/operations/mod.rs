//! Built-in operations derived against the full Linear schema, one module
//! per entity. Selection order is the `--json` output order, and operation
//! names match the struct names.

pub mod agent_session;
pub mod comment;
pub mod common;
pub mod cycle;
pub mod document;
pub mod initiative;
pub mod issue;
pub mod issue_read;
pub mod label;
pub mod milestone;
pub mod project;
pub mod release;
pub mod status_update;
pub mod team;
pub mod template;
pub mod upload;
pub mod user;
