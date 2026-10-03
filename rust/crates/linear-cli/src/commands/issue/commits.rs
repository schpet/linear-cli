//! `issue commits`: the jj commits whose trailers name an issue.
use crate::{
    cli::issue::IssueCommits,
    config::Vcs,
    ctx::Ctx,
    error::{Error, Result, ResultExt},
    graphql::transport::GraphQlTransport,
    platform::vcs_script::{self, ChildOutcome, CommandSpec, ProcessRunner, Program},
};
use serde::Deserialize;
use std::{num::NonZeroU8, path::Path};
pub fn run(ctx: &Ctx, args: &IssueCommits) -> Result<()> {
    show_commits(ctx, args).context("Failed to show commits")
}

fn show_commits(ctx: &Ctx, args: &IssueCommits) -> Result<()> {
    check_vcs(super::vcs(ctx))?;
    let identifier = super::require(ctx, args.issue_id.as_deref())?;
    let client = ctx.client()?;
    ctx.spin(true, lookup(client, &identifier))?;
    ctx.flush()?;
    show(
        &mut vcs_script::NativeProcessRunner,
        &identifier,
        ctx.cwd(),
        &ctx.config().child_env,
    )
}
pub fn check_vcs(vcs: Vcs) -> Result<(), Error> {
    match vcs {
        Vcs::Jj => Ok(()),
        Vcs::Git => Err(Error::new("commits is only supported with jj-vcs")
            .with_hint("This command requires jujutsu (jj) version control.")),
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
/// Look up the issue; a missing or null issue is "not found".
pub async fn lookup(transport: &GraphQlTransport, identifier: &str) -> Result<(), Error> {
    let request = super::id::request(identifier);
    let result: Lookup = transport
        .execute(&request)
        .await
        .map_err(|failure| failure.or_not_found("Issue", identifier))?;
    if result
        .issue
        .and_then(|issue| issue.id)
        .is_some_and(|id| !id.is_empty())
    {
        Ok(())
    } else {
        Err(Error::not_found("Issue", identifier))
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
/// Passes a child's failing exit status through as this command's own.
pub fn child_status(outcome: ChildOutcome) -> Result<()> {
    let code = match outcome {
        ChildOutcome::Code(code) => code,
        ChildOutcome::Signal(signal) => 128_i32.checked_add(signal).ok_or_else(|| {
            Error::new(format!(
                "Child signal {signal} cannot be represented as an exit code"
            ))
        })?,
    };
    let code = u8::try_from(code).map_err(|_| {
        Error::new(format!(
            "Child exit code {code} is outside supported range 0..255"
        ))
    })?;
    match NonZeroU8::new(code) {
        Some(code) => Err(Error::exit(code)),
        None => Ok(()),
    }
}
pub fn show(
    runner: &mut impl ProcessRunner,
    identifier: &str,
    cwd: &Path,
    env: &crate::config::ChildEnvOverlay,
) -> Result<()> {
    let captured = runner.capture(&probe_spec(identifier), cwd, env)?;
    // The probe's exit status and stderr are ignored; only its output matters.
    if vcs_script::decoded_trim(&captured.stdout).is_empty() {
        return Err(Error::not_found("Commits", identifier));
    }
    child_status(runner.inherit(&show_spec(identifier), cwd, env)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn child_exit_status_becomes_the_command_exit_status() {
        for (outcome, expected) in [
            (ChildOutcome::Code(0), 0),
            (ChildOutcome::Code(7), 7),
            (ChildOutcome::Code(255), 255),
            (ChildOutcome::Signal(15), 143),
        ] {
            let code = child_status(outcome).map_or_else(|error| error.exit_code(), |()| 0);
            assert_eq!(code, expected, "{outcome:?}");
        }
        for code in [-1, 256, i32::MAX, i32::MIN] {
            let error = child_status(ChildOutcome::Code(code)).expect_err("out of range");
            assert_eq!(
                error.message(),
                format!("Child exit code {code} is outside supported range 0..255")
            );
        }
    }
}
