#![forbid(unsafe_code)]
#![deny(
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod app;
mod auth;
pub mod cli;
mod client;
mod commands;
mod config;
mod ctx;
mod error;
mod graphql;
mod platform;
mod refs;
mod workflow_states;
