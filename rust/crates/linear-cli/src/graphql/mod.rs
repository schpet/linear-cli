//! Typed GraphQL foundation built on Cynic against the committed Linear SDL.
//!
//! `build.rs` registers `graphql/schema.graphql` under the name `linear`; every
//! derive in this module tree is checked against that schema at compile time.

pub mod edit;
pub mod envelope;
pub mod operations;
pub mod pagination;
pub mod scalars;
pub mod transport;

/// Cynic marker types generated from the registered Linear schema.
///
/// The module must be reachable as `schema` from every derive site; operation
/// modules import it with `use crate::graphql::schema;`.
pub use linear_schema::schema;

pub mod bulk_error;

pub(crate) mod source_response;
