//! Markdown rendering for a terminal.
//!
//! Only terminal output is rendered; piped output stays raw Markdown. Source
//! line breaks are kept, and text wider than the terminal wraps at spaces with
//! list and quote indentation carried onto the next line. Tables shrink their
//! columns to fit, wrapping cell text, and become one record per row when the
//! terminal is too narrow for a grid. With `styled` on, emphasis,
//! headings and code become SGR styles and links become OSC-8 hyperlinks;
//! with it off the output is plain text that keeps the Markdown cues
//! (`#` headings, backticks, link URLs) needed to read it.

use std::num::NonZeroU16;
use std::path::Path;

use console::Style;
use pulldown_cmark::{
    Alignment, CodeBlockKind, Event, HeadingLevel, LinkType, Options, Parser, Tag, TagEnd,
};
use reqwest::Url;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::platform::terminal_text;

/// Renderer width when a terminal reports no usable size.
pub const FALLBACK_COLUMNS: NonZeroU16 = NonZeroU16::MIN.saturating_add(79);

/// Hyperlink target template for local files when none is configured.
const DEFAULT_FILE_LINK: &str = "file://{host}{path}";

const BULLETS: [&str; 3] = ["•", "◦", "▪"];

/// Wrapped text keeps at least this many columns however deep it is nested.
const MIN_TEXT_WIDTH: usize = 10;

/// A table column narrower than this cannot show its cells usefully; such
/// tables print one record per row instead.
const MIN_CELL_WIDTH: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RenderOptions {
    /// Terminal width; it sizes thematic breaks.
    pub columns: NonZeroU16,
    /// Emit SGR styles and OSC-8 hyperlinks.
    pub styled: bool,
    /// Hyperlink target for local image paths, with `{host}` and `{path}`
    /// placeholders (for example `vscode://file{path}`).
    pub file_link: String,
}

impl RenderOptions {
    /// `hyperlink_format` is the configured local-file link template;
    /// unset, empty or `default` means `file://{host}{path}`.
    pub fn for_terminal(columns: NonZeroU16, styled: bool, hyperlink_format: Option<&str>) -> Self {
        let file_link = match hyperlink_format {
            Some(format) if !format.is_empty() && format != "default" => format,
            Some(_) | None => DEFAULT_FILE_LINK,
        };
        Self {
            columns,
            styled,
            file_link: file_link.to_owned(),
        }
    }
}

/// Renders Markdown for a terminal. Nonempty output ends with a line feed.
pub fn render(markdown: &str, options: &RenderOptions) -> String {
    let markdown = terminal_text::multiline(markdown);
    let parser = Parser::new_ext(
        &markdown,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    );
    let mut renderer = Renderer::new(options);
    for event in parser {
        renderer.event(event);
    }
    renderer.out
}

/// An OSC-8 terminal hyperlink.
pub fn hyperlink(text: &str, url: &str) -> String {
    format!("\x1b]8;;{}\x1b\\{text}\x1b]8;;\x1b\\", sanitize(url))
}

/// Document text with terminal control characters (other than tab and line
/// feed) replaced, so remote content cannot inject escape sequences.
fn sanitize(text: &str) -> String {
    text.chars()
        .map(|c| {
            if c.is_control() && c != '\n' && c != '\t' {
                '\u{fffd}'
            } else {
                c
            }
        })
        .collect()
}

/// A container whose lines carry a prefix.
enum Container {
    Quote,
    /// A list item: continuation lines are indented by the marker's width;
    /// the marker itself is printed on the item's first line.
    Item {
        marker: String,
        printed: bool,
    },
    /// Code block lines are indented.
    Code,
}

struct List {
    /// The next number of an ordered list.
    next: Option<u64>,
    /// A loose list separates its items with blank lines.
    loose: bool,
}

struct Link {
    url: String,
    /// Visible text, to tell whether the URL must be printed after it.
    text: String,
}

struct Image {
    url: String,
    alt: String,
}

/// Inline content waiting to be laid out into lines.
#[derive(Clone)]
enum Span {
    Text {
        text: String,
        style: Style,
    },
    /// Starts an OSC-8 hyperlink (styled output only).
    LinkStart(String),
    LinkEnd,
}

#[derive(Default)]
struct Cell {
    spans: Vec<Span>,
}

impl Cell {
    fn width(&self) -> usize {
        spans_width(&self.spans)
    }
}

struct Table {
    alignments: Vec<Alignment>,
    rows: Vec<Vec<Cell>>,
    in_head: bool,
}

struct Renderer<'o> {
    options: &'o RenderOptions,
    out: String,
    containers: Vec<Container>,
    lists: Vec<List>,
    links: Vec<Link>,
    image: Option<Image>,
    table: Option<Table>,
    /// Inline content of the current line, laid out when the line ends.
    pending: Vec<Span>,
    at_line_start: bool,
    /// A block ended; the next one starts after a blank line.
    gap: bool,
    heading: Option<HeadingLevel>,
    strong: usize,
    emphasis: usize,
    strikethrough: usize,
    host: Option<String>,
}

impl<'o> Renderer<'o> {
    fn new(options: &'o RenderOptions) -> Self {
        Self {
            options,
            out: String::new(),
            containers: Vec::new(),
            lists: Vec::new(),
            links: Vec::new(),
            image: None,
            table: None,
            pending: Vec::new(),
            at_line_start: true,
            gap: false,
            heading: None,
            strong: 0,
            emphasis: 0,
            strikethrough: 0,
            host: None,
        }
    }

    fn event(&mut self, event: Event<'_>) {
        match event {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(text) => {
                if matches!(self.containers.last(), Some(Container::Code)) {
                    let code = self.style().cyan();
                    self.lines(&sanitize(&text), &code);
                } else {
                    self.inline(&sanitize(&text), &self.style());
                }
            }
            Event::Code(code) => {
                let code = sanitize(&code);
                if self.options.styled {
                    self.inline(&code, &self.style().cyan());
                } else {
                    self.inline(&format!("`{code}`"), &self.style());
                }
            }
            Event::Html(html) => {
                let gray = Style::new().black().bright();
                self.lines(&sanitize(&html), &gray);
            }
            Event::InlineHtml(html) => {
                self.inline(&sanitize(&html), &self.style().black().bright())
            }
            Event::SoftBreak | Event::HardBreak => {
                if let Some(cell) = self.table.as_mut().and_then(|table| table.cell()) {
                    cell.spans.push(Span::Text {
                        text: " ".to_owned(),
                        style: Style::new(),
                    });
                } else if let Some(image) = &mut self.image {
                    image.alt.push(' ');
                } else {
                    self.newline();
                }
            }
            Event::Rule => {
                self.start_block();
                let columns = self.text_width().min(80);
                let rule = self.paint(&"─".repeat(columns), &Style::new().black().bright());
                self.write(&rule);
                self.end_block();
            }
            Event::TaskListMarker(checked) => {
                self.inline(if checked { "[x] " } else { "[ ] " }, &self.style());
            }
            Event::InlineMath(_) | Event::DisplayMath(_) | Event::FootnoteReference(_) => {
                unreachable!("math and footnotes are not enabled")
            }
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {
                if matches!(self.containers.last(), Some(Container::Item { .. }))
                    && let Some(list) = self.lists.last_mut()
                {
                    list.loose = true;
                }
                self.start_block();
            }
            Tag::Heading { level, .. } => {
                self.start_block();
                self.heading = Some(level);
                if !self.options.styled {
                    let marks = "#".repeat(heading_depth(level));
                    self.inline(&format!("{marks} "), &Style::new());
                }
            }
            Tag::BlockQuote(_) => {
                self.start_block();
                self.containers.push(Container::Quote);
            }
            Tag::CodeBlock(kind) => {
                self.start_block();
                if let CodeBlockKind::Fenced(info) = kind {
                    let lang = info.split_whitespace().next().unwrap_or_default();
                    if !lang.is_empty() {
                        let label = self.paint(&sanitize(lang), &Style::new().black().bright());
                        self.write(&label);
                        self.newline();
                    }
                }
                self.containers.push(Container::Code);
            }
            Tag::HtmlBlock => self.start_block(),
            Tag::List(start) => {
                self.start_block();
                self.lists.push(List {
                    next: start,
                    loose: false,
                });
            }
            Tag::Item => {
                let depth = self.lists.len().saturating_sub(1);
                if self.gap && self.lists.last().is_some_and(|list| list.loose) {
                    self.gap = false;
                    self.blank_line();
                }
                let list = self.lists.last_mut().expect("list items are inside a list");
                let marker = match &mut list.next {
                    Some(number) => {
                        let marker = format!("{number}. ");
                        *number = number.saturating_add(1);
                        marker
                    }
                    None => format!("{} ", BULLETS.get(depth % BULLETS.len()).unwrap_or(&"-")),
                };
                self.containers.push(Container::Item {
                    marker,
                    printed: false,
                });
            }
            Tag::Table(alignments) => {
                self.start_block();
                self.table = Some(Table {
                    alignments,
                    rows: Vec::new(),
                    in_head: false,
                });
            }
            Tag::TableHead => {
                let table = self.table.as_mut().expect("a table head is inside a table");
                table.in_head = true;
                table.rows.push(Vec::new());
            }
            Tag::TableRow => {
                let table = self.table.as_mut().expect("a table row is inside a table");
                table.rows.push(Vec::new());
            }
            Tag::TableCell => {
                let table = self.table.as_mut().expect("a table cell is inside a table");
                table
                    .rows
                    .last_mut()
                    .expect("a table cell is inside a row")
                    .push(Cell::default());
            }
            Tag::Emphasis => self.emphasis += 1,
            Tag::Strong => self.strong += 1,
            Tag::Strikethrough => self.strikethrough += 1,
            Tag::Link {
                link_type,
                dest_url,
                ..
            } => {
                let url = match link_type {
                    LinkType::Email => format!("mailto:{dest_url}"),
                    _ => dest_url.into_string(),
                };
                let url = sanitize(&url);
                if self.options.styled {
                    self.span(Span::LinkStart(url.clone()));
                }
                self.links.push(Link {
                    url,
                    text: String::new(),
                });
            }
            Tag::Image { dest_url, .. } => {
                self.image = Some(Image {
                    url: sanitize(&dest_url),
                    alt: String::new(),
                });
            }
            Tag::FootnoteDefinition(_)
            | Tag::DefinitionList
            | Tag::DefinitionListTitle
            | Tag::DefinitionListDefinition
            | Tag::Superscript
            | Tag::Subscript
            | Tag::MetadataBlock(_) => {
                unreachable!(
                    "footnotes, definition lists, sub/superscript and metadata are not enabled"
                )
            }
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph | TagEnd::HtmlBlock => self.end_block(),
            TagEnd::Heading(_) => {
                self.heading = None;
                self.end_block();
            }
            TagEnd::BlockQuote(_) => {
                self.containers.pop();
                self.end_block();
            }
            TagEnd::CodeBlock => {
                self.containers.pop();
                self.end_block();
            }
            TagEnd::List(_) => {
                self.lists.pop();
                self.end_block();
            }
            TagEnd::Item => {
                self.flush_inline();
                if !self.at_line_start {
                    self.newline();
                }
                let printed = matches!(
                    self.containers.last(),
                    Some(Container::Item { printed: true, .. })
                );
                if !printed {
                    // An empty item still shows its marker.
                    self.write("");
                    self.newline();
                }
                self.containers.pop();
                if !self.lists.last().is_some_and(|list| list.loose) {
                    self.gap = false;
                }
            }
            TagEnd::Table => {
                let table = self.table.take().expect("a table ends after it starts");
                self.table(&table);
                self.end_block();
            }
            TagEnd::TableHead => {
                let table = self.table.as_mut().expect("a table head is inside a table");
                table.in_head = false;
            }
            TagEnd::TableRow | TagEnd::TableCell => {}
            TagEnd::Emphasis => self.emphasis -= 1,
            TagEnd::Strong => self.strong -= 1,
            TagEnd::Strikethrough => self.strikethrough -= 1,
            TagEnd::Link => {
                let link = self.links.pop().expect("a link ends after it starts");
                if self.options.styled {
                    self.span(Span::LinkEnd);
                } else if link.text != link.url && !link.url.is_empty() {
                    self.inline(&format!(" ({})", link.url), &self.style());
                }
            }
            TagEnd::Image => {
                let image = self.image.take().expect("an image ends after it starts");
                self.image(&image);
            }
            TagEnd::FootnoteDefinition
            | TagEnd::DefinitionList
            | TagEnd::DefinitionListTitle
            | TagEnd::DefinitionListDefinition
            | TagEnd::Superscript
            | TagEnd::Subscript
            | TagEnd::MetadataBlock(_) => {
                unreachable!(
                    "footnotes, definition lists, sub/superscript and metadata are not enabled"
                )
            }
        }
    }

    /// The style for inline text at the current position.
    fn style(&self) -> Style {
        let mut style = Style::new();
        if self.strong > 0
            || self.heading.is_some()
            || self.table.as_ref().is_some_and(|t| t.in_head)
        {
            style = style.bold();
        }
        if self.heading == Some(HeadingLevel::H1) {
            style = style.underlined();
        }
        if self.emphasis > 0 {
            style = style.italic();
        }
        if self.strikethrough > 0 {
            style = style.strikethrough();
        }
        if !self.links.is_empty() {
            style = style.blue().underlined();
        }
        style
    }

    fn paint(&self, text: &str, style: &Style) -> String {
        if self.options.styled && !text.is_empty() {
            style.clone().force_styling(true).apply_to(text).to_string()
        } else {
            text.to_owned()
        }
    }

    /// Inline text without line breaks, to wherever inline content goes.
    fn inline(&mut self, text: &str, style: &Style) {
        for link in &mut self.links {
            link.text.push_str(text);
        }
        if let Some(image) = &mut self.image {
            image.alt.push_str(text);
            return;
        }
        self.span(Span::Text {
            text: text.to_owned(),
            style: style.clone(),
        });
    }

    /// Inline content for the table cell being filled or the current line.
    /// Inside an image only the alt text counts.
    fn span(&mut self, span: Span) {
        if let Some(cell) = self.table.as_mut().and_then(|table| table.cell()) {
            cell.spans.push(span);
        } else if self.image.is_none() {
            self.pending.push(span);
        }
    }

    /// Multi-line block text, each line painted separately.
    fn lines(&mut self, text: &str, style: &Style) {
        self.flush_inline();
        let mut lines = text.split('\n').peekable();
        while let Some(line) = lines.next() {
            if lines.peek().is_none() {
                if !line.is_empty() {
                    let painted = self.paint(line, style);
                    self.write(&painted);
                }
            } else if line.is_empty() && self.at_line_start {
                self.blank_line();
            } else {
                let painted = self.paint(line, style);
                self.write(&painted);
                self.newline();
            }
        }
    }

    /// Writes text without line breaks, after any pending inline content.
    fn write(&mut self, text: &str) {
        self.flush_inline();
        self.emit(text);
    }

    /// Writes text, preceded by the container prefixes at the start of a line.
    fn emit(&mut self, text: &str) {
        if self.at_line_start {
            let prefix = self.prefix();
            self.out.push_str(&prefix);
            self.at_line_start = false;
        }
        self.out.push_str(text);
    }

    fn newline(&mut self) {
        self.flush_inline();
        self.out.push('\n');
        self.at_line_start = true;
    }

    /// Lays the pending inline content out in lines that fit the terminal.
    fn flush_inline(&mut self) {
        if self.pending.is_empty() {
            return;
        }
        let spans = std::mem::take(&mut self.pending);
        let width = self.text_width();
        for (index, line) in layout(&spans, width, false, self.options.styled)
            .iter()
            .enumerate()
        {
            if index > 0 {
                self.out.push('\n');
                self.at_line_start = true;
            }
            self.emit(&line.painted);
        }
    }

    /// Columns left for text after the container prefixes, never so few
    /// that every word lands on its own line.
    fn text_width(&self) -> usize {
        let prefix: usize = self
            .containers
            .iter()
            .map(|container| match container {
                Container::Quote => 2,
                Container::Item { marker, .. } => marker.width(),
                Container::Code => 4,
            })
            .sum();
        usize::from(self.options.columns.get())
            .saturating_sub(prefix)
            .max(MIN_TEXT_WIDTH)
    }

    fn prefix(&mut self) -> String {
        let styled = self.options.styled;
        let bar = if styled {
            Style::new()
                .black()
                .bright()
                .force_styling(true)
                .apply_to("│ ")
                .to_string()
        } else {
            "│ ".to_owned()
        };
        let mut prefix = String::new();
        for container in &mut self.containers {
            match container {
                Container::Quote => prefix.push_str(&bar),
                Container::Item { marker, printed } => {
                    if *printed {
                        prefix.push_str(&" ".repeat(marker.width()));
                    } else {
                        *printed = true;
                        if styled {
                            prefix.push_str(
                                &Style::new()
                                    .black()
                                    .bright()
                                    .force_styling(true)
                                    .apply_to(&*marker)
                                    .to_string(),
                            );
                        } else {
                            prefix.push_str(marker);
                        }
                    }
                }
                Container::Code => prefix.push_str("    "),
            }
        }
        prefix
    }

    /// An empty line that keeps quote bars but never prints list markers.
    fn blank_line(&mut self) {
        self.flush_inline();
        let mut prefix = String::new();
        for container in &self.containers {
            if matches!(container, Container::Quote) {
                prefix.push('│');
            }
            prefix.push(' ');
        }
        let bar = self.paint(prefix.trim_end(), &Style::new().black().bright());
        self.out.push_str(&bar);
        self.newline();
    }

    fn start_block(&mut self) {
        self.flush_inline();
        if !self.at_line_start {
            self.newline();
        }
        if self.gap {
            self.gap = false;
            self.blank_line();
        }
    }

    fn end_block(&mut self) {
        self.flush_inline();
        if !self.at_line_start {
            self.newline();
        }
        self.gap = true;
    }

    fn image(&mut self, image: &Image) {
        let spans = if self.options.styled {
            let target = self.image_target(&image.url);
            let gray = Style::new().black().bright();
            vec![
                Span::Text {
                    text: format!("Image: {} ", image.alt),
                    style: gray.clone(),
                },
                Span::LinkStart(target),
                Span::Text {
                    text: image.url.clone(),
                    style: gray.underlined(),
                },
                Span::LinkEnd,
            ]
        } else {
            vec![Span::Text {
                text: format!("![{}]({})", image.alt, image.url),
                style: Style::new(),
            }]
        };
        if let Some(cell) = self.table.as_mut().and_then(|table| table.cell()) {
            cell.spans.extend(spans);
        } else {
            self.pending.extend(spans);
        }
    }

    /// Where an image hyperlink points: URLs as they are, local paths (such
    /// as downloaded images) through the file link template.
    fn image_target(&mut self, url: &str) -> String {
        if !Path::new(url).is_absolute() && Url::parse(url).is_ok() {
            return url.to_owned();
        }
        let mut target = self.options.file_link.clone();
        if target.contains("{host}") {
            let host = self
                .host
                .get_or_insert_with(|| gethostname::gethostname().to_string_lossy().into_owned());
            target = target.replacen("{host}", host, 1);
        }
        target.replacen("{path}", &encode_path(url), 1)
    }

    /// A grid with box-drawing borders, its columns shrunk (and cell text
    /// wrapped) to fit the terminal; one record per row when even that
    /// cannot fit.
    fn table(&mut self, table: &Table) {
        let columns = table.rows.iter().map(Vec::len).max().unwrap_or_default();
        let natural: Vec<usize> = (0..columns)
            .map(|column| {
                table
                    .rows
                    .iter()
                    .filter_map(|row| row.get(column))
                    .map(Cell::width)
                    .max()
                    .unwrap_or_default()
            })
            .collect();
        // Each column has a border and a space on either side, plus the
        // closing border.
        let borders = 3 * columns + 1;
        let available = self.text_width().saturating_sub(borders);
        let widths = if natural.iter().sum::<usize>() <= available {
            natural
        } else if available >= MIN_CELL_WIDTH * columns {
            fit_columns(&natural, available)
        } else {
            return self.table_records(table);
        };
        let rule = |left: &str, middle: &str, right: &str| {
            let segments: Vec<String> = widths.iter().map(|width| "─".repeat(width + 2)).collect();
            format!("{left}{}{right}", segments.join(middle))
        };
        let empty = Cell::default();
        self.write(&rule("┌", "┬", "┐"));
        self.newline();
        for (index, row) in table.rows.iter().enumerate() {
            if index > 0 {
                self.write(&rule("├", "┼", "┤"));
                self.newline();
            }
            let cells: Vec<Vec<Laid>> = widths
                .iter()
                .enumerate()
                .map(|(column, &width)| {
                    layout(
                        &row.get(column).unwrap_or(&empty).spans,
                        width,
                        true,
                        self.options.styled,
                    )
                })
                .collect();
            let height = cells.iter().map(Vec::len).max().unwrap_or_default().max(1);
            for line_index in 0..height {
                let mut line = String::from("│");
                for (column, (lines, width)) in cells.iter().zip(&widths).enumerate() {
                    let (painted, used) = lines
                        .get(line_index)
                        .map_or(("", 0), |laid| (laid.painted.as_str(), laid.width));
                    let space = width.saturating_sub(used);
                    let (left, right) = match table.alignments.get(column) {
                        Some(Alignment::Center) => (space / 2, space - space / 2),
                        Some(Alignment::Right) => (space, 0),
                        Some(Alignment::Left | Alignment::None) | None => (0, space),
                    };
                    line.push_str(&format!(
                        " {}{painted}{} │",
                        " ".repeat(left),
                        " ".repeat(right)
                    ));
                }
                self.write(&line);
                self.newline();
            }
        }
        self.write(&rule("└", "┴", "┘"));
        self.newline();
    }

    /// A table too wide for a grid: each body row as `Header: value` lines,
    /// rows separated by blank lines.
    fn table_records(&mut self, table: &Table) {
        let Some((head, body)) = table.rows.split_first() else {
            return;
        };
        for (index, row) in body.iter().enumerate() {
            if index > 0 {
                self.blank_line();
            }
            for (column, cell) in row.iter().enumerate() {
                if let Some(header) = head.get(column).filter(|header| header.width() > 0) {
                    self.pending.extend(header.spans.iter().cloned());
                    self.pending.push(Span::Text {
                        text: ": ".to_owned(),
                        style: Style::new(),
                    });
                }
                self.pending.extend(cell.spans.iter().cloned());
                self.newline();
            }
        }
    }
}

/// The shortest columns keep their width and the rest share what is left
/// of `available` equally.
fn fit_columns(natural: &[usize], available: usize) -> Vec<usize> {
    let mut order: Vec<usize> = (0..natural.len()).collect();
    order.sort_by_key(|&column| natural.get(column).copied().unwrap_or_default());
    let mut widths = vec![0; natural.len()];
    let mut remaining = available;
    for (placed, &column) in order.iter().enumerate() {
        let share = remaining / (natural.len() - placed);
        let width = natural.get(column).copied().unwrap_or_default().min(share);
        if let Some(slot) = widths.get_mut(column) {
            *slot = width;
        }
        remaining -= width;
    }
    widths
}

/// One laid-out line: painted text and the columns it takes.
struct Laid {
    painted: String,
    width: usize,
}

fn spans_width(spans: &[Span]) -> usize {
    spans
        .iter()
        .map(|span| match span {
            Span::Text { text, .. } => text.width(),
            Span::LinkStart(_) | Span::LinkEnd => 0,
        })
        .sum()
}

/// Lays `spans` out in lines of at most `width` columns, breaking at spaces
/// and dropping the spaces a break replaces. A word wider than a line is
/// split when `split_words` is on and overflows the line otherwise (long
/// URLs stay copyable that way). A hyperlink that crosses a break is closed
/// at the end of the line and reopened on the next.
fn layout(spans: &[Span], width: usize, split_words: bool, styled: bool) -> Vec<Laid> {
    let mut lines = Lines {
        width: width.max(1),
        split_words,
        done: Vec::new(),
        line: Vec::new(),
        line_width: 0,
        gap: Vec::new(),
        gap_width: 0,
        word: Vec::new(),
        word_width: 0,
    };
    for span in spans {
        lines.push(span);
    }
    let mut open_link: Option<&str> = None;
    lines
        .finish()
        .iter()
        .map(|line| {
            let mut painted = String::new();
            if let Some(url) = open_link {
                painted.push_str(&link_start(url));
            }
            for span in line {
                match span {
                    Span::Text { text, style } if styled => {
                        let style = style.clone().force_styling(true);
                        painted.push_str(&style.apply_to(text).to_string());
                    }
                    Span::Text { text, .. } => painted.push_str(text),
                    Span::LinkStart(url) => {
                        painted.push_str(&link_start(url));
                        open_link = Some(url);
                    }
                    Span::LinkEnd => {
                        painted.push_str(LINK_END);
                        open_link = None;
                    }
                }
            }
            if open_link.is_some() {
                painted.push_str(LINK_END);
            }
            Laid {
                painted,
                width: spans_width(line),
            }
        })
        .collect()
}

/// Greedy line filling for [`layout`]: words are collected until a space
/// ends them, then placed on the current line or the next.
struct Lines {
    width: usize,
    split_words: bool,
    done: Vec<Vec<Span>>,
    line: Vec<Span>,
    line_width: usize,
    /// Spaces before the word being collected.
    gap: Vec<Span>,
    gap_width: usize,
    word: Vec<Span>,
    word_width: usize,
}

impl Lines {
    fn push(&mut self, span: &Span) {
        let Span::Text { text, style } = span else {
            self.word.push(span.clone());
            return;
        };
        for (is_space, run) in runs(text) {
            let piece = Span::Text {
                text: run.to_owned(),
                style: style.clone(),
            };
            if is_space {
                if !self.word.is_empty() {
                    self.place_word();
                }
                self.gap.push(piece);
                self.gap_width += run.width();
            } else {
                self.word.push(piece);
                self.word_width += run.width();
            }
        }
    }

    fn place_word(&mut self) {
        let gap = std::mem::take(&mut self.gap);
        let gap_width = std::mem::take(&mut self.gap_width);
        if self.line_width > 0 && self.line_width + gap_width + self.word_width > self.width {
            self.break_line();
        }
        if self.line_width > 0 {
            self.line.extend(gap);
            self.line_width += gap_width;
        }
        let word = std::mem::take(&mut self.word);
        let word_width = std::mem::take(&mut self.word_width);
        if !self.split_words || word_width <= self.width {
            self.line.extend(word);
            self.line_width += word_width;
            return;
        }
        for span in word {
            let Span::Text { text, style } = span else {
                self.line.push(span);
                continue;
            };
            let mut chunk = String::new();
            for ch in text.chars() {
                let ch_width = ch.width().unwrap_or(0);
                if self.line_width > 0 && self.line_width + ch_width > self.width {
                    if !chunk.is_empty() {
                        self.line.push(Span::Text {
                            text: std::mem::take(&mut chunk),
                            style: style.clone(),
                        });
                    }
                    self.break_line();
                }
                chunk.push(ch);
                self.line_width += ch_width;
            }
            if !chunk.is_empty() {
                self.line.push(Span::Text { text: chunk, style });
            }
        }
    }

    fn break_line(&mut self) {
        self.done.push(std::mem::take(&mut self.line));
        self.line_width = 0;
    }

    fn finish(mut self) -> Vec<Vec<Span>> {
        if !self.word.is_empty() {
            self.place_word();
        }
        if !self.line.is_empty() {
            self.done.push(self.line);
        }
        self.done
    }
}

/// `text` split into alternating runs of spaces and of everything else.
fn runs(text: &str) -> impl Iterator<Item = (bool, &str)> {
    let is_space = |ch: char| ch == ' ' || ch == '\t';
    let mut rest = text;
    std::iter::from_fn(move || {
        let first = rest.chars().next()?;
        let space = is_space(first);
        let end = rest
            .find(|ch: char| is_space(ch) != space)
            .unwrap_or(rest.len());
        let (run, tail) = rest.split_at(end);
        rest = tail;
        Some((space, run))
    })
}

fn link_start(url: &str) -> String {
    format!("\x1b]8;;{url}\x1b\\")
}

const LINK_END: &str = "\x1b]8;;\x1b\\";

impl Table {
    /// The cell being filled, if any.
    fn cell(&mut self) -> Option<&mut Cell> {
        self.rows.last_mut().and_then(|row| row.last_mut())
    }
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

/// Percent-encodes a file path for a URL, keeping `/` separators.
fn encode_path(path: &str) -> String {
    let mut encoded = String::with_capacity(path.len());
    for byte in path.bytes() {
        if byte.is_ascii_alphanumeric() || b"/-._~".contains(&byte) {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(styled: bool) -> RenderOptions {
        RenderOptions::for_terminal(FALLBACK_COLUMNS, styled, Some("https://open/{path}"))
    }

    fn plain(markdown: &str) -> String {
        render(markdown, &options(false))
    }

    #[test]
    fn control_characters_in_the_source_never_reach_the_terminal() {
        let styled = render(
            "Hi \u{1b}]8;;https://evil\u{7}there\n\n[x](https://a/\u{1b}[2J)",
            &options(true),
        );
        assert!(!styled.contains("evil\u{7}"), "{styled:?}");
        assert!(!styled.contains("\u{1b}[2J"), "{styled:?}");
        assert!(
            styled.contains("Hi \u{FFFD}]8;;https://evil\u{FFFD}there"),
            "{styled:?}"
        );
    }

    #[test]
    fn plain_output_keeps_structure_readable() {
        let document = "# Title\n\nSome *emphasis*, `code` and [a link](https://example.com).\nNext line.\n\n## Section\n\n- one\n- two\n  - nested\n\n1. first\n2. second\n\n> quoted\n> more\n\n```rs\nlet x = 1;\n\nlet y = 2;\n```\n\n---\n\n- [x] done\n- [ ] todo";
        assert_eq!(
            plain(document),
            "# Title\n\nSome emphasis, `code` and a link (https://example.com).\nNext line.\n\n## Section\n\n• one\n• two\n  ◦ nested\n\n1. first\n2. second\n\n│ quoted\n│ more\n\nrs\n    let x = 1;\n\n    let y = 2;\n\n────────────────────────────────────────────────────────────────────────────────\n\n• [x] done\n• [ ] todo\n"
        );
    }

    #[test]
    fn loose_lists_and_multi_paragraph_items_keep_blank_lines() {
        assert_eq!(
            plain("- one\n\n  more\n- two\n\n> a\n>\n> b"),
            "• one\n\n  more\n\n• two\n\n│ a\n│\n│ b\n"
        );
        assert_eq!(
            plain("10. ten\n11. eleven\n-"),
            "10. ten\n11. eleven\n\n• \n"
        );
    }

    #[test]
    fn autolinks_and_reference_links_show_their_url_once() {
        assert_eq!(
            plain("<https://a.example> [ref] [mail](mailto:x@y.z)\n\n[ref]: https://b.example"),
            "https://a.example ref (https://b.example) mail (mailto:x@y.z)\n"
        );
    }

    #[test]
    fn tables_align_by_display_width() {
        assert_eq!(
            plain("| A | Number |\n| --- | ---: |\n| 猫 | 2 |\n| **b** |"),
            "┌────┬────────┐\n│ A  │ Number │\n├────┼────────┤\n│ 猫 │      2 │\n├────┼────────┤\n│ b  │        │\n└────┴────────┘\n"
        );
    }

    #[test]
    fn styled_headings_drop_their_markers_and_links_become_hyperlinks() {
        let styled = render("## Section\n\n[docs](https://example.com)", &options(true));
        assert!(!styled.contains("##"), "{styled:?}");
        assert!(styled.contains("Section"));
        assert!(
            styled.contains("\x1b]8;;https://example.com\x1b\\"),
            "{styled:?}"
        );
        assert!(!styled.contains("(https://example.com)"));
    }

    #[test]
    fn images_link_local_paths_through_the_template() {
        assert_eq!(plain("![shot](/tmp/a b.png)"), "![shot](/tmp/a b.png)\n");
        let styled = render("![shot](</tmp/a b#1.png>)", &options(true));
        assert!(
            styled.contains("\x1b]8;;https://open//tmp/a%20b%231.png\x1b\\"),
            "{styled:?}"
        );
        let remote = render("![shot](https://example.com/a.png)", &options(true));
        assert!(remote.contains("\x1b]8;;https://example.com/a.png\x1b\\"));
        let default =
            RenderOptions::for_terminal(FALLBACK_COLUMNS, true, Some("default")).file_link;
        assert_eq!(default, "file://{host}{path}");
    }

    #[test]
    fn control_characters_are_replaced_and_deep_nesting_renders() {
        assert_eq!(plain("hi\u{1b}[31m"), "hi\u{fffd}[31m\n");
        let deep = render(&format!("{}x", "> ".repeat(500)), &options(true));
        assert!(deep.ends_with("x\n"));
    }

    #[test]
    fn unstyled_output_has_no_escapes() {
        let output =
            plain("# A\n\n**b** _c_ ~~d~~ `e` [f](https://g) ![h](/i)\n\n| x |\n|---|\n| y |");
        assert!(!output.contains('\x1b'), "{output:?}");
    }

    fn narrow(markdown: &str, columns: u16) -> String {
        let columns = NonZeroU16::new(columns).expect("nonzero width");
        render(markdown, &RenderOptions::for_terminal(columns, false, None))
    }

    #[test]
    fn paragraphs_wrap_at_spaces_to_the_terminal_width() {
        assert_eq!(
            narrow("The quick brown fox jumps over the lazy dog.", 20),
            "The quick brown fox\njumps over the lazy\ndog.\n"
        );
        // Source line breaks stay; a word wider than the line is not split.
        assert_eq!(
            narrow("short\nhttps://example.com/a/very/long/path ok", 20),
            "short\nhttps://example.com/a/very/long/path\nok\n"
        );
        assert_eq!(
            narrow("**bold**text stays together and `code spans` too", 20),
            "boldtext stays\ntogether and `code\nspans` too\n"
        );
    }

    #[test]
    fn wrapped_lines_keep_list_and_quote_indentation() {
        assert_eq!(
            narrow(
                "- alpha beta gamma delta epsilon\n  - zeta eta theta iota",
                20
            ),
            "• alpha beta gamma\n  delta epsilon\n  ◦ zeta eta theta\n    iota\n"
        );
        assert_eq!(
            narrow("> alpha beta gamma delta epsilon", 20),
            "│ alpha beta gamma\n│ delta epsilon\n"
        );
        assert_eq!(
            narrow("1. alpha beta gamma delta", 20),
            "1. alpha beta gamma\n   delta\n"
        );
    }

    #[test]
    fn hyperlinks_split_by_a_wrap_are_reopened_on_the_next_line() {
        let columns = NonZeroU16::new(12).expect("nonzero width");
        let options = RenderOptions::for_terminal(columns, true, None);
        let styled = render("see [the linked docs](https://x.test)", &options);
        let lines: Vec<&str> = styled.lines().collect();
        assert_eq!(lines.len(), 2, "{styled:?}");
        for line in lines {
            assert_eq!(
                line.matches("\x1b]8;;https://x.test\x1b\\").count(),
                1,
                "{line:?}"
            );
            assert!(line.ends_with("\x1b]8;;\x1b\\"), "{line:?}");
        }
    }

    #[test]
    fn wide_tables_shrink_their_columns_and_wrap_cells() {
        let table = "| Name | Notes |\n| --- | --- |\n| A | one two three four five six seven eight |\n| Bee | x |";
        let rendered = narrow(table, 30);
        assert_eq!(
            rendered,
            "┌──────┬─────────────────────┐\n\
             │ Name │ Notes               │\n\
             ├──────┼─────────────────────┤\n\
             │ A    │ one two three four  │\n\
             │      │ five six seven      │\n\
             │      │ eight               │\n\
             ├──────┼─────────────────────┤\n\
             │ Bee  │ x                   │\n\
             └──────┴─────────────────────┘\n"
        );
        let long_words = narrow(
            "| a | b |\n|---|---|\n| abcdefghijklmnop | qrstuvwxyz0123 |",
            20,
        );
        for line in long_words.lines() {
            assert_eq!(line.width(), 20, "{long_words}");
        }
    }

    #[test]
    fn tables_too_wide_for_a_grid_print_one_record_per_row() {
        let table = "| A | B | C | D | E |\n|---|---|---|---|---|\n| 1 | 2 | 3 | 4 | 5 alpha beta gamma |\n| 6 | 7 | 8 | 9 | 0 |";
        assert_eq!(
            narrow(table, 20),
            "A: 1\nB: 2\nC: 3\nD: 4\nE: 5 alpha beta\ngamma\n\nA: 6\nB: 7\nC: 8\nD: 9\nE: 0\n"
        );
    }
}
