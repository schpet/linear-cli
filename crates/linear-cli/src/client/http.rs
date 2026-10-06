//! Requests outside GraphQL operations: Markdown image and attachment
//! downloads, the raw `api` command and signed uploads. They share the
//! client's connection settings and read bodies through the same bounded
//! [`collect`]: `linear api` with the API deadline and cap, downloads with
//! their own larger ones.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{StatusCode, Url};

use super::config::{Deadline, EndpointUrl, ResponseCap};
use super::error::{NetworkPhase, RawHttpResponse, SanitizedReqwestError, classify_failure};
use super::{CONTENT_TYPE_VALUE, LinearClient};
use crate::error::{Error, Failure};

/// A failure below HTTP classification, before the client attaches its
/// origin.
#[derive(Debug)]
pub(super) enum ExchangeFailure {
    ResponseTooLarge {
        status: StatusCode,
        limit: ResponseCap,
    },
    Timeout,
    Network {
        phase: NetworkPhase,
        source: SanitizedReqwestError,
    },
}

pub(super) fn classify_network(error: reqwest::Error) -> ExchangeFailure {
    if error.is_timeout() {
        return ExchangeFailure::Timeout;
    }
    let phase = if error.is_connect() {
        NetworkPhase::Connect
    } else if error.is_request() {
        NetworkPhase::Request
    } else if error.is_body() || error.is_decode() {
        NetworkPhase::Body
    } else {
        NetworkPhase::Other
    };
    ExchangeFailure::Network {
        phase,
        source: SanitizedReqwestError::new(error),
    }
}

/// Reads the body chunk by chunk, stopping before the cap is exceeded.
pub(super) async fn collect(
    mut response: reqwest::Response,
    limit: ResponseCap,
) -> Result<RawHttpResponse, ExchangeFailure> {
    let status = response.status();
    let headers = std::mem::take(response.headers_mut());
    let too_large = || ExchangeFailure::ResponseTooLarge { status, limit };
    if let Some(declared) = response.content_length()
        && u64::try_from(limit.bytes()).is_ok_and(|cap| declared > cap)
    {
        return Err(too_large());
    }
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(classify_network)? {
        if body.len() + chunk.len() > limit.bytes() {
            return Err(too_large());
        }
        body.extend_from_slice(&chunk);
    }
    Ok(RawHttpResponse {
        status,
        headers,
        body,
    })
}

/// A short, display-safe description of a failed non-GraphQL request. A
/// timeout or network failure is [`Failure::Unavailable`]; an oversized body
/// is classified by its status.
fn bounded_failure(prefix: &str, failure: ExchangeFailure, deadline: Deadline) -> Error {
    match failure {
        ExchangeFailure::ResponseTooLarge { status, limit } => Error::failed(
            status_failure(status),
            format!(
                "{prefix}: response exceeds the {} byte limit",
                limit.bytes()
            ),
        ),
        ExchangeFailure::Timeout => Error::failed(
            Failure::Unavailable,
            format!(
                "{prefix}: did not complete within {:?}",
                deadline.duration()
            ),
        ),
        ExchangeFailure::Network { source, .. } => Error::failed(
            Failure::Unavailable,
            format!("{prefix}: {}", source.root_message()),
        )
        .with_source(source),
    }
}

/// The failure class of a response with `status` and no usable body.
fn status_failure(status: StatusCode) -> Failure {
    classify_failure(status, &[])
}

/// The failure class of a download or signed upload that got `status` from a
/// host other than the API: unavailable for 408, 429 and 5xx, otherwise
/// general. A 401 or 403 there is the URL's own access, which logging in to
/// Linear again does not fix.
fn storage_failure(status: StatusCode) -> Failure {
    match status_failure(status) {
        Failure::Unavailable => Failure::Unavailable,
        Failure::General | Failure::NotFound | Failure::Auth => Failure::General,
    }
}

/// Whether a download from `url` carries the API key: only for exactly
/// `https://uploads.linear.app` (default port, no user name or password),
/// where Linear serves private uploads.
fn authenticates(url: &Url) -> bool {
    url.scheme() == "https"
        && url.host_str() == Some("uploads.linear.app")
        && url.port().is_none()
        && url.username().is_empty()
        && url.password().is_none()
}

impl LinearClient {
    /// Downloads an image referenced from Markdown.
    pub async fn download_markdown_image(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.download(url, "Failed to download image").await
    }

    /// Downloads an issue attachment.
    pub async fn download_issue_attachment(&self, url: &str) -> Result<Vec<u8>, Error> {
        self.download(url, "Failed to download").await
    }

    /// GETs an `http(s)` URL within the download deadline and size cap. The
    /// API key is sent only to Linear's private upload origin (see
    /// [`authenticates`]); a redirect elsewhere drops it, and no redirect may
    /// leave HTTPS.
    async fn download(&self, original: &str, failure_prefix: &str) -> Result<Vec<u8>, Error> {
        let url = Url::parse(original)
            .map_err(|error| Error::new(format!("Invalid URL: '{original}'")).with_source(error))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(Error::new(format!(
                "{failure_prefix}: unsupported URL scheme '{}'",
                url.scheme()
            )));
        }
        let authenticated = authenticates(&url);
        let mut request = self
            .http
            .get(url)
            .timeout(self.download_deadline.duration());
        if authenticated {
            request = request.header(AUTHORIZATION, self.api_key.header_value());
        }
        let failed = |failure| bounded_failure(failure_prefix, failure, self.download_deadline);
        let response = request
            .send()
            .await
            .map_err(|error| failed(classify_network(error)))?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::failed(
                storage_failure(status),
                format!("{failure_prefix}: {status}"),
            ));
        }
        let response = collect(response, self.max_download_bytes)
            .await
            .map_err(failed)?;
        Ok(response.body)
    }

    /// POSTs a raw GraphQL body for the `api` command and returns the status
    /// and body text unclassified, within the API deadline and size cap.
    pub async fn fetch_api(&self, body: String) -> Result<(StatusCode, String), Error> {
        let response = self
            .http
            .post(self.endpoint.url.clone())
            .timeout(self.deadline.duration())
            .header(AUTHORIZATION, self.api_key.header_value())
            .header(CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE_VALUE))
            .body(body)
            .send()
            .await
            .map_err(|error| {
                bounded_failure(
                    &format!("Request to {} failed", self.endpoint),
                    classify_network(error),
                    self.deadline,
                )
            })?;
        let response = collect(response, self.max_response_bytes)
            .await
            .map_err(|failure| {
                bounded_failure(
                    "Failed to read API response; the request was sent and may have taken effect",
                    failure,
                    self.deadline,
                )
            })?;
        Ok((
            response.status,
            String::from_utf8_lossy(&response.body).into_owned(),
        ))
    }

    /// PUTs a file to a pre-signed upload URL with exactly the headers Linear
    /// returned for it; the API key is never sent. There is no total deadline
    /// because uploads can be large.
    pub async fn put_signed(
        &self,
        url: &str,
        headers: HeaderMap,
        body: Vec<u8>,
    ) -> Result<(), Error> {
        let invalid = || Error::new("Invalid signed upload URL");
        let mut url = Url::parse(url).map_err(|_| invalid())?;
        url.set_fragment(None);
        let target = EndpointUrl::from_url(url).map_err(|_| invalid())?;
        let failed = |failure: Failure, reason: String| {
            Error::failed(
                failure,
                format!(
                    "Signed upload to {target} failed: {reason}; the object may already be \
                     stored remotely; no comment or attachment was created"
                ),
            )
        };
        let response = self
            .http
            .put(target.url.clone())
            .headers(headers)
            .body(body)
            .send()
            .await
            .map_err(|error| {
                let error = SanitizedReqwestError::new(error);
                failed(Failure::Unavailable, error.root_message()).with_source(error)
            })?;
        let status = response.status();
        if status.is_success() {
            return Ok(());
        }
        let response = collect(response, self.max_response_bytes)
            .await
            .map_err(|failure| match failure {
                ExchangeFailure::ResponseTooLarge { limit, .. } => failed(
                    storage_failure(status),
                    format!("response exceeded {} bytes", limit.bytes()),
                ),
                ExchangeFailure::Timeout => failed(Failure::Unavailable, "timed out".to_owned()),
                ExchangeFailure::Network { source, .. } => {
                    failed(Failure::Unavailable, source.root_message()).with_source(source)
                }
            })?;
        Err(Error::failed(
            storage_failure(response.status),
            format!(
                "Failed to upload file: {} - {}",
                response.status,
                String::from_utf8_lossy(&response.body)
            ),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_exact_https_upload_origin_gets_the_api_key() {
        let authenticated = |url: &str| authenticates(&Url::parse(url).expect("URL"));
        assert!(authenticated("https://uploads.linear.app/org/file.png"));
        assert!(authenticated("https://uploads.linear.app:443/org/file.png"));
        for url in [
            "http://uploads.linear.app/org/file.png",
            "http://uploads.linear.app:443/org/file.png",
            "https://uploads.linear.app:8443/org/file.png",
            "https://user@uploads.linear.app/org/file.png",
            "https://:secret@uploads.linear.app/org/file.png",
            "https://uploads.linear.app.example.com/file.png",
            "https://public.linear.app/file.png",
        ] {
            assert!(!authenticated(url), "{url}");
        }
    }
}
