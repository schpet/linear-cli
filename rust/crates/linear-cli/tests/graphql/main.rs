//! Wire format, response classification, transport and pagination.
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

/// An envelope for an arbitrary GraphQL document.
fn raw_request(
    document: &str,
    variables: Option<serde_json::Map<String, serde_json::Value>>,
    operation_name: Option<&str>,
) -> linear_cli::graphql::envelope::GraphQlRequest<serde_json::Map<String, serde_json::Value>> {
    linear_cli::graphql::envelope::GraphQlRequest {
        query: document.to_owned(),
        variables,
        operation_name: operation_name.map(str::to_owned),
    }
}
