//! Public wire and envelope contracts for the F02A Cynic foundation.
//!
//! Every test inspects serialized JSON or a parsed fixture body, never only
//! Rust values, because the contract is what crosses the wire.

mod envelope;
mod output;
mod scalars;
mod union;
mod wire;
