use std::borrow::Cow;
use std::collections::{BTreeMap, HashSet};
use std::ops::Range;
use std::sync::Arc;

use minijinja::value::{Object, ObjectRepr, Serde, Value, merge_maps};
use minijinja::{Error, ErrorKind, Output, State, Template};
use pulldown_cmark::{Event, Parser, Tag};
use serde::Serialize;

use crate::platform::markdown_terminal::{self, InlineContent, InlineSpan, LabelStyle};
use crate::templates::markdown::push_markdown_link;

use super::{
    Fragment, Fragments, STYLED_KEY, Styled, TOKEN_START, charge_size, inline, token_fragment,
};

const VIEW_KEY: &str = "\0linear-issue-view";

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum ViewSectionKind {
    Hierarchy,
    Attachments,
    Documents,
    Comments,
}

#[derive(Debug)]
pub(crate) struct ViewSection {
    kind: ViewSectionKind,
    markdown: Arc<ViewMarkdown>,
}

impl ViewSection {
    pub(crate) fn value(markdown: String, kind: ViewSectionKind) -> Value {
        Self::from_markdown(
            ViewMarkdown {
                source: markdown,
                inlines: Vec::new(),
            },
            kind,
        )
    }

    pub(crate) fn from_markdown(markdown: ViewMarkdown, kind: ViewSectionKind) -> Value {
        if markdown.source.is_empty() {
            Value::from("")
        } else {
            Value::from_object(Self {
                kind,
                markdown: Arc::new(markdown),
            })
        }
    }
}

impl Object for ViewSection {
    fn repr(self: &Arc<Self>) -> ObjectRepr {
        ObjectRepr::Plain
    }

    fn render(self: &Arc<Self>, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.markdown.inlines.is_empty() {
            return f.write_str(&self.markdown.source);
        }
        let mut output = String::new();
        self.markdown.write_plain(&mut output);
        f.write_str(&output)
    }
}

#[derive(Debug)]
pub(crate) enum ViewPart {
    Markdown(ViewMarkdown),
    Section(ViewSectionKind, Arc<ViewMarkdown>),
}

impl ViewPart {
    pub(crate) fn write_plain(&self, out: &mut String) {
        match self {
            Self::Markdown(markdown) => markdown.write_plain(out),
            Self::Section(_, markdown) => markdown.write_plain(out),
        }
    }
}

#[derive(Debug, Default)]
pub(crate) struct ViewMarkdown {
    pub source: String,
    pub inlines: Vec<InlineContent>,
}

impl ViewMarkdown {
    pub(crate) fn push_inline(&mut self, spans: Vec<InlineSpan>) {
        self.inlines.push(InlineContent {
            source_range: markdown_terminal::push_inline_marker(&mut self.source),
            spans,
        });
    }

    fn expanded_bytes(&self) -> usize {
        self.inlines
            .iter()
            .fold(self.source.len(), |total, inline| {
                total.saturating_add(inline_bytes(&inline.spans))
            })
    }

    fn write_plain(&self, out: &mut String) {
        let mut end = 0;
        for inline in &self.inlines {
            let before = self
                .source
                .get(end..inline.source_range.start)
                .expect("view source ranges follow each other");
            out.push_str(&safe_pipe_text(before));
            for span in &inline.spans {
                let text = safe_pipe_text(&span.text);
                match &span.url {
                    Some(url) => push_markdown_link(out, &text, url),
                    None => out.push_str(&text),
                }
            }
            end = inline.source_range.end;
        }
        let tail = self
            .source
            .get(end..)
            .expect("view source ranges end on a character boundary");
        out.push_str(&safe_pipe_text(tail));
    }
}

// Preserve raw pipe line endings and indentation, without passing terminal
// commands or incomplete private template tokens through to another program.
fn safe_pipe_text(text: &str) -> Cow<'_, str> {
    let safe = |ch: char| !ch.is_control() || matches!(ch, '\n' | '\r' | '\t');
    if text.chars().all(safe) {
        Cow::Borrowed(text)
    } else {
        Cow::Owned(
            text.chars()
                .map(|ch| if safe(ch) { ch } else { '\u{fffd}' })
                .collect(),
        )
    }
}

pub(super) enum Content {
    Inline(Vec<InlineSpan>),
    Markdown(Arc<str>),
    Section(ViewSectionKind, Arc<ViewMarkdown>),
}

pub(super) fn is_view(state: &State) -> bool {
    state
        .lookup(VIEW_KEY)
        .is_some_and(|value| value.downcast_object_ref::<Styled>().is_some())
}

pub(in crate::templates) fn render_markdown(
    template: &Template<'_, '_>,
    context: &impl Serialize,
    styled: bool,
) -> Result<Vec<ViewPart>, Error> {
    let mut flags = BTreeMap::from([(VIEW_KEY, Value::from_object(Styled))]);
    if styled {
        flags.insert(STYLED_KEY, Value::from_object(Styled));
    }
    let rendered = template.render_captured(merge_maps([
        Value::from(Serde(context)),
        Value::from(flags),
    ]))?;
    let blocks = standalone_markdown(rendered.output(), rendered.state());
    let mut markdown = ViewMarkdown::default();
    let mut parts = Vec::new();
    let mut expanded = 0;
    let mut used_sections = HashSet::new();
    let mut used_fragments = HashSet::new();
    for chunk in chunks(rendered.output(), rendered.state()) {
        match chunk {
            Chunk::Text(text) => markdown.source.push_str(text),
            Chunk::Fragment {
                range,
                key,
                content: Fragment::View(Content::Markdown(source)),
            } => {
                if !used_fragments.insert(key) {
                    charge_size(&mut expanded, source.len())?;
                }
                let standalone = blocks.contains(&range.start);
                if standalone {
                    push_markdown_part(&mut parts, &mut markdown);
                }
                markdown.source.push_str(source);
                if standalone {
                    push_markdown_part(&mut parts, &mut markdown);
                }
            }
            Chunk::Fragment {
                key,
                content: Fragment::View(Content::Inline(spans)),
                ..
            } => {
                if !used_fragments.insert(key) {
                    charge_size(&mut expanded, inline_bytes(spans))?;
                }
                markdown.push_inline(spans.clone());
            }
            Chunk::Fragment {
                content: Fragment::View(Content::Section(kind, source)),
                ..
            } => {
                // The first copy of each fetched section fits at any size.
                if !used_sections.insert(*kind) {
                    charge_size(&mut expanded, source.expanded_bytes())?;
                }
                push_markdown_part(&mut parts, &mut markdown);
                parts.push(ViewPart::Section(*kind, Arc::clone(source)));
            }
            Chunk::Fragment {
                content: Fragment::Link(_),
                ..
            } => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    "display link used outside its template",
                ));
            }
        }
    }
    push_markdown_part(&mut parts, &mut markdown);
    Ok(parts)
}

// A complete Markdown block has its own parse. An open code fence or HTML
// block in issue data can then consume only the rest of that value. The
// template's own Markdown still decides whether an interpolation is in a
// paragraph, code fence, list, or other surrounding structure.
fn standalone_markdown(output: &str, state: &State) -> HashSet<usize> {
    let Some(fragments) = state.get_extension::<Fragments>() else {
        return HashSet::new();
    };
    let mut blocks = HashSet::new();
    let mut segment_start = 0;
    let mut has_markdown = false;
    for chunk in chunks(output, state) {
        if let Chunk::Fragment {
            range,
            content: Fragment::View(content),
            ..
        } = chunk
        {
            match content {
                Content::Markdown(_) => has_markdown = true,
                Content::Section(..) => {
                    if has_markdown {
                        collect_standalone(
                            output,
                            segment_start..range.start,
                            fragments,
                            &mut blocks,
                        );
                    }
                    segment_start = range.end;
                    has_markdown = false;
                }
                Content::Inline(_) => {}
            }
        }
    }
    if has_markdown {
        collect_standalone(output, segment_start..output.len(), fragments, &mut blocks);
    }
    blocks
}

fn collect_standalone(
    output: &str,
    segment: Range<usize>,
    fragments: &Fragments,
    blocks: &mut HashSet<usize>,
) {
    let source = output
        .get(segment.clone())
        .expect("native section tokens begin and end on character boundaries");
    let mut depth = 0;
    for (event, range) in Parser::new(source).into_offset_iter() {
        if matches!(event, Event::Start(Tag::Paragraph)) && depth == 0 {
            let paragraph = source.get(range.clone()).expect("valid paragraph range");
            let content = paragraph.trim();
            if let Some((_, Fragment::View(Content::Markdown(_)), "")) = content
                .strip_prefix(TOKEN_START)
                .and_then(|tail| token_fragment(tail, fragments))
            {
                let start = range.start + paragraph.len() - paragraph.trim_start().len();
                let (before, rest) = source.split_at(start);
                let (_, after) = rest.split_at(content.len());
                if starts_at_blank_line(before.rsplit('\n'))
                    && starts_at_blank_line(after.split('\n'))
                {
                    blocks.insert(segment.start + start);
                }
            }
        }
        match event {
            Event::Start(_) => depth += 1,
            Event::End(_) => depth -= 1,
            _ => {}
        }
    }
}

fn starts_at_blank_line<'a>(lines: impl Iterator<Item = &'a str>) -> bool {
    lines
        .take(2)
        .all(|line| line.trim_matches([' ', '\t', '\r']).is_empty())
}

fn push_markdown_part(parts: &mut Vec<ViewPart>, markdown: &mut ViewMarkdown) {
    if !markdown.source.is_empty() {
        parts.push(ViewPart::Markdown(std::mem::take(markdown)));
    }
}

pub(in crate::templates) fn format_view_value(
    out: &mut Output,
    state: &mut State,
    value: &Value,
) -> Result<(), Error> {
    if let Some(section) = value.downcast_object_ref::<ViewSection>() {
        let token = register(
            state,
            Content::Section(section.kind, Arc::clone(&section.markdown)),
        )?;
        return write!(out, "{token}").map_err(Error::from);
    }
    if value.is_safe() {
        // MiniJinja marks a macro or block capture safe with autoescaping on.
        // Its interpolations have already passed through this formatter.
        return write!(out, "{value}").map_err(Error::from);
    }
    let text = value
        .as_str()
        .map_or_else(|| Cow::Owned(value.to_string()), Cow::Borrowed);
    if text.is_empty() {
        return Ok(());
    }
    charge_work(state, text.len())?;
    // Slicing or concatenating a capture can make it an ordinary string.
    // Keep any intact registered tokens while treating its other bytes as
    // literal. Only the renderer's private registry can recognize a token.
    let ranges = chunks(&text, state)
        .filter_map(|chunk| match chunk {
            Chunk::Fragment { range, .. } => Some(range),
            Chunk::Text(_) => None,
        })
        .collect::<Vec<_>>();
    let mut end = 0;
    for range in ranges {
        let before = text
            .get(end..range.start)
            .expect("token ranges are ordered");
        write_literal(out, state, before)?;
        let token = text.get(range.clone()).expect("valid token range");
        out.write_str(token).map_err(Error::from)?;
        end = range.end;
    }
    let tail = text.get(end..).expect("token end is a character boundary");
    write_literal(out, state, tail)
}

fn write_literal(out: &mut Output, state: &mut State, text: &str) -> Result<(), Error> {
    if text.is_empty() {
        return Ok(());
    }
    let token = register(state, Content::Inline(vec![literal(text)]))?;
    write!(out, "{token}").map_err(Error::from)
}

pub(in crate::templates) fn markdown(state: &mut State, args: &[Value]) -> Result<Value, Error> {
    let [value] = args else {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "expected markdown(value)",
        ));
    };
    let text = view_string(value, "markdown value must be a string")?;
    if value.is_safe() || text.is_empty() {
        return Ok(value.clone());
    }
    charge_work(state, text.len())?;
    if chunks(text, state).any(|chunk| matches!(chunk, Chunk::Fragment { .. })) {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "markdown() received a transformed template capture. Apply markdown() to the source value before capturing it.",
        ));
    }
    register(state, Content::Markdown(Arc::from(text)))
}

pub(in crate::templates) fn label(state: &mut State, args: &[Value]) -> Result<Value, Error> {
    let [name, value] = args else {
        return Err(Error::new(
            ErrorKind::InvalidOperation,
            "expected label(name, content)",
        ));
    };
    let name = view_string(name, "label name must be a string")?;
    let text = view_string(value, "label content must be a string")?;
    let style = match name {
        "field_label" => LabelStyle::FieldLabel,
        "footer" => LabelStyle::Footer,
        "issue_heading" => LabelStyle::IssueHeading,
        _ => return Ok(value.clone()),
    };
    let mut spans = compose(state, text, "label content")?;
    for span in &mut spans {
        // These styles are idempotent. Keep the innermost occurrence of a
        // repeated label and bound the path by the number of known labels.
        if !span.labels.contains(&style) {
            span.labels.push(style);
        }
    }
    register_inline(state, spans)
}

pub(super) fn hyperlink(state: &mut State, url: Option<&str>, text: &str) -> Result<Value, Error> {
    let mut spans = compose(state, text, "hyperlink text")?;
    let url = url.map(Arc::from);
    for span in &mut spans {
        if let Cow::Owned(text) = inline(&span.text, false, false) {
            span.text = Arc::from(text);
        }
        span.url = url.clone();
    }
    charge_work(state, inline_bytes(&spans))?;
    register_inline(state, spans)
}

fn view_string<'a>(value: &'a Value, error: &'static str) -> Result<&'a str, Error> {
    value
        .as_str()
        .ok_or_else(|| Error::new(ErrorKind::InvalidOperation, error))
}

fn register(state: &mut State, content: Content) -> Result<Value, Error> {
    state
        .get_or_insert_extension_with(Fragments::default)
        .register(Fragment::View(content))
}

pub(super) fn register_inline(state: &mut State, spans: Vec<InlineSpan>) -> Result<Value, Error> {
    if spans.is_empty() {
        Ok(Value::from(""))
    } else {
        register(state, Content::Inline(spans))
    }
}

fn literal(text: &str) -> InlineSpan {
    InlineSpan {
        text: Arc::from(text),
        labels: Vec::new(),
        url: None,
    }
}

fn inline_bytes(spans: &[InlineSpan]) -> usize {
    spans.iter().fold(0, |total, span| {
        total
            .saturating_add(span.text.len())
            .saturating_add(span.url.as_ref().map_or(0, |url| url.len() + 16))
    })
}

fn charge_work(state: &State, bytes: usize) -> Result<(), Error> {
    state.consume_fuel(u64::try_from(bytes / 16 + 1).expect("byte count fits in u64"))
}

fn compose(state: &State, text: &str, role: &str) -> Result<Vec<InlineSpan>, Error> {
    let mut out = Vec::new();
    for chunk in chunks(text, state) {
        match chunk {
            Chunk::Text(text) => {
                charge_work(state, text.len())?;
                out.push(literal(text));
            }
            Chunk::Fragment {
                content: Fragment::View(Content::Inline(spans)),
                ..
            } => {
                let bytes = inline_bytes(spans);
                charge_work(state, bytes)?;
                out.extend_from_slice(spans);
            }
            Chunk::Fragment { .. } => {
                return Err(Error::new(
                    ErrorKind::InvalidOperation,
                    format!("Markdown and view sections cannot be used as {role}"),
                ));
            }
        }
    }
    Ok(out)
}

enum Chunk<'a> {
    Text(&'a str),
    Fragment {
        range: Range<usize>,
        key: &'a str,
        content: &'a Fragment,
    },
}

fn chunks<'a>(text: &'a str, state: &'a State) -> impl Iterator<Item = Chunk<'a>> {
    let fragments = state.get_extension::<Fragments>();
    let mut rest = text;
    std::iter::from_fn(move || {
        if rest.is_empty() {
            return None;
        }
        match rest.split_once(TOKEN_START) {
            Some((before, _)) if !before.is_empty() => {
                rest = rest.get(before.len()..).expect("chunk ends at a delimiter");
                Some(Chunk::Text(before))
            }
            Some((_, tail)) => {
                if let Some((key, fragment, after)) =
                    fragments.and_then(|fragments| token_fragment(tail, fragments))
                {
                    let range = text.len() - rest.len()..text.len() - after.len();
                    rest = after;
                    Some(Chunk::Fragment {
                        range,
                        key,
                        content: fragment,
                    })
                } else {
                    rest = tail;
                    Some(Chunk::Text("\u{fffd}"))
                }
            }
            None => Some(Chunk::Text(std::mem::take(&mut rest))),
        }
    })
}
