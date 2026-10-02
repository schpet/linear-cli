//! User-authored runtime GraphQL documents are an explicitly dynamic API boundary.
use crate::{
    cli::api::Api,
    commands::text_input,
    error::{AppError, AppErrorKind, ExitStatus},
    graphql::transport::GraphQlTransport,
    js_value::{JsObject, JsValue, js_number, js_pretty, js_stringify},
};
use std::collections::HashSet;

pub const CONTEXT: &str = "API request failed";
#[derive(Debug)]
pub enum JsonDecode {
    Value(JsValue),
    Malformed,
}
/// Borrowed whole-input syntax probing distinguishes malformed source raw/string
/// branches from valid JSON outside the explicitly accepted typed codec domain.
pub fn decode(text: &str, sent: bool) -> Result<JsonDecode, AppError> {
    match serde_json::from_str::<JsValue>(text) {
        Ok(value) => Ok(JsonDecode::Value(value)),
        Err(error) => match serde_json::from_str::<&serde_json::value::RawValue>(text) {
            Err(_) => Ok(JsonDecode::Malformed),
            Ok(_) => Err(AppError::new(
                AppErrorKind::Validation,
                format!(
                    "JSON is outside the supported typed domain{}",
                    if sent {
                        "; request sent, any effects unknown; do not retry automatically"
                    } else {
                        "; request not sent"
                    }
                ),
            )
            .with_source(error)),
        },
    }
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
fn parsed_or_string(text: String) -> Result<JsValue, AppError> {
    match decode(&text, false)? {
        JsonDecode::Value(value) => Ok(value),
        JsonDecode::Malformed => Ok(JsValue::String(text)),
    }
}
fn plain(text: &str) -> JsValue {
    match text {
        "true" => JsValue::Bool(true),
        "false" => JsValue::Bool(false),
        "null" | "Infinity" | "-Infinity" => JsValue::Null,
        _ => match text.parse::<f64>() {
            Ok(n) if n.is_finite() && js_number(n) == text => JsValue::Number(n),
            _ => JsValue::String(text.to_owned()),
        },
    }
}
pub fn variables(action: &Api) -> Result<JsObject, AppError> {
    let mut variables = JsObject::default();
    if let Some(text) = action.variables_json.as_deref().filter(|s| !s.is_empty()) {
        let value = match decode(text, false)? {
            JsonDecode::Value(v) => v,
            JsonDecode::Malformed => {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    format!("Invalid JSON for --variables-json: {text}"),
                )
                .with_suggestion(
                    "Provide a valid JSON object, e.g. --variables-json '{\"key\": \"value\"}'",
                ));
            }
        };
        match value {
            JsValue::Object(obj) => {
                for (key, value) in obj.entries() {
                    variables.assign(key.clone(), value.clone());
                }
            }
            other => {
                let kind = match other {
                    JsValue::Null | JsValue::Object(_) => "object",
                    JsValue::Bool(_) => "boolean",
                    JsValue::Number(_) => "number",
                    JsValue::String(_) => "string",
                    JsValue::Array(_) => "array",
                };
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    format!("--variables-json must be a JSON object, got {kind}"),
                )
                .with_suggestion(
                    "Provide a JSON object, e.g. --variables-json '{\"key\": \"value\"}'",
                ));
            }
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
            parsed_or_string(text)?
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
            parsed_or_string(text.trim().to_owned())?
        } else {
            plain(raw)
        };
        variables.assign(entry.key.clone(), value);
    }
    Ok(variables)
}
pub fn request(query: &str, variables: &JsObject) -> String {
    let mut body = JsObject::default();
    body.assign("query".into(), JsValue::String(query.into()));
    if !variables.is_empty() {
        body.assign("variables".into(), JsValue::Object(variables.clone()));
    }
    js_stringify(&JsValue::Object(body))
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
fn json_output(value: &JsValue, raw: &str, tty: bool) -> String {
    if tty {
        format!("{}\n", js_pretty(value))
    } else if matches!(value, JsValue::String(_)) {
        raw.to_owned()
    } else {
        js_stringify(value)
    }
}
fn has_errors(value: &JsValue) -> bool {
    matches!(value.get("errors"),Some(JsValue::Array(v)) if !v.is_empty())
}
fn count_connections(value: &JsValue) -> usize {
    match value {
        JsValue::Object(object)
            if object.get("nodes").is_some() && object.get("pageInfo").is_some() =>
        {
            1
        }
        JsValue::Object(object) => object
            .entries()
            .iter()
            .map(|(_, v)| count_connections(v))
            .sum(),
        JsValue::Array(values) => values.iter().map(count_connections).sum(),
        _ => 0,
    }
}
fn find_page(value: &JsValue) -> Option<(Vec<JsValue>, bool, JsValue)> {
    match value {
        JsValue::Object(object) => {
            if let (Some(nodes), Some(info @ (JsValue::Object(_) | JsValue::Array(_)))) =
                (object.get("nodes"), object.get("pageInfo"))
            {
                return Some((
                    match nodes {
                        JsValue::Array(v) => v.clone(),
                        _ => vec![],
                    },
                    info.get("hasNextPage").is_some_and(JsValue::truthy),
                    info.get("endCursor").cloned().unwrap_or(JsValue::Null),
                ));
            }
            object.entries().iter().find_map(|(_, v)| find_page(v))
        }
        JsValue::Array(values) => values.iter().find_map(find_page),
        _ => None,
    }
}
pub async fn execute(
    transport: &GraphQlTransport,
    query: &str,
    variables: JsObject,
    paginate: bool,
    silent: bool,
    tty: bool,
) -> Result<ApiOutput, AppError> {
    let mut nodes = Vec::new();
    let mut cursor = JsValue::Null;
    let mut sent = HashSet::new();
    loop {
        let mut vars = variables.clone();
        if paginate {
            let key = js_stringify(&cursor);
            if !sent.insert(key) {
                return Err(AppError::new(
                    AppErrorKind::Validation,
                    "Repeated pagination cursor; request not sent, prior requests may have had effects",
                ));
            }
            vars.assign("after".into(), cursor.clone());
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
        let parsed = match decode(&text, true)? {
            JsonDecode::Malformed => {
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
            }
            JsonDecode::Value(value) => value,
        };
        if parsed == JsValue::Null {
            if paginate {
                return Err(AppError::new(
                    AppErrorKind::GraphQl,
                    "Cannot read properties of null (reading 'errors')",
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
            Some((page, next, end)) => {
                nodes.extend(page);
                if !next || !end.truthy() {
                    break;
                }
                cursor = end;
            }
        }
    }
    let all = JsValue::Array(nodes);
    Ok(output(
        ExitStatus::Success,
        json_output(&all, &js_stringify(&all), tty),
        tty,
        false,
        silent,
    ))
}

/// Deno's strict stdout.writeSync exposes the original IO diagnostic. Keep the
/// typed OutputFailure as the source so stream routing/finalization still work.
pub fn source_output_error(mut error: AppError) -> AppError {
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
