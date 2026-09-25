//! F02B Gate 2 fixed-host transport probe.
//!
//! Test-only `examples/` binary that `rust/parity/runner/f02b-fixed-host-driver.ts`
//! runs inside the P03C2 confined lane through the runner's real `executeCase`.
//! It reads the runner-owned `HTTPS_PROXY`, `NO_PROXY`, `SSL_CERT_FILE`,
//! `LINEAR_API_KEY` and `LINEAR_GRAPHQL_ENDPOINT` itself (this is not F03
//! environment discovery), forms one typed [`TransportConfig`] through
//! [`ConfinedTransportEnv`], and runs exactly one scenario from a finite enum
//! named by its single argument. Nothing else is accepted on the command
//! line: no URLs, proxy endpoints, paths or keys. The fake asset URLs, the
//! fake response bytes and the deliberately wrong fake key are compiled in.
//!
//! Every scenario builds its GraphQL and asset transports from the **same**
//! configuration, so the loopback GraphQL POST and the fixed-host GETs go
//! through one client factory. The POST reaches `http://127.0.0.1` because
//! `Proxy::https` never intercepts a plain-`http` URL (scheme-based
//! non-interception), not because of `NO_PROXY`.
//!
//! Output is one bounded, deterministic line per step on stdout; the first
//! failing step ends the run with exit 1. Usage or environment errors exit 2
//! with a message on stderr and nothing on stdout. No line ever contains the
//! key, a signed query, a path or the CA path: failures print only the
//! transport's secret-safe `Display`.

#![forbid(unsafe_code)]
#![deny(
    clippy::as_conversions,
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::indexing_slicing
)]

use std::ffi::OsString;
use std::fmt;
use std::io::{self, Write};
use std::net::IpAddr;
use std::process::ExitCode;
use std::time::Duration;

use cynic::QueryBuilder;
use linear_cli::graphql::envelope::GraphQlRequest;
use linear_cli::graphql::operations::teams::{GetTeams, GetTeamsVariables};
use linear_cli::graphql::transport::{
    ApiKey, ApiKeyError, AssetFailure, AssetHttpTransport, AssetUrl, AssetUrlError, CaMode,
    ConfigError, ConfinedEnvError, ConfinedEnvValues, ConfinedTransportEnv, Deadline, EndpointUrl,
    EndpointUrlError, GraphQlTransport, NetworkPhase, ProxyMode, ProxyUrl, ProxyUrlError,
    ResponseCap, TransportBuildError, TransportConfig,
};
use serde_json::{Value, json};

const PRIVATE_URL: &str = "https://uploads.linear.app/private.png?token=fake";
const PRIVATE_BYTES: &[u8] = b"PRIVATE-PROBE-BYTES";
const PRIVATE_REDIRECT_BYTES: &[u8] = b"PRIVATE-REDIRECT-BYTES";
const PUBLIC_URL: &str = "https://public.linear.app/public.png?token=fake";
const PUBLIC_BYTES: &[u8] = b"PUBLIC-PROBE-BYTES";
const WRONG_PATH_URL: &str = "https://uploads.linear.app/other.png?token=fake";
const THIRD_HOST_URL: &str = "https://api.linear.app/private.png?token=fake";
/// A fake key that differs from the case's `LINEAR_API_KEY`.
const WRONG_KEY: &str = "lin_api_fake_wrong";
/// A loopback port nothing listens on inside the lane.
const CLOSED_PROXY: &str = "http://127.0.0.1:1";
/// Finite per-transport total deadline; the runner's case deadline is outer.
const DEADLINE: Duration = Duration::from_secs(10);
/// Well above every fixture body and well below the fixture's 4 MiB bound.
const CAP_BYTES: usize = 1024 * 1024;
/// Below every fixture body, for the bounded-rejection scenario.
const SMALL_CAP_BYTES: usize = 8;

/// Compare the entire typed GraphQL response with the compiled fake fixture.
/// Never include either payload in a diagnostic: a changed fixture could hold
/// tokens or signed URLs, and only the fact of mismatch is part of this probe.
fn expected_teams() -> Value {
    json!({
        "teams": {
            "nodes": [{
                "id": "team-1", "name": "Engineering", "key": "ENG",
                "description": null, "icon": null, "color": "#0000ff",
                "cyclesEnabled": false,
                "createdAt": "2026-01-01T00:00:00.000Z",
                "updatedAt": "2026-01-02T00:00:00.000Z",
                "archivedAt": null,
                "organization": { "id": "org-1", "name": "Acme" }
            }],
            "pageInfo": { "hasNextPage": false, "endCursor": null }
        }
    })
}

fn teams_match_fixture(data: &GetTeams) -> bool {
    serde_json::to_value(data).is_ok_and(|actual| actual == expected_teams())
}

#[derive(Debug)]
enum ProbeBuildError {
    CaRead,
    CaNotRegularFile,
    CaTooLarge,
    CaEmpty,
    CaPem,
    CaNoCertificates,
    Proxy,
    Client,
}

impl From<TransportBuildError> for ProbeBuildError {
    fn from(error: TransportBuildError) -> Self {
        match error {
            TransportBuildError::CaRead { .. } => Self::CaRead,
            TransportBuildError::CaNotRegularFile { .. } => Self::CaNotRegularFile,
            TransportBuildError::CaTooLarge { .. } => Self::CaTooLarge,
            TransportBuildError::CaEmpty { .. } => Self::CaEmpty,
            TransportBuildError::CaPem { .. } => Self::CaPem,
            TransportBuildError::CaNoCertificates { .. } => Self::CaNoCertificates,
            TransportBuildError::Proxy(_) => Self::Proxy,
            TransportBuildError::Client(_) => Self::Client,
        }
    }
}

impl fmt::Display for ProbeBuildError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let detail = match self {
            Self::CaRead => "CA bundle could not be read",
            Self::CaNotRegularFile => "CA bundle is not a regular file",
            Self::CaTooLarge => "CA bundle exceeds the size limit",
            Self::CaEmpty => "CA bundle is empty",
            Self::CaPem => "CA bundle is not a valid PEM bundle",
            Self::CaNoCertificates => "CA bundle contains no certificates",
            Self::Proxy => "proxy configuration was rejected",
            Self::Client => "HTTP client could not be built",
        };
        f.write_str(detail)
    }
}

// ---------------------------------------------------------------------------
// Scenarios

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Scenario {
    /// Cynic POST, private GET with auth, public GET without.
    Both,
    /// Cynic POST, then a private GET that follows one same-origin redirect.
    Redirect,
    /// Private GET under an 8-byte cap: bounded rejection, exact fixture consumption.
    CapBelowBody,
    /// Private GET with a different fake key: the fixture's required header fails.
    WrongAuth,
    /// Private GET for an undeclared path/query.
    WrongPath,
    /// `Both` plus a second private GET after the fixture's final step.
    ExtraGet,
    /// Private GET with only the public roots: TLS must fail at the client.
    PublicRootsOnly,
    /// Private GET through a loopback port nothing listens on.
    WrongProxyPort,
    /// Cynic POST with `first: 50`: the GraphQL matcher fails.
    GraphqlWrongVariables,
    /// Private GET with no proxy: must fail under the lane's egress/DNS denial.
    DirectEgress,
    /// A third host is rejected by the URL type before any request.
    ThirdHost,
}

impl Scenario {
    const ALL: [(&'static str, Scenario); 11] = [
        ("both", Scenario::Both),
        ("redirect", Scenario::Redirect),
        ("cap-below-body", Scenario::CapBelowBody),
        ("wrong-auth", Scenario::WrongAuth),
        ("wrong-path", Scenario::WrongPath),
        ("extra-get", Scenario::ExtraGet),
        ("public-roots-only", Scenario::PublicRootsOnly),
        ("wrong-proxy-port", Scenario::WrongProxyPort),
        ("graphql-wrong-variables", Scenario::GraphqlWrongVariables),
        ("direct-egress", Scenario::DirectEgress),
        ("third-host", Scenario::ThirdHost),
    ];

    fn parse(name: &str) -> Option<Self> {
        Self::ALL
            .iter()
            .find(|(candidate, _)| *candidate == name)
            .map(|(_, scenario)| *scenario)
    }

    fn names() -> String {
        Self::ALL
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>()
            .join(", ")
    }

    fn steps(self) -> Vec<Step> {
        let private = Step::Get {
            label: "private",
            url: PRIVATE_URL,
            expected: PRIVATE_BYTES,
            report: Report::Display,
        };
        let public = Step::Get {
            label: "public",
            url: PUBLIC_URL,
            expected: PUBLIC_BYTES,
            report: Report::Display,
        };
        match self {
            Self::Both => vec![Step::GraphQl { first: 100 }, private, public],
            Self::Redirect => vec![
                Step::GraphQl { first: 100 },
                Step::Get {
                    label: "private",
                    url: PRIVATE_URL,
                    expected: PRIVATE_REDIRECT_BYTES,
                    report: Report::Display,
                },
            ],
            Self::CapBelowBody | Self::WrongAuth | Self::PublicRootsOnly | Self::WrongProxyPort => {
                vec![private]
            }
            Self::DirectEgress => vec![Step::Get {
                label: "private",
                url: PRIVATE_URL,
                expected: PRIVATE_BYTES,
                report: Report::PhaseOnly,
            }],
            Self::WrongPath => vec![Step::Get {
                label: "private",
                url: WRONG_PATH_URL,
                expected: PRIVATE_BYTES,
                report: Report::Display,
            }],
            Self::ExtraGet => vec![Step::GraphQl { first: 100 }, private, public, private],
            Self::GraphqlWrongVariables => vec![Step::GraphQl { first: 50 }],
            Self::ThirdHost => vec![Step::Reject {
                label: "third-host",
                url: THIRD_HOST_URL,
            }],
        }
    }
}

#[derive(Clone, Copy, Debug)]
enum Step {
    GraphQl {
        first: i32,
    },
    Get {
        label: &'static str,
        url: &'static str,
        expected: &'static [u8],
        report: Report,
    },
    /// Parse only; the URL type must refuse it before any request.
    Reject {
        label: &'static str,
        url: &'static str,
    },
}

/// How a failed GET is printed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Report {
    /// The transport's secret-safe `Display`, including the root message.
    Display,
    /// Phase and origin only. The direct-egress scenario fails at the lane's
    /// confinement boundary, whose OS text is not a transport contract.
    PhaseOnly,
}

// ---------------------------------------------------------------------------
// Settings from the runner-owned environment

#[derive(Debug)]
enum ProbeError {
    Usage(String),
    NotUnicode(&'static str),
    Env(ConfinedEnvError),
    Key(ApiKeyError),
    Endpoint(EndpointUrlError),
    EndpointNotHttpLoopback,
    Config(ConfigError),
    Proxy(ProxyUrlError),
    Build(ProbeBuildError),
    CompiledUrl(AssetUrlError),
    Runtime(io::Error),
    Stdout(io::Error),
}

impl fmt::Display for ProbeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(detail) => write!(
                f,
                "{detail}; usage: f02b_fixed_host_probe <scenario> where scenario is one of {}",
                Scenario::names()
            ),
            Self::NotUnicode(name) => write!(f, "{name} is not valid UTF-8"),
            Self::Env(source) => write!(f, "runner environment rejected: {source}"),
            Self::Key(source) => write!(f, "LINEAR_API_KEY rejected: {source}"),
            Self::Endpoint(source) => write!(f, "LINEAR_GRAPHQL_ENDPOINT rejected: {source}"),
            Self::EndpointNotHttpLoopback => {
                write!(
                    f,
                    "LINEAR_GRAPHQL_ENDPOINT must be a plain-http loopback URL"
                )
            }
            Self::Config(source) => write!(f, "probe limits rejected: {source}"),
            Self::Proxy(source) => write!(f, "compiled proxy override rejected: {source}"),
            Self::Build(source) => write!(f, "transport could not be built: {source}"),
            Self::CompiledUrl(source) => write!(f, "compiled asset URL rejected: {source}"),
            Self::Runtime(source) => write!(f, "runtime could not start: {source}"),
            Self::Stdout(source) => write!(f, "stdout write failed: {source}"),
        }
    }
}

struct Settings {
    endpoint: EndpointUrl,
    api_key: ApiKey,
    config: TransportConfig,
}

fn env_var(name: &'static str) -> Result<Option<String>, ProbeError> {
    match std::env::var_os(name) {
        None => Ok(None),
        Some(value) => value
            .into_string()
            .map(Some)
            .map_err(|_| ProbeError::NotUnicode(name)),
    }
}

fn is_loopback(host: &str) -> bool {
    if host == "localhost" {
        return true;
    }
    let bare = host
        .strip_prefix('[')
        .and_then(|inner| inner.strip_suffix(']'))
        .unwrap_or(host);
    bare.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
}

fn read_settings(scenario: Scenario) -> Result<Settings, ProbeError> {
    let values = ConfinedEnvValues {
        https_proxy: env_var("HTTPS_PROXY")?,
        http_proxy: env_var("HTTP_PROXY")?,
        all_proxy: env_var("ALL_PROXY")?,
        no_proxy: env_var("NO_PROXY")?,
        ssl_cert_file: env_var("SSL_CERT_FILE")?,
    };
    // The runner-owned values must be exact before any scenario override.
    let confined = ConfinedTransportEnv::parse(&values).map_err(ProbeError::Env)?;
    let key_text = env_var("LINEAR_API_KEY")?.unwrap_or_default();
    let endpoint_text = env_var("LINEAR_GRAPHQL_ENDPOINT")?.unwrap_or_default();
    let endpoint = EndpointUrl::parse(&endpoint_text).map_err(ProbeError::Endpoint)?;
    let loopback =
        endpoint.url().scheme() == "http" && endpoint.url().host_str().is_some_and(is_loopback);
    if !loopback {
        return Err(ProbeError::EndpointNotHttpLoopback);
    }
    let api_key = match scenario {
        Scenario::WrongAuth => ApiKey::new(WRONG_KEY.to_owned()),
        _ => ApiKey::new(key_text),
    }
    .map_err(ProbeError::Key)?;
    let cap_bytes = match scenario {
        Scenario::CapBelowBody => SMALL_CAP_BYTES,
        _ => CAP_BYTES,
    };
    let deadline = Deadline::new(DEADLINE).map_err(ProbeError::Config)?;
    let cap = ResponseCap::new(cap_bytes).map_err(ProbeError::Config)?;
    let mut config = confined.into_config(deadline, cap);
    match scenario {
        Scenario::PublicRootsOnly => config.ca = CaMode::PublicRoots,
        Scenario::WrongProxyPort => {
            config.proxy = ProxyMode::HttpsConnect {
                url: ProxyUrl::parse(CLOSED_PROXY).map_err(ProbeError::Proxy)?,
                bypass_loopback: true,
            };
        }
        Scenario::DirectEgress => {
            config.proxy = ProxyMode::Direct;
            config.ca = CaMode::PublicRoots;
        }
        Scenario::Both
        | Scenario::Redirect
        | Scenario::CapBelowBody
        | Scenario::WrongAuth
        | Scenario::WrongPath
        | Scenario::ExtraGet
        | Scenario::GraphqlWrongVariables
        | Scenario::ThirdHost => {}
    }
    Ok(Settings {
        endpoint,
        api_key,
        config,
    })
}

fn parse_scenario(args: &[OsString]) -> Result<Scenario, ProbeError> {
    let [argument] = args else {
        return Err(ProbeError::Usage(format!(
            "expected exactly one argument, got {}",
            args.len()
        )));
    };
    let Some(name) = argument.to_str() else {
        return Err(ProbeError::Usage("scenario name is not UTF-8".to_owned()));
    };
    Scenario::parse(name).ok_or_else(|| ProbeError::Usage("unknown scenario".to_owned()))
}

// ---------------------------------------------------------------------------
// Steps

/// `Ok(true)` when the step passed, `Ok(false)` when it failed (the line was
/// printed); `Err` is a probe error (exit 2).
async fn run_step(
    out: &mut impl Write,
    settings: &Settings,
    step: Step,
) -> Result<bool, ProbeError> {
    match step {
        Step::GraphQl { first } => {
            let transport = GraphQlTransport::new(
                settings.endpoint.clone(),
                settings.api_key.clone(),
                settings.config.clone(),
            )
            .map_err(|error| ProbeError::Build(error.into()))?;
            let request = GraphQlRequest::with_variables(GetTeams::build(GetTeamsVariables {
                filter: None,
                first: Some(first),
                after: None,
            }));
            match transport.execute::<GetTeams, _>(&request).await {
                Ok(data) if teams_match_fixture(&data) => {
                    emit(out, &format!("graphql ok teams={}", data.teams.nodes.len()))?;
                    Ok(true)
                }
                Ok(_) => {
                    emit(out, "graphql data differs")?;
                    Ok(false)
                }
                Err(failure) => {
                    emit(out, &format!("graphql failed: {failure}"))?;
                    Ok(false)
                }
            }
        }
        Step::Get {
            label,
            url,
            expected,
            report,
        } => {
            let url = AssetUrl::parse(url).map_err(ProbeError::CompiledUrl)?;
            let transport =
                AssetHttpTransport::new(settings.api_key.clone(), settings.config.clone())
                    .map_err(|error| ProbeError::Build(error.into()))?;
            match transport.get(&url).await {
                Ok(asset) => {
                    let status = asset.response.status.as_u16();
                    if status != 200 {
                        emit(out, &format!("{label} status {status}"))?;
                        return Ok(false);
                    }
                    if asset.response.body != expected {
                        emit(
                            out,
                            &format!("{label} body differs ({} bytes)", asset.response.body.len()),
                        )?;
                        return Ok(false);
                    }
                    emit(
                        out,
                        &format!(
                            "{label} ok requests={} bytes={}",
                            asset.requests,
                            asset.response.body.len()
                        ),
                    )?;
                    Ok(true)
                }
                Err(failure) => {
                    emit(
                        out,
                        &format!("{label} failed: {}", describe(&failure, report)),
                    )?;
                    Ok(false)
                }
            }
        }
        Step::Reject { label, url } => match AssetUrl::parse(url) {
            Ok(_) => {
                emit(out, &format!("{label} accepted unexpectedly"))?;
                Ok(false)
            }
            Err(error) => {
                emit(out, &format!("{label} rejected: {error}"))?;
                Ok(false)
            }
        },
    }
}

fn describe(failure: &AssetFailure, report: Report) -> String {
    match (report, failure) {
        (Report::PhaseOnly, AssetFailure::Network { origin, phase, .. }) => {
            let phase = match phase {
                NetworkPhase::Connect => "connection to",
                NetworkPhase::Request => "request to",
                NetworkPhase::Body => "reading the response from",
                NetworkPhase::Other => "exchange with",
            };
            format!("{phase} {origin} failed (root message elided)")
        }
        (Report::PhaseOnly | Report::Display, failure) => failure.to_string(),
    }
}

fn emit(out: &mut impl Write, line: &str) -> Result<(), ProbeError> {
    writeln!(out, "{line}")
        .and_then(|()| out.flush())
        .map_err(ProbeError::Stdout)
}

async fn run(settings: &Settings, scenario: Scenario) -> Result<ExitCode, ProbeError> {
    let stdout = io::stdout();
    let mut out = stdout.lock();
    for step in scenario.steps() {
        if !run_step(&mut out, settings, step).await? {
            return Ok(ExitCode::from(1));
        }
    }
    Ok(ExitCode::SUCCESS)
}

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    let outcome = parse_scenario(&args).and_then(|scenario| {
        let settings = read_settings(scenario)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(ProbeError::Runtime)?;
        runtime.block_on(run(&settings, scenario))
    });
    match outcome {
        Ok(code) => code,
        Err(error) => {
            eprintln!("f02b_fixed_host_probe: {error}");
            ExitCode::from(2)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{ProbeBuildError, ProbeError, expected_teams, teams_match_fixture};
    use linear_cli::graphql::envelope::parse_response;
    use linear_cli::graphql::operations::teams::GetTeams;
    use linear_cli::graphql::transport::TransportBuildError;
    use std::io;
    use std::path::PathBuf;

    #[test]
    fn entire_typed_fixture_is_compared() {
        let body = serde_json::json!({ "data": expected_teams() }).to_string();
        let parsed = parse_response::<GetTeams>(body.as_bytes());
        assert!(parsed.is_ok(), "typed fixture: {parsed:?}");
        let Ok(data) = parsed else { return };
        assert!(teams_match_fixture(&data));
        let mut changed = data.clone();
        assert_eq!(changed.teams.nodes.len(), 1);
        for node in &mut changed.teams.nodes {
            node.name = "Changed".to_owned();
        }
        assert!(!teams_match_fixture(&changed));
        for node in &mut changed.teams.nodes {
            node.name = "Engineering".to_owned();
        }
        changed.teams.page_info.has_next_page = true;
        assert!(!teams_match_fixture(&changed));
    }

    #[test]
    fn ca_build_errors_never_expose_path_in_display_or_debug() {
        let marker = "/private/signed-ca-path.pem";
        let path = PathBuf::from(marker);
        for error in [
            TransportBuildError::CaRead {
                path: path.clone(),
                source: io::Error::new(io::ErrorKind::NotFound, "secret source"),
            },
            TransportBuildError::CaNotRegularFile { path: path.clone() },
            TransportBuildError::CaEmpty { path: path.clone() },
            TransportBuildError::CaNoCertificates { path },
        ] {
            let sanitized = ProbeError::Build(ProbeBuildError::from(error));
            assert!(!sanitized.to_string().contains(marker));
            assert!(!format!("{sanitized:?}").contains(marker));
            assert!(!format!("{sanitized:?}").contains("secret source"));
        }
    }
}
