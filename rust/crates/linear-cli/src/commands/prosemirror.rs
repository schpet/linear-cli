//! Best-effort Markdown for the ProseMirror documents Linear stores in a
//! template's `descriptionData` / `contentData`. Its only frozen caller is
//! `template view`.
//!
//! The whole tree is read into typed nodes first, so a malformed node reports
//! its `doc.content[...]` path before anything renders. Nodes and marks this
//! converter does not know stay visible instead of being dropped. String
//! handling follows JavaScript: lines split on `\n` only, a line start follows
//! any ECMAScript line terminator, and `\s` / `trimEnd` use its whitespace set.

use crate::commands::template_data::{JsObject, JsValue, js_number};
use crate::error::{AppError, AppErrorKind};
use crate::text::js_space;

#[derive(Clone, Debug, PartialEq)]
enum NodeKind {
    Doc,
    Paragraph,
    Heading { level: f64 },
    BulletList,
    OrderedList { order: f64 },
    TodoList,
    ListItem,
    TodoItem,
    CodeBlock { language: String },
    Blockquote,
    HorizontalRule,
    Text,
    HardBreak,
    Image { alt: String, src: String },
    Other(String),
}

impl NodeKind {
    fn read(name: &str, attrs: Option<&JsObject>) -> Self {
        match name {
            "doc" => Self::Doc,
            "paragraph" => Self::Paragraph,
            "heading" => Self::Heading {
                level: attr_number(attrs, "level", 1.0),
            },
            "bullet_list" => Self::BulletList,
            "ordered_list" => Self::OrderedList {
                order: attr_number(attrs, "order", 1.0),
            },
            "todo_list" => Self::TodoList,
            "list_item" => Self::ListItem,
            "todo_item" => Self::TodoItem,
            "code_block" => Self::CodeBlock {
                language: attr_string(attrs, "language"),
            },
            "blockquote" => Self::Blockquote,
            "horizontal_rule" => Self::HorizontalRule,
            "text" => Self::Text,
            "hard_break" => Self::HardBreak,
            "image" => Self::Image {
                alt: attr_string(attrs, "alt"),
                src: attr_string(attrs, "src"),
            },
            other => Self::Other(other.to_owned()),
        }
    }

    fn name(&self) -> &str {
        match self {
            Self::Doc => "doc",
            Self::Paragraph => "paragraph",
            Self::Heading { .. } => "heading",
            Self::BulletList => "bullet_list",
            Self::OrderedList { .. } => "ordered_list",
            Self::TodoList => "todo_list",
            Self::ListItem => "list_item",
            Self::TodoItem => "todo_item",
            Self::CodeBlock { .. } => "code_block",
            Self::Blockquote => "blockquote",
            Self::HorizontalRule => "horizontal_rule",
            Self::Text => "text",
            Self::HardBreak => "hard_break",
            Self::Image { .. } => "image",
            Self::Other(name) => name,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct Node {
    kind: NodeKind,
    content: Vec<Node>,
    text: Option<String>,
    marks: Vec<Mark>,
    /// `attrs.label`: the display text of mentions and similar inline atoms,
    /// read for every node type.
    label: String,
    /// `attrs.done === true || attrs.checked === true`, read by a todo list
    /// for each of its children whatever their type.
    done: bool,
}

#[derive(Clone, Debug, PartialEq)]
enum Mark {
    Bold,
    Italic,
    Code,
    Strike,
    Link {
        href: String,
    },
    /// Underline, text color and other decorations have no Markdown form.
    Other,
}

fn validation(message: String) -> AppError {
    AppError::new(AppErrorKind::Validation, message)
}

fn as_record(value: &JsValue) -> Option<&JsObject> {
    match value {
        JsValue::Object(object) => Some(object),
        _ => None,
    }
}

fn attrs_of(object: &JsObject) -> Option<&JsObject> {
    object.get("attrs").and_then(as_record)
}

fn attr_string(attrs: Option<&JsObject>, key: &str) -> String {
    match attrs.and_then(|attrs| attrs.get(key)) {
        Some(JsValue::String(value)) => value.clone(),
        _ => String::new(),
    }
}

fn attr_number(attrs: Option<&JsObject>, key: &str, fallback: f64) -> f64 {
    match attrs.and_then(|attrs| attrs.get(key)) {
        Some(JsValue::Number(value)) if value.is_finite() => *value,
        _ => fallback,
    }
}

fn attr_true(attrs: Option<&JsObject>, key: &str) -> bool {
    matches!(
        attrs.and_then(|attrs| attrs.get(key)),
        Some(JsValue::Bool(true))
    )
}

/// A present, non-null property that must be an array.
fn optional_array<'a>(
    object: &'a JsObject,
    key: &str,
    path: &str,
) -> Result<&'a [JsValue], AppError> {
    match object.get(key) {
        None | Some(JsValue::Null) => Ok(&[]),
        Some(JsValue::Array(items)) => Ok(items),
        Some(_) => Err(validation(format!(
            "Invalid ProseMirror node at {path}: \"{key}\" must be an array"
        ))),
    }
}

fn read_mark(value: &JsValue, path: &str) -> Result<Mark, AppError> {
    let Some((object, name)) = as_record(value).and_then(|object| match object.get("type") {
        Some(JsValue::String(name)) => Some((object, name)),
        _ => None,
    }) else {
        return Err(validation(format!(
            "Invalid ProseMirror mark at {path}: expected an object with a string \"type\""
        )));
    };
    Ok(match name.as_str() {
        "bold" | "strong" => Mark::Bold,
        "italic" | "em" => Mark::Italic,
        "code" => Mark::Code,
        "strike" | "strikethrough" => Mark::Strike,
        "link" => Mark::Link {
            href: attr_string(attrs_of(object), "href"),
        },
        _ => Mark::Other,
    })
}

/// Validate one node, then its children in order, then its marks.
fn read_node(value: &JsValue, path: &str) -> Result<Node, AppError> {
    let Some((object, name)) = as_record(value).and_then(|object| match object.get("type") {
        Some(JsValue::String(name)) => Some((object, name)),
        _ => None,
    }) else {
        return Err(validation(format!(
            "Invalid ProseMirror node at {path}: expected an object with a string \"type\""
        )));
    };
    let content = optional_array(object, "content", path)?;
    let marks = optional_array(object, "marks", path)?;
    let text = match object.get("text") {
        None | Some(JsValue::Null) => None,
        Some(JsValue::String(text)) => Some(text.clone()),
        Some(_) => {
            return Err(validation(format!(
                "Invalid ProseMirror node at {path}: \"text\" must be a string"
            )));
        }
    };
    let attrs = attrs_of(object);
    let content = content
        .iter()
        .enumerate()
        .map(|(index, child)| read_node(child, &format!("{path}.content[{index}]")))
        .collect::<Result<Vec<_>, _>>()?;
    let marks = marks
        .iter()
        .enumerate()
        .map(|(index, mark)| read_mark(mark, &format!("{path}.marks[{index}]")))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Node {
        kind: NodeKind::read(name, attrs),
        content,
        text,
        marks,
        label: attr_string(attrs, "label"),
        done: attr_true(attrs, "done") || attr_true(attrs, "checked"),
    })
}

/// Convert a ProseMirror document to Markdown, or fail when the value is not
/// a ProseMirror document at all.
pub fn to_markdown(doc: &JsValue) -> Result<String, AppError> {
    let root = read_node(doc, "doc")?;
    if root.kind != NodeKind::Doc {
        return Err(validation(format!(
            "Expected a ProseMirror document, got a \"{}\" node",
            root.kind.name()
        )));
    }
    Ok(render_blocks(&root.content)
        .trim_end_matches(js_space)
        .to_owned())
}

fn is_line_terminator(ch: char) -> bool {
    matches!(ch, '\n' | '\r' | '\u{2028}' | '\u{2029}')
}

/// Escape characters Markdown would otherwise interpret, then a leading
/// `-`, `+` or `1.` (followed by whitespace) on any line.
fn escape_markdown(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for ch in text.chars() {
        if matches!(
            ch,
            '\\' | '*' | '_' | '`' | '[' | ']' | '~' | '<' | '>' | '#'
        ) {
            escaped.push('\\');
        }
        escaped.push(ch);
    }
    let mut output = String::with_capacity(escaped.len());
    let mut rest = escaped.as_str();
    loop {
        let (line, terminator, next) =
            match rest.char_indices().find(|(_, ch)| is_line_terminator(*ch)) {
                Some((index, ch)) => (&rest[..index], Some(ch), &rest[index + ch.len_utf8()..]),
                None => (rest, None, ""),
            };
        escape_list_marker(line, terminator, &mut output);
        match terminator {
            Some(ch) => {
                output.push(ch);
                rest = next;
            }
            None => break,
        }
    }
    output
}

/// `^([ \t]*)(?:([-+])|(\d+)\.)(?=\s)` on one line; the lookahead may see the
/// line terminator that follows it.
fn escape_list_marker(line: &str, terminator: Option<char>, output: &mut String) {
    let space_end = line
        .find(|ch: char| ch != ' ' && ch != '\t')
        .unwrap_or(line.len());
    let after_space = &line[space_end..];
    let followed_by_space = |index: usize| {
        after_space[index..]
            .chars()
            .next()
            .or(terminator)
            .is_some_and(js_space)
    };
    let digits = after_space
        .find(|ch: char| !ch.is_ascii_digit())
        .unwrap_or(after_space.len());
    if after_space.starts_with(['-', '+']) && followed_by_space(1) {
        output.push_str(&line[..space_end]);
        output.push('\\');
        output.push_str(after_space);
    } else if digits > 0 && after_space[digits..].starts_with('.') && followed_by_space(digits + 1)
    {
        output.push_str(&line[..space_end + digits]);
        output.push('\\');
        output.push_str(&after_space[digits..]);
    } else {
        output.push_str(line);
    }
}

fn longest_backtick_run(text: &str) -> usize {
    text.split(|ch: char| ch != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
}

/// A code span whose fence is longer than any backtick run inside it.
fn code_span(text: &str) -> String {
    let longest = longest_backtick_run(text);
    let fence = "`".repeat(longest + 1);
    if longest == 0 {
        format!("{fence}{text}{fence}")
    } else {
        format!("{fence} {text} {fence}")
    }
}

fn apply_marks(text: &str, marks: &[Mark]) -> String {
    let mut result = if marks.contains(&Mark::Code) {
        code_span(text)
    } else {
        escape_markdown(text)
    };
    for mark in marks {
        result = match mark {
            Mark::Bold => format!("**{result}**"),
            Mark::Italic => format!("_{result}_"),
            // Already fenced above, before the other marks wrap it.
            Mark::Code | Mark::Other => result,
            Mark::Strike => format!("~~{result}~~"),
            Mark::Link { href } if !href.is_empty() => format!("[{result}]({href})"),
            Mark::Link { .. } => result,
        };
    }
    result
}

fn render_inline(nodes: &[Node]) -> String {
    nodes.iter().map(render_inline_node).collect()
}

fn render_inline_node(node: &Node) -> String {
    match &node.kind {
        NodeKind::Text => apply_marks(node.text.as_deref().unwrap_or(""), &node.marks),
        NodeKind::HardBreak => "\n".to_owned(),
        NodeKind::Image { alt, src } => format!("![{alt}]({src})"),
        kind => {
            if !node.label.is_empty() {
                apply_marks(&node.label, &node.marks)
            } else if let Some(text) = &node.text {
                apply_marks(text, &node.marks)
            } else if !node.content.is_empty() {
                render_inline(&node.content)
            } else {
                format!("[{}]", kind.name())
            }
        }
    }
}

fn indent_continuation(text: &str, indent: &str) -> String {
    text.split('\n')
        .enumerate()
        .map(|(index, line)| {
            if index == 0 || line.is_empty() {
                line.to_owned()
            } else {
                format!("{indent}{line}")
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_list_items(items: &[Node], marker: impl Fn(&Node, f64) -> String) -> String {
    let mut index = 0.0;
    items
        .iter()
        .map(|item| {
            let prefix = marker(item, index);
            index += 1.0;
            // Every marker is ASCII, so its byte length is its JS length.
            let indent = " ".repeat(prefix.len());
            let body = match item.kind {
                NodeKind::ListItem | NodeKind::TodoItem => render_blocks(&item.content),
                _ => render_block(item),
            };
            format!("{prefix}{}", indent_continuation(&body, &indent))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// `"#".repeat(level)` after clamping to 1..=6; `repeat` truncates fractions.
fn heading_marker(level: f64) -> String {
    let level = level.clamp(1.0, 6.0);
    let count = if level < 2.0 {
        1
    } else if level < 3.0 {
        2
    } else if level < 4.0 {
        3
    } else if level < 5.0 {
        4
    } else if level < 6.0 {
        5
    } else {
        6
    };
    "#".repeat(count)
}

fn render_block(node: &Node) -> String {
    match &node.kind {
        NodeKind::Paragraph => render_inline(&node.content),
        NodeKind::Heading { level } => {
            format!(
                "{} {}",
                heading_marker(*level),
                render_inline(&node.content)
            )
        }
        NodeKind::BulletList => render_list_items(&node.content, |_, _| "- ".to_owned()),
        // `start + index` in JavaScript number arithmetic.
        NodeKind::OrderedList { order } => render_list_items(&node.content, |_, index| {
            format!("{}. ", js_number(order + index))
        }),
        NodeKind::TodoList => render_list_items(&node.content, |item, _| {
            (if item.done { "- [x] " } else { "- [ ] " }).to_owned()
        }),
        NodeKind::CodeBlock { language } => {
            let code: String = node
                .content
                .iter()
                .map(|child| child.text.as_deref().unwrap_or(""))
                .collect();
            // The fence must be longer than any backtick run inside the code.
            let fence = "`".repeat((longest_backtick_run(&code) + 1).max(3));
            format!("{fence}{language}\n{code}\n{fence}")
        }
        NodeKind::Blockquote => render_blocks(&node.content)
            .split('\n')
            .map(|line| format!("> {line}"))
            .collect::<Vec<_>>()
            .join("\n"),
        NodeKind::HorizontalRule => "---".to_owned(),
        NodeKind::Text | NodeKind::HardBreak | NodeKind::Image { .. } => render_inline_node(node),
        kind => {
            if !node.content.is_empty() {
                render_blocks(&node.content)
            } else if let Some(text) = &node.text {
                apply_marks(text, &node.marks)
            } else {
                format!("[unsupported {} node]", kind.name())
            }
        }
    }
}

fn render_blocks(nodes: &[Node]) -> String {
    nodes
        .iter()
        .map(render_block)
        .collect::<Vec<_>>()
        .join("\n\n")
}
