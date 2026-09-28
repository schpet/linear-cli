use std::num::NonZeroU16;

use linear_cli::config::NoColor;
use linear_cli::platform::markdown_terminal::{HostSource, RenderOptions, render};

fn options(columns: u16, no_color: NoColor, format: Option<&str>) -> RenderOptions {
    RenderOptions::for_terminal(
        NonZeroU16::new(columns).unwrap(),
        no_color,
        true,
        format,
        HostSource::Fixed("example-host".to_owned()),
    )
}

#[test]
fn structural_markdown_and_width_are_readable() {
    let document = "# Heading\n\n1. First\n2. Second\n\n- Alpha\n- Beta\n\n> Quoted\n\n`code` and [link](https://example.com)\n\n```rs\nlet x = 1;\n```\n\n| A | B |\n| --- | ---: |\n| 猫 | 2 |\n\n---";
    for columns in [40, 120] {
        let output = render(document, &options(columns, NoColor::Nonempty, None)).unwrap();
        for fragment in [
            "# Heading",
            "1. First",
            "2. Second",
            "- Alpha",
            "┃ Quoted",
            " code ",
            "[link](https://example.com)",
            "codeblock [rs]",
            "let x = 1;",
            "猫",
            "│",
            "─",
        ] {
            assert!(
                output.contains(fragment),
                "missing {fragment:?}: {output:?}"
            );
        }
        assert!(output.ends_with('\n'));
        assert!(!output.contains("\x1b["));
    }
}

#[test]
fn thematic_break_uses_terminal_width_up_to_eighty_columns() {
    assert_eq!(
        render("---", &options(40, NoColor::Nonempty, None)).unwrap(),
        format!("{}\n", "_".repeat(40))
    );
    assert_eq!(
        render("---", &options(120, NoColor::Nonempty, None)).unwrap(),
        format!("{}\n", "_".repeat(80))
    );
}

#[test]
fn no_color_has_three_distinct_process_states() {
    let markdown = "# Heading\n\n![alt](picture.png)";
    let absent = render(markdown, &options(80, NoColor::Absent, Some("default"))).unwrap();
    let empty = render(markdown, &options(80, NoColor::Empty, Some("default"))).unwrap();
    let nonempty = render(markdown, &options(80, NoColor::Nonempty, Some("default"))).unwrap();
    assert!(absent.contains("\x1b["));
    assert!(absent.contains("\x1b]8;;file://example-hostpicture.png\x1b\\"));
    assert!(empty.contains("\x1b["));
    assert!(!empty.contains("\x1b]8;;"));
    assert!(!nonempty.contains("\x1b["));
    assert!(!nonempty.contains("\x1b]8;;"));
}

#[test]
fn image_url_encoding_and_template_replace_first_only() {
    let local = render(
        "![alt](a%20b#c?d-é.png)",
        &options(
            80,
            NoColor::Absent,
            Some("https://{host}/{path}/{host}/{path}"),
        ),
    )
    .unwrap();
    assert!(
        local.contains("\x1b]8;;https://example-host/a%2520b%23c?d-%C3%A9.png/{host}/{path}\x1b\\"),
        "{local:?}"
    );

    let remote = render(
        "![alt](https://example.com/a%20b#c)",
        &options(80, NoColor::Absent, Some("default")),
    )
    .unwrap();
    assert!(remote.contains("\x1b]8;;https://example.com/a%20b#c\x1b\\"));
    assert!(!remote.contains("example-host"));
}

#[test]
fn control_input_is_sanitized_and_deep_nesting_is_bounded() {
    let text = render("hi\u{1b}[31m", &options(80, NoColor::Nonempty, None)).unwrap();
    assert!(text.contains("hi�[31m"));
    let allowed = format!("{}x", "> ".repeat(64));
    assert!(render(&allowed, &options(80, NoColor::Nonempty, None)).is_ok());
    let rejected = format!("{}x", "> ".repeat(65));
    let error = render(&rejected, &options(80, NoColor::Nonempty, None)).unwrap_err();
    assert_eq!(error.kind, linear_cli::error::AppErrorKind::Validation);
    assert_eq!(
        error.message,
        "Markdown nesting deeper than 64 levels cannot be rendered"
    );
}
