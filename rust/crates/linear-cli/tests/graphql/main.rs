//! Public wire, envelope, transport and pagination contracts for the F02
//! Cynic foundation.
//!
//! Every test inspects serialized JSON, a parsed fixture body or bytes that
//! crossed a loopback socket, never only Rust values, because the contract is
//! what crosses the wire.

mod auth_list;
mod auth_whoami;
mod envelope;
mod network;
mod output;
mod pagination;
mod scalars;
mod transport;
mod union;
mod wire;

mod source_response;
