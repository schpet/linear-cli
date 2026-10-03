//! Reading user-supplied text (bodies, descriptions, document content) from files and stdin.
use crate::error::Error;
use std::{io::Read, path::Path};

/// Reads a whole UTF-8 file, dropping a leading byte-order mark. Invalid UTF-8
/// surfaces as an `InvalidData` error rather than being replaced.
pub fn read_file(path: impl AsRef<Path>) -> std::io::Result<String> {
    std::fs::read_to_string(path).map(strip_bom)
}

/// Reads all of a non-terminal stdin as UTF-8. Whitespace-only input counts as
/// no content; otherwise trailing whitespace (such as the final newline) is dropped.
pub fn read_stdin(mut reader: impl Read) -> Result<Option<String>, Error> {
    let mut text = String::new();
    reader
        .read_to_string(&mut text)
        .map_err(|error| Error::new(format!("Failed to read stdin: {error}")).with_source(error))?;
    let text = strip_bom(text);
    let content = text.trim_end();
    Ok((!content.trim_start().is_empty()).then(|| content.to_owned()))
}

fn strip_bom(text: String) -> String {
    match text.strip_prefix('\u{feff}') {
        Some(rest) => rest.to_owned(),
        None => text,
    }
}

pub fn edited_body(text: &str) -> Option<String> {
    let text = text.trim();
    (!text.is_empty()).then(|| text.to_owned())
}
