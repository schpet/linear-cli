//! HTTP transport for GraphQL operations (F02B Gate 1).
//!
//! One `reqwest` client per [`GraphQlTransport`], built once from already
//! validated [`EndpointUrl`], [`ApiKey`] and [`TransportConfig`] values.
//! Credential, workspace and endpoint-override resolution belong to F03/F04;
//! this module never reads the environment.
//!
//! Every response is captured first (status, headers, exact body bytes up to a
//! finite cap) and classified afterwards, so the raw `api` command can
//! reproduce its own handling of any status while typed built-ins get the
//! F02A envelope classification. GraphQL `errors` win over HTTP status; a
//! non-2xx body without GraphQL errors is an HTTP failure that keeps its bytes.
//!
//! Secrets: every stored `reqwest::Error` goes through
//! [`SanitizedReqwestError::new`] (which strips the URL) and failures only ever
//! print the endpoint's scheme/host/port, never its path, query or fragment.
//! `ApiKey` redacts itself in `Debug` and `Display`.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io;
use std::net::IpAddr;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, USER_AGENT};
use reqwest::redirect::Policy;
use reqwest::tls::Certificate;
use reqwest::{Client, NoProxy, Proxy, StatusCode, Url};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};

use crate::error::{AppError, AppErrorKind};
use crate::graphql::envelope::{
    GraphQlRequest, ResponseError, ResponseGraphQlError, graphql_message, parse_response,
};

/// The `User-Agent` sent on every request; pinned to the frozen oracle's
/// `schpet-linear-cli/2.6.0` while `Cargo.toml` carries the parity version.
pub const USER_AGENT_VALUE: &str = concat!("schpet-linear-cli/", env!("CARGO_PKG_VERSION"));

/// `Content-Type` sent on every GraphQL POST.
pub const CONTENT_TYPE_VALUE: &str = "application/json";

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

impl Error for EndpointUrlError {}

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

    /// `scheme://host[:port]`, the only part shown in messages.
    pub fn origin(&self) -> &str {
        &self.origin
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
    /// The bytes passed the scan but the HTTP header type still rejected
    /// them. Cannot occur after the scan; kept so there is no panic path.
    Unrepresentable,
}

impl fmt::Display for ApiKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "API key is empty"),
            Self::InvalidByte { index } => write!(
                f,
                "API key contains a byte at index {index} that is not printable ASCII"
            ),
            Self::Unrepresentable => write!(f, "API key is not a valid HTTP header value"),
        }
    }
}

impl Error for ApiKeyError {}

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
        let mut value = HeaderValue::from_str(&text).map_err(|_| ApiKeyError::Unrepresentable)?;
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

/// A finite, non-zero total deadline covering connect, send and body read.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deadline(Duration);

impl Deadline {
    /// Production default proposed by the addendum.
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

/// A finite cap on collected response bytes. There is no unbounded mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponseCap(NonZeroUsize);

impl ResponseCap {
    /// 8 MiB, the default the addendum fixes.
    pub const DEFAULT_BYTES: usize = 8 * 1024 * 1024;
    /// 64 MiB, the reviewed ceiling an explicit caller override may reach.
    pub const MAX_BYTES: usize = 64 * 1024 * 1024;
    /// `MIN + (DEFAULT_BYTES - 1)` is exactly `DEFAULT_BYTES`; written this way
    /// because `NonZeroUsize::new` returns an `Option` that a const cannot
    /// unwrap without a panic path.
    pub const DEFAULT: Self = Self(NonZeroUsize::MIN.saturating_add(Self::DEFAULT_BYTES - 1));

    pub fn new(bytes: usize) -> Result<Self, ConfigError> {
        let Some(value) = NonZeroUsize::new(bytes) else {
            return Err(ConfigError::ZeroResponseCap);
        };
        if bytes > Self::MAX_BYTES {
            return Err(ConfigError::ResponseCapAboveCeiling {
                requested: bytes,
                ceiling: Self::MAX_BYTES,
            });
        }
        Ok(Self(value))
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
    ResponseCapAboveCeiling { requested: usize, ceiling: usize },
}

impl fmt::Display for ConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroDeadline => write!(f, "transport deadline must be greater than zero"),
            Self::ZeroResponseCap => write!(f, "response cap must be greater than zero"),
            Self::ResponseCapAboveCeiling { requested, ceiling } => write!(
                f,
                "response cap {requested} bytes exceeds the reviewed ceiling of {ceiling} bytes"
            ),
        }
    }
}

impl Error for ConfigError {}

/// An `http://` proxy URL for HTTPS `CONNECT`, restricted to a loopback host
/// because the only mode in scope is the confined P03C test lane.
#[derive(Clone, PartialEq, Eq)]
pub struct ProxyUrl {
    url: Url,
    origin: String,
}

/// Why a string is not an acceptable [`ProxyUrl`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProxyUrlError {
    Invalid(String),
    Scheme(String),
    NotLoopback,
    Credentials,
    Path,
    Query,
    Fragment,
}

impl fmt::Display for ProxyUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(detail) => write!(f, "proxy URL is not valid: {detail}"),
            Self::Scheme(scheme) => write!(f, "proxy URL scheme must be http, not {scheme}"),
            Self::NotLoopback => write!(f, "proxy URL host must be a loopback address"),
            Self::Credentials => write!(f, "proxy URL must not carry credentials"),
            Self::Path => write!(f, "proxy URL must not carry a path"),
            Self::Query => write!(f, "proxy URL must not carry a query"),
            Self::Fragment => write!(f, "proxy URL must not carry a fragment"),
        }
    }
}

impl Error for ProxyUrlError {}

impl ProxyUrl {
    pub fn parse(text: &str) -> Result<Self, ProxyUrlError> {
        let url = Url::parse(text).map_err(|error| ProxyUrlError::Invalid(error.to_string()))?;
        if url.scheme() != "http" {
            return Err(ProxyUrlError::Scheme(url.scheme().to_owned()));
        }
        if !url.host_str().is_some_and(is_loopback_host) {
            return Err(ProxyUrlError::NotLoopback);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ProxyUrlError::Credentials);
        }
        if !matches!(url.path(), "" | "/") {
            return Err(ProxyUrlError::Path);
        }
        if url.query().is_some() {
            return Err(ProxyUrlError::Query);
        }
        if url.fragment().is_some() {
            return Err(ProxyUrlError::Fragment);
        }
        let origin = match url.port() {
            Some(port) => format!("http://{}:{port}", url.host_str().unwrap_or_default()),
            None => format!("http://{}", url.host_str().unwrap_or_default()),
        };
        Ok(Self { url, origin })
    }

    pub fn origin(&self) -> &str {
        &self.origin
    }
}

/// `localhost`, `127.0.0.0/8` or `[::1]` as `Url::host_str` spells them.
fn is_loopback_host(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    let bare = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host);
    bare.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

impl fmt::Debug for ProxyUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ProxyUrl").field(&self.origin).finish()
    }
}

/// How outbound connections are routed.
#[derive(Clone, Debug)]
pub enum ProxyMode {
    /// Explicit `no_proxy()`: ambient proxy variables are ignored.
    Direct,
    /// HTTPS requests go through `CONNECT` on `url`; loopback destinations
    /// bypass the proxy when `bypass_loopback` is set. Plain `http` requests
    /// are never proxied by this mode.
    HttpsConnect {
        url: ProxyUrl,
        bypass_loopback: bool,
    },
}

/// Which roots verify TLS peers.
#[derive(Clone, Debug)]
pub enum CaMode {
    /// The bundled WebPKI roots only.
    PublicRoots,
    /// The bundled roots plus every certificate in this PEM bundle, read and
    /// validated once at construction.
    PublicRootsPlusPem(PathBuf),
}

/// Everything the transport needs beyond endpoint and key.
#[derive(Clone, Debug)]
pub struct TransportConfig {
    pub proxy: ProxyMode,
    pub ca: CaMode,
    pub deadline: Deadline,
    pub max_response_bytes: ResponseCap,
}

impl TransportConfig {
    /// Direct routing, public roots, the default deadline and cap.
    pub fn direct() -> Self {
        Self {
            proxy: ProxyMode::Direct,
            ca: CaMode::PublicRoots,
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
        let mut current: &(dyn Error + 'static) = &self.0;
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

impl Error for SanitizedReqwestError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
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
    CaNotRegularFile {
        path: PathBuf,
    },
    CaEmpty {
        path: PathBuf,
    },
    CaPem {
        path: PathBuf,
        source: SanitizedReqwestError,
    },
    CaNoCertificates {
        path: PathBuf,
    },
    Proxy(SanitizedReqwestError),
    Client(SanitizedReqwestError),
}

impl fmt::Display for TransportBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CaRead { path, .. } => {
                write!(f, "CA bundle {} could not be read", path.display())
            }
            Self::CaNotRegularFile { path } => {
                write!(f, "CA bundle {} is not a regular file", path.display())
            }
            Self::CaEmpty { path } => write!(f, "CA bundle {} is empty", path.display()),
            Self::CaPem { path, .. } => {
                write!(f, "CA bundle {} is not a valid PEM bundle", path.display())
            }
            Self::CaNoCertificates { path } => {
                write!(f, "CA bundle {} contains no certificates", path.display())
            }
            Self::Proxy(_) => write!(f, "proxy configuration was rejected"),
            Self::Client(_) => write!(f, "HTTP client could not be built"),
        }
    }
}

impl Error for TransportBuildError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::CaRead { source, .. } => Some(source),
            Self::CaPem { source, .. } | Self::Proxy(source) | Self::Client(source) => Some(source),
            Self::CaNotRegularFile { .. }
            | Self::CaEmpty { .. }
            | Self::CaNoCertificates { .. } => None,
        }
    }
}

fn load_pem_bundle(path: &Path) -> Result<Vec<Certificate>, TransportBuildError> {
    let metadata = fs::symlink_metadata(path).map_err(|source| TransportBuildError::CaRead {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(TransportBuildError::CaNotRegularFile {
            path: path.to_path_buf(),
        });
    }
    let bytes = fs::read(path).map_err(|source| TransportBuildError::CaRead {
        path: path.to_path_buf(),
        source,
    })?;
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Err(TransportBuildError::CaEmpty {
            path: path.to_path_buf(),
        });
    }
    let certificates =
        Certificate::from_pem_bundle(&bytes).map_err(|error| TransportBuildError::CaPem {
            path: path.to_path_buf(),
            source: SanitizedReqwestError::new(error),
        })?;
    if certificates.is_empty() {
        return Err(TransportBuildError::CaNoCertificates {
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
            .map_err(|error| TransportBuildError::CaPem {
                path: path.to_path_buf(),
                source: SanitizedReqwestError::new(error),
            })?;
    }
    Ok(certificates)
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
    /// Non-2xx (including 3xx, which is never followed) without GraphQL
    /// errors; the exact response is retained (boxed to keep the error small).
    Http {
        response: Box<RawHttpResponse>,
        body: HttpBodyShape,
    },
    /// 2xx whose body is malformed JSON, not an envelope, has neither `data`
    /// nor `errors`, or does not match the operation's types.
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
            Self::Http { response, .. } => {
                write!(f, "unexpected HTTP status {}", response.status)
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

impl Error for TransportFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
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

impl From<TransportFailure> for AppError {
    fn from(failure: TransportFailure) -> Self {
        let message = failure.to_string();
        match failure {
            TransportFailure::RequestBody(source) => {
                AppError::new(AppErrorKind::Invariant, message).with_source(source)
            }
            TransportFailure::GraphQl { .. } => AppError::new(AppErrorKind::GraphQl, message),
            TransportFailure::Response(source) => AppError::from(source),
            TransportFailure::Http {
                body: HttpBodyShape::Unusable(source),
                ..
            } => AppError::new(AppErrorKind::Transport, message).with_source(source),
            TransportFailure::Http {
                body: HttpBodyShape::Data,
                ..
            }
            | TransportFailure::ResponseTooLarge { .. }
            | TransportFailure::Timeout { .. } => AppError::new(AppErrorKind::Transport, message),
            TransportFailure::Network { source, .. } => {
                AppError::new(AppErrorKind::Transport, message).with_source(source)
            }
        }
    }
}

/// Classifies a captured response for a typed operation.
///
/// Order: GraphQL errors first (any status), then non-2xx as HTTP failure,
/// then the F02A envelope classification of a 2xx body.
pub fn classify_typed<T: DeserializeOwned>(
    response: RawHttpResponse,
) -> Result<T, TransportFailure> {
    let parsed = parse_response::<T>(&response.body);
    let success = response.status.is_success();
    match (parsed, success) {
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
    /// Builds the client once: HTTP/1.1, no redirects, no retries, a total
    /// deadline, explicit proxy routing and explicit roots.
    pub fn new(
        endpoint: EndpointUrl,
        api_key: ApiKey,
        config: TransportConfig,
    ) -> Result<Self, TransportBuildError> {
        let mut builder = Client::builder()
            .http1_only()
            .redirect(Policy::none())
            .referer(false)
            .timeout(config.deadline.duration())
            .retry(reqwest::retry::never());
        builder = match &config.proxy {
            ProxyMode::Direct => builder.no_proxy(),
            ProxyMode::HttpsConnect {
                url,
                bypass_loopback,
            } => {
                let mut proxy = Proxy::https(url.url.clone()).map_err(|error| {
                    TransportBuildError::Proxy(SanitizedReqwestError::new(error))
                })?;
                if *bypass_loopback {
                    proxy = proxy.no_proxy(NoProxy::from_string("127.0.0.1,localhost"));
                }
                builder.proxy(proxy)
            }
        };
        match &config.ca {
            CaMode::PublicRoots => {}
            CaMode::PublicRootsPlusPem(path) => {
                for certificate in load_pem_bundle(path)? {
                    builder = builder.add_root_certificate(certificate);
                }
            }
        }
        let client = builder
            .build()
            .map_err(|error| TransportBuildError::Client(SanitizedReqwestError::new(error)))?;
        Ok(Self {
            client,
            endpoint,
            api_key,
            deadline: config.deadline,
            max_response_bytes: config.max_response_bytes,
        })
    }

    pub fn endpoint(&self) -> &EndpointUrl {
        &self.endpoint
    }

    /// Sends an arbitrary document (the raw `api` path) and returns the exact
    /// response without classifying it. `variables` is sent only when present.
    pub async fn send_raw(
        &self,
        document: &str,
        variables: Option<Map<String, Value>>,
        operation_name: Option<&str>,
    ) -> Result<RawHttpResponse, TransportFailure> {
        let request = GraphQlRequest {
            query: document.to_owned(),
            variables,
            operation_name: operation_name.map(str::to_owned),
        };
        self.send_request(&request).await
    }

    /// Sends a prepared envelope and returns the exact response.
    pub async fn send_request<V: Serialize>(
        &self,
        request: &GraphQlRequest<V>,
    ) -> Result<RawHttpResponse, TransportFailure> {
        let body = serde_json::to_vec(request).map_err(TransportFailure::RequestBody)?;
        self.send_bytes(body).await
    }

    /// Sends a typed operation's envelope and classifies the response.
    pub async fn execute<T: DeserializeOwned, V: Serialize>(
        &self,
        request: &GraphQlRequest<V>,
    ) -> Result<T, TransportFailure> {
        let response = self.send_request(request).await?;
        classify_typed(response)
    }

    async fn send_bytes(&self, body: Vec<u8>) -> Result<RawHttpResponse, TransportFailure> {
        let request = self
            .client
            .post(self.endpoint.url.clone())
            .header(AUTHORIZATION, self.api_key.header_value())
            .header(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE))
            .header(CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE_VALUE))
            .body(body);
        let exchange = async {
            let response = request
                .send()
                .await
                .map_err(|error| self.classify_network(error))?;
            self.collect(response).await
        };
        match tokio::time::timeout(self.deadline.duration(), exchange).await {
            Ok(result) => result,
            Err(_elapsed) => Err(TransportFailure::Timeout {
                origin: self.endpoint.origin.clone(),
                deadline: self.deadline,
            }),
        }
    }

    /// Reads the body chunk by chunk, stopping before the cap is exceeded.
    async fn collect(
        &self,
        mut response: reqwest::Response,
    ) -> Result<RawHttpResponse, TransportFailure> {
        let status = response.status();
        let headers = std::mem::take(response.headers_mut());
        let limit = self.max_response_bytes;
        let too_large = || TransportFailure::ResponseTooLarge { status, limit };
        if let Some(declared) = response.content_length() {
            let over = match u64::try_from(limit.bytes()) {
                Ok(cap) => declared > cap,
                Err(_) => false,
            };
            if over {
                return Err(too_large());
            }
        }
        let mut body = Vec::new();
        loop {
            let chunk = response
                .chunk()
                .await
                .map_err(|error| self.classify_network(error))?;
            let Some(chunk) = chunk else { break };
            let total = body.len().checked_add(chunk.len()).ok_or_else(too_large)?;
            if total > limit.bytes() {
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

    fn classify_network(&self, error: reqwest::Error) -> TransportFailure {
        if error.is_timeout() {
            return TransportFailure::Timeout {
                origin: self.endpoint.origin.clone(),
                deadline: self.deadline,
            };
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
        TransportFailure::Network {
            origin: self.endpoint.origin.clone(),
            phase,
            source: SanitizedReqwestError::new(error),
        }
    }
}
