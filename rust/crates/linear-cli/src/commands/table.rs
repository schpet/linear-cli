//! Shared table presentation primitives for list commands.

pub fn terminal_color(color: &str) -> Option<String> {
    let hex = color.strip_prefix('#')?;
    let rgb = match hex.len() {
        6 => {
            let red = u8::from_str_radix(hex.get(0..2)?, 16).ok()?;
            let green = u8::from_str_radix(hex.get(2..4)?, 16).ok()?;
            let blue = u8::from_str_radix(hex.get(4..6)?, 16).ok()?;
            (red, green, blue)
        }
        3 => {
            let red = u8::from_str_radix(hex.get(0..1)?, 16).ok()? * 17;
            let green = u8::from_str_radix(hex.get(1..2)?, 16).ok()? * 17;
            let blue = u8::from_str_radix(hex.get(2..3)?, 16).ok()? * 17;
            (red, green, blue)
        }
        _ => return None,
    };
    Some(format!("\x1b[38;2;{};{};{}m", rgb.0, rgb.1, rgb.2))
}

/// Render already-padded header cells with the frozen per-cell underline codes.
pub fn underlined_header(cells: &[String], color: bool) -> String {
    if !color {
        return format!("{}\n", cells.join(" "));
    }
    let mut line = String::new();
    for (index, cell) in cells.iter().enumerate() {
        if index > 0 {
            line.push(' ');
        }
        line.push_str("\x1b[4m");
        line.push_str(cell);
        line.push_str(if index + 1 == cells.len() {
            "\x1b[0m"
        } else {
            "\x1b[24m"
        });
    }
    line.push('\n');
    line
}

pub fn stdout_columns(is_terminal: bool) -> usize {
    if !is_terminal {
        return 120;
    }
    if let Some((terminal_size::Width(width), _)) =
        terminal_size::terminal_size_of(std::io::stdout())
    {
        return usize::from(width);
    }
    0
}

#[cfg(test)]
mod tests {
    use super::{stdout_columns, terminal_color, underlined_header};

    #[test]
    fn rgb_color_and_underlined_header_preserve_control_bytes() {
        assert_eq!(
            terminal_color("#abc").as_deref(),
            Some("\x1b[38;2;170;187;204m")
        );
        assert_eq!(
            terminal_color("#010203").as_deref(),
            Some("\x1b[38;2;1;2;3m")
        );
        assert_eq!(terminal_color("abc"), None);
        let cells = ["KEY".to_owned(), "NAME ".to_owned()];
        assert_eq!(underlined_header(&cells, false), "KEY NAME \n");
        assert_eq!(
            underlined_header(&cells, true),
            "\x1b[4mKEY\x1b[24m \x1b[4mNAME \x1b[0m\n"
        );
    }

    #[test]
    fn non_terminal_width_is_fixed() {
        assert_eq!(stdout_columns(false), 120);
    }
}
