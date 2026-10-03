mod comment_add;
mod cycle_view;
mod prosemirror;
mod relative_time;
mod release_lookup;

mod agent_session;

mod markdown_assets;

mod markdown_download;

mod delete_pair;

mod project_write_server;

mod issue_archive_delete;

mod issue_read;

mod issue_start_pr;

mod issue_write;

mod source_response_effects;

/// Parses `words` (without the program name) with the real grammar.
pub fn parse(words: &[std::ffi::OsString]) -> Result<linear_cli::cli::Cli, clap::Error> {
    use clap::Parser;
    linear_cli::cli::Cli::try_parse_from(
        std::iter::once(std::ffi::OsString::from("linear")).chain(words.iter().cloned()),
    )
}
