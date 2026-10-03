//! The colors offered when a label or initiative is created interactively.
use crate::cli::values::hex_color;
use crate::error::Result;
use crate::platform::prompt::{Prompter, Text};

/// Linear's default label color.
pub const INDIGO: &str = "#5E6AD2";

/// Named colors, in menu order.
pub const PALETTE: [(&str, &str); 10] = [
    ("Red", "#EB5757"),
    ("Orange", "#F2994A"),
    ("Yellow", "#F2C94C"),
    ("Green", "#27AE60"),
    ("Teal", "#0D9488"),
    ("Blue", "#2F80ED"),
    ("Indigo", INDIGO),
    ("Purple", "#8B5CF6"),
    ("Pink", "#BB6BD9"),
    ("Gray", "#6B6F76"),
];

/// The menu label for a palette color: `Red (#EB5757)`.
pub fn label(name: &str, hex: &str) -> String {
    format!("{name} ({hex})")
}

/// Asks for a hex color outside the palette.
pub fn custom(prompter: &Prompter<'_>) -> Result<String> {
    Ok(prompter
        .parsed(
            Text::new("Enter hex color (e.g., #FF5733):").required(),
            &hex_color,
        )?
        .expect("a required answer is never blank"))
}
