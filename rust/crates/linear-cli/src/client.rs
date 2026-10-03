//! The Linear API client: GraphQL operations plus the other HTTP requests the
//! CLI makes with the same connection settings (the raw `api` command, signed
//! uploads and Markdown image or attachment downloads, in [`http`]).
//!
//! Each [`LinearClient`] owns one `reqwest` client. Proxies come from the
//! standard `HTTP_PROXY`/`HTTPS_PROXY`/`ALL_PROXY`/`NO_PROXY` variables, which
//! reqwest reads itself. Certificates are verified against the operating
//! system's roots and the bundled Mozilla roots, plus any bundle named in
//! [`ClientConfig`].
//!
//! GraphQL responses are captured whole (status, headers, body up to a cap)
//! and then classified: GraphQL `errors` win over the HTTP status, and a
//! non-2xx body without GraphQL errors is an HTTP failure that keeps its bytes.
//!
//! Secrets: every stored `reqwest::Error` has its URL stripped, and failures
//! only print the endpoint's scheme/host/port, never its path or query.
//! `ApiKey` redacts itself in `Debug` and `Display`.

mod config;
mod error;
mod http;

use reqwest::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
use serde::Serialize;
use serde::de::DeserializeOwned;

use cynic::Operation;

use crate::graphql::envelope::{GraphQlRequest, LegacyRequest, ResponseError, parse_response};

pub use config::{
    ApiKey, ApiKeyError, ClientBuildError, ClientConfig, Deadline, EndpointUrl, ResponseCap,
};
pub use error::{HttpBodyShape, RawHttpResponse, RequestError};

use config::build_client;
use error::redact;
use http::{ExchangeFailure, classify_network, collect};

/// `Content-Type` sent on every GraphQL POST.
pub const CONTENT_TYPE_VALUE: &str = "application/json";

/// Classifies a captured response for a typed operation.
///
/// GraphQL errors take precedence at any status; other non-2xx responses are
/// HTTP failures. A body that is not JSON at all is reported with its status
/// and content type rather than as a parser error.
pub fn classify_typed<T: DeserializeOwned>(response: RawHttpResponse) -> Result<T, RequestError> {
    let parsed = match parse_response::<T>(&response.body) {
        Err(ResponseError::MalformedJson(source)) if !declares_json(&response.headers) => {
            Err(ResponseError::NotJson {
                status: response.status,
                content_type: content_type(&response.headers),
                source,
            })
        }
        other => other,
    };
    match (parsed, response.status.is_success()) {
        (Ok(data), true) => Ok(data),
        (Ok(_), false) => Err(RequestError::Http {
            response: Box::new(response),
            body: HttpBodyShape::Data,
        }),
        (
            Err(ResponseError::GraphQl {
                errors,
                partial_data,
            }),
            _,
        ) => Err(RequestError::GraphQl {
            status: response.status,
            headers: response.headers,
            errors,
            partial_data,
        }),
        (Err(error), true) => Err(RequestError::Response(error)),
        (Err(error), false) => Err(RequestError::Http {
            response: Box::new(response),
            body: HttpBodyShape::Unusable(error),
        }),
    }
}

pub(super) fn content_type(headers: &HeaderMap) -> Option<String> {
    headers
        .get(CONTENT_TYPE)
        .map(|value| String::from_utf8_lossy(value.as_bytes()).into_owned())
}

fn declares_json(headers: &HeaderMap) -> bool {
    content_type(headers).is_some_and(|value| value.to_ascii_lowercase().contains("json"))
}

/// One configured HTTP client bound to an endpoint and key.
#[derive(Clone, Debug)]
pub struct LinearClient {
    http: reqwest::Client,
    endpoint: EndpointUrl,
    api_key: ApiKey,
    deadline: Deadline,
    max_response_bytes: ResponseCap,
}

impl LinearClient {
    pub fn new(
        endpoint: EndpointUrl,
        api_key: ApiKey,
        config: ClientConfig,
    ) -> Result<Self, ClientBuildError> {
        Ok(Self {
            http: build_client(&config)?,
            endpoint,
            api_key,
            deadline: config.deadline,
            max_response_bytes: config.max_response_bytes,
        })
    }

    pub fn endpoint(&self) -> &EndpointUrl {
        &self.endpoint
    }

    /// Runs a query operation; `Q`'s derive fixes the document and the
    /// variables type.
    pub async fn query<Q, V>(&self, variables: V) -> Result<Q, RequestError>
    where
        Q: cynic::QueryBuilder<V> + DeserializeOwned,
        V: Serialize,
    {
        self.execute(Q::build(variables)).await
    }

    /// Runs a mutation operation. Mutations are sent once and never retried.
    pub async fn mutate<M, V>(&self, variables: V) -> Result<M, RequestError>
    where
        M: cynic::MutationBuilder<V> + DeserializeOwned,
        V: Serialize,
    {
        self.execute(M::build(variables)).await
    }

    /// Sends a built operation and decodes its data as `T`, which is usually
    /// the operation's own type; `linear schema` decodes into raw JSON.
    pub async fn execute<T, F, V>(&self, operation: Operation<F, V>) -> Result<T, RequestError>
    where
        T: DeserializeOwned,
        V: Serialize,
    {
        let request = GraphQlRequest::new(operation).map_err(RequestError::RequestBody)?;
        let response = self.send_request(&request).await?;
        self.classify(response)
    }

    /// Sends a request body and returns the exact response.
    pub async fn send_request(
        &self,
        request: &GraphQlRequest,
    ) -> Result<RawHttpResponse, RequestError> {
        let body = serde_json::to_vec(request).map_err(RequestError::RequestBody)?;
        self.post(body).await
    }

    async fn post(&self, body: Vec<u8>) -> Result<RawHttpResponse, RequestError> {
        let response = self
            .http
            .post(self.endpoint.url.clone())
            .timeout(self.deadline.duration())
            .header(AUTHORIZATION, self.api_key.header_value())
            .header(CONTENT_TYPE, HeaderValue::from_static(CONTENT_TYPE_VALUE))
            .body(body)
            .send()
            .await
            .map_err(|error| self.failure(classify_network(error)))?;
        collect(response, self.max_response_bytes)
            .await
            .map_err(|failure| self.failure(failure))
    }

    /// [`classify_typed`], with the API key redacted from retained HTTP bodies.
    fn classify<T: DeserializeOwned>(&self, response: RawHttpResponse) -> Result<T, RequestError> {
        classify_typed(response).map_err(|failure| match failure {
            RequestError::Http { mut response, body } => {
                response.body = redact(&response.body, self.api_key.value.as_bytes());
                RequestError::Http { response, body }
            }
            other => other,
        })
    }

    /// Sends a prepared envelope and returns the exact response.
    pub async fn send_legacy<V: Serialize>(
        &self,
        request: &LegacyRequest<V>,
    ) -> Result<RawHttpResponse, RequestError> {
        let body = serde_json::to_vec(request).map_err(RequestError::RequestBody)?;
        self.post(body).await
    }

    /// Sends a typed operation's envelope and classifies the response.
    pub async fn execute_legacy<T: DeserializeOwned, V: Serialize>(
        &self,
        request: &LegacyRequest<V>,
    ) -> Result<T, RequestError> {
        let response = self.send_legacy(request).await?;
        self.classify(response)
    }

    fn failure(&self, failure: ExchangeFailure) -> RequestError {
        match failure {
            ExchangeFailure::ResponseTooLarge { status, limit } => {
                RequestError::ResponseTooLarge { status, limit }
            }
            ExchangeFailure::Timeout => RequestError::Timeout {
                origin: self.endpoint.origin.clone(),
                deadline: self.deadline,
            },
            ExchangeFailure::Network { phase, source } => RequestError::Network {
                origin: self.endpoint.origin.clone(),
                phase,
                source,
            },
        }
    }
}

#[cfg(test)]
mod tests;
