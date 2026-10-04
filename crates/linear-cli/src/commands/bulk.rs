//! The bulk archive/delete/move commands: reading the IDs, running every
//! item with a progress line, and the summary.
use std::{collections::HashSet, io::Read, path::Path};

use futures_util::{StreamExt, stream};

use crate::cli::BulkArgs;
use crate::ctx::Ctx;
use crate::error::{Error, Result};

pub struct BulkInput<'a> {
    pub argv: Option<&'a [String]>,
    pub file: Option<&'a Path>,
    pub stdin: bool,
}
impl<'a> From<&'a BulkArgs> for BulkInput<'a> {
    fn from(args: &'a BulkArgs) -> Self {
        Self {
            argv: args.bulk.as_deref(),
            file: args.bulk_file.as_deref(),
            stdin: args.bulk_stdin,
        }
    }
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
pub fn collect_ids(input: &BulkInput<'_>, stdin: &mut impl Read) -> Result<Vec<String>> {
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

/// How a bulk command names what it does: `archive`/`archived`.
#[derive(Clone, Copy, Debug)]
pub struct Verb {
    pub present: &'static str,
    pub past: &'static str,
}

/// A listed item that was looked up and can be changed.
pub struct Found<T> {
    /// The reference as it was listed.
    pub original: String,
    /// How the item is shown, like `ENG-1: Fix login`.
    pub name: String,
    pub item: T,
}

impl<T> Found<T> {
    /// The row for this item once `op` has run.
    pub fn result(&self, outcome: std::result::Result<(), Error>) -> BulkResult {
        BulkResult {
            id: self.original.clone(),
            name: Some(self.name.clone()),
            outcome: match outcome {
                Ok(()) => BulkOutcome::Succeeded,
                Err(error) => BulkOutcome::Failed(error.message().to_owned()),
            },
        }
    }
}

/// A listed item that could not be looked up, and why.
pub struct Skipped {
    pub original: String,
    pub reason: String,
}

impl From<Skipped> for BulkResult {
    fn from(skipped: Skipped) -> Self {
        Self {
            id: skipped.original,
            name: None,
            outcome: BulkOutcome::Failed(skipped.reason),
        }
    }
}

/// Looks up every listed item, five at a time behind a spinner, before
/// anything is confirmed or changed. Both lists keep the input order.
pub fn look_up<I, T, F, Fut>(ctx: &Ctx, items: Vec<I>, op: F) -> (Vec<Found<T>>, Vec<Skipped>)
where
    F: FnMut(I) -> Fut,
    Fut: Future<Output = std::result::Result<Found<T>, Skipped>>,
{
    let rows: Vec<_> = ctx.spin(true, stream::iter(items).map(op).buffered(5).collect());
    let mut found = Vec::new();
    let mut missing = Vec::new();
    for row in rows {
        match row {
            Ok(item) => found.push(item),
            Err(row) => missing.push(row),
        }
    }
    (found, missing)
}

/// What a bulk command is about to change, and the listed items it skips
/// because they could not be looked up.
pub fn preview<T>(found: &[Found<T>], missing: &[Skipped], noun: &str, verb: Verb) -> String {
    let mut output = String::new();
    if !found.is_empty() {
        output.push_str(&format!(
            "{} to {}:\n",
            count(found.len(), noun),
            verb.present
        ));
        for item in found {
            output.push_str(&format!("  {}\n", item.name));
        }
    }
    if !missing.is_empty() {
        output.push_str(&format!(
            "Skipping {} that could not be found:\n",
            count(missing.len(), noun)
        ));
        for skipped in missing {
            output.push_str(&format!("  {}: {}\n", skipped.original, skipped.reason));
        }
    }
    output
}

/// `count` of `noun`, as in `1 issue` or `3 issues`.
pub fn count(count: usize, noun: &str) -> String {
    format!("{count} {noun}{}", if count == 1 { "" } else { "s" })
}

/// Runs `op` for every item, five at a time, keeping the input order in the
/// results. While stderr is a terminal a progress line counts them off.
pub fn run<T, F, Fut>(ctx: &Ctx, items: Vec<T>, op: F) -> Result<Vec<BulkResult>>
where
    F: FnMut(T) -> Fut,
    Fut: Future<Output = BulkResult>,
{
    let total = items.len();
    let show_progress = ctx.terminal().stderr_tty;
    let results = ctx.block_on(async {
        let mut rows = stream::iter(items).map(op).buffered(5);
        let mut results: Vec<BulkResult> = Vec::with_capacity(total);
        while let Some(row) = rows.next().await {
            results.push(row);
            if show_progress {
                let succeeded = results.iter().filter(|row| row.succeeded()).count();
                ctx.eprint(progress(results.len(), total, succeeded))?;
            }
        }
        Ok::<_, Error>(results)
    })?;
    if show_progress && total > 0 {
        let width = progress(total, total, total).chars().count();
        ctx.eprint(format!("\r{}\r", " ".repeat(width)))?;
    }
    Ok(results)
}

/// Prints the outcome of every item and fails (already reported) when any did.
pub fn report(ctx: &Ctx, results: &[BulkResult], noun: &str, verb: Verb) -> Result<()> {
    let (summary, failed) = summary(results, noun, verb);
    ctx.print(summary)?;
    if failed {
        return Err(Error::reported());
    }
    Ok(())
}

/// The summary after a bulk run, and whether anything failed. `noun` is
/// singular, as in `issue`.
pub fn summary(results: &[BulkResult], noun: &str, verb: Verb) -> (String, bool) {
    let total = results.len();
    let succeeded = results.iter().filter(|row| row.succeeded()).count();
    let failed = total - succeeded;
    let count = |n: usize| count(n, noun);
    let mut output = String::from("\n");
    if failed == 0 {
        output.push_str(&format!(
            "✓ Successfully {} {}\n",
            verb.past,
            count(succeeded)
        ));
        return (output, false);
    }
    if succeeded == 0 {
        output.push_str(&format!(
            "✗ Failed to {} all {}\n",
            verb.present,
            count(total)
        ));
    } else {
        output.push_str(&format!(
            "Completed: {succeeded}/{} {}\n  ✓ Succeeded: {succeeded}\n  ✗ Failed: {failed}\n",
            count(total),
            verb.past
        ));
    }
    output.push_str("\nFailed operations:\n");
    for row in results {
        if let BulkOutcome::Failed(error) = &row.outcome {
            let name = row
                .name
                .as_deref()
                .filter(|name| !name.is_empty())
                .map_or_else(String::new, |name| format!(" ({name})"));
            output.push_str(&format!("  - {}{name}: {error}\n", row.id));
        }
    }
    (output, true)
}

fn progress(completed: usize, total: usize, succeeded: usize) -> String {
    // The percentage rounded half up, in integer arithmetic.
    let percent = (completed * 200 + total) / (total * 2);
    format!(
        "\r⏳ {completed}/{total} ({percent}%) - ✓ {succeeded} ✗ {}",
        completed - succeeded
    )
}

#[cfg(test)]
mod tests {
    use super::{BulkInput, BulkOutcome, BulkResult, Verb, collect_ids, summary};

    const ARCHIVE: Verb = Verb {
        present: "archive",
        past: "archived",
    };

    fn row(id: &str, outcome: BulkOutcome) -> BulkResult {
        BulkResult {
            id: id.to_owned(),
            name: Some(format!("{id}: Title")),
            outcome,
        }
    }

    #[test]
    fn summaries_count_successes_and_list_failures() {
        let ok = row("ENG-1", BulkOutcome::Succeeded);
        let failed = row("ENG-2", BulkOutcome::Failed("Issue not found".to_owned()));
        assert_eq!(
            summary(std::slice::from_ref(&ok), "issue", ARCHIVE),
            ("\n✓ Successfully archived 1 issue\n".to_owned(), false)
        );
        assert_eq!(
            summary(&[ok, failed.clone()], "issue", ARCHIVE),
            (
                "\nCompleted: 1/2 issues archived\n  ✓ Succeeded: 1\n  ✗ Failed: 1\n\nFailed operations:\n  - ENG-2 (ENG-2: Title): Issue not found\n"
                    .to_owned(),
                true
            )
        );
        assert!(
            summary(&[failed], "issue", ARCHIVE)
                .0
                .starts_with("\n✗ Failed to archive all 1 issue\n")
        );
    }

    #[test]
    fn ids_come_from_argv_file_and_stdin_without_duplicates() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("ids");
        std::fs::write(&file, "\u{feff}ENG-1,ENG-2\n ENG-1\tENG-3\n").expect("write ids");
        let argv = vec![" Raw, argv ".to_owned()];
        let input = BulkInput {
            argv: Some(&argv),
            file: Some(&file),
            stdin: true,
        };
        assert_eq!(
            collect_ids(&input, &mut &b"ENG-4,ENG-2"[..]).expect("ids"),
            [" Raw, argv ", "ENG-1", "ENG-2", "ENG-3", "ENG-4"]
        );
    }

    #[test]
    fn ids_must_be_utf8() {
        let dir = tempfile::tempdir().expect("temp dir");
        let file = dir.path().join("ids");
        std::fs::write(&file, b"ENG-1,\xff").expect("write ids");
        let from_file = BulkInput {
            argv: None,
            file: Some(&file),
            stdin: false,
        };
        let error = collect_ids(&from_file, &mut &b""[..]).expect_err("invalid file");
        assert!(error.message().starts_with("Bulk file must be valid UTF-8"));
        let from_stdin = BulkInput {
            argv: None,
            file: None,
            stdin: true,
        };
        let error = collect_ids(&from_stdin, &mut &b"\xff"[..]).expect_err("invalid stdin");
        assert_eq!(error.message(), "Bulk stdin must be valid UTF-8");
    }
}
