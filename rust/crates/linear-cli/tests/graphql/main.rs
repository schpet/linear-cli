//! Public wire, envelope, transport and pagination contracts for the F02
//! Cynic foundation.
//!
//! Every test inspects serialized JSON, a parsed fixture body or bytes that
//! crossed a loopback socket, never only Rust values, because the contract is
//! what crosses the wire.

mod envelope;
mod output;
mod pagination;
mod scalars;
mod transport;
mod union;
mod wire;
