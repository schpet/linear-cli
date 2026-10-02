#![forbid(unsafe_code)]
#![deny(
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod app;
pub mod auth;
pub mod cli;
pub mod commands;
pub mod config;
pub mod ctx;
pub mod error;
pub mod graphql;
pub mod platform;
pub mod refs;
pub mod workflow_states;
