//! Source ClientError messages for bulk executors that expose exception.message.
//! Ordinary command failures retain the existing friendly error path.
use crate::{
    error::{AppError, AppErrorKind},
    graphql::{
        envelope::GraphQlRequest,
        transport::{GraphQlTransport, RawHttpResponse, TransportFailure},
    },
};
use serde::{
    Serialize, Serializer,
    de::DeserializeOwned,
    ser::{Error as _, SerializeMap, SerializeSeq},
};
use serde_json::Value;

pub enum BulkExchangeFailure {
    Strict(AppError),
    Ordinary(String),
}
impl BulkExchangeFailure {
    pub fn into_error(self) -> AppError {
        match self {
            Self::Strict(error) => error,
            Self::Ordinary(message) => AppError::new(AppErrorKind::GraphQl, message),
        }
    }
}
fn strict(message: &str) -> BulkExchangeFailure {
    BulkExchangeFailure::Strict(AppError::new(AppErrorKind::GraphQl, message))
}
/// JS property enumeration and binary64 JSON numbers, including integer-backed values.
pub(crate) struct JsValue<'a>(pub(crate) &'a Value);
fn index(key: &str) -> Option<u32> {
    let value = key.parse::<u32>().ok()?;
    (value != u32::MAX && value.to_string() == key).then_some(value)
}
impl Serialize for JsValue<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self.0 {
            Value::Null => serializer.serialize_none(),
            Value::Bool(value) => serializer.serialize_bool(*value),
            Value::String(value) => serializer.serialize_str(value),
            Value::Number(value) => {
                let number = value.as_f64().ok_or_else(|| {
                    S::Error::custom("JSON metadata number is not representable as binary64")
                })?;
                crate::json_number::finite_js_number(number)
                    .map_err(S::Error::custom)?
                    .serialize(serializer)
            }
            Value::Array(values) => {
                let mut sequence = serializer.serialize_seq(Some(values.len()))?;
                for value in values {
                    sequence.serialize_element(&JsValue(value))?
                }
                sequence.end()
            }
            Value::Object(values) => {
                let mut keys: Vec<_> = values
                    .keys()
                    .filter_map(|key| index(key).map(|n| (n, key)))
                    .collect();
                keys.sort_by_key(|(n, _)| *n);
                let mut map = serializer.serialize_map(Some(values.len()))?;
                for (_, key) in keys {
                    let value = values
                        .get(key)
                        .ok_or_else(|| S::Error::custom("metadata key disappeared"))?;
                    map.serialize_entry(key, &JsValue(value))?
                }
                for (key, value) in values {
                    if index(key).is_none() {
                        map.serialize_entry(key, &JsValue(value))?
                    }
                }
                map.end()
            }
        }
    }
}
#[derive(Serialize)]
struct ResponseMetadata<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<JsValue<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    errors: Option<JsValue<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    extensions: Option<JsValue<'a>>,
    status: u16,
    headers: serde_json::Map<String, Value>,
    body: &'a str,
}
#[derive(Serialize)]
struct RequestMetadata<'a> {
    query: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    variables: Option<JsValue<'a>>,
}
#[derive(Serialize)]
struct Metadata<'a> {
    response: ResponseMetadata<'a>,
    request: RequestMetadata<'a>,
}
/// Observe the same captured response; never repeat an exchange or change its limits.
pub fn source_error<V: Serialize>(
    response: &RawHttpResponse,
    request: &GraphQlRequest<V>,
) -> Result<Option<String>, BulkExchangeFailure> {
    let source = super::source_response::SourceResponse::classify(response);
    source_error_classified(response, request, &source)
}
fn source_error_classified<V: Serialize>(
    response: &RawHttpResponse,
    request: &GraphQlRequest<V>,
    source: &super::source_response::SourceResponse,
) -> Result<Option<String>, BulkExchangeFailure> {
    let body = source.text.as_str();
    let json = source.is_json();
    let parsed = source.parsed();
    if response.status.is_success() && !json {
        return Ok(Some(source.invalid_execution_message()));
    }
    if response.status.is_success() && parsed.is_none() {
        return Err(strict(
            "native-bulk-response-json-syntax: response body is not valid JSON",
        ));
    }
    if matches!(parsed, Some(Value::Array(_))) {
        return Err(strict(
            "native-bulk-single-response-envelope: expected one response object, got an array",
        ));
    }
    let object = parsed.and_then(Value::as_object);
    if object
        .and_then(|o| o.get("errors"))
        .is_some_and(Value::is_object)
    {
        return Err(strict(
            "native-bulk-error-array-shape: GraphQL errors must be an array",
        ));
    }
    let valid = object.is_some_and(|o| {
        o.get("data").is_none_or(|v| v.is_object() || v.is_null())
            && o.get("errors").is_none_or(Value::is_array)
            && o.get("extensions").is_none_or(Value::is_object)
    });
    if response.status.is_success() && !valid {
        return Err(strict(
            "native-bulk-execution-field-shape: invalid execution result fields",
        ));
    }
    let fields = valid.then_some(object).flatten();
    let errors = fields
        .and_then(|o| o.get("errors"))
        .and_then(Value::as_array);
    if let Some(errors) = errors {
        for error in errors {
            if !error.is_object() || !error.get("message").is_some_and(Value::is_string) {
                return Err(strict(
                    "native-bulk-execution-field-shape: GraphQL error entry must have a string message",
                ));
            }
        }
    }
    if response.status.is_success() && errors.is_none_or(Vec::is_empty) {
        return Ok(None);
    }
    let fallback = format!("GraphQL Error (Code: {})", response.status.as_u16());
    let first = errors
        .and_then(|e| e.first())
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .unwrap_or(&fallback);
    let variables = request
        .variables
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|e| {
            BulkExchangeFailure::Strict(
                AppError::new(
                    AppErrorKind::Invariant,
                    "could not serialize request metadata",
                )
                .with_source(e),
            )
        })?;
    let metadata = Metadata {
        response: ResponseMetadata {
            data: fields.and_then(|o| o.get("data")).map(JsValue),
            errors: fields.and_then(|o| o.get("errors")).map(JsValue),
            extensions: fields.and_then(|o| o.get("extensions")).map(JsValue),
            status: response.status.as_u16(),
            headers: serde_json::Map::new(),
            body,
        },
        request: RequestMetadata {
            query: request.query.strip_suffix('\n').unwrap_or(&request.query),
            variables: variables.as_ref().map(JsValue),
        },
    };
    let serialized = serde_json::to_string(&metadata).map_err(|e| {
        BulkExchangeFailure::Strict(
            AppError::new(
                AppErrorKind::Invariant,
                "could not serialize source bulk exception metadata",
            )
            .with_source(e),
        )
    })?;
    Ok(Some(format!("{first}: {serialized}")))
}
pub async fn execute<T: DeserializeOwned, V: Serialize>(
    transport: &GraphQlTransport,
    request: &GraphQlRequest<V>,
) -> Result<T, BulkExchangeFailure> {
    let response = transport
        .send_request(request)
        .await
        .map_err(|e| BulkExchangeFailure::Ordinary(AppError::from(e).to_string()))?;
    let source = super::source_response::SourceResponse::classify(&response);
    let message = source_error_classified(&response, request, &source)?;
    let result = super::transport::classify_typed_classified(response, source);
    if let Some(message) = message {
        return Err(BulkExchangeFailure::Ordinary(message));
    }
    result.map_err(|error| match error {
        TransportFailure::Response(_) | TransportFailure::RequestBody(_) => {
            BulkExchangeFailure::Strict(AppError::from(error))
        }
        other => BulkExchangeFailure::Ordinary(AppError::from(other).to_string()),
    })
}

/// Opt-in source exception class; ordinary transport and old callers stay unchanged.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceExceptionKind {
    Plain,
    Client,
}
#[derive(Debug)]
pub struct SourceException {
    pub kind: SourceExceptionKind,
    pub message: String,
    pub preferred_message: Option<String>,
}
impl SourceException {
    pub fn is_not_found(&self) -> bool {
        if self.kind != SourceExceptionKind::Client {
            return false;
        }
        let message = self
            .preferred_message
            .as_deref()
            .unwrap_or(&self.message)
            .to_lowercase();
        message.contains("not found") || message.contains("could not find")
    }
}
pub enum ObservedExchangeFailure {
    Strict(AppError),
    Ordinary(SourceException),
}
impl ObservedExchangeFailure {
    pub fn into_error(self) -> AppError {
        match self {
            Self::Strict(error) => error,
            Self::Ordinary(error) => AppError::new(AppErrorKind::GraphQl, error.message),
        }
    }
}
pub fn observe_source_error<V: Serialize>(
    response: &RawHttpResponse,
    request: &GraphQlRequest<V>,
) -> Result<Option<SourceException>, BulkExchangeFailure> {
    let source = super::source_response::SourceResponse::classify(response);
    observe_source_error_classified(response, request, &source)
}
fn observe_source_error_classified<V: Serialize>(
    response: &RawHttpResponse,
    request: &GraphQlRequest<V>,
    source: &super::source_response::SourceResponse,
) -> Result<Option<SourceException>, BulkExchangeFailure> {
    let Some(message) = source_error_classified(response, request, source)? else {
        return Ok(None);
    };
    let json = source.is_json();
    let kind = if !response.status.is_success() || json {
        SourceExceptionKind::Client
    } else {
        SourceExceptionKind::Plain
    };
    let mut preferred_message = None;
    if kind == SourceExceptionKind::Client
        && json
        && let Some(parsed) = source.parsed()
        && let Some(first) = parsed
            .get("errors")
            .and_then(Value::as_array)
            .and_then(|errors| errors.first())
    {
        if let Some(extensions) = first.get("extensions") {
            let extensions = extensions.as_object().ok_or_else(|| {
                strict("native-bulk-not-found-metadata-shape: error extensions must be an object")
            })?;
            if let Some(value) = extensions.get("userPresentableMessage") {
                let value = value.as_str().ok_or_else(|| strict("native-bulk-not-found-metadata-shape: presentable message must be a string"))?;
                if !value.is_empty() {
                    preferred_message = Some(value.to_owned());
                }
            }
        }
        if preferred_message.is_none() {
            preferred_message = first
                .get("message")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned);
        }
    }
    Ok(Some(SourceException {
        kind,
        message,
        preferred_message,
    }))
}
pub async fn execute_observed<T: DeserializeOwned, V: Serialize>(
    transport: &GraphQlTransport,
    request: &GraphQlRequest<V>,
) -> Result<T, ObservedExchangeFailure> {
    let response = transport.send_request(request).await.map_err(|error| {
        ObservedExchangeFailure::Ordinary(SourceException {
            kind: SourceExceptionKind::Plain,
            message: AppError::from(error).to_string(),
            preferred_message: None,
        })
    })?;
    let source = super::source_response::SourceResponse::classify(&response);
    let observation =
        observe_source_error_classified(&response, request, &source).map_err(|failure| {
            match failure {
                BulkExchangeFailure::Strict(error) => ObservedExchangeFailure::Strict(error),
                BulkExchangeFailure::Ordinary(message) => {
                    ObservedExchangeFailure::Ordinary(SourceException {
                        kind: SourceExceptionKind::Plain,
                        message,
                        preferred_message: None,
                    })
                }
            }
        })?;
    if let Some(error) = observation {
        return Err(ObservedExchangeFailure::Ordinary(error));
    }
    super::transport::classify_typed_classified(response, source).map_err(|error| match error {
        TransportFailure::Response(_) | TransportFailure::RequestBody(_) => {
            ObservedExchangeFailure::Strict(AppError::from(error))
        }
        other => ObservedExchangeFailure::Ordinary(SourceException {
            kind: SourceExceptionKind::Plain,
            message: AppError::from(other).to_string(),
            preferred_message: None,
        }),
    })
}
