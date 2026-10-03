//! The table every list command prints.
//!
//! Columns are as wide as their widest cell, measured in terminal columns.
//! When stdout is a terminal narrower than the table, the flexible columns
//! give up width (widest first, never below [`FLEX_MIN`]) and their cells are
//! cut at a character boundary with `…`. Piped output is never truncated.
use crate::ctx::Ctx;
use crate::platform::style;
use unicode_width::UnicodeWidthChar;

/// Flexible columns keep at least this many terminal columns (or their full
/// width when narrower) so their values stay recognizable.
pub const FLEX_MIN: usize = 20;

const GUTTER: &str = "  ";

type Painter = Box<dyn Fn(&str, bool) -> String>;

pub struct Column {
    header: &'static str,
    flexible: bool,
}

impl Column {
    /// A column that always shows its cells in full.
    pub fn fixed(header: &'static str) -> Self {
        Self {
            header,
            flexible: false,
        }
    }

    /// A column that shrinks when the table is wider than the terminal.
    pub fn flexible(header: &'static str) -> Self {
        Self {
            header,
            flexible: true,
        }
    }
}

pub struct Cell {
    text: String,
    paint: Option<Painter>,
}

impl Cell {
    pub fn styled(text: impl Into<String>, paint: impl Fn(&str, bool) -> String + 'static) -> Self {
        Self {
            text: text.into(),
            paint: Some(Box::new(paint)),
        }
    }
}

impl<T: Into<String>> From<T> for Cell {
    fn from(text: T) -> Self {
        Self {
            text: text.into(),
            paint: None,
        }
    }
}

pub struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<Cell>>,
}

impl Table {
    pub fn new(columns: impl IntoIterator<Item = Column>) -> Self {
        Self {
            columns: columns.into_iter().collect(),
            rows: Vec::new(),
        }
    }

    pub fn row(&mut self, cells: impl IntoIterator<Item = Cell>) {
        let cells: Vec<Cell> = cells.into_iter().collect();
        assert_eq!(
            cells.len(),
            self.columns.len(),
            "every row has one cell per column"
        );
        self.rows.push(cells);
    }

    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// The table for stdout: fitted to the terminal when stdout is one.
    pub fn render_for(&self, ctx: &Ctx) -> String {
        self.render(stdout_width(ctx), ctx.color())
    }

    /// Every row as one line. `width` is the terminal width to fit, or `None`
    /// to print every cell in full.
    pub fn render(&self, width: Option<usize>, color: bool) -> String {
        let widths = self.widths(width);
        let mut output = String::new();
        let headers: Vec<Cell> = self
            .columns
            .iter()
            .map(|column| Cell::styled(column.header, style::heading))
            .collect();
        for row in std::iter::once(&headers).chain(&self.rows) {
            // Cells after the last non-empty one would only add trailing space.
            let shown = row
                .iter()
                .rposition(|cell| !cell.text.is_empty())
                .map_or(0, |last| last + 1);
            for (index, (cell, &width)) in row.iter().zip(&widths).take(shown).enumerate() {
                if index > 0 {
                    output.push_str(GUTTER);
                }
                let text = fit(&cell.text, width, index + 1 == shown);
                match &cell.paint {
                    Some(paint) => output.push_str(&paint(&text, color)),
                    None => output.push_str(&text),
                }
            }
            output.push('\n');
        }
        output
    }

    fn widths(&self, available: Option<usize>) -> Vec<usize> {
        let mut widths: Vec<usize> = self
            .columns
            .iter()
            .map(|column| display_width(column.header))
            .collect();
        for row in &self.rows {
            for (width, cell) in widths.iter_mut().zip(row) {
                *width = (*width).max(display_width(&cell.text));
            }
        }
        let Some(available) = available else {
            return widths;
        };
        let gutters = GUTTER.len() * self.columns.len().saturating_sub(1);
        let mut excess = (widths.iter().sum::<usize>() + gutters).saturating_sub(available);
        let floors: Vec<usize> = widths
            .iter()
            .zip(&self.columns)
            .map(|(&width, column)| {
                if column.flexible {
                    width.min(FLEX_MIN)
                } else {
                    width
                }
            })
            .collect();
        while excess > 0 {
            let widest = widths
                .iter_mut()
                .zip(&floors)
                .filter(|(width, floor)| **width > **floor)
                .max_by_key(|(width, _)| **width);
            let Some((width, _)) = widest else { break };
            *width -= 1;
            excess -= 1;
        }
        widths
    }
}

/// `text` cut to `width` columns, padded to it unless it ends the line.
fn fit(text: &str, width: usize, last: bool) -> String {
    let mut text = truncate(text, width);
    if !last {
        let padding = width.saturating_sub(display_width(&text));
        text.extend(std::iter::repeat_n(' ', padding));
    }
    text
}

/// The terminal width when stdout is a terminal of known size.
pub fn stdout_width(ctx: &Ctx) -> Option<usize> {
    if !ctx.stdout_tty() {
        return None;
    }
    terminal_size::terminal_size_of(std::io::stdout())
        .map(|(terminal_size::Width(width), _)| usize::from(width))
}

/// How many terminal columns `text` occupies.
pub fn display_width(text: &str) -> usize {
    text.chars().map(|ch| ch.width().unwrap_or(0)).sum()
}

/// `text` shortened to at most `width` columns, ending in `…` when cut.
/// Characters are never split.
pub fn truncate(text: &str, width: usize) -> String {
    if display_width(text) <= width {
        return text.to_owned();
    }
    let Some(room) = width.checked_sub(1) else {
        return String::new();
    };
    let mut result = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let next = used + ch.width().unwrap_or(0);
        if next > room {
            break;
        }
        result.push(ch);
        used = next;
    }
    result.push('…');
    result
}

#[cfg(test)]
mod tests {
    use super::{Cell, Column, Table, display_width, truncate};
    use crate::platform::style;

    fn sample() -> Table {
        let mut table = Table::new([
            Column::fixed("KEY"),
            Column::flexible("NAME"),
            Column::fixed("ID"),
        ]);
        table.row([
            Cell::from("ENG"),
            Cell::from("Engineering and design"),
            Cell::from("t1"),
        ]);
        table.row([
            Cell::from("OPS"),
            Cell::from("Operations"),
            Cell::styled("t2", style::gray),
        ]);
        table
    }

    #[test]
    fn widths_count_terminal_columns() {
        assert_eq!(display_width("ab"), 2);
        assert_eq!(display_width("界"), 2);
        assert_eq!(display_width("e\u{301}"), 1);
    }

    #[test]
    fn truncation_keeps_whole_characters() {
        assert_eq!(truncate("short", 10), "short");
        assert_eq!(truncate("abcdefgh", 6), "abcde…");
        assert_eq!(truncate("界界界界", 6), "界界…");
        assert_eq!(truncate("界界", 2), "…");
        assert_eq!(truncate("abc", 0), "");
    }

    #[test]
    fn unfitted_tables_show_every_cell_without_trailing_space() {
        assert_eq!(
            sample().render(None, false),
            "KEY  NAME                    ID\n\
             ENG  Engineering and design  t1\n\
             OPS  Operations              t2\n"
        );
    }

    #[test]
    fn flexible_columns_shrink_to_the_terminal_but_not_below_the_floor() {
        let fitted = format!(
            "KEY  {:20}  ID\nENG  {:20}  t1\nOPS  {:20}  t2\n",
            "NAME", "Engineering and des…", "Operations"
        );
        assert_eq!(sample().render(Some(29), false), fitted);
        assert_eq!(sample().render(Some(10), false), fitted);
        assert_eq!(
            sample().render(Some(80), false),
            sample().render(None, false)
        );
    }

    #[test]
    fn color_styles_headers_and_painted_cells() {
        let rendered = sample().render(None, true);
        let mut lines = rendered.lines();
        assert_eq!(
            lines.next(),
            Some(
                "\u{1b}[1m\u{1b}[4mKEY\u{1b}[0m  \u{1b}[1m\u{1b}[4mNAME                  \u{1b}[0m  \u{1b}[1m\u{1b}[4mID\u{1b}[0m"
            )
        );
        assert_eq!(
            lines.nth(1),
            Some("OPS  Operations              \u{1b}[38;5;8mt2\u{1b}[0m")
        );
    }
}
