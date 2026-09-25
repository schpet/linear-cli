use std::ffi::OsString;
use std::path::{Path, PathBuf};

use linear_cli::config::{OsFamily, ProcessEnvSnapshot, TransportEnvError, TransportEnvInputs};
use linear_cli::graphql::transport::{CaMode, Deadline, ProxyMode, ResponseCap};

fn inputs(os: OsFamily, vars: &[(&str, &str)]) -> TransportEnvInputs {
    let snapshot = ProcessEnvSnapshot::from_vars_os(
        PathBuf::from("/work"),
        os,
        vars.iter()
            .map(|(name, value)| (OsString::from(name), OsString::from(value))),
    )
    .expect("synthetic process environment");
    TransportEnvInputs::from_process(&snapshot)
}

fn absolute_ca() -> &'static str {
    if cfg!(windows) {
        "C:\\private\\ca.pem"
    } else {
        "/private/ca.pem"
    }
}

fn unix(vars: &[(&str, &str)]) -> TransportEnvInputs {
    inputs(OsFamily::Unix, vars)
}

#[test]
fn default_and_empty_inputs_are_direct_with_finite_defaults() {
    for vars in [
        vec![],
        vec![
            ("HTTPS_PROXY", ""),
            ("HTTP_PROXY", ""),
            ("ALL_PROXY", ""),
            ("NO_PROXY", ""),
            ("SSL_CERT_FILE", ""),
            ("SSL_CERT_DIR", ""),
            ("DENO_CERT", ""),
            ("DENO_TLS_CA_STORE", ""),
        ],
    ] {
        let config = unix(&vars).production().expect("direct transport");
        assert!(matches!(config.proxy, ProxyMode::Direct));
        assert!(matches!(config.ca, CaMode::PublicRoots));
        assert_eq!(config.deadline, Deadline::DEFAULT);
        assert_eq!(config.max_response_bytes, ResponseCap::DEFAULT);
    }
}

#[test]
fn injected_finite_deadline_and_cap_replace_defaults_only_for_callers() {
    let deadline = Deadline::new(std::time::Duration::from_millis(175)).expect("deadline");
    let cap = ResponseCap::new(1024).expect("cap");
    let config = unix(&[]).resolve(deadline, cap).expect("test override");
    assert_eq!(config.deadline, deadline);
    assert_eq!(config.max_response_bytes, cap);
    assert_eq!(
        unix(&[]).production().expect("production").deadline,
        Deadline::DEFAULT
    );
}

#[test]
fn additive_ca_accepts_matching_deno_cert_without_proxy() {
    for vars in [
        vec![("SSL_CERT_FILE", absolute_ca())],
        vec![
            ("SSL_CERT_FILE", absolute_ca()),
            ("DENO_CERT", absolute_ca()),
        ],
    ] {
        let config = unix(&vars).production().expect("additive CA");
        assert!(matches!(config.proxy, ProxyMode::Direct));
        assert!(
            matches!(config.ca, CaMode::PublicRootsPlusPem(path) if path == Path::new(absolute_ca()))
        );
    }
}

#[test]
fn exact_loopback_proxy_shape_accepts_either_bypass_order() {
    for no_proxy in ["127.0.0.1,localhost", "localhost,127.0.0.1"] {
        let config = unix(&[
            ("HTTPS_PROXY", "http://127.0.0.1:43111"),
            ("NO_PROXY", no_proxy),
            ("SSL_CERT_FILE", absolute_ca()),
            ("DENO_CERT", absolute_ca()),
        ])
        .production()
        .expect("loopback mode");
        assert!(matches!(
            config.proxy,
            ProxyMode::HttpsConnect {
                bypass_loopback: true,
                ..
            }
        ));
        assert!(matches!(config.ca, CaMode::PublicRootsPlusPem(_)));
        assert!(!format!("{config:?}").contains("127.0.0.1:43111"));
    }
}

#[test]
fn unix_upper_lower_conflicts_are_explicit_even_for_equal_values() {
    for (upper, lower) in [
        ("HTTPS_PROXY", "https_proxy"),
        ("HTTP_PROXY", "http_proxy"),
        ("ALL_PROXY", "all_proxy"),
        ("NO_PROXY", "no_proxy"),
    ] {
        let error = unix(&[(upper, "same"), (lower, "same")])
            .production()
            .expect_err("conflict");
        assert!(matches!(error, TransportEnvError::Conflict { .. }));
        assert!(error.to_string().contains(upper));
        assert!(error.to_string().contains(lower));
        assert!(!error.to_string().contains("same"));
    }
}

#[test]
fn every_unsupported_nonempty_setting_fails_without_value_echo() {
    for name in [
        "HTTP_PROXY",
        "http_proxy",
        "ALL_PROXY",
        "all_proxy",
        "https_proxy",
        "no_proxy",
        "SSL_CERT_DIR",
        "DENO_TLS_CA_STORE",
    ] {
        let error = unix(&[(name, "sentinel-private-value")])
            .production()
            .expect_err("unsupported setting");
        assert!(
            matches!(error, TransportEnvError::Unsupported { .. }),
            "{name}: {error}"
        );
        assert!(error.to_string().contains(name));
        assert!(!error.to_string().contains("sentinel-private-value"));
    }
}

#[test]
fn incomplete_proxy_and_deno_cert_cannot_fall_back_to_direct() {
    let cases = [
        (vec![("HTTPS_PROXY", "http://127.0.0.1:1")], "NO_PROXY"),
        (vec![("NO_PROXY", "127.0.0.1,localhost")], "HTTPS_PROXY"),
        (
            vec![
                ("HTTPS_PROXY", "http://127.0.0.1:1"),
                ("NO_PROXY", "127.0.0.1,localhost"),
            ],
            "SSL_CERT_FILE",
        ),
    ];
    for (vars, missing) in cases {
        assert!(matches!(
            unix(&vars).production(),
            Err(TransportEnvError::Incomplete { missing: actual }) if actual == missing
        ));
    }
    for vars in [
        vec![("DENO_CERT", "/private/a.pem")],
        vec![
            ("DENO_CERT", "/private/a.pem"),
            ("SSL_CERT_FILE", "/private/b.pem"),
        ],
    ] {
        assert!(matches!(
            unix(&vars).production(),
            Err(TransportEnvError::DenoCertMismatch)
        ));
    }
}

#[test]
fn malformed_proxy_policy_and_relative_ca_are_errors() {
    let base = [
        ("HTTPS_PROXY", "http://127.0.0.1:1"),
        ("NO_PROXY", "127.0.0.1,localhost"),
        ("SSL_CERT_FILE", absolute_ca()),
    ];
    let mut vars = base;
    vars[0].1 = "https://proxy.example.invalid:443";
    assert!(matches!(
        unix(&vars).production(),
        Err(TransportEnvError::Proxy(_))
    ));
    let mut vars = base;
    vars[1].1 = "*";
    assert!(matches!(
        unix(&vars).production(),
        Err(TransportEnvError::NoProxy(_))
    ));
    let mut vars = base;
    vars[2].1 = "relative.pem";
    assert!(matches!(
        unix(&vars).production(),
        Err(TransportEnvError::CaPathNotAbsolute)
    ));
}

#[test]
fn windows_normalization_uses_original_name_for_diagnostics() {
    let error = inputs(OsFamily::Windows, &[("http_proxy", "http://127.0.0.1:1")])
        .production()
        .expect_err("unsupported proxy");
    assert!(matches!(error, TransportEnvError::Unsupported { name } if name == "http_proxy"));
    let config = inputs(OsFamily::Windows, &[("ssl_cert_file", absolute_ca())])
        .production()
        .expect("case-folded CA setting");
    assert!(matches!(config.ca, CaMode::PublicRootsPlusPem(_)));
}

#[test]
fn captured_inputs_are_immutable_when_ambient_process_changes() {
    let captured = unix(&[]);
    // The adapter never calls std::env; a separate snapshot cannot affect it.
    let later = unix(&[("HTTP_PROXY", "sentinel-private-value")]);
    assert!(!format!("{later:?}").contains("sentinel-private-value"));
    assert!(matches!(
        captured.production().expect("direct").proxy,
        ProxyMode::Direct
    ));
    assert!(later.production().is_err());
}

#[test]
fn malformed_transport_values_are_absent_from_display_and_debug() {
    for proxy in [
        "corp.example:3128",
        "https://sentinel.invalid",
        "http://user:sentinel@127.0.0.1:1",
    ] {
        let captured = unix(&[
            ("HTTPS_PROXY", proxy),
            ("NO_PROXY", "127.0.0.1,localhost"),
            ("SSL_CERT_FILE", absolute_ca()),
        ]);
        assert!(!format!("{captured:?}").contains("sentinel"));
        assert!(!format!("{captured:?}").contains("corp.example"));
        let error = captured.production().expect_err("invalid proxy");
        for rendered in [error.to_string(), format!("{error:?}")] {
            assert!(!rendered.contains("sentinel"), "{rendered}");
            assert!(!rendered.contains("corp.example"), "{rendered}");
        }
    }
    let captured = unix(&[
        ("HTTPS_PROXY", "http://127.0.0.1:1"),
        ("NO_PROXY", "sentinel.invalid"),
        ("SSL_CERT_FILE", absolute_ca()),
    ]);
    let error = captured.production().expect_err("invalid bypass");
    assert!(!error.to_string().contains("sentinel"));
    assert!(!format!("{error:?}").contains("sentinel"));
}
