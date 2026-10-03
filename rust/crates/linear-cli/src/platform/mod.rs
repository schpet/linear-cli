//! Operating-system integration: terminals, subprocesses, prompts, pagers,
//! editors, version control and Markdown rendering.

pub mod child;
pub mod collation;
pub mod editor;
pub mod gh_script;
pub mod markdown_assets;
pub mod markdown_terminal;
pub mod opener;
pub mod output;
pub mod pager;
pub mod prompt;
pub mod spinner;
pub mod style;
pub mod vcs;
pub mod vcs_script;
