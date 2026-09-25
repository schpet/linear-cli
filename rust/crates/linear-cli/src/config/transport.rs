//! Closed process-environment inputs for a network transport.
//!
//! Startup captures these values once, but only a network action resolves
//! them. The dotenv overlay is for child processes and never supplies them.
use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::path::PathBuf;

use crate::error::{AppError, AppErrorKind};
use crate::graphql::transport::{
    CaMode, Deadline, NoProxyError, ProxyMode, ProxyUrl, ProxyUrlError, ResponseCap,
    TransportConfig, parse_no_proxy,
};

use super::runtime::ProcessEnvSnapshot;
use super::source::OsFamily;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum TransportKey {
    HttpsProxy,
    LowerHttpsProxy,
    HttpProxy,
    LowerHttpProxy,
    AllProxy,
    LowerAllProxy,
    NoProxy,
    LowerNoProxy,
    SslCertFile,
    SslCertDir,
    DenoCert,
    DenoTlsCaStore,
}

impl TransportKey {
    const ALL: [Self; 12] = [
        Self::HttpsProxy,
        Self::LowerHttpsProxy,
        Self::HttpProxy,
        Self::LowerHttpProxy,
        Self::AllProxy,
        Self::LowerAllProxy,
        Self::NoProxy,
        Self::LowerNoProxy,
        Self::SslCertFile,
        Self::SslCertDir,
        Self::DenoCert,
        Self::DenoTlsCaStore,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::HttpsProxy => "HTTPS_PROXY",
            Self::LowerHttpsProxy => "https_proxy",
            Self::HttpProxy => "HTTP_PROXY",
            Self::LowerHttpProxy => "http_proxy",
            Self::AllProxy => "ALL_PROXY",
            Self::LowerAllProxy => "all_proxy",
            Self::NoProxy => "NO_PROXY",
            Self::LowerNoProxy => "no_proxy",
            Self::SslCertFile => "SSL_CERT_FILE",
            Self::SslCertDir => "SSL_CERT_DIR",
            Self::DenoCert => "DENO_CERT",
            Self::DenoTlsCaStore => "DENO_TLS_CA_STORE",
        }
    }

    const fn windows_key(self) -> Option<&'static str> {
        match self {
            Self::LowerHttpsProxy
            | Self::LowerHttpProxy
            | Self::LowerAllProxy
            | Self::LowerNoProxy => None,
            other => Some(other.name()),
        }
    }
}

#[derive(Clone, Eq, PartialEq)]
struct CapturedVar {
    original_name: String,
    value: String,
}

impl fmt::Debug for CapturedVar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CapturedVar")
            .field("name", &self.original_name)
            .field("value", &"<redacted>")
            .finish()
    }
}

/// Only the original process transport variables, with no credential values.
#[derive(Clone, Eq, PartialEq)]
pub struct TransportEnvInputs {
    values: BTreeMap<TransportKey, CapturedVar>,
}

impl fmt::Debug for TransportEnvInputs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TransportEnvInputs")
            .field(
                "names",
                &self
                    .values
                    .values()
                    .map(|value| &value.original_name)
                    .collect::<Vec<_>>(),
            )
            .finish()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TransportEnvError {
    Conflict { upper: String, lower: String },
    Unsupported { name: String },
    Incomplete { missing: &'static str },
    DenoCertMismatch,
    Proxy(ProxyUrlError),
    NoProxy(NoProxyError),
    CaPathNotAbsolute,
}

impl fmt::Display for TransportEnvError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { upper, lower } => {
                write!(f, "{upper} and {lower} are both set")
            }
            Self::Unsupported { name } => {
                write!(f, "{name} is not supported by this transport mode")
            }
            Self::Incomplete { missing } => {
                write!(f, "transport proxy configuration requires {missing}")
            }
            Self::DenoCertMismatch => write!(f, "DENO_CERT requires an identical SSL_CERT_FILE"),
            Self::Proxy(error) => write!(f, "HTTPS_PROXY is not usable: {error}"),
            Self::NoProxy(error) => write!(f, "NO_PROXY is not usable: {error}"),
            Self::CaPathNotAbsolute => write!(f, "SSL_CERT_FILE must be an absolute path"),
        }
    }
}

impl Error for TransportEnvError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Proxy(error) => Some(error),
            Self::NoProxy(error) => Some(error),
            Self::Conflict { .. }
            | Self::Unsupported { .. }
            | Self::Incomplete { .. }
            | Self::DenoCertMismatch
            | Self::CaPathNotAbsolute => None,
        }
    }
}

impl From<TransportEnvError> for AppError {
    fn from(error: TransportEnvError) -> Self {
        AppError::new(AppErrorKind::Validation, error.to_string())
            .with_suggestion("Use direct public roots, an absolute SSL_CERT_FILE, or the documented loopback HTTPS proxy mode.")
            .with_source(error)
    }
}

impl TransportEnvInputs {
    pub fn from_process(process: &ProcessEnvSnapshot) -> Self {
        let mut values = BTreeMap::new();
        for key in TransportKey::ALL {
            let lookup = if process.inputs.os == OsFamily::Windows {
                key.windows_key()
            } else {
                Some(key.name())
            };
            let Some(lookup) = lookup else { continue };
            if let Some(value) = process.inputs.process_env.get(lookup) {
                let original_name = process
                    .original_names
                    .get(lookup)
                    .cloned()
                    .unwrap_or_else(|| lookup.to_owned());
                values.insert(
                    key,
                    CapturedVar {
                        original_name,
                        value: value.clone(),
                    },
                );
            }
        }
        Self { values }
    }

    fn nonempty(&self, key: TransportKey) -> Option<&str> {
        self.values
            .get(&key)
            .and_then(|captured| (!captured.value.is_empty()).then_some(captured.value.as_str()))
    }

    fn name(&self, key: TransportKey) -> String {
        self.values
            .get(&key)
            .map(|captured| captured.original_name.clone())
            .unwrap_or_else(|| key.name().to_owned())
    }

    /// Resolves the reviewed v3 policy without consulting ambient state.
    pub fn resolve(
        &self,
        deadline: Deadline,
        max_response_bytes: ResponseCap,
    ) -> Result<TransportConfig, TransportEnvError> {
        for (upper, lower) in [
            (TransportKey::HttpsProxy, TransportKey::LowerHttpsProxy),
            (TransportKey::HttpProxy, TransportKey::LowerHttpProxy),
            (TransportKey::AllProxy, TransportKey::LowerAllProxy),
            (TransportKey::NoProxy, TransportKey::LowerNoProxy),
        ] {
            if self.nonempty(upper).is_some() && self.nonempty(lower).is_some() {
                return Err(TransportEnvError::Conflict {
                    upper: self.name(upper),
                    lower: self.name(lower),
                });
            }
        }
        for key in [
            TransportKey::HttpProxy,
            TransportKey::LowerHttpProxy,
            TransportKey::AllProxy,
            TransportKey::LowerAllProxy,
            TransportKey::LowerHttpsProxy,
            TransportKey::LowerNoProxy,
            TransportKey::SslCertDir,
            TransportKey::DenoTlsCaStore,
        ] {
            if self.nonempty(key).is_some() {
                return Err(TransportEnvError::Unsupported {
                    name: self.name(key),
                });
            }
        }
        let https_proxy = self.nonempty(TransportKey::HttpsProxy);
        let no_proxy = self.nonempty(TransportKey::NoProxy);
        let cert = self.nonempty(TransportKey::SslCertFile);
        let deno_cert = self.nonempty(TransportKey::DenoCert);
        if deno_cert.is_some() && deno_cert != cert {
            return Err(TransportEnvError::DenoCertMismatch);
        }
        let ca = match cert {
            Some(path) => {
                let path = PathBuf::from(path);
                if !path.is_absolute() {
                    return Err(TransportEnvError::CaPathNotAbsolute);
                }
                CaMode::PublicRootsPlusPem(path)
            }
            None => CaMode::PublicRoots,
        };
        let proxy = match (https_proxy, no_proxy) {
            (None, None) => ProxyMode::Direct,
            (Some(_), None) => {
                return Err(TransportEnvError::Incomplete {
                    missing: "NO_PROXY",
                });
            }
            (None, Some(_)) => {
                return Err(TransportEnvError::Incomplete {
                    missing: "HTTPS_PROXY",
                });
            }
            (Some(_), Some(_)) if cert.is_none() => {
                return Err(TransportEnvError::Incomplete {
                    missing: "SSL_CERT_FILE",
                });
            }
            (Some(url), Some(policy)) => {
                let url = ProxyUrl::parse(url).map_err(TransportEnvError::Proxy)?;
                parse_no_proxy(policy).map_err(TransportEnvError::NoProxy)?;
                ProxyMode::HttpsConnect {
                    url,
                    bypass_loopback: true,
                }
            }
        };
        Ok(TransportConfig {
            proxy,
            ca,
            deadline,
            max_response_bytes,
        })
    }

    pub fn production(&self) -> Result<TransportConfig, TransportEnvError> {
        self.resolve(Deadline::DEFAULT, ResponseCap::DEFAULT)
    }
}
