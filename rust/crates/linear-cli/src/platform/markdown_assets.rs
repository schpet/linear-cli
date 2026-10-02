//! Download images referenced in Markdown into a local cache; extraction is in markdown_ast.
use crate::error::{AppError, AppErrorKind};
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    future::Future,
    path::{Path, PathBuf},
};
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub url: String,
    pub alt: Option<String>,
}
pub fn ordered_assets(content: &str) -> Result<Vec<Asset>, AppError> {
    let (images, links) = super::markdown_ast::extract(content)?;
    let mut seen = HashSet::new();
    Ok(images
        .into_iter()
        .chain(links)
        .filter(|asset| seen.insert(asset.url.clone()))
        .collect())
}
pub fn sanitized_filename(alt: Option<&str>) -> String {
    let Some(alt) = alt.filter(|alt| !alt.is_empty()) else {
        return "image".to_owned();
    };
    let mut value: String = alt
        .chars()
        .filter(|ch| {
            !matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*')
                && !('\0'..='\u{1f}').contains(ch)
                && !('\u{80}'..='\u{9f}').contains(ch)
        })
        .collect();
    if value.chars().all(|ch| ch == '.') {
        value.clear();
    }
    let reserved = value.split('.').next().unwrap_or("").to_ascii_lowercase();
    if matches!(reserved.as_str(), "con" | "prn" | "aux" | "nul")
        || (reserved.len() == 4
            && (reserved.starts_with("com") || reserved.starts_with("lpt"))
            && reserved.as_bytes().get(3).is_some_and(u8::is_ascii_digit))
    {
        value.clear();
    }
    value = value.trim_end_matches(['.', ' ']).to_owned();
    while value.len() > 255 {
        value.pop();
    }
    value
}
/// Lexical POSIX path join: no filesystem canonicalization or cwd expansion.
pub fn posix_join(parts: &[&str]) -> String {
    let joined = parts
        .iter()
        .copied()
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>()
        .join("/");
    if joined.is_empty() {
        return ".".to_owned();
    }
    let absolute = joined.starts_with('/');
    let trailing = joined.ends_with('/');
    let mut segments = Vec::new();
    for segment in joined.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if segments.last().is_some_and(|last| *last != "..") {
                    segments.pop();
                } else if !absolute {
                    segments.push("..");
                }
            }
            value => segments.push(value),
        }
    }
    let mut value = segments.join("/");
    if absolute {
        value.insert(0, '/');
    }
    if value.is_empty() {
        value.push('.');
    }
    if trailing && value != "/" {
        value.push('/');
    }
    value
}
pub fn cache_root(tmpdir: Option<&str>, tmp: Option<&str>, temp: Option<&str>) -> PathBuf {
    let root = [tmpdir, tmp, temp]
        .into_iter()
        .flatten()
        .find(|value| !value.is_empty())
        .unwrap_or("/tmp");
    PathBuf::from(posix_join(&[root, "linear-cli-images"]))
}
fn cache_location(root: &Path, asset: &Asset) -> Result<(PathBuf, PathBuf), AppError> {
    let root = root.to_str().ok_or_else(|| {
        AppError::new(
            AppErrorKind::Validation,
            "Image cache root is not valid UTF-8",
        )
    })?;
    let digest = Sha256::digest(asset.url.as_bytes());
    let mut prefix = String::new();
    for byte in digest.iter().take(8) {
        prefix.push_str(&format!("{byte:02x}"));
    }
    let directory = posix_join(&[root, &prefix]);
    let filename = sanitized_filename(asset.alt.as_deref());
    // Empty name returns the hash directory itself, with no trailing separator.
    let path = posix_join(&[&directory, &filename]);
    Ok((PathBuf::from(directory), PathBuf::from(path)))
}
pub fn cache_path(root: &Path, asset: &Asset) -> Result<PathBuf, AppError> {
    cache_location(root, asset).map(|(_, path)| path)
}
pub struct Downloaded {
    pub paths: HashMap<String, String>,
}
pub async fn download_with<F, Fut, E>(
    content: &str,
    root: &Path,
    fetch: F,
    emit_failure: E,
) -> Result<Downloaded, AppError>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, AppError>>,
    E: FnMut(&[u8]) -> Result<(), AppError>,
{
    download_sources_with(&[content], root, fetch, emit_failure).await
}
/// Each body contributes its images then upload links,
/// with URL deduplication across bodies retaining the first label.
pub async fn download_sources_with<F, Fut, E>(
    sources: &[&str],
    root: &Path,
    mut fetch: F,
    mut emit_failure: E,
) -> Result<Downloaded, AppError>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>, AppError>>,
    E: FnMut(&[u8]) -> Result<(), AppError>,
{
    let mut assets = Vec::new();
    let mut seen = HashSet::new();
    for source in sources {
        for asset in ordered_assets(source)? {
            if seen.insert(asset.url.clone()) {
                assets.push(asset);
            }
        }
    }
    let mut paths = HashMap::new();
    for asset in assets {
        // Create the cache directory before checking the file or parsing the URL.
        let result = async {
            let (directory, path) = cache_location(root, &asset)?;
            std::fs::create_dir_all(&directory).map_err(io_error)?;
            if std::fs::metadata(&path).is_err() {
                let body = fetch(asset.url.clone()).await?;
                std::fs::write(&path, body).map_err(io_error)?;
            }
            path.to_str().map(str::to_owned).ok_or_else(|| {
                AppError::new(
                    AppErrorKind::Validation,
                    "Image cache path is not valid UTF-8",
                )
            })
        }
        .await;
        match result {
            Ok(path) => {
                paths.insert(asset.url, path);
            }
            Err(error) => {
                emit_failure(
                    format!("Failed to download {}: {}\n", asset.url, error.message).as_bytes(),
                )?;
            }
        }
    }
    Ok(Downloaded { paths })
}
fn io_error(error: std::io::Error) -> AppError {
    AppError::new(AppErrorKind::IoProcess, error.to_string()).with_source(error)
}

/// Attachments sanitize the supplied title without the image alt fallback.
pub fn sanitized_attachment_filename(title: &str) -> String {
    if title.is_empty() {
        String::new()
    } else {
        sanitized_filename(Some(title))
    }
}
