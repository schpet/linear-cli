use std::collections::BTreeMap;

use linear_cli::app::{AppContext, run, write_final_error};
use linear_cli::error::{AppError, AppErrorKind, ExitStatus};

const FROZEN_MARKDOWN_BARE: &str = include_str!("../../../../parity/runner/cases/c085-bare.json");

#[test]
fn markdown_prints_the_frozen_reference_without_credentials() {
    let case: serde_json::Value =
        serde_json::from_str(FROZEN_MARKDOWN_BARE).expect("frozen Markdown case is valid JSON");
    let expected = case["expected"]["stdout"]["utf8"]
        .as_str()
        .expect("frozen Markdown stdout is UTF-8");
    let (status, stdout, stderr) = invoke(&["markdown"]);
    assert_eq!(status, ExitStatus::Success);
    assert_eq!(stdout, expected);
    assert!(stderr.is_empty());
    assert!(stdout.contains("+++ [Server log]\n\nMarkdown content"));
    assert!(stdout.ends_with("both required.\n"));

    let (status, workspace_stdout, stderr) = invoke(&["markdown", "--workspace", "bogus"]);
    assert_eq!(status, ExitStatus::Success);
    assert_eq!(workspace_stdout, expected);
    assert!(stderr.is_empty());
}

#[cfg(unix)]
#[test]
fn markdown_closed_stdout_is_quiet() {
    use std::os::fd::OwnedFd;
    use std::os::unix::net::UnixStream;
    use std::process::Stdio;

    let (reader, writer) = UnixStream::pair().expect("create output pipe");
    drop(reader);
    let sandbox = super::startup::BinarySandbox::new();
    let output = sandbox
        .command()
        .arg("markdown")
        .stdout(Stdio::from(OwnedFd::from(writer)))
        .stderr(Stdio::piped())
        .output()
        .expect("Markdown binary runs");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
}

fn invoke(args: &[&str]) -> (ExitStatus, String, String) {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut context = AppContext {
        startup: super::startup::empty_startup(std::env::temp_dir(), &[]),
        cwd: std::env::temp_dir(),
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdout_tty: false,
        stderr_tty: false,
        stdout_finalization: None,
    };
    let args = args.iter().map(|arg| (*arg).to_owned()).collect::<Vec<_>>();
    let status = match run(&args, &mut context) {
        Ok(status) => status,
        Err(error) => write_final_error(&mut context, &error).expect("writable stderr"),
    };
    (
        status,
        String::from_utf8(stdout).expect("UTF-8 stdout"),
        String::from_utf8(stderr).expect("UTF-8 stderr"),
    )
}

#[test]
fn every_route_and_alias_reaches_its_canonical_help_boundary() {
    let mut aliases = 0;
    for route in linear_cli::cli::ROUTES {
        let canonical = route.path.split(' ').skip(1).collect::<Vec<_>>();
        for replacement in
            std::iter::once(None).chain(route.aliases.iter().map(|alias| Some(*alias)))
        {
            let mut words = canonical.clone();
            if let Some(alias) = replacement {
                aliases += 1;
                let last = words.last_mut().expect("non-root alias");
                *last = alias;
            }
            words.push("--help");
            let (status, stdout, stderr) = invoke(&words);
            assert!(
                matches!(status, ExitStatus::Success | ExitStatus::UsageFailure),
                "{} {words:?} returned {status:?}",
                route.path
            );
            assert_eq!(
                stdout,
                linear_cli::cli::render::help(route, true, status == ExitStatus::Success)
                    .expect("route help"),
                "{} {words:?} help route",
                route.path
            );
            if status == ExitStatus::Success {
                assert!(stderr.is_empty(), "{} {words:?} stderr", route.path);
            } else {
                assert!(stderr.contains("error:"), "{} {words:?} usage", route.path);
            }
        }
    }
    assert_eq!(linear_cli::cli::ROUTES.len(), 110);
    assert_eq!(aliases, 36);
}

#[test]
fn known_bare_routes_and_short_version() {
    assert_eq!(
        invoke(&[]),
        (
            ExitStatus::Success,
            "Use --help to see available commands\n".to_owned(),
            String::new()
        )
    );
    assert_eq!(
        invoke(&["docs"]),
        (
            ExitStatus::Success,
            "Use --help to see available subcommands\n".to_owned(),
            String::new()
        )
    );
    assert_eq!(
        invoke(&["-V"]),
        (
            ExitStatus::Success,
            "3.0.0-alpha.1\n".to_owned(),
            String::new()
        )
    );
}

#[test]
fn unimplemented_aliases_fail_visibly() {
    let (status, stdout, stderr) = invoke(&["issue", "list"]);
    assert_eq!(status, ExitStatus::HandledFailure);
    assert!(stdout.is_empty());
    assert!(
        stderr.contains("linear issue mine is registered, but this action is not implemented yet")
    );
}

#[test]
fn leaf_positionals_reach_the_registered_unimplemented_action() {
    let (status, stdout, stderr) = invoke(&["issue", "view", "ABC-1"]);
    assert_eq!(status, ExitStatus::HandledFailure);
    assert!(stdout.is_empty());
    assert_eq!(
        stderr,
        "✗ linear issue view is registered, but this action is not implemented yet\n"
    );
}

#[test]
fn usage_and_domain_validation_have_distinct_statuses_and_writers() {
    let (status, stdout, stderr) = invoke(&["frobnicate"]);
    assert_eq!(status, ExitStatus::UsageFailure);
    let root = linear_cli::cli::root().expect("root route");
    assert_eq!(
        stdout,
        linear_cli::cli::render::help(root, true, false).expect("root help")
    );
    assert_eq!(
        stderr,
        "\x1b[31m  \x1b[1merror\x1b[22m: Unknown command \"frobnicate\". Did you mean command \"project\"?\n\x1b[39m\n"
    );

    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut context = AppContext {
        startup: super::startup::empty_startup(std::env::temp_dir(), &[]),
        cwd: std::env::temp_dir(),
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdout_tty: false,
        stderr_tty: false,
        stdout_finalization: None,
    };
    let error = AppError::new(
        AppErrorKind::Validation,
        "Cannot specify both --body and --body-file",
    );
    assert_eq!(
        write_final_error(&mut context, &error).unwrap(),
        ExitStatus::HandledFailure
    );
    assert!(stdout.is_empty());
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "✗ Cannot specify both --body and --body-file\n"
    );
}

#[test]
fn handled_error_tty_color_wraps_complete_lines() {
    let mut stdout = Vec::new();
    let mut stderr = Vec::new();
    let mut context = AppContext {
        startup: super::startup::empty_startup(std::env::temp_dir(), &[]),
        cwd: std::env::temp_dir(),
        stdout: &mut stdout,
        stderr: &mut stderr,
        stdout_tty: true,
        stderr_tty: true,
        stdout_finalization: None,
    };
    let error = AppError::new(AppErrorKind::Auth, "missing key");
    assert_eq!(
        write_final_error(&mut context, &error).unwrap(),
        ExitStatus::HandledFailure
    );
    assert_eq!(
        stderr,
        b"\x1b[31m\xe2\x9c\x97 missing key\x1b[39m\n\x1b[90m  Run `linear auth login` to authenticate.\x1b[39m\n"
    );
}

#[cfg(unix)]
#[test]
fn unrelated_non_utf8_environment_does_not_block_short_version() {
    use std::ffi::OsString;
    use std::os::unix::ffi::OsStringExt;

    let sandbox = super::startup::BinarySandbox::new();
    let output = sandbox
        .command()
        .arg("-V")
        .env("JUNK", OsString::from_vec(vec![0xff]))
        .output()
        .expect("binary runs");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"3.0.0-alpha.1\n");
    assert!(output.stderr.is_empty());
}

#[test]
fn typed_error_context_and_suggestion_survive() {
    let error = AppError::new(AppErrorKind::Auth, "missing key")
        .with_context("load credentials")
        .with_context("Failed to view issue");
    assert_eq!(
        error.display_message(),
        "Failed to view issue: load credentials: missing key"
    );
    assert_eq!(
        error.suggestion.as_deref(),
        Some("Run `linear auth login` to authenticate.")
    );
    assert_eq!(
        AppError::not_found("Issue", "ABC-1").display_message(),
        "Issue not found: ABC-1"
    );
}

#[test]
fn empty_no_color_preserves_color_but_nonempty_disables_it() {
    for (value, expected_color) in [(None, true), (Some(""), true), (Some("1"), false)] {
        let mut stdout = Vec::new();
        let mut stderr = Vec::new();
        let mut environment = BTreeMap::new();
        if let Some(value) = value {
            environment.insert("NO_COLOR".to_owned(), value.to_owned());
        }
        let mut context = AppContext {
            startup: super::startup::empty_startup(
                std::env::temp_dir(),
                &environment
                    .iter()
                    .map(|(key, value)| (key.as_str(), value.as_str()))
                    .collect::<Vec<_>>(),
            ),
            cwd: std::env::temp_dir(),
            stdout: &mut stdout,
            stderr: &mut stderr,
            stdout_tty: false,
            stderr_tty: true,
            stdout_finalization: None,
        };
        assert_eq!(context.no_color(), !expected_color);
        assert_eq!(context.help_color(), expected_color);
        assert_eq!(context.handled_color(), expected_color);
        let error = AppError::new(AppErrorKind::Auth, "missing key");
        assert_eq!(
            write_final_error(&mut context, &error).expect("writable stderr"),
            ExitStatus::HandledFailure
        );
        let diagnostic = String::from_utf8(stderr).expect("UTF-8 stderr");
        assert_eq!(diagnostic.contains("\x1b["), expected_color);
    }
}
