//! Cynic marker types generated from the maintained Linear SDL.

#![forbid(unsafe_code)]
// cynic generates the schema module as a single line of several megabytes,
// and clippy's `four_forward_slashes` lint takes minutes on it
// (rust-lang/rust-clippy#15671). Every other lint together takes seconds.
// The lint looks for `////` comments above items, and the only hand-written
// item here is the module declaration.
#![allow(clippy::four_forward_slashes)]

#[cynic::schema("linear")]
pub mod schema {}
