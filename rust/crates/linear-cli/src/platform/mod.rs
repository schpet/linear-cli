//! Operating-system integration: terminals, subprocesses, prompts, pagers,
//! editors, version control and Markdown rendering.

pub mod child;
pub mod collation;
pub mod editor;
pub mod gh;
pub mod markdown_assets;
pub mod markdown_terminal;
pub mod opener;
pub mod output;
pub mod pager;
pub mod private_file;
pub mod process;
pub mod prompt;
pub mod spinner;
pub mod style;
pub mod vcs;
