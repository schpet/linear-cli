//! Requests outside GraphQL operations: Markdown image and attachment
//! downloads, the raw `api` command and signed uploads. They share the
//! client's connection settings, plus the bounded body collection GraphQL
//! responses use.

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use reqwest::{StatusCode, Url};

use super::config::{EndpointUrl, ResponseCap};
use super::error::{NetworkPhase, RawHttpResponse, SanitizedReqwestError};
use super::{CONTENT_TYPE_VALUE, LinearClient};
use crate::error::Error;

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

/// A short, display-safe description of a failed non-GraphQL request.
fn request_error(prefix: &str, error: reqwest::Error) -> Error {
    let error = SanitizedReqwestError::new(error);
    Error::new(format!("{prefix}: {}", error.root_message())).with_source(error)
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

    /// GETs an `http(s)` URL. The API key is sent only to Linear's private
    /// upload host; reqwest drops it if a redirect leaves that host.
    async fn download(&self, original: &str, failure_prefix: &str) -> Result<Vec<u8>, Error> {
        let url = Url::parse(original)
            .map_err(|error| Error::new(format!("Invalid URL: '{original}'")).with_source(error))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(Error::new(format!(
                "{failure_prefix}: unsupported URL scheme '{}'",
                url.scheme()
            )));
        }
        let authenticated = url.host_str() == Some("uploads.linear.app");
        let mut request = self.http.get(url);
        if authenticated {
            request = request.header(AUTHORIZATION, self.api_key.header_value());
        }
        let response = request
            .send()
            .await
            .map_err(|error| request_error(failure_prefix, error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(Error::new(format!("{failure_prefix}: {status}")));
        }
        let body = response
            .bytes()
            .await
            .map_err(|error| request_error(failure_prefix, error))?;
        Ok(body.to_vec())
    }

    /// POSTs a raw GraphQL body for the `api` command and returns the status
    /// and body text unclassified, with no deadline or size cap.
    pub async fn fetch_api(&self, body: String) -> Result<(u16, String), Error> {
        let response = self
            .http
            .post(self.endpoint.url.clone())
            .header(AUTHORIZATION, self.api_key.header_value())
            .header(CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE_VALUE))
            .body(body)
            .send()
            .await
            .map_err(|error| {
                request_error(&format!("Request to {} failed", self.endpoint), error)
            })?;
        let status = response.status().as_u16();
        let bytes = response.bytes().await.map_err(|error| {
            request_error(
                "Failed to read API response; the request was sent and may have taken effect",
                error,
            )
        })?;
        Ok((status, String::from_utf8_lossy(&bytes).into_owned()))
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
        let failed = |reason: String| {
            Error::new(format!(
                "Signed upload to {target} failed: {reason}; the object may already be \
                     stored remotely; no comment or attachment was created"
            ))
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
                failed(error.root_message()).with_source(error)
            })?;
        if response.status().is_success() {
            return Ok(());
        }
        let response = collect(response, self.max_response_bytes)
            .await
            .map_err(|failure| match failure {
                ExchangeFailure::ResponseTooLarge { limit, .. } => {
                    failed(format!("response exceeded {} bytes", limit.bytes()))
                }
                ExchangeFailure::Timeout => failed("timed out".to_owned()),
                ExchangeFailure::Network { source, .. } => {
                    failed(source.root_message()).with_source(source)
                }
            })?;
        Err(Error::new(format!(
            "Failed to upload file: {} - {}",
            response.status,
            String::from_utf8_lossy(&response.body)
        )))
    }
}
