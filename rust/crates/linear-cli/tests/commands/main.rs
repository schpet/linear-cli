mod auth_list;
mod auth_whoami;
mod comment_add;
mod cycle_view;
mod delete_server;
mod document_comment_list;
mod initiative_comment_list;
mod initiative_create;
mod initiative_list;
mod initiative_update_list;
mod initiative_view;
mod label_create;
mod label_delete;
mod label_list;
mod project_comment_list;
mod project_delete;
mod project_list;
mod project_update_list;
mod project_view;
mod prosemirror;
mod relative_time;
mod template_json;
mod template_list;
mod template_view;
mod user_list;

mod initiative_unarchive;

mod agent_session;

mod initiative_projects;

mod initiative_bulk;

mod document_reads;
mod markdown_assets;

mod markdown_download;

mod auth_local;

mod delete_pair;

mod document_write;

mod project_create;

mod project_update;

mod project_write_server;

mod issue_archive_delete;

mod update_create;

mod initiative_update;

mod issue_read;

mod issue_start_pr;

mod issue_write;

mod issue_write_phase_checkbox;

mod source_response_effects;

use linear_cli::auth::keyring::KeyringReader;
use linear_cli::auth::{CredentialManifest, CredentialStore, LookupResult};

/// Runs `future` on a fresh current-thread runtime.
pub fn block_on_network<T>(
    future: impl std::future::Future<Output = linear_cli::error::Result<T>>,
) -> linear_cli::error::Result<T> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("runtime")
        .block_on(future)
}

/// Parses `words` (without the program name) with the real grammar.
pub fn parse(words: &[std::ffi::OsString]) -> Result<linear_cli::cli::Cli, clap::Error> {
    use clap::Parser;
    linear_cli::cli::Cli::try_parse_from(
        std::iter::once(std::ffi::OsString::from("linear")).chain(words.iter().cloned()),
    )
}

/// A canned keyring answer for one workspace.
pub struct LookupReply {
    pub workspace: String,
    pub result: LookupResult,
}

struct Replies(Vec<LookupReply>);

impl KeyringReader for Replies {
    fn lookup(&self, workspace: &str) -> LookupResult {
        self.0
            .iter()
            .find(|reply| reply.workspace == workspace)
            .map_or(LookupResult::Miss, |reply| reply.result.clone())
    }
}

/// A store whose keyring answers with `replies`.
pub fn hydrate(
    manifest: CredentialManifest,
    replies: Vec<LookupReply>,
) -> Result<CredentialStore, std::convert::Infallible> {
    Ok(CredentialStore::new(manifest, Box::new(Replies(replies))))
}
