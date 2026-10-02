//! One requested document page with exact connection and source table output.
use crate::commands::{
    display::{pad, truncate_js},
    table::{time_ago, underlined_header, utf16_len},
};
use crate::error::{AppError, AppErrorKind};
use crate::graphql::{
    envelope::GraphQlRequest,
    operations::{documents::*, teams::PageInfo},
    transport::GraphQlTransport,
};
use cynic::QueryBuilder;
use std::time::SystemTime;
pub const CONTEXT: &str = "Failed to list documents";
pub fn request(
    filter: Option<DocumentFilter>,
    first: i32,
) -> GraphQlRequest<ListDocumentsVariables> {
    GraphQlRequest::with_variables(ListDocuments::build(ListDocumentsVariables {
        filter,
        first: Some(first),
    }))
}
pub async fn fetch(
    transport: &GraphQlTransport,
    filter: Option<DocumentFilter>,
    first: i32,
) -> Result<DocumentConnection, AppError> {
    let data: ListDocuments = transport
        .execute(&request(filter, first))
        .await
        .map_err(AppError::from)?;
    Ok(data.documents.unwrap_or(DocumentConnection {
        nodes: Vec::new(),
        page_info: PageInfo {
            has_next_page: false,
            end_cursor: None,
        },
    }))
}
pub fn json(documents: &DocumentConnection) -> Result<Vec<u8>, AppError> {
    let mut out = serde_json::to_vec_pretty(documents).map_err(|error| {
        AppError::new(AppErrorKind::Invariant, "could not serialize documents").with_source(error)
    })?;
    out.push(b'\n');
    Ok(out)
}
pub fn attachment(doc: &ListedDocument) -> String {
    if let Some(project) = &doc.project
        && !project.name.is_empty()
    {
        return format!("Project: {}", project.name);
    }
    if let Some(issue) = &doc.issue
        && !issue.identifier.is_empty()
    {
        return format!("Issue: {}", issue.identifier);
    }
    if let Some(initiative) = &doc.initiative
        && !initiative.name.is_empty()
    {
        return format!("Initiative: {}", initiative.name);
    }
    if let Some(team) = &doc.team {
        return format!("Team: {} ({})", team.name, team.key);
    }
    if let Some(cycle) = &doc.cycle {
        let name = cycle
            .name
            .as_deref()
            .filter(|name| !name.is_empty())
            .map_or(String::new(), |name| format!(" — {name}"));
        return format!("Cycle: {} #{}{name}", cycle.team.key, cycle.number);
    }
    if let Some(release) = &doc.release {
        let version = release
            .version
            .as_deref()
            .filter(|version| !version.is_empty())
            .map_or(String::new(), |version| format!(" ({version})"));
        return format!("Release: {}{version}", release.name);
    }
    "-".to_owned()
}
pub fn text(
    documents: &DocumentConnection,
    columns: usize,
    color: bool,
    now: SystemTime,
) -> String {
    if documents.nodes.is_empty() {
        return "No documents found.\n".to_owned();
    }
    let labels: Vec<_> = documents.nodes.iter().map(attachment).collect();
    let ages: Vec<_> = documents
        .nodes
        .iter()
        .map(|doc| time_ago(&doc.updated_at.0, now))
        .collect();
    let slug_width = documents
        .nodes
        .iter()
        .map(|doc| utf16_len(&doc.slug_id))
        .max()
        .unwrap_or(0)
        .max(4);
    let attachment_width = labels
        .iter()
        .map(|label| utf16_len(label))
        .max()
        .unwrap_or(0)
        .max(10);
    let updated_width = ages
        .iter()
        .map(|age| utf16_len(age))
        .max()
        .unwrap_or(0)
        .max(7);
    let available = columns
        .saturating_sub(slug_width + attachment_width + updated_width + 4)
        .max(10);
    let title_width = documents
        .nodes
        .iter()
        .map(|doc| utf16_len(&doc.title))
        .max()
        .unwrap_or(0)
        .min(available);
    let mut out = underlined_header(
        &[
            pad("SLUG", slug_width),
            pad("TITLE", title_width),
            pad("ATTACHMENT", attachment_width),
            pad("UPDATED", updated_width),
        ],
        color,
    );
    for ((doc, label), age) in documents.nodes.iter().zip(labels).zip(ages) {
        let row = format!(
            "{} {} {} %c{}%c",
            pad(&doc.slug_id, slug_width),
            truncate_js(&doc.title, title_width),
            pad(&label, attachment_width),
            pad(&age, updated_width)
        );
        out.push_str(&console_row(&row, color));
        out.push('\n');
    }
    out
}

// Preserve Deno console placeholders in user fields: they consume the same
// two style arguments as the trailing updated-column %c markers.
fn console_row(format: &str, color: bool) -> String {
    let arguments = ["color: gray", ""];
    let mut next = 0;
    let mut styled = false;
    let mut output = String::new();
    let mut characters = format.chars().peekable();
    while let Some(character) = characters.next() {
        if character != '%' {
            output.push(character);
            continue;
        }
        match characters.peek().copied() {
            Some('%') => {
                characters.next();
                output.push('%');
            }
            Some(kind @ ('c' | 's' | 'd' | 'i' | 'f' | 'o' | 'O')) if next < arguments.len() => {
                characters.next();
                let argument = match next {
                    0 => "color: gray",
                    1 => "",
                    _ => unreachable!("Console style argument index"),
                };
                next += 1;
                match kind {
                    'c' if color => {
                        styled = true;
                        output.push_str(if argument.is_empty() {
                            "\x1b[39m"
                        } else {
                            "\x1b[38;2;128;128;128m"
                        });
                    }
                    'c' => {}
                    's' => output.push_str(argument),
                    'd' | 'i' | 'f' => output.push_str("NaN"),
                    'o' | 'O' => {
                        if color {
                            output.push_str("\x1b[32m");
                        }
                        output.push('"');
                        output.push_str(argument);
                        output.push('"');
                        if color {
                            output.push_str("\x1b[39m");
                        }
                    }
                    _ => unreachable!("Matched console substitution"),
                }
            }
            _ => output.push('%'),
        }
    }
    if styled {
        output.push_str("\x1b[0m");
    }
    for argument in arguments.iter().skip(next) {
        output.push(' ');
        output.push_str(argument);
    }
    output
}
