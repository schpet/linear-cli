//! How requests fail: sanitized `reqwest` errors, captured responses and the
//! classified [`RequestError`].

use std::error::Error as StdError;
use std::fmt;

use reqwest::StatusCode;
use reqwest::header::HeaderMap;

use super::config::{Deadline, ResponseCap};
use super::content_type;
use crate::error::{Error, Failure};
use crate::graphql::envelope::{
    ResponseError, ResponseGraphQlError, graphql_failure, graphql_message,
};

/// A `reqwest::Error` with its URL removed before it is stored or chained.
///
/// This is the only way a `reqwest::Error` enters this module's error types.
#[derive(Debug)]
pub struct SanitizedReqwestError(reqwest::Error);

impl SanitizedReqwestError {
    pub(super) fn new(error: reqwest::Error) -> Self {
        Self(error.without_url())
    }

    /// The innermost message of the source chain (for example the OS
    /// connection error), used to give failures a concrete reason.
    pub(super) fn root_message(&self) -> String {
        let mut current: &(dyn StdError + 'static) = &self.0;
        while let Some(next) = current.source() {
            current = next;
        }
        current.to_string()
    }
}

impl fmt::Display for SanitizedReqwestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}

impl StdError for SanitizedReqwestError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        self.0.source()
    }
}

/// One HTTP response exactly as received: status, headers and body bytes.
///
/// `Debug` prints sizes, not header values or body bytes.
#[derive(Clone, PartialEq, Eq)]
pub struct RawHttpResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Vec<u8>,
}

impl fmt::Debug for RawHttpResponse {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RawHttpResponse")
            .field("status", &self.status)
            .field("headers", &format_args!("<{} headers>", self.headers.len()))
            .field("body", &format_args!("<{} bytes>", self.body.len()))
            .finish()
    }
}

impl RawHttpResponse {
    /// The body as display-safe text: invalid UTF-8 replaced, control
    /// characters (including terminal escapes) dropped and whitespace runs
    /// collapsed to one space.
    pub(super) fn body_text(&self) -> String {
        String::from_utf8_lossy(&self.body)
            .split_whitespace()
            .map(|word| word.chars().filter(|c| !c.is_control()).collect::<String>())
            .filter(|word| !word.is_empty())
            .collect::<Vec<_>>()
            .join(" ")
    }
}

/// Longest body excerpt shown in an HTTP failure message; `LINEAR_DEBUG`
/// shows the whole body.
const BODY_EXCERPT_CHARS: usize = 200;

/// A short excerpt of a non-2xx body worth showing next to its status: plain
/// text or unusable JSON (say, a proxy's or rate limiter's explanation).
/// Omitted for HTML pages, well-formed data envelopes, empty bodies and
/// bodies that only repeat the status reason.
fn body_excerpt(response: &RawHttpResponse, body: &HttpBodyShape) -> Option<String> {
    match body {
        HttpBodyShape::Data => return None,
        HttpBodyShape::Unusable(_) => {}
    }
    if content_type(&response.headers)
        .is_some_and(|value| value.to_ascii_lowercase().contains("html"))
    {
        return None;
    }
    let text = response.body_text();
    if text.is_empty()
        || response
            .status
            .canonical_reason()
            .is_some_and(|reason| reason.eq_ignore_ascii_case(&text))
    {
        return None;
    }
    let mut chars = text.chars();
    let excerpt: String = chars.by_ref().take(BODY_EXCERPT_CHARS).collect();
    Some(match chars.next() {
        Some(_) => format!("{excerpt}…"),
        None => excerpt,
    })
}

/// Replaces every occurrence of `secret` in `body`, so a server or proxy
/// that echoes the request cannot leak the key into an error message.
pub(super) fn redact(body: &[u8], secret: &[u8]) -> Vec<u8> {
    let mut redacted = Vec::with_capacity(body.len());
    let mut rest = body;
    while let Some(at) = rest
        .windows(secret.len())
        .position(|window| window == secret)
    {
        let (before, matched) = rest.split_at(at);
        redacted.extend_from_slice(before);
        redacted.extend_from_slice(b"<redacted>");
        rest = matched
            .get(secret.len()..)
            .expect("the match starts a window of the secret's length");
    }
    redacted.extend_from_slice(rest);
    redacted
}

/// What a non-2xx body without GraphQL errors contained.
#[derive(Debug)]
pub enum HttpBodyShape {
    /// A well-formed envelope carrying `data` and no errors.
    Data,
    /// Malformed JSON, a non-envelope shape, or neither `data` nor `errors`.
    Unusable(ResponseError),
}

/// Which stage of the exchange a network error came from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkPhase {
    Connect,
    Request,
    Body,
    Other,
}

impl NetworkPhase {
    fn describe(self) -> &'static str {
        match self {
            Self::Connect => "connection to",
            Self::Request => "request to",
            Self::Body => "reading the response from",
            Self::Other => "exchange with",
        }
    }
}

/// Every way a GraphQL exchange can fail after construction.
///
/// `Debug` prints the response header count, never header names or values,
/// like [`RawHttpResponse`].
pub enum RequestError {
    /// The request envelope (usually its variables) could not be serialized.
    RequestBody(serde_json::Error),
    /// The body carried a non-empty GraphQL `errors` array, whatever the status.
    GraphQl {
        status: StatusCode,
        headers: HeaderMap,
        errors: Vec<ResponseGraphQlError>,
        partial_data: bool,
    },
    /// Non-2xx without GraphQL errors; the exact response is retained (boxed
    /// to keep the error small).
    Http {
        response: Box<RawHttpResponse>,
        body: HttpBodyShape,
    },
    /// 2xx whose body is not JSON, not an envelope, has neither `data` nor
    /// `errors`, or does not match the operation's types.
    Response(ResponseError),
    /// The body exceeded the configured cap; nothing partial is returned.
    ResponseTooLarge {
        status: StatusCode,
        limit: ResponseCap,
    },
    /// The total deadline elapsed before the body was fully read.
    Timeout { origin: String, deadline: Deadline },
    /// Connection, request or body-read failure below HTTP.
    Network {
        origin: String,
        phase: NetworkPhase,
        source: SanitizedReqwestError,
    },
}

impl fmt::Debug for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestBody(source) => f.debug_tuple("RequestBody").field(source).finish(),
            Self::GraphQl {
                status,
                headers,
                errors,
                partial_data,
            } => f
                .debug_struct("GraphQl")
                .field("status", status)
                .field("headers", &format_args!("<{} headers>", headers.len()))
                .field("errors", errors)
                .field("partial_data", partial_data)
                .finish(),
            Self::Http { response, body } => f
                .debug_struct("Http")
                .field("response", response)
                .field("body", body)
                .finish(),
            Self::Response(source) => f.debug_tuple("Response").field(source).finish(),
            Self::ResponseTooLarge { status, limit } => f
                .debug_struct("ResponseTooLarge")
                .field("status", status)
                .field("limit", limit)
                .finish(),
            Self::Timeout { origin, deadline } => f
                .debug_struct("Timeout")
                .field("origin", origin)
                .field("deadline", deadline)
                .finish(),
            Self::Network {
                origin,
                phase,
                source,
            } => f
                .debug_struct("Network")
                .field("origin", origin)
                .field("phase", phase)
                .field("source", source)
                .finish(),
        }
    }
}

impl fmt::Display for RequestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::RequestBody(source) => {
                write!(f, "request could not be serialized: {source}")
            }
            Self::GraphQl { errors, .. } => match graphql_message(errors) {
                Some(message) => f.write_str(&message),
                None => write!(f, "GraphQL request failed without an error message"),
            },
            Self::Http { response, body } => {
                write!(f, "unexpected HTTP status {}", response.status)?;
                match body_excerpt(response, body) {
                    Some(excerpt) => write!(f, ": {excerpt}"),
                    None => Ok(()),
                }
            }
            Self::Response(source) => fmt::Display::fmt(source, f),
            Self::ResponseTooLarge { status, limit } => write!(
                f,
                "response body exceeds the {} byte limit (HTTP {status})",
                limit.bytes()
            ),
            Self::Timeout { origin, deadline } => write!(
                f,
                "request to {origin} did not complete within {:?}",
                deadline.duration()
            ),
            Self::Network {
                origin,
                phase,
                source,
            } => write!(
                f,
                "{} {origin} failed: {}",
                phase.describe(),
                source.root_message()
            ),
        }
    }
}

impl StdError for RequestError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::RequestBody(source) => Some(source),
            Self::Response(source) => Some(source),
            Self::Http {
                body: HttpBodyShape::Unusable(source),
                ..
            } => Some(source),
            Self::Network { source, .. } => Some(source),
            Self::Http {
                body: HttpBodyShape::Data,
                ..
            }
            | Self::GraphQl { .. }
            | Self::ResponseTooLarge { .. }
            | Self::Timeout { .. } => None,
        }
    }
}

/// The failure class of a response with `status` and GraphQL `errors`
/// (empty when the body had none or could not be read).
///
/// HTTP 401 and 403 are [`Failure::Auth`] and HTTP 408, 429 and 5xx are
/// [`Failure::Unavailable`], combined with the class of the errors; another
/// status, such as a 404 from a misconfigured endpoint, adds nothing beyond
/// [`Failure::General`]. A recognized missing entity under HTTP 400 is still
/// [`Failure::NotFound`].
pub fn classify_failure(status: StatusCode, errors: &[ResponseGraphQlError]) -> Failure {
    let by_status = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Some(Failure::Auth),
        StatusCode::REQUEST_TIMEOUT | StatusCode::TOO_MANY_REQUESTS => Some(Failure::Unavailable),
        status if status.is_server_error() => Some(Failure::Unavailable),
        _ => None,
    };
    Failure::fold(by_status.into_iter().chain(graphql_failure(errors))).unwrap_or(Failure::General)
}

impl RequestError {
    /// Whether the request may have reached Linear and taken effect anyway: a
    /// timeout, a network failure after connecting (a reset after the request
    /// was written surfaces as a request-phase error), or an undecodable
    /// success response. Errors Linear reported mean nothing changed.
    pub fn outcome_unknown(&self) -> bool {
        match self {
            Self::Timeout { .. } | Self::Response(_) => true,
            Self::Network { phase, .. } => !matches!(phase, NetworkPhase::Connect),
            Self::ResponseTooLarge { status, .. } => status.is_success(),
            Self::RequestBody(_) | Self::GraphQl { .. } | Self::Http { .. } => false,
        }
    }

    /// The failure class this ends a command with.
    pub fn failure(&self) -> Failure {
        match self {
            Self::GraphQl { status, errors, .. } => classify_failure(*status, errors),
            Self::Http { response, .. } => classify_failure(response.status, &[]),
            Self::ResponseTooLarge { status, .. } => classify_failure(*status, &[]),
            Self::Timeout { .. } | Self::Network { .. } => Failure::Unavailable,
            Self::RequestBody(_) | Self::Response(_) => Failure::General,
        }
    }

    /// Whether Linear answered that the requested entity does not exist, and
    /// nothing worse: a missing entity next to a rejected key is not "absent".
    pub fn is_not_found(&self) -> bool {
        self.failure() == Failure::NotFound
    }

    /// [`Error::not_found`] for `entity` `identifier` when Linear answered
    /// that it does not exist, else this failure as an error.
    pub fn or_not_found(self, entity: &str, identifier: &str) -> Error {
        if self.is_not_found() {
            Error::not_found(entity, identifier)
        } else {
            Error::from(self)
        }
    }

    /// The error for a failed create, noting that `entity` may already exist
    /// when the outcome is unknown. Creates are never retried.
    pub fn into_create_error(self, entity: &str) -> Error {
        let uncertain = self.outcome_unknown();
        let mut error = Error::from(self);
        if uncertain {
            error.push_message(&format!("; {entity} may already exist"));
        }
        error
    }
}

impl From<RequestError> for Error {
    fn from(failure: RequestError) -> Self {
        let message = failure.to_string();
        let class = failure.failure();
        let error = |message| Error::failed(class, message);
        match failure {
            RequestError::RequestBody(source) => error(message).with_source(source),
            RequestError::GraphQl {
                status,
                errors,
                partial_data,
                ..
            } => {
                // The summary omits arbitrary response extensions, headers and
                // the request URL.
                error(message).with_debug_detail(format!(
                    "GraphQL HTTP {status}; errors={}; partial_data={partial_data}",
                    errors.len()
                ))
            }
            // Only a 2xx body that is not usable data: always general.
            RequestError::Response(source) => Error::from(source),
            RequestError::Http { response, body } => {
                let error = error(message).with_debug_detail(format!(
                    "HTTP {} body: {}",
                    response.status,
                    response.body_text()
                ));
                match body {
                    HttpBodyShape::Unusable(source) => error.with_source(source),
                    HttpBodyShape::Data => error,
                }
            }
            RequestError::ResponseTooLarge { .. } | RequestError::Timeout { .. } => error(message),
            RequestError::Network { source, .. } => error(message).with_source(source),
        }
    }
}
