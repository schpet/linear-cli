//! The line a terminal view ends with, pointing at the entity in Linear, as
//! `gh issue view` ends with the issue's GitHub URL.
use crate::platform::{markdown_terminal, style, terminal_text};

/// What a view's footer links to.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Linked {
    Issue,
    Project,
    Document,
    Initiative,
    Cycle,
}

impl Linked {
    fn noun(self) -> &'static str {
        match self {
            Self::Issue => "issue",
            Self::Project => "project",
            Self::Document => "document",
            Self::Initiative => "initiative",
            Self::Cycle => "cycle",
        }
    }
}

/// `View this <noun> on Linear: <url>`, gray and with the URL linked when
/// `styled`. Control characters in the URL cannot break out of the line or
/// the link.
pub fn footer(linked: Linked, url: &str, styled: bool) -> String {
    let url = terminal_text::single_line(url);
    let shown = if styled {
        markdown_terminal::hyperlink(&url, &url)
    } else {
        url.into_owned()
    };
    style::gray(
        &format!("View this {} on Linear: {shown}", linked.noun()),
        styled,
    )
}

/// `rendered` followed by a blank line and the footer. Only trailing line
/// feeds of `rendered` are replaced; other trailing text is kept.
pub fn append(rendered: &str, footer: &str) -> String {
    let body = rendered.trim_end_matches('\n');
    if body.is_empty() {
        format!("{footer}\n")
    } else {
        format!("{body}\n\n{footer}\n")
    }
}

#[cfg(test)]
mod tests {
    use super::{Linked, append, footer};

    #[test]
    fn plain_footer_names_the_entity_and_shows_the_url() {
        assert_eq!(
            footer(
                Linked::Cycle,
                "https://linear.app/acme/team/ENG/cycle/5",
                false
            ),
            "View this cycle on Linear: https://linear.app/acme/team/ENG/cycle/5"
        );
    }

    #[test]
    fn styled_footer_is_gray_and_links_the_url() {
        let url = "https://linear.app/acme/issue/ENG-1";
        assert_eq!(
            footer(Linked::Issue, url, true),
            format!(
                "\x1b[38;5;8mView this issue on Linear: \x1b]8;;{url}\x1b\\{url}\x1b]8;;\x1b\\\x1b[0m"
            )
        );
    }

    #[test]
    fn control_characters_in_the_url_stay_inside_the_footer() {
        let shown = footer(Linked::Project, "https://x.test/a\n\x1b]8;;evil\x07b", true);
        assert!(!shown.contains('\n'), "{shown:?}");
        assert!(!shown.contains("\x1b]8;;evil"), "{shown:?}");
        assert!(!shown.contains('\x07'), "{shown:?}");
    }

    #[test]
    fn append_separates_the_footer_with_one_blank_line() {
        for body in ["text", "text\n", "text\n\n\n"] {
            assert_eq!(append(body, "F"), "text\n\nF\n", "{body:?}");
        }
        assert_eq!(append("text  \n", "F"), "text  \n\nF\n");
        assert_eq!(append("", "F"), "F\n");
        assert_eq!(append("\n", "F"), "F\n");
    }
}
