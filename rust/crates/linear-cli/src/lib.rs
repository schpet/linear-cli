#![forbid(unsafe_code)]
#![deny(
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

pub mod app;
pub mod cli;
pub mod error;
