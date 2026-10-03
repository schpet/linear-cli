//! HTTP transport for GraphQL operations, the raw `api` command, signed
//! uploads and Markdown image downloads.
//!
//! Each [`GraphQlTransport`] owns one `reqwest` client. Proxies come from the
//! standard `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY`/`NO_PROXY` variables, which
//! reqwest reads itself. Certificates are verified against the operating
//! system's roots and the bundled Mozilla roots, plus any bundle named in
//! [`TransportConfig`].
//!
//! GraphQL responses are captured whole (status, headers, body up to a cap)
//! and then classified: GraphQL `errors` win over the HTTP status, and a
//! non-2xx body without GraphQL errors is an HTTP failure that keeps its bytes.
//!
//! Secrets: every stored `reqwest::Error` has its URL stripped, and failures
//! only print the endpoint's scheme/host/port, never its path or query.
//! `ApiKey` redacts itself in `Debug` and `Display`.

use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::redirect::Policy;
use reqwest::tls::Certificate;
use reqwest::{Client, StatusCode, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;

use crate::error::Error;
use crate::graphql::envelope::{
    GraphQlRequest, ResponseError, ResponseGraphQlError, graphql_message, is_not_found,
    parse_response,
};

/// The `User-Agent` sent on every request.
pub const USER_AGENT_VALUE: &str = concat!("schpet-linear-cli/", env!("CARGO_PKG_VERSION"));

/// `Content-Type` sent on every GraphQL POST.
pub const CONTENT_TYPE_VALUE: &str = "application/json";

/// Redirects followed per request before giving up.
const MAX_REDIRECTS: usize = 20;

// ---------------------------------------------------------------------------
// Endpoint

/// A validated `http` or `https` GraphQL endpoint.
///
/// The full URL (including any path and query) is used for requests; only the
/// origin (`scheme://host[:port]`) is ever displayed, because signed URLs can
/// carry tokens in their query string.
#[derive(Clone, PartialEq, Eq)]
pub struct EndpointUrl {
    url: Url,
    origin: String,
}

/// Why a string is not an acceptable [`EndpointUrl`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EndpointUrlError {
    /// The text is not a URL; carries the parser's description (never the input).
    Invalid(String),
    /// The scheme is neither `http` nor `https`.
    Scheme(String),
    /// The URL has no host.
    MissingHost,
    /// The URL carries a username or password.
    Credentials,
    /// The URL carries a fragment.
    Fragment,
}

impl fmt::Display for EndpointUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(detail) => write!(f, "endpoint URL is not valid: {detail}"),
            Self::Scheme(scheme) => {
                write!(f, "endpoint URL scheme must be http or https, not {scheme}")
            }
            Self::MissingHost => write!(f, "endpoint URL has no host"),
            Self::Credentials => write!(f, "endpoint URL must not carry credentials"),
            Self::Fragment => write!(f, "endpoint URL must not carry a fragment"),
        }
    }
}

impl StdError for EndpointUrlError {}

impl EndpointUrl {
    /// Parses and validates an endpoint. Path and query are preserved.
    pub fn parse(text: &str) -> Result<Self, EndpointUrlError> {
        let url = Url::parse(text).map_err(|error| EndpointUrlError::Invalid(error.to_string()))?;
        Self::from_url(url)
    }

    fn from_url(url: Url) -> Result<Self, EndpointUrlError> {
        match url.scheme() {
            "http" | "https" => {}
            other => return Err(EndpointUrlError::Scheme(other.to_owned())),
        }
        let Some(host) = url.host_str() else {
            return Err(EndpointUrlError::MissingHost);
        };
        if !url.username().is_empty() || url.password().is_some() {
            return Err(EndpointUrlError::Credentials);
        }
        if url.fragment().is_some() {
            return Err(EndpointUrlError::Fragment);
        }
        let origin = match url.port() {
            Some(port) => format!("{}://{host}:{port}", url.scheme()),
            None => format!("{}://{host}", url.scheme()),
        };
        Ok(Self { url, origin })
    }

    /// The full request URL.
    pub fn url(&self) -> &Url {
        &self.url
    }
}

impl fmt::Display for EndpointUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.origin)
    }
}

impl fmt::Debug for EndpointUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("EndpointUrl").field(&self.origin).finish()
    }
}

// ---------------------------------------------------------------------------
// API key

/// A Linear API key, sent verbatim as the `Authorization` header value.
///
/// Redacted in `Debug` and `Display`; the header value is marked sensitive.
#[derive(Clone)]
pub struct ApiKey {
    value: HeaderValue,
}

/// Why a string is not an acceptable [`ApiKey`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ApiKeyError {
    Empty,
    /// A byte outside visible ASCII and space (control characters, CR/LF, tab,
    /// non-ASCII) at this index.
    InvalidByte {
        index: usize,
    },
}

impl fmt::Display for ApiKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "API key is empty"),
            Self::InvalidByte { index } => write!(
                f,
                "API key contains a byte at index {index} that is not printable ASCII"
            ),
        }
    }
}

impl StdError for ApiKeyError {}

impl ApiKey {
    /// Accepts non-empty visible ASCII including spaces; rejects everything
    /// else so the value can never split or inject a header.
    pub fn new(text: String) -> Result<Self, ApiKeyError> {
        if text.is_empty() {
            return Err(ApiKeyError::Empty);
        }
        if let Some(index) = text.bytes().position(|byte| !(0x20..=0x7e).contains(&byte)) {
            return Err(ApiKeyError::InvalidByte { index });
        }
        #[allow(clippy::expect_used, reason = "every byte was checked above")]
        let mut value =
            HeaderValue::from_str(&text).expect("visible ASCII is a valid header value");
        value.set_sensitive(true);
        Ok(Self { value })
    }

    fn header_value(&self) -> HeaderValue {
        self.value.clone()
    }
}

impl fmt::Debug for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ApiKey(<redacted>)")
    }
}

impl fmt::Display for ApiKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("<redacted>")
    }
}

// ---------------------------------------------------------------------------
// Configuration

/// A non-zero total deadline for one GraphQL exchange: connect, send and body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deadline(Duration);

impl Deadline {
    pub const DEFAULT: Self = Self(Duration::from_secs(30));

    pub fn new(duration: Duration) -> Result<Self, ConfigError> {
        if duration.is_zero() {
            return Err(ConfigError::ZeroDeadline);
        }
        Ok(Self(duration))
    }

    pub fn duration(self) -> Duration {
        self.0
    }
}

/// A cap on collected GraphQL response bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponseCap(NonZeroUsize);

impl ResponseCap {
    /// 64 MiB: far above any real Linear page, small enough to stop a runaway body.
    pub const DEFAULT: Self = Self(NonZeroUsize::MIN.saturating_add(64 * 1024 * 1024 - 1));

    pub fn new(bytes: usize) -> Result<Self, ConfigError> {
        NonZeroUsize::new(bytes)
            .map(Self)
            .ok_or(ConfigError::ZeroResponseCap)
    }

    pub fn bytes(self) -> usize {
        self.0.get()
    }
}

/// Invalid [`TransportConfig`] values.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfigError {
    ZeroDeadline,
    ZeroResponseCap,
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDeadline => write!(f, "transport deadline must be greater than zero"),
            Self::ZeroResponseCap => write!(f, "response cap must be greater than zero"),
        }
    }
}

impl StdError for ConfigError {}

/// Everything the transport needs beyond endpoint and key.
#[derive(Clone, Debug)]
pub struct TransportConfig {
    /// A PEM bundle whose certificates are trusted in addition to the
    /// built-in roots (`SSL_CERT_FILE`).
    pub ca_bundle: Option<PathBuf>,
    pub deadline: Deadline,
    pub max_response_bytes: ResponseCap,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            ca_bundle: None,
            deadline: Deadline::DEFAULT,
            max_response_bytes: ResponseCap::DEFAULT,
        }
    }
}

// ---------------------------------------------------------------------------
// Sanitized reqwest errors

/// A `reqwest::Error` with its URL removed before it is stored or chained.
///
/// This is the only way a `reqwest::Error` enters this module's error types.
#[derive(Debug)]
pub struct SanitizedReqwestError(reqwest::Error);

impl SanitizedReqwestError {
    fn new(error: reqwest::Error) -> Self {
        Self(error.without_url())
    }

    /// The innermost message of the source chain (for example the OS
    /// connection error), used to give failures a concrete reason.
    fn root_message(&self) -> String {
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

// ---------------------------------------------------------------------------
// Construction errors

/// Why a [`GraphQlTransport`] could not be built.
#[derive(Debug)]
pub enum TransportBuildError {
    CaRead {
        path: PathBuf,
        source: io::Error,
    },
    CaInvalid {
        path: PathBuf,
        source: SanitizedReqwestError,
    },
    CaEmpty {
        path: PathBuf,
    },
    Client(SanitizedReqwestError),
}

impl fmt::Display for TransportBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CaRead { path, .. } => {
                write!(f, "CA bundle {} could not be read", path.display())
            }
            Self::CaInvalid { path, .. } => {
                write!(f, "CA bundle {} is not a valid PEM bundle", path.display())
            }
            Self::CaEmpty { path } => {
                write!(f, "CA bundle {} contains no certificates", path.display())
            }
            Self::Client(_) => write!(f, "HTTP client could not be built"),
        }
    }
}

impl StdError for TransportBuildError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::CaRead { source, .. } => Some(source),
            Self::CaInvalid { source, .. } | Self::Client(source) => Some(source),
            Self::CaEmpty { .. } => None,
        }
    }
}

impl From<TransportBuildError> for Error {
    fn from(error: TransportBuildError) -> Self {
        let message = match &error {
            TransportBuildError::CaRead { .. }
            | TransportBuildError::CaInvalid { .. }
            | TransportBuildError::CaEmpty { .. } => format!("SSL_CERT_FILE: {error}"),
            TransportBuildError::Client(_) => error.to_string(),
        };
        Error::new(message).with_source(error)
    }
}

fn load_pem_bundle(path: &Path) -> Result<Vec<Certificate>, TransportBuildError> {
    let bytes = fs::read(path).map_err(|source| TransportBuildError::CaRead {
        path: path.to_path_buf(),
        source,
    })?;
    let invalid = |error| TransportBuildError::CaInvalid {
        path: path.to_path_buf(),
        source: SanitizedReqwestError::new(error),
    };
    let certificates = Certificate::from_pem_bundle(&bytes).map_err(invalid)?;
    if certificates.is_empty() {
        return Err(TransportBuildError::CaEmpty {
            path: path.to_path_buf(),
        });
    }
    // `from_pem_bundle` only checks the PEM framing; the DER inside is parsed
    // when a root store is built. Build a throwaway store per certificate so
    // a bad certificate is attributed to the bundle, not to the client.
    for certificate in &certificates {
        Client::builder()
            .tls_built_in_root_certs(false)
            .add_root_certificate(certificate.clone())
            .build()
            .map_err(invalid)?;
    }
    Ok(certificates)
}

/// Builds the transport's client: HTTP/1.1, bounded redirects, no retries,
/// gzip/brotli/deflate responses, proxies from the environment, and trusted
/// roots from the operating system, the bundled Mozilla set and the optional
/// bundle.
fn build_client(config: &TransportConfig) -> Result<Client, TransportBuildError> {
    let mut builder = Client::builder()
        .user_agent(USER_AGENT_VALUE)
        .redirect(Policy::limited(MAX_REDIRECTS))
        .referer(false)
        .retry(reqwest::retry::never());
    if let Some(path) = &config.ca_bundle {
        for certificate in load_pem_bundle(path)? {
            builder = builder.add_root_certificate(certificate);
        }
    }
    builder
        .build()
        .map_err(|error| TransportBuildError::Client(SanitizedReqwestError::new(error)))
}

// ---------------------------------------------------------------------------
// Responses and failures

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
    fn body_text(&self) -> String {
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
fn redact(body: &[u8], secret: &[u8]) -> Vec<u8> {
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
pub enum TransportFailure {
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

impl fmt::Debug for TransportFailure {
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

impl fmt::Display for TransportFailure {
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

impl StdError for TransportFailure {
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

impl TransportFailure {
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

    /// Whether Linear answered that the requested entity does not exist.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::GraphQl { errors, .. } if is_not_found(errors))
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

impl From<TransportFailure> for Error {
    fn from(failure: TransportFailure) -> Self {
        let message = failure.to_string();
        match failure {
            TransportFailure::RequestBody(source) => Error::new(message).with_source(source),
            TransportFailure::GraphQl {
                status,
                errors,
                partial_data,
                ..
            } => {
                // The summary omits arbitrary response extensions, headers and
                // the request URL.
                Error::new(message).with_debug_detail(format!(
                    "GraphQL HTTP {status}; errors={}; partial_data={partial_data}",
                    errors.len()
                ))
            }
            TransportFailure::Response(source) => Error::from(source),
            TransportFailure::Http { response, body } => {
                let error = Error::new(message).with_debug_detail(format!(
                    "HTTP {} body: {}",
                    response.status,
                    response.body_text()
                ));
                match body {
                    HttpBodyShape::Unusable(source) => error.with_source(source),
                    HttpBodyShape::Data => error,
                }
            }
            TransportFailure::ResponseTooLarge { .. } | TransportFailure::Timeout { .. } => {
                Error::new(message)
            }
            TransportFailure::Network { source, .. } => Error::new(message).with_source(source),
        }
    }
}

/// Classifies a captured response for a typed operation.
///
/// GraphQL errors take precedence at any status; other non-2xx responses are
/// HTTP failures. A body that is not JSON at all is reported with its status
/// and content type rather than as a parser error.
pub fn classify_typed<T: DeserializeOwned>(
    response: RawHttpResponse,
) -> Result<T, TransportFailure> {
    let parsed = match parse_response::<T>(&response.body) {
        Err(ResponseError::MalformedJson(source)) if !declares_json(&response.headers) => {
            Err(ResponseError::NotJson {
                status: response.status,
                content_type: content_type(&response.headers),
                source,
            })
        }
        other => other,
    };
    match (parsed, response.status.is_success()) {
        (Ok(data), true) => Ok(data),
        (Ok(_), false) => Err(TransportFailure::Http {
            response: Box::new(response),
            body: HttpBodyShape::Data,
        }),
        (
            Err(ResponseError::GraphQl {
                errors,
                partial_data,
            }),
            _,
        ) => Err(TransportFailure::GraphQl {
            status: response.status,
            headers: response.headers,
            errors,
            partial_data,
        }),
        (Err(error), true) => Err(TransportFailure::Response(error)),
        (Err(error), false) => Err(TransportFailure::Http {
            response: Box::new(response),
            body: HttpBodyShape::Unusable(error),
        }),
    }
}

fn content_type(headers: &HeaderMap) -> Option<String> {
    headers
        .get(CONTENT_TYPE)
        .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
}

fn declares_json(headers: &HeaderMap) -> bool {
    content_type(headers).is_some_and(|value| value.to_ascii_lowercase().contains("json"))
}

// ---------------------------------------------------------------------------
// Exchange helpers

/// A failure below HTTP classification, before the transport attaches its
/// origin.
#[derive(Debug)]
enum ExchangeFailure {
    ResponseTooLarge {
        status: StatusCode,
        limit: ResponseCap,
    },
    Timeout,
    Network {
        phase: NetworkPhase,
        source: SanitizedReqwestError,
    },
}

fn classify_network(error: reqwest::Error) -> ExchangeFailure {
    if error.is_timeout() {
        return ExchangeFailure::Timeout;
    }
    let phase = if error.is_connect() {
        NetworkPhase::Connect
    } else if error.is_request() {
        NetworkPhase::Request
    } else if error.is_body() || error.is_decode() {
        NetworkPhase::Body
    } else {
        NetworkPhase::Other
    };
    ExchangeFailure::Network {
        phase,
        source: SanitizedReqwestError::new(error),
    }
}

/// Reads the body chunk by chunk, stopping before the cap is exceeded.
async fn collect(
    mut response: reqwest::Response,
    limit: ResponseCap,
) -> Result<RawHttpResponse, ExchangeFailure> {
    let status = response.status();
    let headers = std::mem::take(response.headers_mut());
    let too_large = || ExchangeFailure::ResponseTooLarge { status, limit };
    if let Some(declared) = response.content_length()
        && u64::try_from(limit.bytes()).is_ok_and(|cap| declared > cap)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(classify_network)? {
        if body.len() + chunk.len() > limit.bytes() {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(RawHttpResponse {
        status,
        headers,
        body,
    })
}

/// A short, display-safe description of a failed non-GraphQL request.
fn request_error(prefix: &str, error: reqwest::Error) -> Error {
    let error = SanitizedReqwestError::new(error);
    Error::new(format!("{prefix}: {}", error.root_message())).with_source(error)
}

// ---------------------------------------------------------------------------
// Transport

/// One configured HTTP client bound to an endpoint and key.
#[derive(Clone, Debug)]
pub struct GraphQlTransport {
    client: Client,
    endpoint: EndpointUrl,
    api_key: ApiKey,
    deadline: Deadline,
    max_response_bytes: ResponseCap,
}

impl GraphQlTransport {
    pub fn new(
        endpoint: EndpointUrl,
        api_key: ApiKey,
        config: TransportConfig,
    ) -> Result<Self, TransportBuildError> {
        Ok(Self {
            client: build_client(&config)?,
            endpoint,
            api_key,
            deadline: config.deadline,
            max_response_bytes: config.max_response_bytes,
        })
    }

    pub fn endpoint(&self) -> &EndpointUrl {
        &self.endpoint
    }

    /// Downloads an image referenced from Markdown.
    pub async fn download_markdown_image(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.download(url, "Failed to download image").await
    }

    /// Downloads an issue attachment.
    pub async fn download_issue_attachment(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.download(url, "Failed to download").await
    }

    /// GETs an `http(s)` URL. The API key is sent only to Linear's private
    /// upload host; reqwest drops it if a redirect leaves that host.
    async fn download(&self, original: &str, failure_prefix: &str) -> Result<Vec<u8>, Error> {
        let url = Url::parse(original)
            .map_err(|error| Error::new(format!("Invalid URL: '{original}'")).with_source(error))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(Error::new(format!(
                "{failure_prefix}: unsupported URL scheme '{}'",
                url.scheme()
            )));
        }
        let authenticated = url.host_str() == Some("uploads.linear.app");
        let mut request = self.client.get(url);
        if authenticated {
            request = request.header(AUTHORIZATION, self.api_key.header_value());
        }
        let response = request
            .send()
            .await
            .map_err(|error| request_error(failure_prefix, error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::new(format!("{failure_prefix}: {status}")));
        }
        let body = response
            .bytes()
            .await
            .map_err(|error| request_error(failure_prefix, error))?;
        Ok(body.to_vec())
    }

    /// POSTs a raw GraphQL body for the `api` command and returns the status
    /// and body text unclassified, with no deadline or size cap.
    pub async fn fetch_api(&self, body: String) -> Result<(u16, String), Error> {
        let response = self
            .client
            .post(self.endpoint.url.clone())
            .header(AUTHORIZATION, self.api_key.header_value())
            .header(CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE_VALUE))
            .body(body)
            .send()
            .await
            .map_err(|error| {
                request_error(&format!("Request to {} failed", self.endpoint), error)
            })?;
        let status = response.status().as_u16();
        let bytes = response.bytes().await.map_err(|error| {
            request_error(
                "Failed to read API response; the request was sent and may have taken effect",
                error,
            )
        })?;
        Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
    }

    /// Sends a prepared envelope and returns the exact response.
    pub async fn send_request<V: Serialize>(
        &self,
        request: &GraphQlRequest<V>,
    ) -> Result<RawHttpResponse, TransportFailure> {
        let body = serde_json::to_vec(request).map_err(TransportFailure::RequestBody)?;
        let response = self
            .client
            .post(self.endpoint.url.clone())
            .timeout(self.deadline.duration())
            .header(AUTHORIZATION, self.api_key.header_value())
            .header(CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE_VALUE))
            .body(body)
            .send()
            .await
            .map_err(|error| self.failure(classify_network(error)))?;
        collect(response, self.max_response_bytes)
            .await
            .map_err(|failure| self.failure(failure))
    }

    /// Sends a typed operation's envelope and classifies the response.
    pub async fn execute<T: DeserializeOwned, V: Serialize>(
        &self,
        request: &GraphQlRequest<V>,
    ) -> Result<T, TransportFailure> {
        let response = self.send_request(request).await?;
        classify_typed(response).map_err(|failure| match failure {
            TransportFailure::Http { mut response, body } => {
                response.body = redact(&response.body, self.api_key.value.as_bytes());
                TransportFailure::Http { response, body }
            }
            other => other,
        })
    }

    fn failure(&self, failure: ExchangeFailure) -> TransportFailure {
        match failure {
            ExchangeFailure::ResponseTooLarge { status, limit } => {
                TransportFailure::ResponseTooLarge { status, limit }
            }
            ExchangeFailure::Timeout => TransportFailure::Timeout {
                origin: self.endpoint.origin.clone(),
                deadline: self.deadline,
            },
            ExchangeFailure::Network { phase, source } => TransportFailure::Network {
                origin: self.endpoint.origin.clone(),
                phase,
                source,
            },
        }
    }

    /// PUTs a file to a pre-signed upload URL with exactly the headers Linear
    /// returned for it; the API key is never sent. There is no total deadline
    /// because uploads can be large.
    pub async fn put_signed(
        &self,
        url: &str,
        headers: HeaderMap,
        body: Vec<u8>,
    ) -> Result<(), Error> {
        let invalid = || Error::new("Invalid signed upload URL");
        let mut url = Url::parse(url).map_err(|_| invalid())?;
        url.set_fragment(None);
        let target = EndpointUrl::from_url(url).map_err(|_| invalid())?;
        let failed = |reason: String| {
            Error::new(format!(
                "Signed upload to {target} failed: {reason}; the object may already be \
                     stored remotely; no comment or attachment was created"
            ))
        };
        let response = self
            .client
            .put(target.url.clone())
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|error| {
                let error = SanitizedReqwestError::new(error);
                failed(error.root_message()).with_source(error)
            })?;
        if response.status().is_success() {
            return Ok(());
        }
        let response = collect(response, self.max_response_bytes)
            .await
            .map_err(|failure| match failure {
                ExchangeFailure::ResponseTooLarge { limit, .. } => {
                    failed(format!("response exceeded {} bytes", limit.bytes()))
                }
                ExchangeFailure::Timeout => failed("timed out".to_owned()),
                ExchangeFailure::Network { source, .. } => {
                    failed(source.root_message()).with_source(source)
                }
            })?;
        Err(Error::new(format!(
            "Failed to upload file: {} - {}",
            response.status,
            String::from_utf8_lossy(&response.body)
        )))
    }
}
