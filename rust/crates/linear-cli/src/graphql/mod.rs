//! Typed GraphQL foundation built on Cynic against the committed Linear SDL.
//!
//! `build.rs` registers `graphql/schema.graphql` under the name `linear`; every
//! derive in this module tree is checked against that schema at compile time.
//! F02A owns the wire types; F02B adds the HTTP transport, response
//! classification and cursor pagination on top of them.

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

pub mod schema_defaults;
pub mod schema_introspection;

pub mod source_query;
pub(crate) mod source_response;
