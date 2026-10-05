//! The table every list command prints.
//!
//! Columns are as wide as their widest cell, measured in terminal columns.
//! When stdout is a terminal narrower than the table, the flexible columns
//! give up width (widest first, never below [`FLEX_MIN`]) and their cells are
//! cut at a character boundary with `…`. If that is not enough, droppable
//! columns are hidden, lowest rank first, and as a last resort flexible
//! columns shrink to [`FLEX_LAST_RESORT`]. Piped output is never truncated.
use std::borrow::Cow;

use crate::ctx::Ctx;
use crate::platform::{style, terminal_text};
use unicode_width::UnicodeWidthChar;

/// Flexible columns keep at least this many terminal columns (or their full
/// width when narrower) so their values stay recognizable.
pub const FLEX_MIN: usize = 12;

/// How far flexible columns shrink when hiding columns was not enough.
pub const FLEX_LAST_RESORT: usize = 6;

const GUTTER: &str = "  ";

type Painter = Box<dyn Fn(&str, bool) -> String>;

pub struct Column {
    header: &'static str,
    flexible: bool,
    drop_rank: Option<u8>,
}

impl Column {
    /// A column that always shows its cells in full.
    pub fn fixed(header: &'static str) -> Self {
        Self {
            header,
            flexible: false,
            drop_rank: None,
        }
    }

    /// A column that shrinks when the table is wider than the terminal.
    pub fn flexible(header: &'static str) -> Self {
        Self {
            header,
            flexible: true,
            drop_rank: None,
        }
    }

    /// Hidden when the terminal is too narrow for the table; columns with a
    /// lower `rank` are hidden first.
    pub fn droppable(self, rank: u8) -> Self {
        Self {
            drop_rank: Some(rank),
            ..self
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
            text: cell_text(text.into()),
            paint: Some(Box::new(paint)),
        }
    }
}

impl<T: Into<String>> From<T> for Cell {
    fn from(text: T) -> Self {
        Self {
            text: cell_text(text.into()),
            paint: None,
        }
    }
}

/// Cell text on one line with no control characters, so a remote value can
/// neither break the row nor reach the terminal as an escape sequence.
fn cell_text(text: String) -> String {
    match terminal_text::single_line(&text) {
        Cow::Borrowed(_) => text,
        Cow::Owned(clean) => clean,
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
            let row: Vec<(&Cell, usize)> = row
                .iter()
                .zip(&widths)
                .filter_map(|(cell, width)| width.map(|width| (cell, width)))
                .collect();
            // Cells after the last non-empty one would only add trailing space.
            let shown = row
                .iter()
                .rposition(|(cell, _)| !cell.text.is_empty())
                .map_or(0, |last| last + 1);
            for (index, &(cell, width)) in row.iter().take(shown).enumerate() {
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

    /// Each column's width, or `None` for a column hidden to fit `available`.
    fn widths(&self, available: Option<usize>) -> Vec<Option<usize>> {
        let mut natural: Vec<usize> = self
            .columns
            .iter()
            .map(|column| display_width(column.header))
            .collect();
        for row in &self.rows {
            for (width, cell) in natural.iter_mut().zip(row) {
                *width = (*width).max(display_width(&cell.text));
            }
        }
        let Some(available) = available else {
            return natural.into_iter().map(Some).collect();
        };
        let mut shown = vec![true; self.columns.len()];
        loop {
            let (widths, fits) = self.shrink(&natural, &shown, available, FLEX_MIN);
            if fits {
                return widths;
            }
            let next = self
                .columns
                .iter()
                .enumerate()
                .filter(|(index, _)| shown.get(*index).copied().unwrap_or(false))
                .filter_map(|(index, column)| column.drop_rank.map(|rank| (rank, index)))
                .min();
            match next.and_then(|(_, index)| shown.get_mut(index)) {
                Some(slot) => *slot = false,
                None => break,
            }
        }
        self.shrink(&natural, &shown, available, FLEX_LAST_RESORT).0
    }

    /// The shown columns' widths after flexible columns give up width (widest
    /// first, down to `floor`) to fit `available`, and whether they fit.
    fn shrink(
        &self,
        natural: &[usize],
        shown: &[bool],
        available: usize,
        floor: usize,
    ) -> (Vec<Option<usize>>, bool) {
        let mut widths: Vec<Option<usize>> = natural
            .iter()
            .zip(shown)
            .map(|(&width, &shown)| shown.then_some(width))
            .collect();
        let count = widths.iter().flatten().count();
        let gutters = GUTTER.len() * count.saturating_sub(1);
        let mut excess =
            (widths.iter().flatten().sum::<usize>() + gutters).saturating_sub(available);
        let floors: Vec<usize> = natural
            .iter()
            .zip(&self.columns)
            .map(|(&width, column)| {
                if column.flexible {
                    width.min(floor)
                } else {
                    width
                }
            })
            .collect();
        while excess > 0 {
            let widest = widths
                .iter_mut()
                .zip(&floors)
                .filter_map(|(width, floor)| width.as_mut().filter(|width| **width > *floor))
                .max_by_key(|width| **width);
            let Some(width) = widest else { break };
            *width -= 1;
            excess -= 1;
        }
        (widths, excess == 0)
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
    crate::platform::pager::stdout_size().map(|size| usize::from(size.columns))
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
    fn remote_cell_text_stays_on_one_line_without_escape_sequences() {
        let mut table = Table::new([Column::fixed("TITLE"), Column::fixed("ID")]);
        table.row([
            Cell::from("Fix\nthe \u{1b}[31mbug\u{1b}[0m"),
            Cell::styled("t\r1", style::gray),
        ]);
        assert_eq!(
            table.render(None, false),
            "TITLE                 ID\n\
             Fix the \u{FFFD}[31mbug\u{FFFD}[0m  t 1\n"
        );
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
            "KEY  {:12}  ID\nENG  {:12}  t1\nOPS  {:12}  t2\n",
            "NAME", "Engineering…", "Operations"
        );
        assert_eq!(sample().render(Some(21), false), fitted);
        let fitted = format!(
            "KEY  {:15}  ID\nENG  {:15}  t1\nOPS  {:15}  t2\n",
            "NAME", "Engineering an…", "Operations"
        );
        assert_eq!(sample().render(Some(24), false), fitted);
        assert_eq!(
            sample().render(Some(80), false),
            sample().render(None, false)
        );
    }

    #[test]
    fn narrow_terminals_hide_droppable_columns_lowest_rank_first() {
        let mut table = Table::new([
            Column::fixed("ID"),
            Column::flexible("TITLE"),
            Column::fixed("STATE").droppable(2),
            Column::fixed("UPDATED").droppable(1),
        ]);
        table.row([
            Cell::from("ENG-1"),
            Cell::from("A title that is long enough to shrink"),
            Cell::from("In Progress"),
            Cell::from("3 days ago"),
        ]);
        let all = table.render(None, false);
        assert!(all.contains("UPDATED"), "{all}");
        let without_updated = table.render(Some(40), false);
        assert_eq!(
            without_updated,
            "ID     TITLE                 STATE\n\
             ENG-1  A title that is lon…  In Progress\n"
        );
        let only_title = table.render(Some(25), false);
        assert_eq!(only_title, "ID     TITLE\nENG-1  A title that is l…\n");
        // When nothing more can be hidden, flexible columns shrink further.
        let squeezed = table.render(Some(13), false);
        assert_eq!(squeezed, "ID     TITLE\nENG-1  A tit…\n");
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
