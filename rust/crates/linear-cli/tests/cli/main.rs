//! Characterization tests that drive the `linear` binary against a loopback mock API.
//! Each command group lives in its own module.
#![cfg(unix)]

mod support;

mod auth;
mod config;
mod cycle;
mod harness;
mod issue_attach;
mod issue_comment;
mod issue_read;
mod issue_vcs;
mod issue_write;
mod project;
mod team;
