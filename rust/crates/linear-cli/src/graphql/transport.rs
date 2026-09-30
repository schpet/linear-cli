//! HTTP transport for GraphQL operations (F02B Gate 1) and the bounded
//! fixed-host asset GET qualified in F02B Gate 2 at the historical 2.6.0
//! identity; v3 probe qualification awaits a separate driver profile.
//!
//! One `reqwest` client per transport, built by the shared [`build_client`]
//! from already validated [`EndpointUrl`], [`ApiKey`] and [`TransportConfig`]
//! values, so a [`GraphQlTransport`] and an [`AssetHttpTransport`] made from
//! one configuration carry identical proxy and CA settings. Credential,
//! workspace and endpoint-override resolution belong to F03/F04; this module
//! never reads the environment. [`ConfinedTransportEnv`] only parses values a
//! test-only caller has already read from the confined parity lane.
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
//! Redirect rejections never carry the `Location` value. `ApiKey` redacts
//! itself in `Debug` and `Display`.

use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, Read};
use std::net::IpAddr;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue, LOCATION, USER_AGENT};
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

/// The `User-Agent` sent on every request, derived from the Rust package
/// version.
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
    Scheme,
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
            Self::Scheme => write!(f, "proxy URL scheme must be http"),
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
            return Err(ProxyUrlError::Scheme);
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
        f.write_str("ProxyUrl(<redacted>)")
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

/// The exact `NO_PROXY` policy of the confined parity lane, and the only
/// loopback bypass [`build_client`] ever installs.
pub const LOOPBACK_NO_PROXY: &str = "127.0.0.1,localhost";

// ---------------------------------------------------------------------------
// Confined-lane environment adapter (F02B Gate 2)

/// Raw values of the runner-owned confined-lane variables, exactly as a
/// test-only caller read them. `None` means the variable is absent; this
/// module never reads the environment itself.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ConfinedEnvValues {
    pub https_proxy: Option<String>,
    pub http_proxy: Option<String>,
    pub all_proxy: Option<String>,
    pub no_proxy: Option<String>,
    pub ssl_cert_file: Option<String>,
}

/// Why a `NO_PROXY` value is not the confined lane's policy. Entries are
/// referred to by index, never quoted, so no value text reaches an error.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum NoProxyError {
    /// The comma-separated entry at `index` is empty or padded with whitespace.
    Blank { index: usize },
    /// The entry at `index` is not a loopback host.
    NotLoopback { index: usize },
    /// The entry at `index` repeats an earlier entry.
    Duplicate { index: usize },
    /// The entries are loopback but not exactly [`LOOPBACK_NO_PROXY`].
    NotConfinedPolicy,
}

impl fmt::Display for NoProxyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Blank { index } => write!(f, "NO_PROXY entry {index} is blank"),
            Self::NotLoopback { index } => {
                write!(f, "NO_PROXY entry {index} is not a loopback host")
            }
            Self::Duplicate { index } => write!(f, "NO_PROXY entry {index} is a duplicate"),
            Self::NotConfinedPolicy => write!(
                f,
                "NO_PROXY entries are not exactly the confined policy {LOOPBACK_NO_PROXY}"
            ),
        }
    }
}

impl Error for NoProxyError {}

/// Why the confined-lane variables do not form a [`ConfinedTransportEnv`].
///
/// Every variant names the variable, never its value: a present-but-empty
/// or malformed value fails here instead of falling back to any default.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfinedEnvError {
    Missing(&'static str),
    Empty(&'static str),
    /// A proxy variable this mode never honours is set. Plain-`http` requests
    /// are never proxied by [`ProxyMode::HttpsConnect`], so an `HTTP_PROXY` or
    /// `ALL_PROXY` value would be silently ignored; it is refused instead.
    Unexpected(&'static str),
    Proxy(ProxyUrlError),
    NoProxy(NoProxyError),
    /// `SSL_CERT_FILE` is not an absolute path.
    CaPathNotAbsolute,
}

impl fmt::Display for ConfinedEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "{name} is not set"),
            Self::Empty(name) => write!(f, "{name} is set but empty"),
            Self::Unexpected(name) => {
                write!(f, "{name} is set but the confined mode never honours it")
            }
            Self::Proxy(source) => write!(f, "HTTPS_PROXY is not usable: {source}"),
            Self::NoProxy(source) => fmt::Display::fmt(source, f),
            Self::CaPathNotAbsolute => write!(f, "SSL_CERT_FILE must be an absolute path"),
        }
    }
}

impl Error for ConfinedEnvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Proxy(source) => Some(source),
            Self::NoProxy(source) => Some(source),
            Self::Missing(_) | Self::Empty(_) | Self::Unexpected(_) | Self::CaPathNotAbsolute => {
                None
            }
        }
    }
}

/// The confined parity lane's transport settings, parsed from the values the
/// runner injects (`HTTPS_PROXY`, `NO_PROXY`, `SSL_CERT_FILE`).
///
/// This is transport-configuration qualification for the P03C lane, not F03
/// production environment discovery: the accepted shape is exactly one
/// loopback `http` CONNECT proxy, the loopback bypass policy
/// [`LOOPBACK_NO_PROXY`], and an absolute PEM path whose contents
/// [`build_client`] validates before any request.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ConfinedTransportEnv {
    proxy: ProxyUrl,
    ca_bundle: PathBuf,
}

impl ConfinedTransportEnv {
    pub fn parse(values: &ConfinedEnvValues) -> Result<Self, ConfinedEnvError> {
        if values.http_proxy.is_some() {
            return Err(ConfinedEnvError::Unexpected("HTTP_PROXY"));
        }
        if values.all_proxy.is_some() {
            return Err(ConfinedEnvError::Unexpected("ALL_PROXY"));
        }
        let https_proxy = required(values.https_proxy.as_deref(), "HTTPS_PROXY")?;
        let proxy = ProxyUrl::parse(https_proxy).map_err(ConfinedEnvError::Proxy)?;
        let no_proxy = required(values.no_proxy.as_deref(), "NO_PROXY")?;
        parse_no_proxy(no_proxy).map_err(ConfinedEnvError::NoProxy)?;
        let ca_bundle = required(values.ssl_cert_file.as_deref(), "SSL_CERT_FILE")?;
        let ca_bundle = PathBuf::from(ca_bundle);
        if !ca_bundle.is_absolute() {
            return Err(ConfinedEnvError::CaPathNotAbsolute);
        }
        Ok(Self { proxy, ca_bundle })
    }

    pub fn proxy(&self) -> &ProxyUrl {
        &self.proxy
    }

    pub fn ca_bundle(&self) -> &Path {
        &self.ca_bundle
    }

    /// `HttpsConnect` with loopback bypass plus public roots and the lane's
    /// PEM, under a caller-chosen finite deadline and cap.
    pub fn into_config(
        self,
        deadline: Deadline,
        max_response_bytes: ResponseCap,
    ) -> TransportConfig {
        TransportConfig {
            proxy: ProxyMode::HttpsConnect {
                url: self.proxy,
                bypass_loopback: true,
            },
            ca: CaMode::PublicRootsPlusPem(self.ca_bundle),
            deadline,
            max_response_bytes,
        }
    }
}

fn required<'a>(value: Option<&'a str>, name: &'static str) -> Result<&'a str, ConfinedEnvError> {
    match value {
        None => Err(ConfinedEnvError::Missing(name)),
        Some("") => Err(ConfinedEnvError::Empty(name)),
        Some(text) => Ok(text),
    }
}

/// Accepts exactly the entries of [`LOOPBACK_NO_PROXY`] in any order.
pub(crate) fn parse_no_proxy(text: &str) -> Result<(), NoProxyError> {
    let mut seen: Vec<&str> = Vec::new();
    for (index, entry) in text.split(',').enumerate() {
        if entry.is_empty() || entry.trim() != entry {
            return Err(NoProxyError::Blank { index });
        }
        if !is_loopback_host(entry) {
            return Err(NoProxyError::NotLoopback { index });
        }
        if seen.contains(&entry) {
            return Err(NoProxyError::Duplicate { index });
        }
        seen.push(entry);
    }
    let mut policy: Vec<&str> = LOOPBACK_NO_PROXY.split(',').collect();
    policy.sort_unstable();
    seen.sort_unstable();
    if seen != policy {
        return Err(NoProxyError::NotConfinedPolicy);
    }
    Ok(())
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
    CaTooLarge {
        path: PathBuf,
        ceiling: u64,
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
            Self::CaTooLarge { path, ceiling } => {
                write!(f, "CA bundle {} exceeds {ceiling} bytes", path.display())
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
            | Self::CaTooLarge { .. }
            | Self::CaEmpty { .. }
            | Self::CaNoCertificates { .. } => None,
        }
    }
}

impl From<TransportBuildError> for AppError {
    fn from(error: TransportBuildError) -> Self {
        let message = match &error {
            TransportBuildError::CaRead { .. }
            | TransportBuildError::CaNotRegularFile { .. }
            | TransportBuildError::CaTooLarge { .. }
            | TransportBuildError::CaEmpty { .. }
            | TransportBuildError::CaPem { .. }
            | TransportBuildError::CaNoCertificates { .. } => {
                format!("SSL_CERT_FILE: {error}")
            }
            TransportBuildError::Proxy(_) | TransportBuildError::Client(_) => error.to_string(),
        };
        AppError::new(AppErrorKind::Validation, message).with_source(error)
    }
}

fn load_pem_bundle(path: &Path) -> Result<Vec<Certificate>, TransportBuildError> {
    const MAX_CA_BYTES: u64 = 4 * 1024 * 1024;
    // `metadata` follows a symlink to a real bundle. Check the target type
    // before opening so ordinary directories and named pipes are refused.
    let metadata = fs::metadata(path).map_err(|source| TransportBuildError::CaRead {
        path: path.to_path_buf(),
        source,
    })?;
    if !metadata.is_file() {
        return Err(TransportBuildError::CaNotRegularFile {
            path: path.to_path_buf(),
        });
    }
    if metadata.len() > MAX_CA_BYTES {
        return Err(TransportBuildError::CaTooLarge {
            path: path.to_path_buf(),
            ceiling: MAX_CA_BYTES,
        });
    }
    let file = fs::File::open(path).map_err(|source| TransportBuildError::CaRead {
        path: path.to_path_buf(),
        source,
    })?;
    let opened = file
        .metadata()
        .map_err(|source| TransportBuildError::CaRead {
            path: path.to_path_buf(),
            source,
        })?;
    if !opened.is_file() {
        return Err(TransportBuildError::CaNotRegularFile {
            path: path.to_path_buf(),
        });
    }
    if opened.len() > MAX_CA_BYTES {
        return Err(TransportBuildError::CaTooLarge {
            path: path.to_path_buf(),
            ceiling: MAX_CA_BYTES,
        });
    }
    let mut bytes = Vec::new();
    file.take(MAX_CA_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|source| TransportBuildError::CaRead {
            path: path.to_path_buf(),
            source,
        })?;
    if u64::try_from(bytes.len()).is_ok_and(|len| len > MAX_CA_BYTES) {
        return Err(TransportBuildError::CaTooLarge {
            path: path.to_path_buf(),
            ceiling: MAX_CA_BYTES,
        });
    }
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
        let graphql_detail = match &failure {
            TransportFailure::GraphQl {
                status,
                errors,
                partial_data,
                ..
            } => Some(format!(
                "GraphQL HTTP {status}; errors={}; partial_data={partial_data}",
                errors.len()
            )),
            _ => None,
        };
        match failure {
            TransportFailure::RequestBody(source) => {
                AppError::new(AppErrorKind::Invariant, message).with_source(source)
            }
            TransportFailure::GraphQl { .. } => {
                // The structured summary is stable and omits arbitrary
                // response extensions, headers and the request URL.
                AppError::new(AppErrorKind::GraphQl, message)
                    .with_debug_detail(graphql_detail.unwrap_or_default())
            }
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
// Shared client construction and exchange failures

/// Builds the one `reqwest::Client` shape every transport in this module
/// uses: HTTP/1.1, no redirects, no retries, a total deadline, explicit proxy
/// routing and explicit roots.
///
/// [`GraphQlTransport::new`] and [`AssetHttpTransport::new`] both call this,
/// so a loopback GraphQL POST and a fixed-host asset GET built from one
/// [`TransportConfig`] share identical proxy and CA settings. `proxy()` and
/// `no_proxy()` each clear reqwest's ambient-variable lookup, so the client
/// routes only as configured.
/// Explicit total-deadline choice; only signed upload exchanges omit it.
enum ClientDeadline {
    Total(Deadline),
    NoTotal,
}
fn build_client(config: &TransportConfig) -> Result<Client, TransportBuildError> {
    build_client_with_deadline(config, ClientDeadline::Total(config.deadline))
}
fn build_signed_upload_client(config: &TransportConfig) -> Result<Client, TransportBuildError> {
    build_client_with_deadline(config, ClientDeadline::NoTotal)
}
fn build_markdown_download_client(config: &TransportConfig) -> Result<Client, TransportBuildError> {
    build_client_with_deadline(config, ClientDeadline::NoTotal)
}
fn build_client_with_deadline(
    config: &TransportConfig,
    deadline: ClientDeadline,
) -> Result<Client, TransportBuildError> {
    let mut builder = Client::builder()
        .http1_only()
        .redirect(Policy::none())
        .referer(false)
        .retry(reqwest::retry::never());
    builder = match deadline {
        ClientDeadline::Total(deadline) => builder.timeout(deadline.duration()),
        // reqwest defaults to no total request timeout. No MAX/floor-budget hack.
        ClientDeadline::NoTotal => builder,
    };
    builder = match &config.proxy {
        ProxyMode::Direct => builder.no_proxy(),
        ProxyMode::HttpsConnect {
            url,
            bypass_loopback,
        } => {
            let mut proxy = Proxy::https(url.url.clone())
                .map_err(|error| TransportBuildError::Proxy(SanitizedReqwestError::new(error)))?;
            if *bypass_loopback {
                proxy = proxy.no_proxy(NoProxy::from_string(LOOPBACK_NO_PROXY));
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
    builder
        .build()
        .map_err(|error| TransportBuildError::Client(SanitizedReqwestError::new(error)))
}

/// A failure below HTTP classification, before the owning transport attaches
/// its origin. Shared by the GraphQL POST and the asset GET paths.
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
        let chunk = response.chunk().await.map_err(classify_network)?;
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

// ---------------------------------------------------------------------------
// Transport

/// One configured HTTP client bound to an endpoint and key.
#[derive(Clone, Debug)]
pub struct GraphQlTransport {
    client: Client,
    signed_upload_client: Client,
    markdown_download_client: Client,
    endpoint: EndpointUrl,
    api_key: ApiKey,
    deadline: Deadline,
    max_response_bytes: ResponseCap,
}

impl GraphQlTransport {
    /// Builds the client once through [`build_client`].
    pub fn new(
        endpoint: EndpointUrl,
        api_key: ApiKey,
        config: TransportConfig,
    ) -> Result<Self, TransportBuildError> {
        let client = build_client(&config)?;
        let signed_upload_client = build_signed_upload_client(&config)?;
        let markdown_download_client = build_markdown_download_client(&config)?;
        Ok(Self {
            client,
            signed_upload_client,
            markdown_download_client,
            endpoint,
            api_key,
            deadline: config.deadline,
            max_response_bytes: config.max_response_bytes,
        })
    }

    pub fn endpoint(&self) -> &EndpointUrl {
        &self.endpoint
    }

    /// Ordinary Markdown images use Fetch-like GETs: arbitrary image hosts,
    /// no total deadline/body cap, no automatic UA or compression negotiation.
    /// Authentication is attached only to the initial private-upload host and
    /// is permanently removed after any cross-origin redirect.
    pub async fn download_markdown_image(&self, original: &str) -> Result<Vec<u8>, AppError> {
        let mut url = Url::parse(original).map_err(|error| {
            AppError::new(
                AppErrorKind::Transport,
                format!("Invalid URL: '{original}'"),
            )
            .with_source(error)
        })?;
        if url.scheme() == "data" {
            let data = data_url::DataUrl::process(original).map_err(|error| {
                AppError::new(
                    AppErrorKind::Transport,
                    "NetworkError when attempting to fetch resource",
                )
                .with_source(error)
            })?;
            return data
                .decode_to_vec()
                .map(|(bytes, _fragment)| bytes)
                .map_err(|error| {
                    AppError::new(
                        AppErrorKind::Transport,
                        "NetworkError when attempting to fetch resource",
                    )
                    .with_source(error)
                });
        }
        if url.scheme() == "file" {
            // Fetch reads an initial readable file URL. Decode its path through
            // the typed URL boundary; cache creation/hit handling stays upstream.
            let path = url.to_file_path().map_err(|()| {
                AppError::new(
                    AppErrorKind::Transport,
                    "NetworkError when attempting to fetch resource",
                )
            })?;
            return std::fs::read(path).map_err(|error| {
                AppError::new(
                    AppErrorKind::Transport,
                    "NetworkError when attempting to fetch resource",
                )
                .with_source(error)
            });
        }
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::new(
                AppErrorKind::Transport,
                "NetworkError when attempting to fetch resource",
            ));
        }
        let mut authenticated = url.host_str() == Some("uploads.linear.app");
        let mut redirects = 0;
        loop {
            let mut request = self.markdown_download_client.get(url.clone());
            if authenticated {
                request = request.header(AUTHORIZATION, self.api_key.header_value());
            }
            let response = request.send().await.map_err(markdown_network_error)?;
            let status = response.status();
            if matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
                && let Some(location) = response.headers().get(LOCATION)
            {
                if redirects == 20 {
                    return Err(AppError::new(
                        AppErrorKind::Transport,
                        "NetworkError when attempting to fetch resource",
                    ));
                }
                let location = location.to_str().map_err(|error| {
                    AppError::new(AppErrorKind::Transport, "Invalid image redirect Location")
                        .with_source(error)
                })?;
                let next = url.join(location).map_err(|error| {
                    AppError::new(AppErrorKind::Transport, "Invalid image redirect URL")
                        .with_source(error)
                })?;
                if !matches!(next.scheme(), "http" | "https") {
                    return Err(AppError::new(
                        AppErrorKind::Transport,
                        "NetworkError when attempting to fetch resource",
                    ));
                }
                authenticated &= url.origin() == next.origin();
                url = next;
                redirects += 1;
                continue;
            }
            if !status.is_success() {
                let phrase = response
                    .extensions()
                    .get::<hyper::ext::ReasonPhrase>()
                    .map(|phrase| {
                        phrase
                            .as_bytes()
                            .iter()
                            .copied()
                            .map(char::from)
                            .collect::<String>()
                    })
                    .unwrap_or_else(|| status.canonical_reason().unwrap_or("").to_owned());
                return Err(AppError::new(
                    AppErrorKind::Transport,
                    format!("Failed to download image: {} {phrase}", status.as_u16()),
                ));
            }
            let encoding = response
                .headers()
                .get(reqwest::header::CONTENT_ENCODING)
                .map(|value| value.to_str().map(str::to_owned))
                .transpose()
                .map_err(|error| {
                    AppError::new(AppErrorKind::Transport, "Invalid image Content-Encoding")
                        .with_source(error)
                })?;
            let body = response.bytes().await.map_err(markdown_network_error)?;
            return decode_markdown_image(encoding.as_deref(), body.to_vec());
        }
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
            let response = request.send().await.map_err(classify_network)?;
            collect(response, self.max_response_bytes).await
        };
        match tokio::time::timeout(self.deadline.duration(), exchange).await {
            Ok(Ok(response)) => Ok(response),
            Ok(Err(failure)) => Err(self.failure(failure)),
            Err(_elapsed) => Err(self.failure(ExchangeFailure::Timeout)),
        }
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
}

fn markdown_network_error(error: reqwest::Error) -> AppError {
    AppError::new(
        AppErrorKind::Transport,
        "NetworkError when attempting to fetch resource",
    )
    .with_source(SanitizedReqwestError::new(error))
}

/// Decode explicitly; enabling reqwest's codec features would also change
/// automatic Accept-Encoding headers on the already-qualified GraphQL client.
pub fn decode_markdown_image(
    encoding: Option<&str>,
    mut body: Vec<u8>,
) -> Result<Vec<u8>, AppError> {
    for coding in encoding.unwrap_or("").split(',').rev().map(str::trim) {
        let mut decoded = Vec::new();
        let result = match coding.to_ascii_lowercase().as_str() {
            "" | "identity" => continue,
            "gzip" | "x-gzip" => {
                flate2::read::MultiGzDecoder::new(body.as_slice()).read_to_end(&mut decoded)
            }
            "br" => brotli_decompressor::Decompressor::new(body.as_slice(), 4096)
                .read_to_end(&mut decoded),
            "deflate" => flate2::read::ZlibDecoder::new(body.as_slice()).read_to_end(&mut decoded),
            _ => {
                return Err(AppError::new(
                    AppErrorKind::Transport,
                    format!("Unsupported image Content-Encoding: {coding}"),
                ));
            }
        };
        result.map_err(|error| {
            AppError::new(AppErrorKind::Transport, "Failed to decode image response")
                .with_source(error)
        })?;
        body = decoded;
    }
    Ok(body)
}

// ---------------------------------------------------------------------------
// Fixed-host asset GET (F02B Gate 2)

/// The two fixed hosts Gate 2 qualifies. Nothing else is ever requested.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AssetHost {
    /// `uploads.linear.app`: private files; `Authorization` is sent on every
    /// request to it, including each redirect hop.
    Uploads,
    /// `public.linear.app`: public files; never authenticated.
    Public,
}

impl AssetHost {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Uploads => "uploads.linear.app",
            Self::Public => "public.linear.app",
        }
    }

    /// Whether requests to this host carry the API key.
    pub const fn authenticated(self) -> bool {
        matches!(self, Self::Uploads)
    }

    fn from_name(host: &str) -> Option<Self> {
        match host {
            "uploads.linear.app" => Some(Self::Uploads),
            "public.linear.app" => Some(Self::Public),
            _ => None,
        }
    }

    /// `https://<host>`, the only part of an asset URL ever displayed.
    pub fn origin(self) -> String {
        format!("https://{}", self.name())
    }
}

/// A validated `https://uploads.linear.app/...` or
/// `https://public.linear.app/...` URL. Path and query (which carry signed
/// tokens) are used for the request and never displayed.
#[derive(Clone, PartialEq, Eq)]
pub struct AssetUrl {
    url: Url,
    host: AssetHost,
}

/// Why a string is not an acceptable [`AssetUrl`]. No variant carries the
/// input or its host.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AssetUrlError {
    /// The text is not a URL; carries the parser's description (never the input).
    Invalid(String),
    /// The scheme is not `https`.
    Scheme(String),
    MissingHost,
    /// The host is neither fixed host.
    HostNotAllowed,
    /// A port other than the default is given.
    Port,
    Credentials,
    Fragment,
}

impl fmt::Display for AssetUrlError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Invalid(detail) => write!(f, "asset URL is not valid: {detail}"),
            Self::Scheme(scheme) => write!(f, "asset URL scheme must be https, not {scheme}"),
            Self::MissingHost => write!(f, "asset URL has no host"),
            Self::HostNotAllowed => write!(f, "asset URL host is not a permitted fixed host"),
            Self::Port => write!(f, "asset URL must not carry an explicit port"),
            Self::Credentials => write!(f, "asset URL must not carry credentials"),
            Self::Fragment => write!(f, "asset URL must not carry a fragment"),
        }
    }
}

impl Error for AssetUrlError {}

impl AssetUrl {
    pub fn parse(text: &str) -> Result<Self, AssetUrlError> {
        let url = Url::parse(text).map_err(|error| AssetUrlError::Invalid(error.to_string()))?;
        Self::from_url(url)
    }

    fn from_url(url: Url) -> Result<Self, AssetUrlError> {
        if url.scheme() != "https" {
            return Err(AssetUrlError::Scheme(url.scheme().to_owned()));
        }
        let Some(host) = url.host_str() else {
            return Err(AssetUrlError::MissingHost);
        };
        let Some(host) = AssetHost::from_name(host) else {
            return Err(AssetUrlError::HostNotAllowed);
        };
        // The parser drops the default port, so any remaining port is explicit.
        if url.port().is_some() {
            return Err(AssetUrlError::Port);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(AssetUrlError::Credentials);
        }
        if url.fragment().is_some() {
            return Err(AssetUrlError::Fragment);
        }
        Ok(Self { url, host })
    }

    pub fn host(&self) -> AssetHost {
        self.host
    }

    /// The full request URL, including path and query.
    pub fn url(&self) -> &Url {
        &self.url
    }

    pub fn origin(&self) -> String {
        self.host.origin()
    }
}

impl fmt::Display for AssetUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.origin())
    }
}

impl fmt::Debug for AssetUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("AssetUrl").field(&self.origin()).finish()
    }
}

/// The most redirects one asset GET follows; the final response is therefore
/// at most the `MAX_REDIRECTS + 1`th request, all under one deadline.
pub const MAX_REDIRECTS: usize = 2;

/// Why a redirect was not followed. No variant carries the `Location` value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RedirectRejection {
    /// A redirect status without a `Location` header.
    MissingLocation,
    /// `Location` is not visible ASCII, contains a backslash, carries a
    /// fragment, or does not parse against the current URL.
    MalformedLocation,
    /// `Location` is absolute, scheme-relative (`//`), or not rooted at `/`.
    NotOriginRelative,
    /// The resolved target is not the same fixed host over `https`.
    CrossOrigin,
    /// More than [`MAX_REDIRECTS`] redirects in one GET.
    TooManyRedirects { limit: usize },
}

impl fmt::Display for RedirectRejection {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingLocation => write!(f, "redirect has no Location header"),
            Self::MalformedLocation => {
                write!(f, "Location is not a well-formed origin-relative path")
            }
            Self::NotOriginRelative => write!(f, "Location is not origin-relative"),
            Self::CrossOrigin => write!(f, "Location resolves to another origin"),
            Self::TooManyRedirects { limit } => write!(f, "more than {limit} redirects"),
        }
    }
}

impl Error for RedirectRejection {}

fn is_redirect_status(status: StatusCode) -> bool {
    matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
}

/// Resolves a `Location` against the current asset URL, accepting only an
/// origin-relative path that lands on the same fixed host over `https`.
pub fn resolve_redirect(
    current: &AssetUrl,
    location: Option<&HeaderValue>,
) -> Result<AssetUrl, RedirectRejection> {
    let Some(location) = location else {
        return Err(RedirectRejection::MissingLocation);
    };
    let Ok(text) = location.to_str() else {
        return Err(RedirectRejection::MalformedLocation);
    };
    if !text.starts_with('/') || text.starts_with("//") {
        return Err(RedirectRejection::NotOriginRelative);
    }
    // The URL parser treats `\` as `/` for https, which would turn `/\host`
    // into a scheme-relative reference; refuse it before resolving.
    if text.contains('\\') {
        return Err(RedirectRejection::MalformedLocation);
    }
    let Ok(resolved) = current.url.join(text) else {
        return Err(RedirectRejection::MalformedLocation);
    };
    let next = AssetUrl::from_url(resolved).map_err(|error| match error {
        AssetUrlError::Fragment | AssetUrlError::Invalid(_) => RedirectRejection::MalformedLocation,
        AssetUrlError::Scheme(_)
        | AssetUrlError::MissingHost
        | AssetUrlError::HostNotAllowed
        | AssetUrlError::Port
        | AssetUrlError::Credentials => RedirectRejection::CrossOrigin,
    })?;
    if next.host != current.host {
        return Err(RedirectRejection::CrossOrigin);
    }
    Ok(next)
}

/// Decides what follows one response of an asset GET: `Ok(None)` means the
/// response is final and is returned as received (whatever its status);
/// `Ok(Some(next))` is the next same-origin hop; `Err` stops the GET.
pub fn follow_redirect(
    current: &AssetUrl,
    redirects_so_far: usize,
    status: StatusCode,
    location: Option<&HeaderValue>,
) -> Result<Option<AssetUrl>, RedirectRejection> {
    if !is_redirect_status(status) {
        return Ok(None);
    }
    if redirects_so_far >= MAX_REDIRECTS {
        return Err(RedirectRejection::TooManyRedirects {
            limit: MAX_REDIRECTS,
        });
    }
    resolve_redirect(current, location).map(Some)
}

/// The final response of an asset GET, with the number of requests it took.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssetResponse {
    /// `1` without redirects, one more per followed redirect.
    pub requests: usize,
    /// Exactly as received: status, headers and body bytes under the cap.
    pub response: RawHttpResponse,
}

/// Every way an asset GET can fail after construction. Only the origin of the
/// URL in flight is recorded, never its path or query.
#[derive(Debug)]
pub enum AssetFailure {
    Redirect {
        origin: String,
        /// The 1-based request whose response carried the rejected redirect.
        request: usize,
        rejection: RedirectRejection,
    },
    ResponseTooLarge {
        origin: String,
        status: StatusCode,
        limit: ResponseCap,
    },
    Timeout {
        origin: String,
        deadline: Deadline,
    },
    Network {
        origin: String,
        phase: NetworkPhase,
        source: SanitizedReqwestError,
    },
}

impl fmt::Display for AssetFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Redirect {
                origin,
                request,
                rejection,
            } => write!(
                f,
                "redirect from {origin} (request {request}) was rejected: {rejection}"
            ),
            Self::ResponseTooLarge {
                origin,
                status,
                limit,
            } => write!(
                f,
                "response from {origin} exceeds the {} byte limit (HTTP {status})",
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

impl Error for AssetFailure {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Redirect { rejection, .. } => Some(rejection),
            Self::Network { source, .. } => Some(source),
            Self::ResponseTooLarge { .. } | Self::Timeout { .. } => None,
        }
    }
}

/// A bounded GET for the two fixed hosts, built by the same [`build_client`]
/// as [`GraphQlTransport`].
///
/// Per request: `User-Agent` always, `Authorization` only when the host in
/// flight is [`AssetHost::Uploads`]; no default headers on the client. Same-
/// origin relative redirects are followed up to [`MAX_REDIRECTS`] with the
/// host and authorization policy re-checked on every hop, under one total
/// deadline; anything else is rejected without recording the `Location`.
/// This is Gate 2 transport qualification, not a general asset API.
#[derive(Clone, Debug)]
pub struct AssetHttpTransport {
    client: Client,
    api_key: ApiKey,
    deadline: Deadline,
    max_response_bytes: ResponseCap,
}

impl AssetHttpTransport {
    pub fn new(api_key: ApiKey, config: TransportConfig) -> Result<Self, TransportBuildError> {
        let client = build_client(&config)?;
        Ok(Self {
            client,
            api_key,
            deadline: config.deadline,
            max_response_bytes: config.max_response_bytes,
        })
    }

    /// GETs `url`, following only same-origin relative redirects, and returns
    /// the final response exactly as received.
    pub async fn get(&self, url: &AssetUrl) -> Result<AssetResponse, AssetFailure> {
        let exchange = async {
            let mut current = url.clone();
            let mut redirects = 0;
            loop {
                let mut request = self
                    .client
                    .get(current.url.clone())
                    .header(USER_AGENT, HeaderValue::from_static(USER_AGENT_VALUE));
                if current.host.authenticated() {
                    request = request.header(AUTHORIZATION, self.api_key.header_value());
                }
                let response = request
                    .send()
                    .await
                    .map_err(|error| self.failure(&current, classify_network(error)))?;
                let response = collect(response, self.max_response_bytes)
                    .await
                    .map_err(|failure| self.failure(&current, failure))?;
                let location = response.headers.get(LOCATION);
                match follow_redirect(&current, redirects, response.status, location) {
                    Ok(None) => {
                        return Ok(AssetResponse {
                            requests: redirects.saturating_add(1),
                            response,
                        });
                    }
                    Ok(Some(next)) => {
                        redirects = redirects.saturating_add(1);
                        current = next;
                    }
                    Err(rejection) => {
                        return Err(AssetFailure::Redirect {
                            origin: current.origin(),
                            request: redirects.saturating_add(1),
                            rejection,
                        });
                    }
                }
            }
        };
        match tokio::time::timeout(self.deadline.duration(), exchange).await {
            Ok(result) => result,
            Err(_elapsed) => Err(self.failure(url, ExchangeFailure::Timeout)),
        }
    }

    fn failure(&self, url: &AssetUrl, failure: ExchangeFailure) -> AssetFailure {
        match failure {
            ExchangeFailure::ResponseTooLarge { status, limit } => AssetFailure::ResponseTooLarge {
                origin: url.origin(),
                status,
                limit,
            },
            ExchangeFailure::Timeout => AssetFailure::Timeout {
                origin: url.origin(),
                deadline: self.deadline,
            },
            ExchangeFailure::Network { phase, source } => AssetFailure::Network {
                origin: url.origin(),
                phase,
                source,
            },
        }
    }
}

// Append inside transport.rs: dedicated signed client reuses the explicit
// proxy/CA/HTTP policy, with no total upload deadline (source Fetch semantics).
// Error bodies remain capped; network errors sanitized; no ambient config.
impl GraphQlTransport {
    /// Signed upload requests carry ONLY their returned headers, never the CLI
    /// API key or GraphQL User-Agent. Fetch-compatible redirects are bounded20.
    pub async fn put_signed(
        &self,
        url: &str,
        headers: HeaderMap,
        body: Vec<u8>,
    ) -> Result<(), AppError> {
        // Fetch accepts HTTP(S) fragments but excludes them from the request.
        // Keep GraphQL endpoint validation unchanged; normalize only uploads.
        let mut initial_url = Url::parse(url)
            .map_err(|_| AppError::new(AppErrorKind::Validation, "Invalid signed upload URL"))?;
        initial_url.set_fragment(None);
        let initial = EndpointUrl::from_url(initial_url)
            .map_err(|_| AppError::new(AppErrorKind::Validation, "Invalid signed upload URL"))?;
        let origin = initial.origin().to_owned();
        let exchange = async {
            let mut url = initial.url().clone();
            let mut headers = headers;
            let mut method = reqwest::Method::PUT;
            let template = self
                .signed_upload_client
                .put(url.clone())
                .body(body)
                .build()
                .map_err(|e| self.upload_failure(classify_network(e), &origin))?;
            let mut include_body = true;
            for redirects in 0..=20 {
                // Request cloning shares reqwest's in-memory bytes across hops.
                let Some(mut request) = template.try_clone() else {
                    unreachable!("in-memory upload request is cloneable");
                };
                *request.method_mut() = method.clone();
                *request.url_mut() = url.clone();
                *request.headers_mut() = headers.clone();
                if !include_body {
                    *request.body_mut() = None;
                }
                let response = self
                    .signed_upload_client
                    .execute(request)
                    .await
                    .map_err(|e| self.upload_failure(classify_network(e), &origin))?;
                let status = response.status();
                if let Some(location) = response.headers().get(LOCATION)
                    && matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308)
                {
                    if redirects == 20 {
                        return Err(AppError::new(
                            AppErrorKind::Transport,
                            "Too many signed upload redirects; object may already be stored remotely; no comment or attachment was created",
                        ));
                    }
                    let location = location.to_str()
                        .map_err(|_| {
                            AppError::new(AppErrorKind::Transport, "Invalid signed upload redirect; object may already be stored remotely; no comment or attachment was created")
                        })?;
                    let mut next = url.join(location).map_err(|_| {
                        AppError::new(AppErrorKind::Transport, "Invalid signed upload redirect; object may already be stored remotely; no comment or attachment was created")
                    })?;
                    next.set_fragment(None);
                    let next = EndpointUrl::from_url(next)
                        .map_err(|_| {
                            AppError::new(AppErrorKind::Transport, "Invalid signed upload redirect; object may already be stored remotely; no comment or attachment was created")
                        })?
                        .url;
                    if url.origin() != next.origin() {
                        headers.remove(AUTHORIZATION);
                        headers.remove("proxy-authorization");
                        headers.remove("www-authenticate");
                    }
                    if status.as_u16() == 303
                        && method != reqwest::Method::GET
                        && method != reqwest::Method::HEAD
                    {
                        method = reqwest::Method::GET;
                        include_body = false;
                        for name in [
                            "content-type",
                            "content-length",
                            "content-encoding",
                            "content-language",
                            "content-location",
                        ] {
                            headers.remove(name);
                        }
                    }
                    url = next;
                    drop(response);
                    continue;
                }
                if status.is_success() {
                    drop(response);
                    return Ok(());
                }
                let response = collect(response, self.max_response_bytes)
                    .await
                    .map_err(|e| self.upload_failure(e, &origin))?;
                return Err(AppError::new(
                    AppErrorKind::Transport,
                    format!(
                        "Failed to upload file: {} {} - {}",
                        response.status.as_u16(),
                        response.status.canonical_reason().unwrap_or(""),
                        String::from_utf8_lossy(&response.body)
                    ),
                ));
            }
            unreachable!("redirect loop returns on final iteration")
        };
        // Deliberately no client or outer total deadline for upload bodies.
        // GraphQL metadata/final mutations retain their ordinary total deadline.
        exchange.await
    }
    fn upload_failure(&self, failure: ExchangeFailure, origin: &str) -> AppError {
        match failure {
            ExchangeFailure::ResponseTooLarge { limit, .. } => AppError::new(
                AppErrorKind::Transport,
                format!("Signed upload response exceeded {} bytes; object may already be stored remotely; no comment or attachment was created", limit.bytes()),
            ),
            ExchangeFailure::Timeout => AppError::new(
                AppErrorKind::Transport,
                format!("Signed upload timed out at {origin}; object may already be stored remotely; no comment or attachment was created"),
            ),
            ExchangeFailure::Network { source, .. } => AppError::new(
                AppErrorKind::Transport,
                format!("Signed upload failed at {origin}; object may already be stored remotely; no comment or attachment was created"),
            )
            .with_source(source),
        }
    }
}
