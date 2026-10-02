//! Characterization tests that drive the `linear` binary against a loopback mock API.
//! Each command group lives in its own module.
#![cfg(unix)]

mod support;

mod auth;
mod config;
mod harness;
