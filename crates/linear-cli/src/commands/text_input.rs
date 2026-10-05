//! Reading user-supplied text (bodies, descriptions, document content) from files and stdin.
//!
//! Every command reads supplied text by one rule: a leading byte-order mark
//! and trailing whitespace (such as the final newline) are dropped, and text
//! that is then empty counts as none. A command makes no text an error for a
//! required body and the same as not passing the flag for an optional field.
use crate::cli::values::TextSource;
use crate::error::Error;
use std::{io::Read, path::Path};

/// Reads a whole `--body-file`-style source: the named file, or all of stdin
/// for `-`. Invalid UTF-8 is an `InvalidData` error.
pub fn read_source(source: &TextSource) -> std::io::Result<Option<String>> {
    match source {
        TextSource::File(path) => read_text_file(path),
        TextSource::Stdin => {
            let mut text = String::new();
            std::io::stdin().lock().read_to_string(&mut text)?;
            Ok(content(text))
        }
    }
}

/// Reads a file of supplied text, such as a path typed at a prompt.
pub fn read_text_file(path: impl AsRef<Path>) -> std::io::Result<Option<String>> {
    read_file(path).map(content)
}

/// Reads a whole UTF-8 file as it is, but for a leading byte-order mark.
/// Invalid UTF-8 surfaces as an `InvalidData` error rather than being replaced.
pub fn read_file(path: impl AsRef<Path>) -> std::io::Result<String> {
    std::fs::read_to_string(path).map(strip_bom)
}

/// Reads all of a non-terminal stdin as UTF-8 supplied text.
pub fn read_stdin(mut reader: impl Read) -> Result<Option<String>, Error> {
    let mut text = String::new();
    reader
        .read_to_string(&mut text)
        .map_err(|error| Error::new(format!("Failed to read stdin: {error}")).with_source(error))?;
    Ok(content(text))
}

fn content(text: String) -> Option<String> {
    let text = strip_bom(text);
    let content = text.trim_end();
    (!content.is_empty()).then(|| content.to_owned())
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
