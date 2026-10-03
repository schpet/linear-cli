//! Request and response envelopes for built-in Cynic operations.
//!
//! The request envelope is owned here rather than borrowed from
//! [`cynic::Operation`]'s Serde shape so the bytes on the wire cannot drift
//! with a Cynic upgrade. The response side classifies a body into exactly one
//! outcome: data, GraphQL errors (with or without partial data), an envelope
//! with neither, malformed JSON, or well-formed JSON that does not match the
//! operation's types. HTTP status handling lives in the transport.

use std::error::Error as StdError;
use std::fmt;

use cynic::{GraphQlError, Operation};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use serde_json::error::Category;

use crate::error::Error;

/// The JSON body sent for one GraphQL operation.
///
/// `variables` and `operationName` are omitted when absent.
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
/// `extensions.userPresentableMessage` is preferred when rendering a message;
/// see [`graphql_message`].
pub type ResponseGraphQlError = GraphQlError<Value>;

/// The raw envelope. `data` stays untyped so GraphQL errors are classified
/// before any attempt to decode partial or absent data into an operation type.
#[derive(Debug, Deserialize)]
struct ResponseEnvelope {
    data: Option<Value>,
    errors: Option<Vec<ResponseGraphQlError>>,
}

/// Why a response body did not yield usable operation data.
pub enum ResponseError {
    /// The body was not syntactically valid JSON (or was truncated).
    MalformedJson(serde_json::Error),
    /// The body is not JSON and was not declared as JSON, e.g. an HTML error
    /// page from a proxy.
    NotJson {
        status: reqwest::StatusCode,
        content_type: Option<String>,
        source: serde_json::Error,
    },
    /// The body was valid JSON but did not match the envelope or the
    /// operation's schema-checked types (wrong type, missing non-null field,
    /// unknown enum variant, non-object top level).
    UnexpectedShape(serde_json::Error),
    /// The server returned one or more GraphQL errors.
    ///
    /// `partial_data` is true when a `data` object accompanied the errors. Any
    /// `errors` array is a failure, so partial data is not returned.
    GraphQl {
        errors: Vec<ResponseGraphQlError>,
        partial_data: bool,
    },
    /// Neither `data` nor a non-empty `errors` array was present.
    MissingData,
}

impl fmt::Debug for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedJson(source) => f.debug_tuple("MalformedJson").field(source).finish(),
            Self::NotJson {
                status,
                content_type,
                ..
            } => f
                .debug_struct("NotJson")
                .field("status", status)
                .field("content_type", content_type)
                .finish(),
            Self::UnexpectedShape(source) => {
                f.debug_tuple("UnexpectedShape").field(source).finish()
            }
            Self::GraphQl {
                errors,
                partial_data,
            } => f
                .debug_struct("GraphQl")
                .field("errors", errors)
                .field("partial_data", partial_data)
                .finish(),
            Self::MissingData => f.write_str("MissingData"),
        }
    }
}

impl fmt::Display for ResponseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MalformedJson(source) => write!(f, "response body is not valid JSON: {source}"),
            Self::NotJson {
                status,
                content_type,
                ..
            } => write!(
                f,
                "Linear returned a non-JSON response (HTTP {status}, content type {})",
                content_type.as_deref().unwrap_or("not set")
            ),
            Self::UnexpectedShape(source) => write!(
                f,
                "response JSON did not match the expected operation shape: {source}"
            ),
            Self::GraphQl { errors, .. } => match graphql_message(errors) {
                Some(message) => write!(f, "{message}"),
                None => write!(f, "GraphQL request failed without an error message"),
            },
            Self::MissingData => write!(f, "response contained neither data nor errors"),
        }
    }
}

impl StdError for ResponseError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::MalformedJson(source)
            | Self::UnexpectedShape(source)
            | Self::NotJson { source, .. } => Some(source),
            Self::GraphQl { .. } | Self::MissingData => None,
        }
    }
}

impl From<ResponseError> for Error {
    fn from(error: ResponseError) -> Self {
        let message = error.to_string();
        match error {
            ResponseError::MalformedJson(source) => Error::new(message).with_source(source),
            ResponseError::NotJson { source, .. } => Error::new(message).with_source(source),
            // Valid JSON that contradicts the schema the types were compiled
            // against is a broken contract, not a transport or GraphQL failure.
            ResponseError::UnexpectedShape(source) => Error::new(message).with_source(source),
            ResponseError::GraphQl { .. } | ResponseError::MissingData => Error::new(message),
        }
    }
}

/// Parses a response body into operation data or a classified failure.
///
/// Any non-empty `errors` array is a failure even when `data` is present. The
/// envelope is read with untyped `data` first, so partial data that no longer
/// fits `T` (for example `{"issueUpdate":null}` next to an error) is reported
/// as the GraphQL error it accompanies rather than as a shape failure. `data`
/// is decoded into `T` only when there are no errors.
pub fn parse_response<T: DeserializeOwned>(body: &[u8]) -> Result<T, ResponseError> {
    let envelope: ResponseEnvelope = serde_json::from_slice(body).map_err(classify_json_error)?;
    decode_envelope(envelope)
}

fn decode_envelope<T: DeserializeOwned>(envelope: ResponseEnvelope) -> Result<T, ResponseError> {
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

/// The message shown for GraphQL errors.
///
/// The first error's `extensions.userPresentableMessage`, else its
/// `message`, skipping errors where both are empty; `None` when none has one.
pub fn graphql_message(errors: &[ResponseGraphQlError]) -> Option<String> {
    errors.iter().find_map(|error| {
        error
            .extensions
            .as_ref()
            .and_then(|extensions| extensions.get("userPresentableMessage"))
            .and_then(Value::as_str)
            .filter(|message| !message.is_empty())
            .or_else(|| Some(error.message.as_str()).filter(|message| !message.is_empty()))
            .map(str::to_owned)
    })
}

/// Whether GraphQL errors describe a missing entity.
///
/// Linear's raw message is `Entity not found: <Type>` and its presentable
/// message reads `Could not find referenced <Type>.`; both spellings match.
pub fn is_not_found(errors: &[ResponseGraphQlError]) -> bool {
    graphql_message(errors).is_some_and(|message| {
        let message = message.to_lowercase();
        message.contains("not found") || message.contains("could not find")
    })
}
