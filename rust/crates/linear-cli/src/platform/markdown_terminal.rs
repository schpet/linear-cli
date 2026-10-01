//! Markdown rendering for terminal stdout.
//!
//! Only TTY output reaches this module; piped output stays raw Markdown. The
//! vocabulary follows the Deno CLI's charmd 0.1.2 renderer: headings keep their
//! `#` markers, emphasis becomes SGR styling, links keep `[text](url)`, and lines
//! are not reflowed because the terminal wraps them. Width only sizes thematic
//! breaks. Deliberate differences are recorded in `rust/compatibility.md`.

use std::borrow::Cow;
use std::collections::VecDeque;
use std::iter::Peekable;
use std::num::NonZeroU16;
use std::ops::Range;

use pulldown_cmark::{
    Alignment, CodeBlockKind, DefaultBrokenLinkCallback, Event, HeadingLevel, LinkType, OffsetIter,
    Options, Parser, Tag, TagEnd,
};
use unicode_width::UnicodeWidthStr;

use crate::config::NoColor;
use crate::error::{AppError, AppErrorKind};
use crate::text::js_space;

/// Renderer width when a terminal reports no usable size.
pub const FALLBACK_COLUMNS: NonZeroU16 = NonZeroU16::MIN.saturating_add(79);

/// Deeper documents are rejected instead of risking unbounded recursion.
pub const MAX_NESTING: usize = 64;

const LIST_ICONS: [&str; 4] = ["-", "◦", "▪", "▸"];

/// Where a local image path's `{host}` comes from.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostSource {
    System,
    Fixed(String),
}

/// OSC-8 links on image destinations, as the Deno CLI's charmd extension adds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ImageHyperlinks {
    template: String,
    host: HostSource,
}

impl ImageHyperlinks {
    /// `default` expands to `file://{host}{path}`.
    pub fn new(format: &str, host: HostSource) -> Self {
        let template = if format == "default" {
            "file://{host}{path}"
        } else {
            format
        };
        Self {
            template: template.to_owned(),
            host,
        }
    }

    pub fn template(&self) -> &str {
        &self.template
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderOptions {
    pub columns: NonZeroU16,
    /// SGR styling; only a nonempty `NO_COLOR` turns it off.
    pub styled: bool,
    pub image_hyperlinks: Option<ImageHyperlinks>,
}

impl RenderOptions {
    /// Source gating: styling follows `NO_COLOR` being nonempty, while image
    /// hyperlinks need a nonempty format, TTY stdout and an absent `NO_COLOR`.
    pub fn for_terminal(
        columns: NonZeroU16,
        no_color: NoColor,
        stdout_tty: bool,
        hyperlink_format: Option<&str>,
        host: HostSource,
    ) -> Self {
        let image_hyperlinks = match hyperlink_format {
            Some(format) if !format.is_empty() && stdout_tty && no_color == NoColor::Absent => {
                Some(ImageHyperlinks::new(format, host))
            }
            Some(_) | None => None,
        };
        Self {
            columns,
            styled: no_color != NoColor::Nonempty,
            image_hyperlinks,
        }
    }
}

/// Render Markdown for a terminal. The result ends with the renderer's own
/// line feed for nonempty documents; callers printing it directly add one more.
pub fn render(markdown: &str, options: &RenderOptions) -> Result<String, AppError> {
    let blocks = parse(markdown)?;
    let mut renderer = Renderer {
        options,
        paint: Painter {
            enabled: options.styled,
        },
        host: None,
    };
    let mut rendered = Vec::with_capacity(blocks.len());
    for block in &blocks {
        rendered.push(renderer.block(block, None)?);
    }
    Ok(rendered.join("\n"))
}

fn invariant(message: impl Into<String>) -> AppError {
    AppError::new(AppErrorKind::Invariant, message)
}

fn too_deep() -> AppError {
    AppError::new(
        AppErrorKind::Validation,
        format!("Markdown nesting deeper than {MAX_NESTING} levels cannot be rendered"),
    )
}

#[derive(Debug)]
enum Inline {
    Text(String),
    Code(String),
    Html(String),
    SoftBreak,
    HardBreak,
    Strong(Vec<Inline>),
    Emphasis(Vec<Inline>),
    Strikethrough(Vec<Inline>),
    Link { url: String, children: Vec<Inline> },
    LinkReference(Vec<Inline>),
    Image { url: String, alt: String },
    ImageReference { alt: String, label: String },
}

#[derive(Debug)]
enum Block {
    Paragraph(Vec<Inline>),
    Heading(usize, Vec<Inline>),
    Quote(Vec<Block>),
    List {
        start: Option<u64>,
        items: Vec<Item>,
    },
    Code {
        lang: Option<String>,
        text: String,
    },
    Html(String),
    Table {
        alignments: Vec<Alignment>,
        rows: Vec<Vec<Vec<Inline>>>,
    },
    Rule,
    Definition(Definition),
}

#[derive(Debug)]
struct Item {
    blocks: Vec<Block>,
    spread: bool,
}

#[derive(Debug)]
struct Definition {
    label: String,
    url: String,
    title: Option<String>,
    span: Range<usize>,
}

struct Spanned {
    block: Block,
    span: Range<usize>,
}

/// Replace terminal control characters from document text, keeping tab and LF.
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|character| {
            if character.is_control() && character != '\n' && character != '\t' {
                '\u{fffd}'
            } else {
                character
            }
        })
        .collect()
}

fn parse(markdown: &str) -> Result<Vec<Block>, AppError> {
    let parser = Parser::new_ext(
        markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH,
    );
    let mut definitions = Vec::new();
    for (_, definition) in parser.reference_definitions().iter() {
        let raw = markdown
            .get(definition.span.clone())
            .ok_or_else(|| invariant("a Markdown link definition span is outside the document"))?;
        definitions.push(Definition {
            label: sanitize(definition_label(raw)?),
            url: sanitize(&definition.dest),
            title: definition.title.as_deref().map(sanitize),
            span: definition.span.clone(),
        });
    }
    definitions.sort_by_key(|definition| definition.span.start);
    let mut builder = Builder {
        events: parser.into_offset_iter().peekable(),
        definitions: definitions.into(),
        source: markdown,
    };
    Ok(builder
        .blocks(None, 0)?
        .into_iter()
        .map(|spanned| spanned.block)
        .collect())
}

/// The label as written between the first unescaped brackets.
fn definition_label(raw: &str) -> Result<&str, AppError> {
    let open = raw
        .find('[')
        .ok_or_else(|| invariant("a Markdown link definition has no label"))?;
    let rest = raw
        .get(open + 1..)
        .ok_or_else(|| invariant("a Markdown link definition has no label"))?;
    let mut escaped = false;
    for (index, character) in rest.char_indices() {
        match character {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            ']' => {
                return rest
                    .get(..index)
                    .ok_or_else(|| invariant("a Markdown link definition label is malformed"));
            }
            _ => {}
        }
    }
    Err(invariant(
        "a Markdown link definition label is unterminated",
    ))
}

fn heading_depth(level: HeadingLevel) -> usize {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn is_inline(event: &Event<'_>) -> bool {
    match event {
        Event::Text(_)
        | Event::Code(_)
        | Event::InlineHtml(_)
        | Event::SoftBreak
        | Event::HardBreak => true,
        Event::Start(tag) => matches!(
            tag,
            Tag::Emphasis | Tag::Strong | Tag::Strikethrough | Tag::Link { .. } | Tag::Image { .. }
        ),
        _ => false,
    }
}

fn unexpected(event: &Event<'_>) -> AppError {
    invariant(format!("unexpected Markdown event {event:?}"))
}

struct Builder<'a> {
    events: Peekable<OffsetIter<'a, DefaultBrokenLinkCallback>>,
    definitions: VecDeque<Definition>,
    source: &'a str,
}

impl<'a> Builder<'a> {
    fn next_event(&mut self) -> Result<(Event<'a>, Range<usize>), AppError> {
        self.events
            .next()
            .ok_or_else(|| invariant("Markdown events ended inside an open element"))
    }

    fn take_definitions(&mut self, before: usize, blocks: &mut Vec<Spanned>) {
        while self
            .definitions
            .front()
            .is_some_and(|definition| definition.span.start < before)
        {
            let Some(definition) = self.definitions.pop_front() else {
                break;
            };
            blocks.push(Spanned {
                span: definition.span.clone(),
                block: Block::Definition(definition),
            });
        }
    }

    fn blocks(&mut self, end: Option<TagEnd>, depth: usize) -> Result<Vec<Spanned>, AppError> {
        if depth > MAX_NESTING {
            return Err(too_deep());
        }
        let mut blocks = Vec::new();
        loop {
            let Some((event, range)) = self.events.peek() else {
                if end.is_some() {
                    return Err(invariant("Markdown events ended inside an open block"));
                }
                self.take_definitions(usize::MAX, &mut blocks);
                return Ok(blocks);
            };
            if is_inline(event) {
                // Tight list items carry their paragraph text without paragraph tags.
                let start = range.start;
                self.take_definitions(start, &mut blocks);
                let (children, span) = self.implicit_paragraph(start, depth)?;
                blocks.push(Spanned {
                    block: Block::Paragraph(children),
                    span,
                });
                continue;
            }
            let (event, range) = self.next_event()?;
            match event {
                Event::End(tag) if Some(tag) == end => {
                    self.take_definitions(range.end, &mut blocks);
                    return Ok(blocks);
                }
                Event::Start(tag) => {
                    self.take_definitions(range.start, &mut blocks);
                    let block = self.block(tag, depth)?;
                    blocks.push(Spanned { block, span: range });
                }
                Event::Rule => {
                    self.take_definitions(range.start, &mut blocks);
                    blocks.push(Spanned {
                        block: Block::Rule,
                        span: range,
                    });
                }
                other => return Err(unexpected(&other)),
            }
        }
    }

    fn implicit_paragraph(
        &mut self,
        start: usize,
        depth: usize,
    ) -> Result<(Vec<Inline>, Range<usize>), AppError> {
        let mut children = Vec::new();
        let mut end = start;
        while let Some((event, _)) = self.events.peek() {
            if !is_inline(event) {
                break;
            }
            let (event, range) = self.next_event()?;
            end = range.end;
            children.push(self.inline(event, depth)?);
        }
        Ok((children, start..end))
    }

    fn block(&mut self, tag: Tag<'a>, depth: usize) -> Result<Block, AppError> {
        let end = tag.to_end();
        Ok(match tag {
            Tag::Paragraph => Block::Paragraph(self.inlines(end, depth)?),
            Tag::Heading { level, .. } => {
                Block::Heading(heading_depth(level), self.inlines(end, depth)?)
            }
            Tag::BlockQuote(_) => Block::Quote(
                self.blocks(Some(end), depth + 1)?
                    .into_iter()
                    .map(|spanned| spanned.block)
                    .collect(),
            ),
            Tag::CodeBlock(kind) => {
                let lang = match kind {
                    CodeBlockKind::Indented => None,
                    CodeBlockKind::Fenced(info) => info
                        .split([' ', '\t'])
                        .next()
                        .filter(|lang| !lang.is_empty())
                        .map(sanitize),
                };
                let text = self.literal(end)?.replace('\r', "");
                let text = text.strip_suffix('\n').unwrap_or(&text);
                Block::Code {
                    lang,
                    text: sanitize(text),
                }
            }
            Tag::HtmlBlock => {
                let html = self.literal(end)?;
                Block::Html(sanitize(html.strip_suffix('\n').unwrap_or(&html)))
            }
            Tag::List(start) => {
                let mut items = Vec::new();
                loop {
                    let (event, _) = self.next_event()?;
                    match event {
                        Event::Start(Tag::Item) => {
                            let children = self.blocks(Some(TagEnd::Item), depth + 1)?;
                            let spread = self.spread(&children);
                            items.push(Item {
                                blocks: children.into_iter().map(|child| child.block).collect(),
                                spread,
                            });
                        }
                        Event::End(tag) if tag == end => break,
                        other => return Err(unexpected(&other)),
                    }
                }
                Block::List { start, items }
            }
            Tag::Table(alignments) => {
                let mut rows = Vec::new();
                loop {
                    let (event, _) = self.next_event()?;
                    match event {
                        Event::Start(Tag::TableHead) => {
                            rows.push(self.table_row(TagEnd::TableHead, depth)?);
                        }
                        Event::Start(Tag::TableRow) => {
                            rows.push(self.table_row(TagEnd::TableRow, depth)?);
                        }
                        Event::End(tag) if tag == end => break,
                        other => return Err(unexpected(&other)),
                    }
                }
                Block::Table { alignments, rows }
            }
            other => return Err(unexpected(&Event::Start(other))),
        })
    }

    /// A list item is spread when a blank line separates two of its children.
    fn spread(&self, children: &[Spanned]) -> bool {
        children.windows(2).any(|pair| {
            let [before, after] = pair else {
                return false;
            };
            let Some(content) = self.source.get(before.span.clone()) else {
                return false;
            };
            let tail =
                before.span.start + content.trim_end_matches([' ', '\t', '\r', '\n', '>']).len();
            let Some(gap) = self.source.get(tail..after.span.start) else {
                return false;
            };
            let lines: Vec<&str> = gap.split('\n').collect();
            lines.len() > 2
                && lines.get(1..lines.len() - 1).is_some_and(|middle| {
                    middle.iter().any(|line| {
                        line.chars()
                            .all(|character| matches!(character, ' ' | '\t' | '\r' | '>'))
                    })
                })
        })
    }

    fn table_row(&mut self, end: TagEnd, depth: usize) -> Result<Vec<Vec<Inline>>, AppError> {
        let mut cells = Vec::new();
        loop {
            let (event, _) = self.next_event()?;
            match event {
                Event::Start(Tag::TableCell) => cells.push(self.inlines(TagEnd::TableCell, depth)?),
                Event::End(tag) if tag == end => return Ok(cells),
                other => return Err(unexpected(&other)),
            }
        }
    }

    fn literal(&mut self, end: TagEnd) -> Result<String, AppError> {
        let mut text = String::new();
        loop {
            let (event, _) = self.next_event()?;
            match event {
                Event::Text(part) | Event::Html(part) => text.push_str(&part),
                Event::End(tag) if tag == end => return Ok(text),
                other => return Err(unexpected(&other)),
            }
        }
    }

    fn inlines(&mut self, end: TagEnd, depth: usize) -> Result<Vec<Inline>, AppError> {
        if depth > MAX_NESTING {
            return Err(too_deep());
        }
        let mut children = Vec::new();
        loop {
            let (event, _) = self.next_event()?;
            match event {
                Event::End(tag) if tag == end => return Ok(children),
                event => children.push(self.inline(event, depth)?),
            }
        }
    }

    fn inline(&mut self, event: Event<'a>, depth: usize) -> Result<Inline, AppError> {
        Ok(match event {
            Event::Text(text) => Inline::Text(sanitize(&text)),
            Event::Code(code) => Inline::Code(sanitize(&code)),
            Event::InlineHtml(html) => Inline::Html(sanitize(&html)),
            Event::SoftBreak => Inline::SoftBreak,
            Event::HardBreak => Inline::HardBreak,
            Event::Start(tag) => {
                let end = tag.to_end();
                match tag {
                    Tag::Emphasis => Inline::Emphasis(self.inlines(end, depth + 1)?),
                    Tag::Strong => Inline::Strong(self.inlines(end, depth + 1)?),
                    Tag::Strikethrough => Inline::Strikethrough(self.inlines(end, depth + 1)?),
                    Tag::Link {
                        link_type,
                        dest_url,
                        ..
                    } => {
                        let children = self.inlines(end, depth + 1)?;
                        match link_type {
                            LinkType::Inline | LinkType::Autolink => Inline::Link {
                                url: sanitize(&dest_url),
                                children,
                            },
                            LinkType::Email => Inline::Link {
                                url: sanitize(&format!("mailto:{dest_url}")),
                                children,
                            },
                            LinkType::Reference | LinkType::Collapsed | LinkType::Shortcut => {
                                Inline::LinkReference(children)
                            }
                            other => {
                                return Err(invariant(format!(
                                    "unexpected Markdown link type {other:?}"
                                )));
                            }
                        }
                    }
                    Tag::Image {
                        link_type,
                        dest_url,
                        id,
                        ..
                    } => {
                        let children = self.inlines(end, depth + 1)?;
                        let mut alt = String::new();
                        plain_text(&children, &mut alt);
                        match link_type {
                            LinkType::Inline => Inline::Image {
                                url: sanitize(&dest_url),
                                alt,
                            },
                            LinkType::Reference | LinkType::Collapsed | LinkType::Shortcut => {
                                Inline::ImageReference {
                                    alt,
                                    label: sanitize(&id),
                                }
                            }
                            other => {
                                return Err(invariant(format!(
                                    "unexpected Markdown image type {other:?}"
                                )));
                            }
                        }
                    }
                    other => return Err(unexpected(&Event::Start(other))),
                }
            }
            other => return Err(unexpected(&other)),
        })
    }
}

/// Image alt text: the text content of its children.
fn plain_text(children: &[Inline], out: &mut String) {
    for child in children {
        match child {
            Inline::Text(text) | Inline::Code(text) | Inline::Html(text) => out.push_str(text),
            Inline::SoftBreak => out.push('\n'),
            Inline::HardBreak => {}
            Inline::Strong(inner)
            | Inline::Emphasis(inner)
            | Inline::Strikethrough(inner)
            | Inline::LinkReference(inner)
            | Inline::Link {
                children: inner, ..
            } => plain_text(inner, out),
            Inline::Image { alt, .. } | Inline::ImageReference { alt, .. } => out.push_str(alt),
        }
    }
}

#[derive(Clone, Copy)]
struct Sgr {
    open: &'static str,
    close: &'static str,
}

const RESET: Sgr = Sgr {
    open: "\x1b[0m",
    close: "\x1b[0m",
};
const BOLD: Sgr = Sgr {
    open: "\x1b[1m",
    close: "\x1b[22m",
};
const ITALIC: Sgr = Sgr {
    open: "\x1b[3m",
    close: "\x1b[23m",
};
const UNDERLINE: Sgr = Sgr {
    open: "\x1b[4m",
    close: "\x1b[24m",
};
const INVERSE: Sgr = Sgr {
    open: "\x1b[7m",
    close: "\x1b[27m",
};
const STRIKETHROUGH: Sgr = Sgr {
    open: "\x1b[9m",
    close: "\x1b[29m",
};
const BLACK: Sgr = Sgr {
    open: "\x1b[30m",
    close: "\x1b[39m",
};
const RED: Sgr = Sgr {
    open: "\x1b[31m",
    close: "\x1b[39m",
};
const GREEN: Sgr = Sgr {
    open: "\x1b[32m",
    close: "\x1b[39m",
};
const YELLOW: Sgr = Sgr {
    open: "\x1b[33m",
    close: "\x1b[39m",
};
const BLUE: Sgr = Sgr {
    open: "\x1b[34m",
    close: "\x1b[39m",
};
const MAGENTA: Sgr = Sgr {
    open: "\x1b[35m",
    close: "\x1b[39m",
};
const CYAN: Sgr = Sgr {
    open: "\x1b[36m",
    close: "\x1b[39m",
};
const WHITE: Sgr = Sgr {
    open: "\x1b[37m",
    close: "\x1b[39m",
};
const GRAY: Sgr = Sgr {
    open: "\x1b[90m",
    close: "\x1b[39m",
};
const BG_WHITE: Sgr = Sgr {
    open: "\x1b[47m",
    close: "\x1b[49m",
};
const BG_GRAY: Sgr = Sgr {
    open: "\x1b[100m",
    close: "\x1b[49m",
};

/// SGR nesting as in `@std/fmt/colors`: a nested close code reopens the
/// enclosing style instead of ending it.
#[derive(Clone, Copy)]
struct Painter {
    enabled: bool,
}

impl Painter {
    fn paint(&self, sgr: Sgr, text: &str) -> String {
        if self.enabled {
            format!(
                "{}{}{}",
                sgr.open,
                text.replace(sgr.close, sgr.open),
                sgr.close
            )
        } else {
            text.to_owned()
        }
    }

    fn paint_all(&self, sgrs: &[Sgr], text: &str) -> String {
        sgrs.iter()
            .rev()
            .fold(text.to_owned(), |inner, sgr| self.paint(*sgr, &inner))
    }
}

/// Visible text of rendered output. Document text is sanitized, so every
/// escape here is one this renderer emitted.
fn visible(rendered: &str) -> String {
    let mut out = String::with_capacity(rendered.len());
    let mut rest = rendered;
    while let Some(index) = rest.find('\x1b') {
        out.push_str(rest.get(..index).unwrap_or_default());
        let tail = rest.get(index..).unwrap_or_default();
        rest = if let Some(osc) = tail.strip_prefix("\x1b]8;;") {
            osc.find("\x1b\\")
                .and_then(|end| osc.get(end + 2..))
                .unwrap_or_default()
        } else if let Some(csi) = tail.strip_prefix("\x1b[") {
            csi.find('m')
                .and_then(|end| csi.get(end + 1..))
                .unwrap_or_default()
        } else {
            tail.get(1..).unwrap_or_default()
        };
    }
    out.push_str(rest);
    out
}

fn width(text: &str) -> usize {
    UnicodeWidthStr::width(visible(text).as_str())
}

fn js_trim(text: &str) -> &str {
    text.trim_matches(js_space)
}

/// ECMAScript `encodeURI`, then the source's `#` to `%23` escape.
fn encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    let mut buffer = [0; 4];
    for character in path.chars() {
        if character.is_ascii_alphanumeric() || ";,/?:@&=+$-_.!~*'()".contains(character) {
            encoded.push(character);
        } else {
            for byte in character.encode_utf8(&mut buffer).bytes() {
                encoded.push_str(&format!("%{byte:02X}"));
            }
        }
    }
    encoded
}

fn replace_first(text: &str, pattern: &str, replacement: &str) -> String {
    text.replacen(pattern, replacement, 1)
}

pub(crate) fn hyperlink(text: &str, url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\{text}\x1b]8;;\x1b\\")
}

fn system_hostname() -> Result<String, AppError> {
    gethostname::gethostname().into_string().map_err(|_| {
        AppError::new(
            AppErrorKind::IoProcess,
            "the hostname for a local image link is not valid UTF-8",
        )
    })
}

struct Renderer<'o> {
    options: &'o RenderOptions,
    paint: Painter,
    host: Option<String>,
}

impl Renderer<'_> {
    fn block(&mut self, block: &Block, item_level: Option<usize>) -> Result<String, AppError> {
        Ok(match block {
            Block::Paragraph(children) => format!("{}\n", self.inlines(children)?),
            Block::Heading(depth, children) => {
                let text = format!("{} {}", "#".repeat(*depth), self.inlines(children)?);
                let styles: &[Sgr] = match depth {
                    1 => &[BOLD, UNDERLINE, RED],
                    2 => &[YELLOW, BOLD],
                    3 => &[GREEN, BOLD],
                    4 => &[MAGENTA, BOLD],
                    5 => &[CYAN, BOLD],
                    _ => &[BLUE, BOLD],
                };
                format!("{}\n", self.paint.paint_all(styles, &text))
            }
            Block::Quote(children) => {
                let mut out = String::new();
                for child in children {
                    let rendered = self.block(child, None)?;
                    let lines: Vec<&str> = rendered.split('\n').collect();
                    let last = lines.len().saturating_sub(1);
                    let quoted: Vec<String> = lines
                        .iter()
                        .enumerate()
                        .map(|(index, line)| {
                            if index == last {
                                return (*line).to_owned();
                            }
                            let styled = self.paint.paint_all(&[GRAY, ITALIC], line);
                            if js_trim(&styled).is_empty() {
                                styled
                            } else {
                                format!("┃ {styled}")
                            }
                        })
                        .collect();
                    out.push_str(&quoted.join("\n"));
                }
                out
            }
            Block::List { start, items } => self.list(*start, items, item_level)?,
            Block::Code { lang, text } => self.code(lang.as_deref(), text),
            Block::Html(html) => self.paint.paint(GRAY, html),
            Block::Table { alignments, rows } => self.table(alignments, rows)?,
            Block::Rule => {
                let columns = usize::from(self.options.columns.get());
                let length = columns.min((columns / 2).max(80));
                format!("{}\n", self.paint.paint(RESET, &"_".repeat(length)))
            }
            Block::Definition(definition) => {
                let label = self.paint.paint(CYAN, &format!("[{}]", definition.label));
                let text = format!(
                    "{label}: [{}]({})\n",
                    definition.title.as_deref().unwrap_or_default(),
                    definition.url
                );
                self.paint.paint_all(&[GRAY, ITALIC], &text)
            }
        })
    }

    fn list(
        &mut self,
        start: Option<u64>,
        items: &[Item],
        item_level: Option<usize>,
    ) -> Result<String, AppError> {
        let level = item_level.map_or(0, |parent| parent + 1);
        let mut out = String::new();
        for (index, item) in items.iter().enumerate() {
            let marker = match start {
                Some(first) => {
                    let offset = u64::try_from(index)
                        .map_err(|_| invariant("a Markdown list has too many items"))?;
                    format!("{}. ", first.saturating_add(offset))
                }
                None => {
                    let icon = LIST_ICONS
                        .get(level.min(LIST_ICONS.len() - 1))
                        .copied()
                        .unwrap_or("-");
                    format!("{icon} ")
                }
            };
            let continuation = " ".repeat(marker.chars().count());
            let mut children = Vec::with_capacity(item.blocks.len());
            for child in &item.blocks {
                children.push(self.block(child, Some(level))?);
            }
            let body = format!(
                "{}{}",
                self.paint.paint(GRAY, &marker),
                children.join(if item.spread { "\n" } else { "" })
            );
            let body = body.strip_suffix('\n').unwrap_or(&body);
            let lines: Vec<String> = body
                .split('\n')
                .enumerate()
                .map(|(line_index, line)| {
                    if line_index == 0 {
                        format!("  {line}")
                    } else {
                        format!("  {continuation}{line}")
                    }
                })
                .collect();
            out.push_str(&lines.join("\n"));
            out.push('\n');
        }
        Ok(out
            .split('\n')
            .map(|line| line.replacen("  ", "", 1))
            .collect::<Vec<_>>()
            .join("\n"))
    }

    fn code(&self, lang: Option<&str>, text: &str) -> String {
        let title = match lang {
            Some(lang) => format!(" codeblock [{lang}]"),
            None => " codeblock ".to_owned(),
        };
        let expanded = text.replace('\t', "    ");
        let lines: Vec<&str> = expanded
            .split('\n')
            .map(|line| if js_trim(line).is_empty() { " " } else { line })
            .collect();
        let max = lines
            .iter()
            .map(|line| width(line))
            .chain([width(&title)])
            .max()
            .unwrap_or_default();
        let pad = |count: usize| {
            self.paint
                .paint(BG_GRAY, &self.paint.paint(GRAY, &" ".repeat(count)))
        };
        let edge = format!("{}{}\n", self.paint.paint(BG_GRAY, " "), pad(max + 3));
        let mut out = self.paint.paint(
            BG_WHITE,
            &format!(
                "{}{}",
                self.paint.paint_all(&[BLACK, ITALIC], &title),
                self.paint
                    .paint(WHITE, &" ".repeat(max - width(&title) + 4))
            ),
        );
        out.push('\n');
        out.push_str(&edge);
        for line in lines {
            out.push_str(&pad(2));
            out.push_str(&self.paint.paint_all(&[BG_GRAY, BLACK, ITALIC], line));
            out.push_str(&pad(max - width(line) + 2));
            out.push('\n');
        }
        out.push_str(&edge);
        out
    }

    fn table(
        &mut self,
        alignments: &[Alignment],
        rows: &[Vec<Vec<Inline>>],
    ) -> Result<String, AppError> {
        let columns = rows.iter().map(Vec::len).max().unwrap_or_default();
        let mut cells: Vec<Vec<String>> = Vec::with_capacity(rows.len());
        for (row_index, row) in rows.iter().enumerate() {
            let mut rendered = Vec::with_capacity(columns);
            for column in 0..columns {
                let content = match row.get(column) {
                    Some(cell) => js_trim(&self.inlines(cell)?).to_owned(),
                    None => String::new(),
                };
                rendered.push(content);
            }
            if row_index == 0 {
                rendered = rendered
                    .iter()
                    .map(|cell| self.paint.paint_all(&[BLUE, BOLD], cell))
                    .collect();
            }
            cells.push(rendered);
        }
        let widths: Vec<usize> = (0..columns)
            .map(|column| {
                cells
                    .iter()
                    .filter_map(|row| row.get(column))
                    .map(|cell| width(cell))
                    .max()
                    .unwrap_or_default()
            })
            .collect();
        let rule = |left: &str, middle: &str, right: &str| {
            let segments: Vec<String> = widths.iter().map(|width| "─".repeat(width + 2)).collect();
            format!("{left}{}{right}", segments.join(middle))
        };
        let lines: Vec<String> = cells
            .iter()
            .map(|row| {
                let padded: Vec<String> = row
                    .iter()
                    .enumerate()
                    .map(|(column, cell)| {
                        let space = widths
                            .get(column)
                            .copied()
                            .unwrap_or_default()
                            .saturating_sub(width(cell));
                        let aligned = match alignments.get(column) {
                            Some(Alignment::Center) => format!(
                                "{}{cell}{}",
                                " ".repeat(space / 2),
                                " ".repeat(space - space / 2)
                            ),
                            Some(Alignment::Right) => format!("{}{cell}", " ".repeat(space)),
                            Some(Alignment::Left | Alignment::None) | None => {
                                format!("{cell}{}", " ".repeat(space))
                            }
                        };
                        format!(" {aligned} ")
                    })
                    .collect();
                format!("│{}│", padded.join("│"))
            })
            .collect();
        Ok(format!(
            "{}\n{}\n{}\n",
            rule("┌", "┬", "┐"),
            lines.join(&format!("\n{}\n", rule("├", "┼", "┤"))),
            rule("└", "┴", "┘")
        ))
    }

    fn inlines(&mut self, children: &[Inline]) -> Result<String, AppError> {
        let paint = self.paint;
        let mut out = String::new();
        for child in children {
            let rendered: Cow<'_, str> = match child {
                Inline::Text(text) => Cow::Borrowed(text),
                Inline::Code(code) => Cow::Owned(paint.paint(INVERSE, &format!(" {code} "))),
                Inline::Html(html) => Cow::Owned(paint.paint(GRAY, html)),
                Inline::SoftBreak | Inline::HardBreak => Cow::Borrowed("\n"),
                Inline::Strong(inner) => Cow::Owned(paint.paint(BOLD, &self.inlines(inner)?)),
                Inline::Emphasis(inner) => Cow::Owned(paint.paint(ITALIC, &self.inlines(inner)?)),
                Inline::Strikethrough(inner) => {
                    Cow::Owned(paint.paint(STRIKETHROUGH, &self.inlines(inner)?))
                }
                Inline::Link { url, children } => {
                    let text = format!("[{}]({url})", self.inlines(children)?);
                    Cow::Owned(paint.paint(CYAN, &text))
                }
                Inline::LinkReference(inner) => {
                    let text = format!("[{}]", self.inlines(inner)?);
                    Cow::Owned(paint.paint_all(&[CYAN, ITALIC], &text))
                }
                Inline::Image { url, alt } => Cow::Owned(self.image(url, alt)?),
                Inline::ImageReference { alt, label } => {
                    let label = paint.paint(CYAN, &format!("[{label}]"));
                    Cow::Owned(self.paint.paint_all(
                        &[GRAY, ITALIC],
                        &format!("Image reference: ![{alt}]{label}"),
                    ))
                }
            };
            out.push_str(&rendered);
        }
        Ok(out)
    }

    fn image(&mut self, url: &str, alt: &str) -> Result<String, AppError> {
        let Some(links) = &self.options.image_hyperlinks else {
            return Ok(self
                .paint
                .paint_all(&[GRAY, ITALIC], &format!("Image: ![{alt}]({url})")));
        };
        let target = if url.starts_with("http://") || url.starts_with("https://") {
            url.to_owned()
        } else {
            let host = match (&self.host, &links.host) {
                (Some(host), _) => host.clone(),
                (None, HostSource::Fixed(host)) => host.clone(),
                (None, HostSource::System) => system_hostname()?,
            };
            let with_host = replace_first(&links.template, "{host}", &host);
            self.host = Some(host);
            replace_first(&with_host, "{path}", &encode_path(url))
        };
        Ok(format!("Image: ![{alt}]({})", hyperlink(url, &target)))
    }
}
