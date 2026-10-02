//! Pieces shared by the bulk archive/delete commands: reading the IDs, one
//! result row per ID, and the progress line.
use std::{collections::HashSet, io::Read, path::Path};

use crate::error::Error;

pub struct BulkInput<'a> {
    pub argv: Option<&'a [String]>,
    pub file: Option<&'a Path>,
    pub stdin: bool,
}
impl BulkInput<'_> {
    pub fn requested(&self) -> bool {
        self.argv.is_some_and(|ids| !ids.is_empty()) || self.file.is_some() || self.stdin
    }
}
fn parse_ids(text: &str) -> impl Iterator<Item = &str> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    text.split(|ch: char| ch == ',' || ch.is_whitespace())
        .filter(|id| !id.is_empty())
}
/// Read and decode every selected input before printing a count or dispatching requests.
/// argv tokens deliberately remain unsplit and untrimmed.
pub fn collect_ids(input: &BulkInput<'_>, stdin: &mut impl Read) -> Result<Vec<String>, Error> {
    let mut ids = input.argv.unwrap_or_default().to_vec();
    if let Some(path) = input.file {
        let bytes = std::fs::read(path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                Error::not_found("File", &path.display().to_string())
            } else {
                Error::new(format!("Failed to read bulk file: {}", path.display()))
                    .with_source(error)
            }
        })?;
        let text = String::from_utf8(bytes).map_err(|error| {
            Error::new(format!("Bulk file must be valid UTF-8: {}", path.display()))
                .with_hint("Re-save the file as UTF-8 text.")
                .with_source(error)
        })?;
        ids.extend(parse_ids(&text).map(str::to_owned));
    }
    if input.stdin {
        let mut bytes = Vec::new();
        stdin
            .read_to_end(&mut bytes)
            .map_err(|error| Error::new("Failed to read bulk stdin").with_source(error))?;
        let text = String::from_utf8(bytes).map_err(|error| {
            Error::new("Bulk stdin must be valid UTF-8")
                .with_hint("Provide UTF-8 text on stdin.")
                .with_source(error)
        })?;
        ids.extend(parse_ids(&text).map(str::to_owned));
    }
    let mut seen = HashSet::new();
    ids.retain(|id| seen.insert(id.clone()));
    Ok(ids)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BulkOutcome {
    Succeeded,
    Failed(String),
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BulkResult {
    pub id: String,
    pub name: Option<String>,
    pub outcome: BulkOutcome,
}
impl BulkResult {
    pub fn succeeded(&self) -> bool {
        matches!(self.outcome, BulkOutcome::Succeeded)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub completed: usize,
    pub total: usize,
    pub succeeded: usize,
}
impl Progress {
    pub fn render(self) -> Vec<u8> {
        // The percentage rounded half up, in integer arithmetic.
        let percent = (self.completed * 200 + self.total) / (self.total * 2);
        format!(
            "\r⏳ Processing: {}/{} ({percent}%) - ✓ {} ✗ {}",
            self.completed,
            self.total,
            self.succeeded,
            self.completed - self.succeeded
        )
        .into_bytes()
    }
}

pub const PROGRESS_CLEAR: &[u8] =
    b"\r                                                                                \r";
