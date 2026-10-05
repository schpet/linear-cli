//! File uploads: validation, MIME types, the signed upload and the resulting links.
use crate::client::LinearClient;
use crate::error::Error;
use crate::graphql::operations::upload::{FileUpload, FileUploadVariables, UploadFileHeader};
use reqwest::header::{HeaderMap, HeaderName, HeaderValue};
use std::path::{Path, PathBuf};
pub const MAX_FILE_SIZE: u64 = 100 * 1024 * 1024;
pub const PUBLIC_SUGGESTION: &str = "Linear only allows public uploads for raster images (png, jpeg, gif, webp, bmp, tiff). Remove --public to upload privately.";
pub fn mime_type(path: &Path) -> &'static str {
    let extension = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match extension.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "bmp" => "image/bmp",
        "tiff" | "tif" => "image/tiff",
        "pdf" => "application/pdf",
        "doc" => "application/msword",
        "docx" => "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
        "xls" => "application/vnd.ms-excel",
        "xlsx" => "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
        "ppt" => "application/vnd.ms-powerpoint",
        "pptx" => "application/vnd.openxmlformats-officedocument.presentationml.presentation",
        "txt" => "text/plain",
        "md" | "markdown" => "text/markdown",
        "csv" => "text/csv",
        "tsv" => "text/tab-separated-values",
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "xml" => "text/xml",
        "js" | "mjs" => "text/javascript",
        "ts" | "tsx" => "text/typescript",
        "jsx" => "text/javascript",
        "json" => "application/json",
        "yaml" | "yml" => "text/yaml",
        "toml" => "text/toml",
        "sh" | "bash" => "text/x-shellscript",
        "py" => "text/x-python",
        "rb" => "text/x-ruby",
        "go" => "text/x-go",
        "rs" => "text/x-rust",
        "java" => "text/x-java",
        "c" | "h" => "text/x-c",
        "cpp" | "hpp" => "text/x-c++",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "7z" => "application/x-7z-compressed",
        "rar" => "application/vnd.rar",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        "ogg" => "audio/ogg",
        "m4a" => "audio/mp4",
        "mp4" => "video/mp4",
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "wasm" => "application/wasm",
        _ => "application/octet-stream",
    }
}
pub fn resolve_public(content_type: &str, requested: bool) -> Result<bool, Error> {
    if requested
        && !matches!(
            content_type,
            "image/png" | "image/jpeg" | "image/gif" | "image/webp" | "image/bmp" | "image/tiff"
        )
    {
        return Err(
            Error::new(format!("Cannot upload {content_type} to a public URL"))
                .with_hint(PUBLIC_SUGGESTION),
        );
    }
    Ok(requested)
}
pub fn validate_file(path: &Path) -> Result<std::fs::Metadata, Error> {
    let info = std::fs::metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            Error::not_found("File", &path.to_string_lossy())
        } else {
            Error::new(format!("Failed to read file metadata: {}", path.display()))
                .with_source(error)
        }
    })?;
    if !info.is_file() {
        return Err(Error::new(format!("Not a file: {}", path.display()))
            .with_hint("Please provide a path to a valid file"));
    }
    Ok(info)
}
/// This deliberately does NOT prevalidate file sizes; sizes are checked
/// during each sequential upload, so earlier uploads may already have succeeded.
pub fn prevalidate(paths: &[PathBuf], public: bool) -> Result<(), Error> {
    for path in paths {
        validate_file(path)?;
        resolve_public(mime_type(path), public)?;
    }
    Ok(())
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PreparedFile {
    pub filename: String,
    pub size: i32,
    pub content_type: &'static str,
    pub public: bool,
}
pub fn prepare(path: &Path, public: bool) -> Result<PreparedFile, Error> {
    let info = validate_file(path)?;
    let size = info.len();
    if size > MAX_FILE_SIZE {
        let hundredths = (u128::from(size) * 100 + 524288) / 1048576;
        let mib = format!("{}.{:02}", hundredths / 100, hundredths % 100);
        return Err(
            Error::new(format!("File too large: {mib}MB exceeds limit of 100MB"))
                .with_hint("Please upload a file smaller than 100MB"),
        );
    }
    let Ok(size) = i32::try_from(size) else {
        unreachable!("100MiB fits GraphQL Int");
    };
    let content_type = mime_type(path);
    let public = resolve_public(content_type, public)?;
    let filename = path
        .file_name()
        .and_then(|x| x.to_str())
        .ok_or_else(|| Error::new("Upload filename must be valid UTF-8"))?
        .to_owned();
    Ok(PreparedFile {
        filename,
        size,
        content_type,
        public,
    })
}
/// Linear's signed upload headers: an exact repeated key replaces the earlier
/// value, while keys differing only in case are sent as repeated headers.
pub fn signed_headers(
    content_type: &str,
    returned: &[UploadFileHeader],
) -> Result<HeaderMap, Error> {
    let mut entries = vec![("content-type".to_owned(), content_type.to_owned())];
    for header in returned {
        if let Some((_, value)) = entries.iter_mut().find(|(key, _)| key == &header.key) {
            *value = header.value.clone();
        } else {
            entries.push((header.key.clone(), header.value.clone()));
        }
    }
    let mut headers = HeaderMap::new();
    for (key, value) in entries {
        let name = HeaderName::from_bytes(key.as_bytes())
            .map_err(|_| Error::new("Invalid signed upload header name"))?;
        let value = value.trim_matches([' ', '\t', '\r', '\n']);
        let value = HeaderValue::from_str(value)
            .map_err(|_| Error::new("Invalid signed upload header value"))?;
        headers.append(name, value);
    }
    Ok(headers)
}
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct UploadedFile {
    pub file: PreparedFile,
    pub asset_url: String,
}
pub async fn upload(
    client: &LinearClient,
    path: &Path,
    file: PreparedFile,
) -> Result<UploadedFile, Error> {
    let response: FileUpload = client
        .mutate(FileUploadVariables {
            content_type: file.content_type.to_owned(),
            filename: file.filename.clone(),
            size: file.size,
            make_public: Some(file.public),
        })
        .await?;
    if !response.file_upload.success {
        return Err(Error::new("Failed to get upload URL from Linear"));
    }
    let target = response
        .file_upload
        .upload_file
        .ok_or_else(|| Error::new("Failed to get upload URL from Linear"))?;
    // The file is read only after the upload URL is issued; no retry or rollback.
    let bytes = std::fs::read(path).map_err(|error| {
        Error::new(format!("Failed to read upload file: {}", path.display())).with_source(error)
    })?;
    let headers = signed_headers(file.content_type, &target.headers)?;
    client
        .put_signed(&target.upload_url, headers, bytes)
        .await?;
    Ok(UploadedFile {
        file,
        asset_url: target.asset_url,
    })
}
pub fn markdown(file: &UploadedFile) -> String {
    format!(
        "{}[{}]({})",
        if file.file.content_type.starts_with("image/") {
            "!"
        } else {
            ""
        },
        file.file.filename,
        file.asset_url
    )
}
pub fn warning(file: &UploadedFile) -> Option<Vec<u8>> {
    file.file.public.then(|| {
        format!(
            "⚠ Uploaded to a public URL readable by anyone: {}\n",
            file.asset_url
        )
        .into_bytes()
    })
}
pub fn output(file: &UploadedFile) -> Vec<u8> {
    super::outcome::done("Uploaded", "file", &file.file.filename, None).into_bytes()
}
