//! `template view`: one template by UUID or exact case-insensitive name, as
//! JSON or as its metadata and what it pre-fills.
//!
//! Text output prints rich-text bodies as indented Markdown, so a pre-fill can
//! be copied into `--description`.

use chrono::{DateTime, Local, TimeZone, Utc};
use cynic::QueryBuilder;
use serde_json::{Map, Number, Value};

use crate::cli::template::TemplateView;
use crate::commands::prosemirror;
use crate::commands::relative_time::format_relative_time;
use crate::commands::template::{json as template_json, list as template_list};
use crate::ctx::Ctx;
use crate::error::{Error, Result, ResultExt};
use crate::graphql::envelope::GraphQlRequest;
use crate::graphql::operations::templates::{
    GetTemplate, GetTemplateVariables, GetTemplates, Template,
};
use crate::graphql::transport::{GraphQlTransport, TransportFailure};
use crate::platform::collation;
use crate::refs::{is_linear_uuid, reject_linear_url};

const LIST_SUGGESTION: &str = "Run `linear template list` to see every template.";
const INDENT: &str = "  ";
/// Keys whose object value is a ProseMirror document holding the body.
const RICH_TEXT_KEYS: [&str; 2] = ["descriptionData", "contentData"];
const LABEL_KEYS: [&str; 2] = ["title", "name"];
const FORM_NOTE: &str = "Form template: yes (its form is filled in inside Linear; applying it from the CLI creates the entity with the form unanswered)";
const FOOTER: &str = "References are IDs. Map them with `linear team states`, `linear label list`, `linear user list`, or `linear project list`.";

/// A template named by UUID or by name.
enum Reference {
    Id(String),
    Name(String),
}

pub fn run(ctx: &Ctx, args: &TemplateView) -> Result<()> {
    view(ctx, args).context("Failed to view template")
}

fn view(ctx: &Ctx, args: &TemplateView) -> Result<()> {
    reject_linear_url(&args.template, "a template name or UUID")?;
    let reference = if is_linear_uuid(&args.template) {
        Reference::Id(args.template.clone())
    } else {
        Reference::Name(args.template.clone())
    };
    let client = ctx.client()?;
    let template = ctx.spin(!args.json, resolve(client, &reference))?;
    if args.json {
        return ctx.print(template_json::render_one(&template));
    }
    let mut text = render_text(&template, Utc::now(), &Local)?;
    text.push('\n');
    ctx.print(text)
}

/// The template by ID, or every template to match the name.
async fn resolve(client: &GraphQlTransport, reference: &Reference) -> Result<Template> {
    match reference {
        Reference::Id(id) => by_id(client, id).await,
        Reference::Name(name) => {
            let data: GetTemplates = client.execute(&template_list::request()).await?;
            select_by_name(name, data.templates)
        }
    }
}

/// The template with `id`. Linear reports a missing template only as a
/// GraphQL error, so when that request fails the template list decides
/// whether it is missing or the error stands.
pub async fn by_id(client: &GraphQlTransport, id: &str) -> Result<Template> {
    let request = GraphQlRequest::with_variables(GetTemplate::build(GetTemplateVariables {
        id: id.to_owned(),
    }));
    let failure = match client.execute::<GetTemplate, _>(&request).await {
        Ok(response) => return Ok(response.template),
        Err(failure @ TransportFailure::GraphQl { .. }) => failure,
        Err(failure) => return Err(Error::from(failure)),
    };
    let data: GetTemplates = client.execute(&template_list::request()).await?;
    if data
        .templates
        .iter()
        .any(|template| template.id.inner() == id)
    {
        Err(Error::from(failure))
    } else {
        Err(Error::not_found("Template", id).with_hint(LIST_SUGGESTION))
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
fn render_text<Tz: TimeZone>(
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
