//! Infer the current issue from the Git branch name or jj trailers (read-only).
use std::io::{self, Read};
use std::path::Path;
use std::process::{Command, ExitStatus, Stdio};
use std::thread;

use crate::config::Vcs;
use crate::error::Error;
use crate::refs::find_issue_identifier;

pub const JJ_TEMPLATE: &str = "trailers.map(|t| if(t.key() == \"Linear-issue\", t.value(), \"\"))";

pub fn parse_jj_trailers(output: &str) -> Option<String> {
    let mut last = None;
    for line in output.split('\n') {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            if last.is_some() {
                return last;
            }
        } else if let Some(id) = find_issue_identifier(trimmed) {
            last = Some(id);
        }
    }
    last
}

pub fn parse_git_branch(
    success: bool,
    stdout: &str,
    stderr: &str,
) -> Result<Option<String>, Error> {
    if !success {
        let error = stderr.trim();
        if error.contains("not a symbolic ref") {
            return Ok(None);
        }
        return Err(Error::new(format!("Failed to get current branch: {error}")));
    }
    Ok(find_issue_identifier(stdout.trim()))
}

fn process_error(stage: &str, error: io::Error) -> Error {
    Error::new(format!("Failed to {stage}: {error}")).with_source(error)
}

fn read_output(mut input: impl Read) -> io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    input.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn probe(
    program: &str,
    args: &[&str],
    cwd: &Path,
) -> Result<(ExitStatus, Vec<u8>, Vec<u8>), Error> {
    let mut child = Command::new(program)
        .args(args)
        .current_dir(cwd)
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| {
            let message = if error.kind() == io::ErrorKind::NotFound {
                format!("Failed to spawn '{program}': entity not found")
            } else {
                format!("Failed to spawn '{program}': {error}")
            };
            Error::new(message).with_source(error)
        })?;
    let stdout = child
        .stdout
        .take()
        .ok_or_else(|| Error::new("VCS stdout was not piped"))?;
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| Error::new("VCS stderr was not piped"))?;
    thread::scope(|scope| {
        let out = scope.spawn(|| read_output(stdout));
        let err = scope.spawn(|| read_output(stderr));
        let status = child.wait().map_err(|e| process_error("wait for VCS", e));
        let stdout = out
            .join()
            .map_err(|_| Error::new("VCS stdout reader panicked"))?
            .map_err(|e| process_error("read VCS stdout", e))?;
        let stderr = err
            .join()
            .map_err(|_| Error::new("VCS stderr reader panicked"))?
            .map_err(|e| process_error("read VCS stderr", e))?;
        Ok((status?, stdout, stderr))
    })
}

pub fn infer_issue(vcs: Vcs, cwd: &Path) -> Result<Option<String>, Error> {
    match vcs {
        Vcs::Git => {
            let (status, stdout, stderr) = probe("git", &["symbolic-ref", "--short", "HEAD"], cwd)?;
            parse_git_branch(
                status.success(),
                &String::from_utf8_lossy(&stdout),
                &String::from_utf8_lossy(&stderr),
            )
        }
        Vcs::Jj => {
            let (status, stdout, _) = probe(
                "jj",
                &["log", "-r", "::@", "-T", JJ_TEMPLATE, "--no-graph"],
                cwd,
            )?;
            Ok(if status.success() {
                parse_jj_trailers(&String::from_utf8_lossy(&stdout))
            } else {
                None
            })
        }
    }
}
