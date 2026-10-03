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

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::Path;

    use super::*;
    use crate::config::OsFamily;
    use crate::graphql::transport::{Deadline, ResponseCap};

    fn production(vars: &[(&str, &str)]) -> TransportConfig {
        let snapshot = ProcessEnvSnapshot::from_vars_os(
            PathBuf::from("/work"),
            OsFamily::Unix,
            vars.iter()
                .map(|(name, value)| (OsString::from(name), OsString::from(value))),
        )
        .expect("process environment");
        TransportEnvInputs::from_process(&snapshot).production()
    }

    #[test]
    fn defaults_apply_without_a_ca_bundle() {
        for vars in [
            &[][..],
            &[("SSL_CERT_FILE", ""), ("DENO_CERT", "")],
            &[("HTTPS_PROXY", "http://proxy.example:3128")],
        ] {
            let config = production(vars);
            assert_eq!(config.ca_bundle, None);
            assert_eq!(config.deadline, Deadline::DEFAULT);
            assert_eq!(config.max_response_bytes, ResponseCap::DEFAULT);
        }
    }

    #[test]
    fn ssl_cert_file_adds_a_ca_bundle() {
        let config = production(&[("SSL_CERT_FILE", "certs/ca.pem")]);
        assert_eq!(config.ca_bundle.as_deref(), Some(Path::new("certs/ca.pem")));
    }

    #[test]
    fn the_legacy_cert_variable_is_a_fallback_for_ssl_cert_file() {
        let config = production(&[("DENO_CERT", "/legacy/ca.pem")]);
        assert_eq!(
            config.ca_bundle.as_deref(),
            Some(Path::new("/legacy/ca.pem"))
        );
        let config = production(&[
            ("DENO_CERT", "/legacy/ca.pem"),
            ("SSL_CERT_FILE", "/current/ca.pem"),
        ]);
        assert_eq!(
            config.ca_bundle.as_deref(),
            Some(Path::new("/current/ca.pem"))
        );
    }
}
