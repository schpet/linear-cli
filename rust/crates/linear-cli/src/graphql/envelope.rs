//! Request and response envelopes for built-in Cynic operations.
//!
//! The request envelope is owned here rather than borrowed from
//! [`cynic::Operation`]'s Serde shape so the bytes on the wire cannot drift
//! with a Cynic upgrade. The response side classifies a body into exactly one
//! outcome: data, GraphQL errors (with or without partial data), an envelope
//! with neither, malformed JSON, or well-formed JSON that does not match the
//! operation's types. HTTP status handling belongs to F02B.

use std::error::Error;
use std::fmt;

use cynic::{GraphQlError, Operation};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::error::Category;

use crate::error::{AppError, AppErrorKind};

/// The JSON body sent for one GraphQL operation.
///
/// `variables` and `operationName` are omitted when absent, mirroring how the
/// Deno oracle's `JSON.stringify` drops `undefined` keys.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct GraphQlRequest<V> {
    pub query: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub variables: Option<V>,
    #[serde(rename = "operationName", skip_serializing_if = "Option::is_none")]
    pub operation_name: Option<String>,
}

impl<V: Serialize> GraphQlRequest<V> {
    /// Wraps an operation whose variables struct is always sent.
    ///
    /// Individual keys inside `variables` may still be omitted by the variables
    /// struct's own `skip_serializing_if` attributes.
    pub fn with_variables<F>(operation: Operation<F, V>) -> Self {
        Self {
            query: operation.query,
            variables: Some(operation.variables),
            operation_name: operation.operation_name.map(|name| name.into_owned()),
        }
    }
}

impl GraphQlRequest<()> {
    /// Wraps an operation with no variables; the `variables` key is omitted.
    pub fn without_variables<F>(operation: Operation<F, ()>) -> Self {
        Self {
            query: operation.query,
            variables: None,
            operation_name: operation.operation_name.map(|name| name.into_owned()),
        }
    }
}

/// A GraphQL error with Linear's `extensions` retained as JSON.
///
/// `extensions.userPresentableMessage` is what the Deno oracle prefers when
/// rendering a message; see [`graphql_message`].
pub type ResponseGraphQlError = GraphQlError<Value>;

/// The raw envelope. `data` stays untyped so GraphQL errors are classified
/// before any attempt to decode partial or absent data into an operation type.
#[derive(Debug, Deserialize)]
struct ResponseEnvelope {
    data: Option<Value>,
    errors: Option<Vec<ResponseGraphQlError>>,
}

/// Why a response body did not yield usable operation data.
#[derive(Debug)]
pub enum ResponseError {
    /// The body was not syntactically valid JSON (or was truncated).
    MalformedJson(serde_json::Error),
    /// The body was valid JSON but did not match the envelope or the
    /// operation's schema-checked types (wrong type, missing non-null field,
    /// unknown enum variant, non-object top level).
    UnexpectedShape(serde_json::Error),
    /// The server returned one or more GraphQL errors.
    ///
    /// `partial_data` is true when a `data` object accompanied the errors. The
    /// oracle treats any `errors` array as a failure, so partial data is not
    /// returned as a result.
    GraphQl {
        errors: Vec<ResponseGraphQlError>,
        partial_data: bool,
    },
    /// Neither `data` nor a non-empty `errors` array was present.
    MissingData,
    /// A mutation payload reported `success: false`.
    MutationRejected,
    /// A mutation payload succeeded but its entity object was `null`.
    MissingPayloadEntity,
}

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedJson(source) => write!(f, "response body is not valid JSON: {source}"),
            Self::UnexpectedShape(source) => write!(
                f,
                "response JSON did not match the expected operation shape: {source}"
            ),
            Self::GraphQl { errors, .. } => match graphql_message(errors) {
                Some(message) => write!(f, "{message}"),
                None => write!(f, "GraphQL request failed without an error message"),
            },
            Self::MissingData => write!(f, "response contained neither data nor errors"),
            Self::MutationRejected => write!(f, "operation reported success: false"),
            Self::MissingPayloadEntity => write!(f, "operation succeeded but returned no entity"),
        }
    }
}

impl Error for ResponseError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::MalformedJson(source) | Self::UnexpectedShape(source) => Some(source),
            Self::GraphQl { .. }
            | Self::MissingData
            | Self::MutationRejected
            | Self::MissingPayloadEntity => None,
        }
    }
}

impl From<ResponseError> for AppError {
    fn from(error: ResponseError) -> Self {
        let message = error.to_string();
        match error {
            ResponseError::MalformedJson(source) => {
                AppError::new(AppErrorKind::Transport, message).with_source(source)
            }
            // Valid JSON that contradicts the schema the types were compiled
            // against is a broken contract, not a transport or GraphQL failure.
            ResponseError::UnexpectedShape(source) => {
                AppError::new(AppErrorKind::Invariant, message).with_source(source)
            }
            ResponseError::GraphQl { .. }
            | ResponseError::MissingData
            | ResponseError::MutationRejected
            | ResponseError::MissingPayloadEntity => AppError::new(AppErrorKind::GraphQl, message),
        }
    }
}

/// Parses a response body into operation data or a classified failure.
///
/// Classification order matches the oracle's graphql-request client: any
/// non-empty `errors` array is a failure even when `data` is present. The
/// envelope is read with untyped `data` first, so partial data that no longer
/// fits `T` (for example `{"issueUpdate":null}` next to an error) is reported
/// as the GraphQL error it accompanies rather than as a shape failure. `data`
/// is decoded into `T` only when there are no errors.
pub fn parse_response<T: DeserializeOwned>(body: &[u8]) -> Result<T, ResponseError> {
    let envelope: ResponseEnvelope = serde_json::from_slice(body).map_err(classify_json_error)?;
    let errors = envelope.errors.unwrap_or_default();
    if !errors.is_empty() {
        return Err(ResponseError::GraphQl {
            errors,
            partial_data: envelope.data.is_some(),
        });
    }
    match envelope.data {
        Some(data) => serde_json::from_value(data).map_err(ResponseError::UnexpectedShape),
        None => Err(ResponseError::MissingData),
    }
}

/// Separates JSON that cannot be parsed from JSON that parsed but does not
/// fit the envelope. `Io` cannot occur when reading from a slice; it is kept
/// on the malformed side so the match stays exhaustive without a wildcard.
fn classify_json_error(error: serde_json::Error) -> ResponseError {
    match error.classify() {
        Category::Syntax | Category::Eof | Category::Io => ResponseError::MalformedJson(error),
        Category::Data => ResponseError::UnexpectedShape(error),
    }
}

/// The message the Deno oracle shows for GraphQL errors.
///
/// Prefers the first error's `extensions.userPresentableMessage`, then its
/// `message`; returns `None` for an empty error list.
pub fn graphql_message(errors: &[ResponseGraphQlError]) -> Option<String> {
    let first = errors.first()?;
    let presentable = first
        .extensions
        .as_ref()
        .and_then(|extensions| extensions.get("userPresentableMessage"))
        .and_then(Value::as_str)
        .filter(|message| !message.is_empty());
    match presentable {
        Some(message) => Some(message.to_owned()),
        None => Some(first.message.clone()),
    }
}

/// Whether GraphQL errors describe a missing entity, by the oracle's rule.
///
/// Linear's raw message is `Entity not found: <Type>` and its presentable
/// message reads `Could not find referenced <Type>.`; both spellings match.
pub fn is_not_found(errors: &[ResponseGraphQlError]) -> bool {
    graphql_message(errors).is_some_and(|message| {
        let message = message.to_lowercase();
        message.contains("not found") || message.contains("could not find")
    })
}

/// Turns a payload `success` flag into a typed result.
pub fn require_success(success: bool) -> Result<(), ResponseError> {
    if success {
        Ok(())
    } else {
        Err(ResponseError::MutationRejected)
    }
}

/// Turns an optional payload entity into a typed result.
pub fn require_entity<T>(entity: Option<T>) -> Result<T, ResponseError> {
    entity.ok_or(ResponseError::MissingPayloadEntity)
}
