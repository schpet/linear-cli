//! `linear api`: send a user-written GraphQL document and print the response.
use crate::client::{LinearClient, classify_failure};
use crate::graphql::envelope::ResponseGraphQlError;
use crate::graphql::pagination::{Page, PageInfo, Pages};
use crate::{
    cli::api::Api,
    commands::text_input,
    ctx::Ctx,
    error::{Error, Failure, Result, ResultExt},
};
use reqwest::StatusCode;
use serde_json::{Map, Number, Value};

pub fn run(ctx: &Ctx, args: &Api) -> Result<()> {
    request_and_print(ctx, args).context("API request failed")
}

fn request_and_print(ctx: &Ctx, args: &Api) -> Result<()> {
    let query = resolve_query(args.graphql_document.as_deref(), ctx.stdin_tty())?;
    let variables = variables(args)?;
    let client = ctx.client()?;
    let response = ctx.block_on(execute(
        client,
        &query,
        variables,
        args.paginate,
        ctx.stdout_tty(),
    ))?;
    // The response is printed either way; the exit status says whether it
    // was a success and, if not, the failure's class.
    let (text, failure) = match response {
        Response::Data(text) => (text, None),
        Response::Errors(text, failure) => (text, Some(failure)),
        Response::HttpError(body, failure) => {
            if !args.silent {
                ctx.eprint(body)?;
            }
            return Err(Error::reported(failure));
        }
    };
    if !args.silent {
        ctx.print(text)?;
    }
    match failure {
        None => Ok(()),
        Some(failure) => Err(Error::reported(failure)),
    }
}

/// Parses response or input text as JSON; `None` when it is not JSON.
fn decode(text: &str) -> Option<Value> {
    serde_json::from_str(text).ok()
}
fn no_query() -> Error {
    Error::invalid("No query provided").with_hint("Provide a query as an argument: linear api '{ viewer { id } }'\n  Or pipe from stdin: echo '{ viewer { id } }' | linear api")
}
fn stdin_all() -> Result<String> {
    Ok(text_input::read_stdin(std::io::stdin().lock())?
        .map(|text| text.trim().to_owned())
        .unwrap_or_default())
}
fn resolve_query(positional: Option<&str>, stdin_tty: bool) -> Result<String> {
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
fn plain(text: &str) -> Result<Value> {
    match text {
        "true" => return Ok(Value::Bool(true)),
        "false" => return Ok(Value::Bool(false)),
        "null" => return Ok(Value::Null),
        _ => {}
    }
    if text.parse::<f64>().is_ok_and(|number| !number.is_finite()) {
        return Err(
            Error::invalid(format!("Variable value {text} is not a finite number"))
                .with_hint("Pass a finite number, or a JSON string through --variables-json."),
        );
    }
    Ok(match serde_json::from_str::<Number>(text) {
        Ok(number) if number.to_string() == text => Value::Number(number),
        _ => Value::String(text.to_owned()),
    })
}
fn variables(action: &Api) -> Result<Map<String, Value>> {
    let mut variables = Map::new();
    if let Some(text) = action.variables_json.as_deref().filter(|s| !s.is_empty()) {
        let value = decode(text).ok_or_else(|| {
            Error::invalid(format!("Invalid JSON for --variables-json: {text}")).with_hint(
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
            return Err(Error::invalid(format!(
                "--variables-json must be a JSON object, got {kind}"
            ))
            .with_hint("Provide a JSON object, e.g. --variables-json '{\"key\": \"value\"}'"));
        }
    }
    for entry in &action.variable {
        let raw = &entry.value;
        let value = if raw == "@-" {
            let text = stdin_all()?;
            if text.is_empty() {
                return Err(Error::invalid("No data on stdin for @- value"));
            }
            parsed_or_string(text)
        } else if let Some(path) = raw.strip_prefix('@') {
            let text = text_input::read_file(path).map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    Error::new(format!("File not found: {path}"))
                } else {
                    Error::new(format!("Failed to read file: {path}")).with_source(e)
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
fn request(query: &str, variables: &Map<String, Value>) -> String {
    let mut body = Map::new();
    body.insert("query".into(), Value::String(query.into()));
    if !variables.is_empty() {
        body.insert("variables".into(), Value::Object(variables.clone()));
    }
    Value::Object(body).to_string()
}
enum Response {
    /// The response body as printed.
    Data(String),
    /// GraphQL errors, or a body that is not JSON, as printed, and their class.
    Errors(String, Failure),
    /// A failed HTTP status, with the body for stderr, and its class.
    HttpError(String, Failure),
}

/// The response as printed: pretty on a terminal, compact when piped, and
/// always ending in one newline.
fn json_output(value: &Value, raw: &str, tty: bool) -> String {
    let text = if tty {
        format!("{value:#}")
    } else if value.is_string() {
        raw.to_owned()
    } else {
        value.to_string()
    };
    format!("{}\n", text.trim_end_matches('\n'))
}
fn has_errors(value: &Value) -> bool {
    value
        .get("errors")
        .and_then(Value::as_array)
        .is_some_and(|errors| !errors.is_empty())
}
/// The class of a response with `status` and body `parsed`, read like a typed
/// operation's (see [`classify_failure`]). An `errors` array that does not
/// have GraphQL's error shape counts as no recognizable errors.
fn failure(status: StatusCode, parsed: Option<&Value>) -> Failure {
    let errors: Vec<ResponseGraphQlError> = parsed
        .and_then(|value| value.get("errors"))
        .cloned()
        .and_then(|errors| serde_json::from_value(errors).ok())
        .unwrap_or_default();
    classify_failure(status, &errors)
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
/// The first connection (an object with `nodes` and `pageInfo`) in `value`.
fn find_connection(value: &Value) -> Option<&Map<String, Value>> {
    match value {
        Value::Object(object) if is_connection(object) => Some(object),
        Value::Object(object) => object.values().find_map(find_connection),
        Value::Array(values) => values.iter().find_map(find_connection),
        _ => None,
    }
}

/// A connection's nodes and page info, which must have the shapes
/// pagination relies on.
fn parse_page(connection: &Map<String, Value>) -> Result<Page<Value>> {
    let hint = "Select `nodes { ... }` and `pageInfo { hasNextPage endCursor }` on the connection.";
    let nodes = match connection.get("nodes") {
        Some(Value::Array(nodes)) => nodes.clone(),
        _ => {
            return Err(
                Error::new("The paginated connection's nodes are not a list").with_hint(hint),
            );
        }
    };
    let page_info = connection
        .get("pageInfo")
        .cloned()
        .map(serde_json::from_value::<PageInfo>)
        .and_then(std::result::Result::ok)
        .ok_or_else(|| {
            Error::new("The paginated connection's pageInfo lacks hasNextPage or endCursor")
                .with_hint(hint)
        })?;
    Ok(Page { nodes, page_info })
}

async fn execute(
    client: &LinearClient,
    query: &str,
    variables: Map<String, Value>,
    paginate: bool,
    tty: bool,
) -> Result<Response> {
    let mut nodes = Vec::new();
    let mut pages = Pages::new(None);
    loop {
        let mut vars = variables.clone();
        if paginate {
            vars.insert(
                "after".into(),
                pages.after().map_or(Value::Null, Value::String),
            );
        }
        let (status, text) = client.fetch_api(request(query, &vars)).await?;
        if status.as_u16() >= 400 {
            let failure = failure(status, decode(&text).as_ref());
            return Ok(Response::HttpError(format!("{text}\n"), failure));
        }
        let Some(parsed) = decode(&text) else {
            return Ok(Response::Errors(format!("{text}\n"), Failure::General));
        };
        if parsed.is_null() {
            if paginate {
                return Err(Error::new("Linear returned a null response body"));
            }
            return Ok(Response::Data(format!("{text}\n")));
        }
        if has_errors(&parsed) {
            let failure = failure(status, Some(&parsed));
            return Ok(Response::Errors(json_output(&parsed, &text, tty), failure));
        }
        if !paginate {
            return Ok(Response::Data(json_output(&parsed, &text, tty)));
        }
        let first_page = pages.after().is_none();
        if first_page
            && parsed
                .get("data")
                .is_some_and(|data| count_connections(data) > 1)
        {
            return Err(Error::new("--paginate does not support queries with multiple paginated connections").with_hint("Use cursor-based pagination manually with $after and pageInfo { hasNextPage endCursor }."));
        }
        let Some(connection) = find_connection(&parsed) else {
            if first_page {
                return Ok(Response::Data(json_output(&parsed, &text, tty)));
            }
            return Err(Error::new(
                "A later page of the response has no paginated connection",
            ));
        };
        let page = parse_page(connection)?;
        let more = pages.advance(page.nodes.len(), &page.page_info)?;
        nodes.extend(page.nodes);
        if !more {
            break;
        }
    }
    let all = Value::Array(nodes);
    Ok(Response::Data(json_output(&all, &all.to_string(), tty)))
}
