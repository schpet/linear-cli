use std::num::NonZeroU16;

use linear_cli::platform::markdown_terminal::{HostSource, RenderOptions, render};

fn options(columns: u16, color: bool, format: Option<&str>) -> RenderOptions {
    RenderOptions::for_terminal(
        NonZeroU16::new(columns).unwrap(),
        color,
        format,
        HostSource::Fixed("example-host".to_owned()),
    )
}

#[test]
fn structural_markdown_and_width_are_readable() {
    let document = "# Heading\n\n1. First\n2. Second\n\n- Alpha\n- Beta\n\n> Quoted\n\n`code` and [link](https://example.com)\n\n```rs\nlet x = 1;\n```\n\n| A | B |\n| --- | ---: |\n| 猫 | 2 |\n\n---";
    for columns in [40, 120] {
        let output = render(document, &options(columns, false, None)).unwrap();
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
        render("---", &options(40, false, None)).unwrap(),
        format!("{}\n", "_".repeat(40))
    );
    assert_eq!(
        render("---", &options(120, false, None)).unwrap(),
        format!("{}\n", "_".repeat(80))
    );
}

#[test]
fn color_controls_styles_and_image_hyperlinks() {
    let markdown = "# Heading\n\n![alt](picture.png)";
    let colored = render(markdown, &options(80, true, Some("default"))).unwrap();
    let plain = render(markdown, &options(80, false, Some("default"))).unwrap();
    assert!(colored.contains("\x1b["));
    assert!(colored.contains("\x1b]8;;file://example-hostpicture.png\x1b\\"));
    assert!(!plain.contains("\x1b["));
    assert!(!plain.contains("\x1b]8;;"));
}

#[test]
fn image_url_encoding_and_template_replace_first_only() {
    let local = render(
        "![alt](a%20b#c?d-é.png)",
        &options(80, true, Some("https://{host}/{path}/{host}/{path}")),
    )
    .unwrap();
    assert!(
        local.contains("\x1b]8;;https://example-host/a%2520b%23c?d-%C3%A9.png/{host}/{path}\x1b\\"),
        "{local:?}"
    );

    let remote = render(
        "![alt](https://example.com/a%20b#c)",
        &options(80, true, Some("default")),
    )
    .unwrap();
    assert!(remote.contains("\x1b]8;;https://example.com/a%20b#c\x1b\\"));
    assert!(!remote.contains("example-host"));
}

#[test]
fn control_input_is_sanitized_and_deep_nesting_is_bounded() {
    let text = render("hi\u{1b}[31m", &options(80, false, None)).unwrap();
    assert!(text.contains("hi�[31m"));
    let allowed = format!("{}x", "> ".repeat(64));
    assert!(render(&allowed, &options(80, false, None)).is_ok());
    let rejected = format!("{}x", "> ".repeat(65));
    let error = render(&rejected, &options(80, false, None)).unwrap_err();
    assert_eq!(
        error.message(),
        "Markdown nesting deeper than 64 levels cannot be rendered"
    );
}
