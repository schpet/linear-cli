use std::ffi::OsString;
use std::path::{Path, PathBuf};

use linear_cli::config::{OsFamily, ProcessEnvSnapshot, TransportEnvInputs};
use linear_cli::graphql::transport::{Deadline, ResponseCap};

fn unix(vars: &[(&str, &str)]) -> TransportEnvInputs {
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/work"),
        OsFamily::Unix,
        vars.iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value))),
    )
    .expect("synthetic process environment");
    TransportEnvInputs::from_process(&snapshot)
}

#[test]
fn defaults_apply_without_a_ca_bundle() {
    for vars in [vec![], vec![("SSL_CERT_FILE", ""), ("DENO_CERT", "")]] {
        let config = unix(&vars).production();
        assert_eq!(config.ca_bundle, None);
        assert_eq!(config.deadline, Deadline::DEFAULT);
        assert_eq!(config.max_response_bytes, ResponseCap::DEFAULT);
    }
}

#[test]
fn proxy_variables_do_not_affect_captured_inputs() {
    let config = unix(&[
        ("HTTPS_PROXY", "http://proxy.example:3128"),
        ("http_proxy", "http://proxy.example:3128"),
        ("NO_PROXY", "internal.example"),
    ])
    .production();
    assert_eq!(config.ca_bundle, None);
}

#[test]
fn ssl_cert_file_adds_a_ca_bundle() {
    let config = unix(&[("SSL_CERT_FILE", "certs/ca.pem")]).production();
    assert_eq!(config.ca_bundle.as_deref(), Some(Path::new("certs/ca.pem")));
}

#[test]
fn deno_cert_is_a_fallback_for_ssl_cert_file() {
    let config = unix(&[("DENO_CERT", "/legacy/ca.pem")]).production();
    assert_eq!(
        config.ca_bundle.as_deref(),
        Some(Path::new("/legacy/ca.pem"))
    );

    let config = unix(&[
        ("DENO_CERT", "/legacy/ca.pem"),
        ("SSL_CERT_FILE", "/current/ca.pem"),
    ])
    .production();
    assert_eq!(
        config.ca_bundle.as_deref(),
        Some(Path::new("/current/ca.pem"))
    );
}
