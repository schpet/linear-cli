//! `template view`: one template by UUID or exact case-insensitive name, as
//! the shared JSON projection or as its metadata and recursive pre-fills.
//!
//! A Linear URL is refused before credentials are selected. A UUID sends one
//! `GetTemplate` with the reference as typed; a name sends one `GetTemplates`
//! and matches in response order; no team is resolved. [`prepare`] and
//! [`run_with`] each attach [`CONTEXT`] once to their own failures.
//!
//! Text output parses `templateData` lazily and reports the first error in
//! render order. Rich-text bodies are printed as the generated Markdown,
//! indented, so a pre-fill can be copied into `--description`; the output does
//! not depend on terminal width or color.

use std::future::Future;

use chrono::{DateTime, TimeZone, Utc};
use cynic::QueryBuilder;
use serde_json::{Map, Number, Value};

use crate::auth::CredentialStore;
use crate::commands::client;
use crate::commands::prosemirror;
use crate::commands::relative_time::format_relative_time;
use crate::commands::template::{json as template_json, list as template_list};
use crate::config::{ConfigOptions, TransportEnvInputs};
use crate::error::{Error, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::templates::{
    GetTemplate, GetTemplateVariables, GetTemplates, Template,
};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use crate::platform::collation;
use crate::refs::{is_linear_uuid, reject_linear_url};

pub const CONTEXT: &str = "Failed to view template";

const LIST_SUGGESTION: &str = "Run `linear template list` to see every template.";
const INDENT: &str = "  ";
/// Keys whose object value is a ProseMirror document holding the body.
const RICH_TEXT_KEYS: [&str; 2] = ["descriptionData", "contentData"];
const LABEL_KEYS: [&str; 2] = ["title", "name"];
const FORM_NOTE: &str = "Form template: yes (its form is filled in inside Linear; applying it from the CLI creates the entity with the form unanswered)";
const FOOTER: &str = "References are IDs. Map them with `linear team states`, `linear label list`, `linear user list`, or `linear project list`.";

/// A reference that is not a Linear URL, classified by the UUID predicate.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TemplateReference {
    /// Sent to `GetTemplate` exactly as typed.
    Id(String),
    /// Matched against every template's lowercased name.
    Name(String),
}

impl TemplateReference {
    /// Refuse any recognized Linear URL, then classify.
    pub fn parse(reference: &str) -> Result<Self, Error> {
        reject_linear_url(reference, "a template name or UUID")?;
        Ok(if is_linear_uuid(reference) {
            Self::Id(reference.to_owned())
        } else {
            Self::Name(reference.to_owned())
        })
    }
}

pub struct Prepared {
    pub reference: TemplateReference,
    pub transport: GraphQlTransport,
}

/// Refuse a URL reference before credential selection, then build the client.
pub fn prepare(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    cli_workspace: Option<&str>,
    transport_env: &TransportEnvInputs,
    reference: &str,
) -> Result<Prepared, Error> {
    prepare_uncontextualized(
        options,
        credentials,
        cli_workspace,
        transport_env,
        reference,
    )
    .context(CONTEXT)
}

fn prepare_uncontextualized(
    options: &ConfigOptions,
    credentials: &CredentialStore,
    cli_workspace: Option<&str>,
    transport_env: &TransportEnvInputs,
    reference: &str,
) -> Result<Prepared, Error> {
    let reference = TemplateReference::parse(reference)?;
    let transport = client::prepare_transport(options, credentials, cli_workspace, transport_env)?;
    Ok(Prepared {
        reference,
        transport,
    })
}

pub fn template_request(id: &str) -> GraphQlRequest<GetTemplateVariables> {
    GraphQlRequest::with_variables(GetTemplate::build(GetTemplateVariables {
        id: id.to_owned(),
    }))
}

/// Resolve the reference with exactly one request, then render it. `now` is
/// read after the request so relative times are measured from then.
pub async fn run_with<OF, OFut, AF, AFut, Tz, Now>(
    reference: &TemplateReference,
    json: bool,
    now: Now,
    zone: &Tz,
    template_fetch: OF,
    templates_fetch: AF,
) -> Result<Vec<u8>, Error>
where
    OF: FnOnce(GraphQlRequest<GetTemplateVariables>) -> OFut,
    OFut: Future<Output = Result<GetTemplate, TransportFailure>>,
    AF: FnOnce(GraphQlRequest<()>) -> AFut,
    AFut: Future<Output = Result<GetTemplates, TransportFailure>>,
    Tz: TimeZone,
    Now: FnOnce() -> DateTime<Utc>,
{
    run_uncontextualized(reference, json, now, zone, template_fetch, templates_fetch)
        .await
        .context(CONTEXT)
}

async fn run_uncontextualized<OF, OFut, AF, AFut, Tz, Now>(
    reference: &TemplateReference,
    json: bool,
    now: Now,
    zone: &Tz,
    template_fetch: OF,
    templates_fetch: AF,
) -> Result<Vec<u8>, Error>
where
    OF: FnOnce(GraphQlRequest<GetTemplateVariables>) -> OFut,
    OFut: Future<Output = Result<GetTemplate, TransportFailure>>,
    AF: FnOnce(GraphQlRequest<()>) -> AFut,
    AFut: Future<Output = Result<GetTemplates, TransportFailure>>,
    Tz: TimeZone,
    Now: FnOnce() -> DateTime<Utc>,
{
    let template = resolve(reference, template_fetch, templates_fetch).await?;
    if json {
        return template_json::render_one(&template);
    }
    let mut text = render_text(&template, now(), zone)?;
    text.push('\n');
    Ok(text.into_bytes())
}

pub async fn run<Tz: TimeZone>(
    transport: &GraphQlTransport,
    reference: &TemplateReference,
    json: bool,
    zone: &Tz,
) -> Result<Vec<u8>, Error> {
    run_with(
        reference,
        json,
        Utc::now,
        zone,
        |request| async move { transport.execute(&request).await },
        |request| async move { transport.execute(&request).await },
    )
    .await
}

/// Whether any raw GraphQL `message` says the ID matched no template. The
/// presentable message is ignored; `Entity not found` does not match.
fn is_missing_template(failure: &TransportFailure) -> bool {
    match failure {
        TransportFailure::GraphQl { errors, .. } => errors.iter().any(|error| {
            error
                .message
                .to_ascii_lowercase()
                .contains("no template found")
        }),
        _ => false,
    }
}

async fn resolve<OF, OFut, AF, AFut>(
    reference: &TemplateReference,
    template_fetch: OF,
    templates_fetch: AF,
) -> Result<Template, Error>
where
    OF: FnOnce(GraphQlRequest<GetTemplateVariables>) -> OFut,
    OFut: Future<Output = Result<GetTemplate, TransportFailure>>,
    AF: FnOnce(GraphQlRequest<()>) -> AFut,
    AFut: Future<Output = Result<GetTemplates, TransportFailure>>,
{
    match reference {
        TemplateReference::Id(id) => match template_fetch(template_request(id)).await {
            Ok(response) => Ok(response.template),
            Err(failure) if is_missing_template(&failure) => {
                Err(Error::not_found("Template", id).with_hint(LIST_SUGGESTION))
            }
            Err(failure) => Err(Error::from(failure)),
        },
        TemplateReference::Name(name) => {
            let templates = templates_fetch(template_list::request())
                .await
                .map_err(Error::from)?
                .templates;
            select_by_name(name, templates)
        }
    }
}

fn select_by_name(reference: &str, templates: Vec<Template>) -> Result<Template, Error> {
    let wanted = reference.to_lowercase();
    let (mut matches, others): (Vec<Template>, Vec<Template>) = templates
        .into_iter()
        .partition(|template| template.name.to_lowercase() == wanted);
    match matches.len() {
        1 => Ok(matches.remove(0)),
        0 => {
            let mut names: Vec<&str> = Vec::new();
            for template in &others {
                if !names.contains(&template.name.as_str()) {
                    names.push(&template.name);
                }
            }
            names.sort_by(|left, right| collation::compare(left, right));
            let suggestion = if names.is_empty() {
                format!("No templates are available here. {LIST_SUGGESTION}")
            } else {
                let quoted: Vec<String> = names.iter().map(|name| format!("\"{name}\"")).collect();
                format!(
                    "Available templates: {}. {LIST_SUGGESTION}",
                    quoted.join(", ")
                )
            };
            Err(Error::not_found("Template", reference).with_hint(suggestion))
        }
        count => {
            let ids: Vec<String> = matches
                .iter()
                .map(|template| {
                    format!(
                        "{} ({}, {})",
                        template.id.inner(),
                        template.template_type,
                        template
                            .team
                            .as_ref()
                            .map_or("Workspace", |team| team.key.as_str())
                    )
                })
                .collect();
            Err(Error::new(format!(
                "Template name \"{reference}\" is ambiguous: it matches {count} templates"
            ))
            .with_hint(format!("Pass the template ID instead: {}", ids.join(", "))))
        }
    }
}

fn capitalize(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => first.to_uppercase().chain(chars).collect(),
        None => String::new(),
    }
}

/// Decode a template's stringified `templateData` into its top-level object.
fn parse_template_data(template: &Template) -> Result<Map<String, Value>, Error> {
    let subject = format!(
        "Template data for \"{}\" ({})",
        template.name,
        template.id.inner()
    );
    let decoded: Value = serde_json::from_str(&template.template_data.0)
        .map_err(|error| Error::new(format!("{subject} is not valid JSON")).with_source(error))?;
    match decoded {
        Value::Object(object) => Ok(object),
        _ => Err(Error::new(format!("{subject} is not a JSON object"))),
    }
}

/// The metadata header, the pre-fills and the footer, without a final newline.
/// `templateData` is parsed first, so its errors precede any output.
pub fn render_text<Tz: TimeZone>(
    template: &Template,
    now: DateTime<Utc>,
    zone: &Tz,
) -> Result<String, Error> {
    let data = parse_template_data(template)?;
    let mut lines: Vec<String> = vec![
        template.name.clone(),
        format!(
            "{} template · {}",
            capitalize(&template.template_type),
            match &template.team {
                Some(team) => format!("Team {} ({})", team.key, team.name),
                None => "Workspace".to_owned(),
            }
        ),
        format!("ID: {}", template.id.inner()),
    ];
    if let Some(description) = template
        .description
        .as_deref()
        .filter(|text| !text.is_empty())
    {
        lines.push(format!("Description: {description}"));
    }
    if template.has_form_fields {
        lines.push(FORM_NOTE.to_owned());
    }
    if let Some(parent) = &template.inherited_from {
        lines.push(format!(
            "Inherited from: {} ({})",
            parent.name,
            parent.id.inner()
        ));
    }
    if let Some(creator) = &template.creator {
        lines.push(format!("Created by: {}", creator.name));
    }
    if let Some(applied) = &template.last_applied_at {
        lines.push(format!(
            "Last applied: {}",
            format_relative_time(&applied.0, now, zone)
        ));
    }
    lines.push(format!(
        "Updated: {}",
        format_relative_time(&template.updated_at.0, now, zone)
    ));
    lines.push(String::new());
    lines.push("Pre-fills:".to_owned());
    if data.is_empty() {
        lines.push(format!("{INDENT}(nothing)"));
    }
    lines.extend(render_entries(data.iter(), INDENT)?);
    lines.push(String::new());
    lines.push(FOOTER.to_owned());
    Ok(lines.join("\n"))
}

fn is_rich_text_key(key: &str) -> bool {
    RICH_TEXT_KEYS.contains(&key)
}

/// Scalars and references first, then the bodies, so the long part reads last.
fn render_entries<'a>(
    entries: impl Iterator<Item = (&'a String, &'a Value)> + Clone,
    indent: &str,
) -> Result<Vec<String>, Error> {
    let mut lines = Vec::new();
    let plain = entries.clone().filter(|(key, _)| !is_rich_text_key(key));
    let rich = entries.filter(|(key, _)| is_rich_text_key(key));
    for (key, value) in plain.chain(rich) {
        lines.extend(render_pre_fill(key, value, indent)?);
    }
    Ok(lines)
}

/// Prefix each non-empty line; lines split on `\n` only, so `\r` stays.
fn indent_block(text: &str, indent: &str) -> String {
    text.split('\n')
        .map(|line| {
            if line.is_empty() {
                line.to_owned()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn priority_name(value: &Number) -> Option<&'static str> {
    match value.as_u64()? {
        0 => Some("none"),
        1 => Some("urgent"),
        2 => Some("high"),
        3 => Some("medium"),
        4 => Some("low"),
        _ => None,
    }
}

/// One pre-filled value at any depth.
fn render_pre_fill(key: &str, value: &Value, indent: &str) -> Result<Vec<String>, Error> {
    let nested = format!("{indent}{INDENT}");
    Ok(match value {
        Value::Object(_) if is_rich_text_key(key) => {
            // The Markdown itself, not a styled and reflowed rendering of it.
            let markdown = prosemirror::to_markdown(value)?;
            vec![format!("{indent}{key}:"), indent_block(&markdown, &nested)]
        }
        Value::Number(number) if key == "priority" => {
            let label = priority_name(number)
                .map(|name| format!(" ({name})"))
                .unwrap_or_default();
            vec![format!("{indent}{key}: {number}{label}")]
        }
        Value::String(text) if text.contains('\n') => vec![
            format!("{indent}{key}:"),
            indent_block(text.trim_end(), &nested),
        ],
        Value::String(text) => vec![format!("{indent}{key}: {text}")],
        Value::Number(number) => vec![format!("{indent}{key}: {number}")],
        Value::Bool(_) | Value::Null => vec![format!("{indent}{key}: {value}")],
        Value::Array(items) if items.is_empty() => vec![format!("{indent}{key}: (none)")],
        Value::Array(items) => {
            let strings: Option<Vec<&str>> = items
                .iter()
                .map(|item| match item {
                    Value::String(text) => Some(text.as_str()),
                    _ => None,
                })
                .collect();
            match strings {
                Some(strings) => vec![format!("{indent}{key}: {}", strings.join(", "))],
                None => {
                    let noun = if items.len() == 1 { "item" } else { "items" };
                    let mut lines = vec![format!("{indent}{key}: {} {noun}", items.len())];
                    for item in items {
                        lines.extend(render_item(item, &nested)?);
                    }
                    lines
                }
            }
        }
        Value::Object(object) => {
            let mut lines = vec![format!("{indent}{key}:")];
            lines.extend(render_entries(object.iter(), &nested)?);
            lines
        }
    })
}

/// The first non-empty string `title` or `name`.
fn item_label(item: &Map<String, Value>) -> Option<&str> {
    LABEL_KEYS.iter().find_map(|key| match item.get(*key) {
        Some(Value::String(text)) if !text.is_empty() => Some(text.as_str()),
        _ => None,
    })
}

/// An item of a list such as `subIssueData`: its label, then its other fields.
/// A `title` or `name` equal to the label is not repeated.
fn render_item(item: &Value, indent: &str) -> Result<Vec<String>, Error> {
    let Value::Object(object) = item else {
        return Ok(vec![format!("{indent}- {item}")]);
    };
    let label = item_label(object);
    let rest = object.iter().filter(|(key, value)| {
        !(label.is_some_and(|label| {
            LABEL_KEYS.contains(&key.as_str())
                && matches!(value, Value::String(text) if text == label)
        }))
    });
    let mut lines = vec![format!("{indent}- {}", label.unwrap_or("(untitled)"))];
    lines.extend(render_entries(rest, &format!("{indent}{INDENT}{INDENT}"))?);
    Ok(lines)
}
