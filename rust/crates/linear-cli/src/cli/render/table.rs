use unicode_width::UnicodeWidthStr;

use crate::error::{AppError, AppErrorKind};

fn invariant(message: &str) -> AppError {
    AppError::new(AppErrorKind::Invariant, message)
}

fn ansi_end(text: &str, start: usize) -> Option<usize> {
    let tail = text.get(start..)?;
    let tail = tail.strip_prefix("\x1b[")?;
    let digits = tail
        .bytes()
        .take_while(|byte| byte.is_ascii_digit() || *byte == b';')
        .count();
    if tail.as_bytes().get(digits) == Some(&b'm') {
        Some(start + 2 + digits + 1)
    } else {
        None
    }
}

fn visible(text: &str) -> String {
    let mut out = String::new();
    let mut position = 0;
    while position < text.len() {
        if let Some(end) = ansi_end(text, position) {
            position = end;
            continue;
        }
        if let Some(ch) = text.get(position..).and_then(|tail| tail.chars().next()) {
            out.push(ch);
            position += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

fn width(text: &str) -> usize {
    UnicodeWidthStr::width(visible(text).as_str())
}

fn consume_words(limit: usize, text: &str) -> String {
    let mut consumed = String::new();
    for (index, word) in text.split('\n').next().unwrap_or("").split(' ').enumerate() {
        if !consumed.is_empty() && width(&consumed) + width(word) >= limit {
            break;
        }
        if index > 0 {
            consumed.push(' ');
        }
        consumed.push_str(word);
    }
    consumed
}

fn consume_chars(limit: usize, text: &str) -> String {
    let mut result = String::new();
    let mut position = 0;
    let line = text.split('\n').next().unwrap_or("");
    while position < line.len() {
        if let Some(end) = ansi_end(line, position) {
            if let Some(code) = line.get(position..end) {
                result.push_str(code);
            }
            position = end;
            continue;
        }
        let Some(ch) = line.get(position..).and_then(|tail| tail.chars().next()) else {
            break;
        };
        if !result.is_empty()
            && width(&result) + UnicodeWidthStr::width(ch.encode_utf8(&mut [0; 4])) > limit
        {
            break;
        }
        result.push(ch);
        position += ch.len_utf8();
    }
    result
}

fn ansi_kind(code: &str) -> Option<(&'static str, bool)> {
    match code {
        "1" | "2" => Some(("22", true)),
        "22" => Some(("22", false)),
        "3" => Some(("23", true)),
        "23" => Some(("23", false)),
        "4" => Some(("24", true)),
        "24" => Some(("24", false)),
        "7" => Some(("27", true)),
        "27" => Some(("27", false)),
        "8" => Some(("28", true)),
        "28" => Some(("28", false)),
        "9" => Some(("29", true)),
        "29" => Some(("29", false)),
        "39" => Some(("39", false)),
        "0" => Some(("0", false)),
        _ => match code.parse::<u8>() {
            Ok(30..=37 | 90..=97) => Some(("39", true)),
            _ => None,
        },
    }
}

fn close_runs(text: &str) -> Result<(String, String), AppError> {
    let mut runs: Vec<(&str, String)> = Vec::new();
    let mut position = 0;
    while position < text.len() {
        if let Some(end) = ansi_end(text, position) {
            let code = text
                .get(position + 2..end - 1)
                .ok_or_else(|| invariant("invalid ANSI span"))?;
            let (kind, open) =
                ansi_kind(code).ok_or_else(|| invariant("unsupported ANSI style in help table"))?;
            runs.retain(|(previous, _)| *previous != kind);
            if open {
                runs.push((kind, code.to_owned()));
            }
            position = end;
        } else {
            let ch = text
                .get(position..)
                .and_then(|tail| tail.chars().next())
                .ok_or_else(|| invariant("invalid table character"))?;
            position += ch.len_utf8();
        }
    }
    let suffix = runs
        .iter()
        .rev()
        .map(|(kind, _)| format!("\x1b[{kind}m"))
        .collect::<String>();
    let prefix = runs
        .iter()
        .map(|(_, code)| format!("\x1b[{code}m"))
        .collect::<String>();
    Ok((suffix, prefix))
}

#[derive(Clone)]
struct Cell {
    rest: String,
    ansi_prefix: String,
}

pub fn render(
    rows: &[Vec<String>],
    max_widths: &[usize],
    paddings: &[usize],
    indent: usize,
) -> Result<String, AppError> {
    if rows.is_empty() {
        return Ok(String::new());
    }
    let columns = rows.iter().map(Vec::len).max().unwrap_or(0);
    if max_widths.len() != columns || paddings.len() != columns {
        return Err(invariant("table column settings do not match row width"));
    }
    let mut widths = Vec::with_capacity(columns);
    for column in 0..columns {
        let cap = *max_widths
            .get(column)
            .ok_or_else(|| invariant("missing table maximum width"))?;
        let widest = rows
            .iter()
            .flat_map(|row| row.get(column).map_or("", String::as_str).split('\n'))
            .map(|line| width(&consume_words(cap, line)))
            .max()
            .unwrap_or(0);
        widths.push(widest.min(cap));
    }
    let mut output = String::new();
    for row in rows {
        let mut cells = row
            .iter()
            .map(|value| Cell {
                rest: value.clone(),
                ansi_prefix: String::new(),
            })
            .collect::<Vec<_>>();
        loop {
            output.push_str(&" ".repeat(indent));
            for column in 0..columns {
                let cell = cells
                    .get_mut(column)
                    .ok_or_else(|| invariant("missing table cell"))?;
                let col_width = *widths
                    .get(column)
                    .ok_or_else(|| invariant("missing table width"))?;
                let length = col_width.min(width(&cell.rest));
                let mut words = consume_words(length, &cell.rest);
                let break_word = width(&words) > length;
                if break_word {
                    words = consume_chars(length, &words);
                }
                let consumed = words.len() + usize::from(!break_word);
                cell.rest = cell.rest.get(consumed..).unwrap_or("").to_owned();
                words.insert_str(0, &cell.ansi_prefix);
                let (suffix, prefix) = close_runs(&words)?;
                words.push_str(&suffix);
                cell.ansi_prefix = prefix;
                let fill = col_width.saturating_sub(width(&words));
                output.push_str(&words);
                output.push_str(&" ".repeat(fill));
                if column + 1 < columns {
                    output.push_str(
                        &" ".repeat(
                            *paddings
                                .get(column)
                                .ok_or_else(|| invariant("missing table padding"))?,
                        ),
                    );
                }
            }
            output.push('\n');
            if cells.iter().all(|cell| cell.rest.is_empty()) {
                break;
            }
        }
    }
    output.pop();
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::render;

    #[test]
    fn wraps_before_equal_width_and_pads_each_column() {
        let rows = vec![vec!["one two three".to_owned(), "x".to_owned()]];
        let result = render(&rows, &[7, 1], &[2, 0], 2);
        assert!(matches!(result, Ok(ref output) if output == "  one two  x\n  three     "));
    }

    #[test]
    fn closes_and_reopens_ansi_across_visual_rows() {
        let rows = vec![vec!["\x1b[31mabcdef\x1b[39m".to_owned()]];
        let result = render(&rows, &[3], &[0], 0);
        assert!(
            matches!(result, Ok(ref output) if output == "\x1b[31mabc\x1b[39m\n\x1b[31mdef\x1b[39m")
        );
    }
}
