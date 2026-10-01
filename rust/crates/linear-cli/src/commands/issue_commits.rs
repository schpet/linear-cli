//! Exact issue commit lookup and jj output, with command-local process policy.
use crate::{
    commands::issue_id,
    config::Vcs,
    error::{AppError, AppErrorKind, ExitStatus},
    graphql::{
        bulk_error::{self, ObservedExchangeFailure},
        transport::GraphQlTransport,
    },
    platform::vcs_script::{self, ChildOutcome, CommandSpec, ProcessRunner, Program},
};
use serde::Deserialize;
use std::{num::NonZeroU8, path::Path};
pub const CONTEXT: &str = "Failed to show commits";
pub fn check_vcs(vcs: Vcs) -> Result<(), AppError> {
    match vcs {
        Vcs::Jj => Ok(()),
        Vcs::Git => Err(AppError::new(
            AppErrorKind::Validation,
            "commits is only supported with jj-vcs",
        )
        .with_suggestion("This command requires jujutsu (jj) version control.")),
    }
}
#[derive(Deserialize)]
struct Lookup {
    #[serde(default)]
    issue: Option<LookupIssue>,
}
struct LookupIssue {
    id: Option<String>,
}
impl<'de> Deserialize<'de> for LookupIssue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct ObjectVisitor;
        impl<'de> serde::de::Visitor<'de> for ObjectVisitor {
            type Value = LookupIssue;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an issue object")
            }
            fn visit_map<A: serde::de::MapAccess<'de>>(
                self,
                map: A,
            ) -> Result<Self::Value, A::Error> {
                #[derive(Deserialize)]
                struct Fields {
                    #[serde(default)]
                    id: Option<String>,
                }
                let fields =
                    Fields::deserialize(serde::de::value::MapAccessDeserializer::new(map))?;
                Ok(LookupIssue { id: fields.id })
            }
        }
        // Derived structs also accept JSON arrays as positional fields. This
        // selected GraphQL field requires an object; absent/null still mean None.
        deserializer.deserialize_map(ObjectVisitor)
    }
}
/// Source optional/missing fields are absence, not old shared issue_id strict shape.
pub async fn lookup(transport: &GraphQlTransport, identifier: &str) -> Result<(), AppError> {
    let mut request = issue_id::request(identifier);
    request.query = request.query.trim_end_matches('\n').to_owned();
    let result: Lookup = bulk_error::execute_observed(transport, &request)
        .await
        .map_err(|failure| lookup_failure(failure, identifier))?;
    if result
        .issue
        .and_then(|issue| issue.id)
        .is_some_and(|id| !id.is_empty())
    {
        Ok(())
    } else {
        Err(AppError::not_found("Issue", identifier))
    }
}
pub fn lookup_failure(failure: ObservedExchangeFailure, identifier: &str) -> AppError {
    match failure {
        ObservedExchangeFailure::Strict(error) => error,
        ObservedExchangeFailure::Ordinary(error) if error.is_not_found() => {
            AppError::not_found("Issue", identifier)
        }
        ObservedExchangeFailure::Ordinary(error) => AppError::new(
            AppErrorKind::GraphQl,
            error.preferred_message.unwrap_or(error.message),
        ),
    }
}
pub fn revset(identifier: &str) -> String {
    format!("description(regex:\"(?m)^Linear-issue:.*{identifier}\")")
}
pub fn probe_spec(identifier: &str) -> CommandSpec {
    CommandSpec::new(
        Program::Jj,
        &[
            "log",
            "-r",
            &revset(identifier),
            "-T",
            "commit_id",
            "--no-graph",
        ],
    )
}
pub fn show_spec(identifier: &str) -> CommandSpec {
    CommandSpec::new(
        Program::Jj,
        &[
            "log",
            "-r",
            &revset(identifier),
            "-p",
            "--git",
            "--no-graph",
            "-T",
            "builtin_log_compact_full_description",
        ],
    )
}
pub fn child_status(outcome: ChildOutcome) -> Result<ExitStatus, AppError> {
    let code = match outcome {
        ChildOutcome::Code(code) => code,
        ChildOutcome::Signal(signal) => 128_i32.checked_add(signal).ok_or_else(|| {
            AppError::new(
                AppErrorKind::IoProcess,
                format!("Child signal {signal} cannot be represented as an exit code"),
            )
        })?,
    };
    let code = u8::try_from(code).map_err(|_| {
        AppError::new(
            AppErrorKind::IoProcess,
            format!("Child exit code {code} is outside supported range 0..255"),
        )
    })?;
    Ok(match NonZeroU8::new(code) {
        Some(code) => ExitStatus::ChildCode(code),
        None => ExitStatus::Success,
    })
}
pub fn show(
    runner: &mut impl ProcessRunner,
    identifier: &str,
    cwd: &Path,
    env: &crate::config::ChildEnvOverlay,
) -> Result<ExitStatus, AppError> {
    let captured = runner.capture(&probe_spec(identifier), cwd, env)?;
    // Source ignores probe exit status/stderr, including nonzero + nonempty.
    if vcs_script::decoded_trim(&captured.stdout).is_empty() {
        return Err(AppError::not_found("Commits", identifier));
    }
    child_status(runner.inherit(&show_spec(identifier), cwd, env)?)
}
