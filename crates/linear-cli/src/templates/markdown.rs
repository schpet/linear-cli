use std::fmt::Write as _;

/// Keep display text literal and keep its destination inside one Markdown link.
pub(crate) fn push_markdown_link(markdown: &mut String, label: &str, url: &str) {
    markdown.push('[');
    push_markdown_label(markdown, label);
    markdown.push_str("](<");
    for ch in url.chars() {
        if matches!(ch, '&' | '<' | '>' | '\\') || ch.is_whitespace() {
            push_entity(markdown, ch);
        } else {
            markdown.push(ch);
        }
    }
    markdown.push_str(">)");
}

pub(crate) fn push_markdown_label(markdown: &mut String, label: &str) {
    let content = label.trim_matches(' ');
    let start = label.len() - label.trim_start_matches(' ').len();
    let end = start + content.len();
    for (index, ch) in label.char_indices() {
        match ch {
            // Keep padding inside the label or emphasis delimiters.
            ' ' if index < start || index >= end => push_entity(markdown, ch),
            '!' | '&' | '\\' | '[' | ']' | '<' | '>' | '*' | '_' | '~' | '`' => {
                markdown.push('\\');
                markdown.push(ch);
            }
            _ => markdown.push(ch),
        }
    }
}

fn push_entity(markdown: &mut String, ch: char) {
    write!(markdown, "&#{};", u32::from(ch)).expect("writing to a String cannot fail");
}
