//! Characterization tests that drive the `linear` binary against a loopback mock API.
//! Each command group lives in its own module.
#![cfg(unix)]

mod support;

mod api;
mod auth;
mod config;
mod cycle;
mod document;
mod harness;
mod initiative;
mod issue_attach;
mod issue_comment;
mod issue_read;
mod issue_vcs;
mod issue_write;
mod label;
mod milestone;
mod misc;
mod project;
mod status_update;
mod team;
mod template;
mod user;
mod web;
