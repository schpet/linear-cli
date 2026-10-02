//! `linear api`: send a user-written GraphQL document and print the response.
use crate::{
    cli::api::Api,
    commands::text_input,
    error::{AppError, AppErrorKind, ExitStatus},
    graphql::transport::GraphQlTransport,
};
use serde_json::{Map, Number, Value};
use std::collections::HashSet;

pub const CONTEXT: &str = "API request failed";

/// Parses response or input text as JSON; `None` when it is not JSON.
pub fn decode(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok()
}
fn no_query() -> AppError {
    AppError::new(AppErrorKind::Validation,"No query provided").with_suggestion("Provide a query as an argument: linear api '{ viewer { id } }'\n  Or pipe from stdin: echo '{ viewer { id } }' | linear api")
}
fn stdin_all() -> Result<String, AppError> {
    Ok(text_input::read_stdin(std::io::stdin().lock())?
        .map(|text| text.trim().to_owned())
        .unwrap_or_default())
}
pub fn resolve_query(positional: Option<&str>, stdin_tty: bool) -> Result<String, AppError> {
    if let Some(query) = positional.filter(|s| !s.is_empty() && *s != "-") {
        return Ok(query.to_owned());
    }
    let content = if positional == Some("-") || !stdin_tty {
        stdin_all()?
    } else {
        String::new()
    };
    if content.is_empty() {
        Err(no_query())
    } else {
        Ok(content)
    }
}
/// File or stdin content is sent as JSON when it parses, otherwise as a string.
fn parsed_or_string(text: String) -> Value {
    decode(&text).unwrap_or(Value::String(text))
}
/// A `--variable` value: booleans, `null` and canonically written numbers are
/// coerced; anything else is a string.
fn plain(text: &str) -> Result<Value, AppError> {
    match text {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        "null" => return Ok(Value::Null),
        _ => {}
    }
    if text.parse::<f64>().is_ok_and(|number| !number.is_finite()) {
        return Err(AppError::new(
            AppErrorKind::Validation,
            format!("Variable value {text} is not a finite number"),
        )
        .with_suggestion("Pass a finite number, or a JSON string through --variables-json."));
    }
    Ok(match serde_json::from_str::<Number>(text) {
        Ok(number) if number.to_string() == text => Value::Number(number),
        _ => Value::String(text.to_owned()),
    })
}
pub fn variables(action: &Api) -> Result<Map<String, Value>, AppError> {
    let mut variables = Map::new();
    if let Some(text) = action.variables_json.as_deref().filter(|s| !s.is_empty()) {
        let value = decode(text).ok_or_else(|| {
            AppError::new(
                AppErrorKind::Validation,
                format!("Invalid JSON for --variables-json: {text}"),
            )
            .with_suggestion(
                "Provide a valid JSON object, e.g. --variables-json '{\"key\": \"value\"}'",
            )
        })?;
        let kind = match value {
            Value::Object(object) => {
                variables = object;
                None
            }
            Value::Null => Some("null"),
            Value::Bool(_) => Some("boolean"),
            Value::Number(_) => Some("number"),
            Value::String(_) => Some("string"),
            Value::Array(_) => Some("array"),
        };
        if let Some(kind) = kind {
            return Err(AppError::new(
                AppErrorKind::Validation,
                format!("--variables-json must be a JSON object, got {kind}"),
            )
            .with_suggestion(
                "Provide a JSON object, e.g. --variables-json '{\"key\": \"value\"}'",
            ));
        }
    }
    for entry in &action.variable {
        let raw = &entry.value;
        let value = if raw == "@-" {
            let text = stdin_all()?;
            if text.is_empty() {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "No data on stdin for @- value",
                ));
            }
            parsed_or_string(text)
        } else if let Some(path) = raw.strip_prefix('@') {
            let text = text_input::read_file(path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    AppError::new(AppErrorKind::Validation, format!("File not found: {path}"))
                } else {
                    AppError::new(
                        AppErrorKind::IoProcess,
                        format!("Failed to read file: {path}"),
                    )
                    .with_source(e)
                }
            })?;
            parsed_or_string(text.trim().to_owned())
        } else {
            plain(raw)?
        };
        variables.insert(entry.key.clone(), value);
    }
    Ok(variables)
}
pub fn request(query: &str, variables: &Map<String, Value>) -> String {
    let mut body = Map::new();
    body.insert("query".into(), Value::String(query.into()));
    if !variables.is_empty() {
        body.insert("variables".into(), Value::Object(variables.clone()));
    }
    Value::Object(body).to_string()
}
pub struct ApiOutput {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
    pub console: bool,
}
fn output(status: ExitStatus, text: String, tty: bool, raw: bool, silent: bool) -> ApiOutput {
    ApiOutput {
        status,
        stdout: if silent { String::new() } else { text },
        stderr: String::new(),
        console: tty || raw,
    }
}
fn json_output(value: &Value, raw: &str, tty: bool) -> String {
    if tty {
        format!("{value:#}\n")
    } else if value.is_string() {
        raw.to_owned()
    } else {
        value.to_string()
    }
}
fn has_errors(value: &Value) -> bool {
    value
        .get("errors")
        .and_then(Value::as_array)
        .is_some_and(|errors| !errors.is_empty())
}
fn is_connection(object: &Map<String, Value>) -> bool {
    object.contains_key("nodes") && object.contains_key("pageInfo")
}
fn count_connections(value: &Value) -> usize {
    match value {
        Value::Object(object) if is_connection(object) => 1,
        Value::Object(object) => object.values().map(count_connections).sum(),
        Value::Array(values) => values.iter().map(count_connections).sum(),
        _ => 0,
    }
}
struct Page {
    nodes: Vec<Value>,
    has_next: bool,
    end_cursor: Option<String>,
}
fn find_page(value: &Value) -> Option<Page> {
    match value {
        Value::Object(object) => {
            if is_connection(object)
                && let Some(info @ (Value::Object(_) | Value::Array(_))) = object.get("pageInfo")
            {
                return Some(Page {
                    nodes: object
                        .get("nodes")
                        .and_then(Value::as_array)
                        .cloned()
                        .unwrap_or_default(),
                    has_next: info.get("hasNextPage").and_then(Value::as_bool) == Some(true),
                    end_cursor: info
                        .get("endCursor")
                        .and_then(Value::as_str)
                        .filter(|cursor| !cursor.is_empty())
                        .map(str::to_owned),
                });
            }
            object.values().find_map(find_page)
        }
        Value::Array(values) => values.iter().find_map(find_page),
        _ => None,
    }
}
pub async fn execute(
    transport: &GraphQlTransport,
    query: &str,
    variables: Map<String, Value>,
    paginate: bool,
    silent: bool,
    tty: bool,
) -> Result<ApiOutput, AppError> {
    let mut nodes = Vec::new();
    let mut cursor: Option<String> = None;
    let mut sent = HashSet::new();
    loop {
        let mut vars = variables.clone();
        if paginate {
            if !sent.insert(cursor.clone()) {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "Repeated pagination cursor; request not sent, prior requests may have had effects",
                ));
            }
            vars.insert(
                "after".into(),
                cursor.clone().map_or(Value::Null, Value::String),
            );
        }
        let (status, text) = transport.fetch_api(request(query, &vars)).await?;
        if status >= 400 {
            return Ok(ApiOutput {
                status: ExitStatus::HandledFailure,
                stdout: String::new(),
                stderr: if silent {
                    String::new()
                } else {
                    format!("{text}\n")
                },
                console: true,
            });
        }
        let Some(parsed) = decode(&text) else {
            return Ok(output(
                if paginate {
                    ExitStatus::HandledFailure
                } else {
                    ExitStatus::Success
                },
                format!("{text}\n"),
                tty,
                true,
                silent,
            ));
        };
        if parsed.is_null() {
            if paginate {
                return Err(AppError::new(
                    AppErrorKind::GraphQl,
                    "Linear returned a null response body",
                ));
            }
            return Ok(output(
                ExitStatus::Success,
                format!("{text}\n"),
                tty,
                true,
                silent,
            ));
        }
        if has_errors(&parsed) {
            return Ok(output(
                ExitStatus::HandledFailure,
                json_output(&parsed, &text, tty),
                tty,
                false,
                silent,
            ));
        }
        if !paginate {
            return Ok(output(
                ExitStatus::Success,
                json_output(&parsed, &text, tty),
                tty,
                false,
                silent,
            ));
        }
        if nodes.is_empty()
            && parsed
                .get("data")
                .is_some_and(|data| count_connections(data) > 1)
        {
            return Err(AppError::new(AppErrorKind::Validation,"--paginate does not support queries with multiple paginated connections").with_suggestion("Use cursor-based pagination manually with $after and pageInfo { hasNextPage endCursor }."));
        }
        match find_page(&parsed) {
            None => {
                return Ok(output(
                    ExitStatus::Success,
                    json_output(&parsed, &text, tty),
                    tty,
                    false,
                    silent,
                ));
            }
            Some(page) => {
                nodes.extend(page.nodes);
                match page.end_cursor {
                    Some(end) if page.has_next => cursor = Some(end),
                    _ => break,
                }
            }
        }
    }
    let all = Value::Array(nodes);
    Ok(output(
        ExitStatus::Success,
        json_output(&all, &all.to_string(), tty),
        tty,
        false,
        silent,
    ))
}

/// Reports a stdout write failure with the underlying IO message, keeping the
/// typed output failure as the error source so stream routing still works.
pub fn stdout_write_error(mut error: AppError) -> AppError {
    use std::error::Error;
    if let Some(source) = error
        .source()
        .and_then(Error::source)
        .and_then(|source| source.downcast_ref::<std::io::Error>())
    {
        error.message = source.to_string();
    }
    error
}
