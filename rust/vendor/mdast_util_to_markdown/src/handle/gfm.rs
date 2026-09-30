//! Default GFM handlers, adapted from the MIT syntax-tree extensions in NOTICE.
use super::Handle;
use crate::{
    construct_name::ConstructName,
    state::{Info, State},
    util::safe::SafeConfig,
};
use alloc::{
    format,
    string::{String, ToString},
    vec::Vec,
};
use markdown::{
    mdast::{
        AlignKind, Delete, FootnoteDefinition, FootnoteReference, Node, Table, TableCell, TableRow,
    },
    message::Message,
};

impl Handle for Delete {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<String, Message> {
        state.enter(ConstructName::Strikethrough);
        let value = state.container_phrasing(node, &Info::new("~~", "~"))?;
        state.exit();
        Ok(format!("~~{}~~", value))
    }
}

impl Handle for FootnoteReference {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<String, Message> {
        state.enter(ConstructName::FootnoteReference);
        state.enter(ConstructName::Reference);
        let label = state.safe(&state.association(self), &SafeConfig::new("[^", "]", None));
        state.exit();
        state.exit();
        Ok(format!("[^{}]", label))
    }
}

impl Handle for FootnoteDefinition {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<String, Message> {
        state.enter(ConstructName::FootnoteDefinition);
        state.enter(ConstructName::Label);
        let label = state.safe(&state.association(self), &SafeConfig::new("[^", "]", None));
        state.exit();
        let content = state.container_flow(node)?;
        let mut value = format!("[^{}]:", label);
        if !content.is_empty() {
            value.push(' ');
            value.push_str(&state.indent_lines(&content, |line, index, blank| {
                if index == 0 || blank {
                    line.to_string()
                } else {
                    format!("    {}", line)
                }
            }));
        }
        state.exit();
        Ok(value)
    }
}

impl Handle for TableCell {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<String, Message> {
        state.enter(ConstructName::TableCell);
        state.enter(ConstructName::Phrasing);
        let value = state.container_phrasing(node, &Info::new("|", "|"))?;
        state.exit();
        state.exit();
        Ok(value)
    }
}

fn row_cells(state: &mut State, node: &Node) -> Result<Vec<String>, Message> {
    assert!(matches!(node, Node::TableRow(_)), "Expected table row");
    state.enter(ConstructName::TableRow);
    let cells = node
        .children()
        .expect("Table row children")
        .iter()
        .map(|cell| {
            assert!(matches!(cell, Node::TableCell(_)), "Expected table cell");
            state.handle(cell, &Info::new("", ""), Some(node))
        })
        .collect::<Result<Vec<_>, _>>()?;
    state.exit();
    Ok(cells)
}

// markdown-table's default width is JS string.length (UTF-16), not display width.
fn render_table(rows: &[Vec<String>], align: &[AlignKind]) -> String {
    let columns = rows
        .iter()
        .map(Vec::len)
        .max()
        .unwrap_or(0)
        .max(align.len());
    let mut widths = alloc::vec![0; columns];
    for row in rows {
        for (index, cell) in row.iter().enumerate() {
            widths[index] = widths[index].max(cell.encode_utf16().count());
        }
    }
    let mut delimiters = Vec::with_capacity(columns);
    for (index, width) in widths.iter_mut().enumerate() {
        let alignment = align.get(index).unwrap_or(&AlignKind::None);
        let left = matches!(alignment, AlignKind::Left | AlignKind::Center);
        let right = matches!(alignment, AlignKind::Right | AlignKind::Center);
        let hyphens = width
            .saturating_sub(usize::from(left) + usize::from(right))
            .max(1);
        let delimiter = format!(
            "{}{}{}",
            if left { ":" } else { "" },
            "-".repeat(hyphens),
            if right { ":" } else { "" }
        );
        *width = (*width).max(delimiter.len());
        delimiters.push(delimiter);
    }
    let mut result = Vec::new();
    for (row_index, row) in rows.iter().enumerate() {
        let mut cells = Vec::with_capacity(columns);
        for (index, width) in widths.iter().enumerate() {
            let cell = row.get(index).map(String::as_str).unwrap_or("");
            let size = width.saturating_sub(cell.encode_utf16().count());
            let before = match align.get(index).unwrap_or(&AlignKind::None) {
                AlignKind::Right => size,
                AlignKind::Center => size.div_ceil(2),
                AlignKind::None | AlignKind::Left => 0,
            };
            cells.push(format!(
                "{}{}{}",
                " ".repeat(before),
                cell,
                " ".repeat(size - before)
            ));
        }
        result.push(format!("| {} |", cells.join(" | ")));
        if row_index == 0 {
            result.push(format!("| {} |", delimiters.join(" | ")));
        }
    }
    result.join("\n")
}

impl Handle for Table {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        _node: &Node,
    ) -> Result<String, Message> {
        state.enter(ConstructName::Table);
        let rows = self
            .children
            .iter()
            .map(|row| row_cells(state, row))
            .collect::<Result<Vec<_>, _>>()?;
        let value = render_table(&rows, &self.align);
        state.exit();
        Ok(value)
    }
}

impl Handle for TableRow {
    fn handle(
        &self,
        state: &mut State,
        _info: &Info,
        _parent: Option<&Node>,
        node: &Node,
    ) -> Result<String, Message> {
        let cells = row_cells(state, node)?;
        Ok(render_table(&[cells], &[])
            .lines()
            .next()
            .unwrap_or("")
            .to_string())
    }
}
