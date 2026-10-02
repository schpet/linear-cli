//! Transport settings read from the process environment at startup.
//!
//! Proxy variables are not captured here: reqwest reads `HTTP_PROXY`,
//! `HTTPS_PROXY`, `ALL_PROXY` and `NO_PROXY` (either case) itself.
use std::path::PathBuf;

use crate::graphql::transport::TransportConfig;

use super::runtime::ProcessEnvSnapshot;

/// The process environment values that shape a network transport.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TransportEnvInputs {
    ca_bundle: Option<PathBuf>,
}

impl TransportEnvInputs {
    pub fn from_process(process: &ProcessEnvSnapshot) -> Self {
        let env = |name| {
            process
                .inputs
                .env(name)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from)
        };
        // DENO_CERT is a deprecated alias kept for users upgrading from 2.x.
        let ca_bundle = env("SSL_CERT_FILE").or_else(|| env("DENO_CERT"));
        Self { ca_bundle }
    }

    /// The transport configuration with default deadline and response cap.
    pub fn production(&self) -> TransportConfig {
        TransportConfig {
            ca_bundle: self.ca_bundle.clone(),
            ..TransportConfig::default()
        }
    }
}
