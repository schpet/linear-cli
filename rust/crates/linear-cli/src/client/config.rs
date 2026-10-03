//! Client settings: the endpoint, the API key, deadlines, response caps and
//! extra trusted certificates, and building the `reqwest` client from them.

use std::error::Error as StdError;
use std::fmt;
use std::fs;
use std::io;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::Duration;

use reqwest::header::HeaderValue;
use reqwest::redirect::Policy;
use reqwest::tls::Certificate;
use reqwest::{Client, Url};

use super::error::SanitizedReqwestError;
use crate::error::Error;

/// The `User-Agent` sent on every request.
pub const USER_AGENT_VALUE: &str = concat!("schpet-linear-cli/", env!("CARGO_PKG_VERSION"));

/// Redirects followed per request before giving up.
const MAX_REDIRECTS: usize = 20;

/// A validated `http` or `https` GraphQL endpoint.
///
/// The full URL (including any path and query) is used for requests; only the
/// origin (`scheme://host[:port]`) is ever displayed, because signed URLs can
/// carry tokens in their query string.
#[derive(Clone, PartialEq, Eq)]
pub struct EndpointUrl {
    pub(super) url: Url,
    pub(super) origin: String,
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

    pub(super) fn from_url(url: Url) -> Result<Self, EndpointUrlError> {
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

/// A Linear API key, sent verbatim as the `Authorization` header value.
///
/// Redacted in `Debug` and `Display`; the header value is marked sensitive.
#[derive(Clone)]
pub struct ApiKey {
    pub(super) value: HeaderValue,
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

    pub(super) fn header_value(&self) -> HeaderValue {
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

/// A non-zero total deadline for one request: connect, send and body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Deadline(pub(super) Duration);

impl Deadline {
    /// API requests, including `linear api`.
    pub const DEFAULT: Self = Self(Duration::from_secs(30));
    /// Image and attachment downloads, which can be large.
    pub const DOWNLOAD: Self = Self(Duration::from_secs(300));

    pub fn duration(self) -> Duration {
        self.0
    }
}

/// A cap on collected response bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResponseCap(pub(super) NonZeroUsize);

impl ResponseCap {
    /// 64 MiB: far above any real Linear page, small enough to stop a runaway body.
    pub const DEFAULT: Self = Self(NonZeroUsize::MIN.saturating_add(64 * 1024 * 1024 - 1));
    /// 256 MiB for one downloaded image or attachment, which is held in memory.
    pub const DOWNLOAD: Self = Self(NonZeroUsize::MIN.saturating_add(256 * 1024 * 1024 - 1));

    pub fn bytes(self) -> usize {
        self.0.get()
    }
}

/// Everything the client needs beyond endpoint and key.
#[derive(Clone, Debug)]
pub struct ClientConfig {
    /// A PEM bundle whose certificates are trusted in addition to the
    /// built-in roots (`SSL_CERT_FILE`).
    pub ca_bundle: Option<PathBuf>,
    /// API requests: GraphQL operations and `linear api`.
    pub deadline: Deadline,
    pub max_response_bytes: ResponseCap,
    /// Image and attachment downloads.
    pub download_deadline: Deadline,
    pub max_download_bytes: ResponseCap,
}

impl Default for ClientConfig {
    fn default() -> Self {
        Self {
            ca_bundle: None,
            deadline: Deadline::DEFAULT,
            max_response_bytes: ResponseCap::DEFAULT,
            download_deadline: Deadline::DOWNLOAD,
            max_download_bytes: ResponseCap::DOWNLOAD,
        }
    }
}

/// Why a [`LinearClient`] could not be built.
#[derive(Debug)]
pub enum ClientBuildError {
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

impl fmt::Display for ClientBuildError {
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

impl StdError for ClientBuildError {
    fn source(&self) -> Option<&(dyn StdError + 'static)> {
        match self {
            Self::CaRead { source, .. } => Some(source),
            Self::CaInvalid { source, .. } | Self::Client(source) => Some(source),
            Self::CaEmpty { .. } => None,
        }
    }
}

impl From<ClientBuildError> for Error {
    fn from(error: ClientBuildError) -> Self {
        let message = match &error {
            ClientBuildError::CaRead { .. }
            | ClientBuildError::CaInvalid { .. }
            | ClientBuildError::CaEmpty { .. } => format!("SSL_CERT_FILE: {error}"),
            ClientBuildError::Client(_) => error.to_string(),
        };
        Error::new(message).with_source(error)
    }
}

fn load_pem_bundle(path: &Path) -> Result<Vec<Certificate>, ClientBuildError> {
    let bytes = fs::read(path).map_err(|source| ClientBuildError::CaRead {
        path: path.to_path_buf(),
        source,
    })?;
    let invalid = |error| ClientBuildError::CaInvalid {
        path: path.to_path_buf(),
        source: SanitizedReqwestError::new(error),
    };
    let certificates = Certificate::from_pem_bundle(&bytes).map_err(invalid)?;
    if certificates.is_empty() {
        return Err(ClientBuildError::CaEmpty {
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

/// Builds the `reqwest` client: HTTP/1.1, bounded redirects, no retries,
/// gzip/brotli/deflate responses, proxies from the environment, and trusted
/// roots from the operating system, the bundled Mozilla set and the optional
/// bundle.
pub(super) fn build_client(config: &ClientConfig) -> Result<Client, ClientBuildError> {
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
        .map_err(|error| ClientBuildError::Client(SanitizedReqwestError::new(error)))
}
