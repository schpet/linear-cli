#[cfg(unix)]
use std::collections::BTreeMap;
use std::ffi::OsStr;
#[cfg(unix)]
use std::ffi::OsString;
#[cfg(unix)]
use std::fs;
use std::io::{self, Write};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::sync::atomic::{AtomicU64, Ordering};

use linear_cli::config::{NoColor, OsFamily};
use linear_cli::platform::markdown_terminal::HostSource;
#[cfg(unix)]
use linear_cli::platform::pager::ProcessPagerRunner;
use linear_cli::platform::pager::{
    Delivery, PagerAttempt, PagerCommand, PagerFailure, PagerRequest, PagerRunner, TerminalSize,
    fallback_commands, page, primary_command, render_and_show, should_page, show, usable_size,
};

#[derive(Default)]
struct FakeRunner {
    attempts: Vec<PagerCommand>,
    bytes: Vec<Vec<u8>>,
    outcomes: Vec<PagerAttempt>,
}

impl PagerRunner for FakeRunner {
    fn run(&mut self, command: &PagerCommand, input: &[u8]) -> PagerAttempt {
        self.attempts.push(command.clone());
        self.bytes.push(input.to_vec());
        self.outcomes.remove(0)
    }
}

struct Stdout {
    bytes: Vec<u8>,
    flushes: usize,
}

impl Write for Stdout {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        Ok(())
    }
}

fn output() -> Stdout {
    Stdout {
        bytes: Vec::new(),
        flushes: 0,
    }
}

fn request<'a>(size: Option<TerminalSize>, pager: Option<&'a str>) -> PagerRequest<'a> {
    PagerRequest {
        enabled: true,
        stdout_tty: true,
        size,
        pager: pager.map(OsStr::new),
        os: OsFamily::Unix,
    }
}

#[test]
fn threshold_is_strict_and_small_terminal_rows_do_not_underflow() {
    for rows in [0, 1, 2] {
        assert!(should_page(
            "x",
            true,
            true,
            Some(TerminalSize { columns: 80, rows })
        ));
    }
    assert!(!should_page(
        "a\nb",
        true,
        true,
        Some(TerminalSize {
            columns: 80,
            rows: 4
        })
    ));
    assert!(should_page(
        "a\nb\nc",
        true,
        true,
        Some(TerminalSize {
            columns: 80,
            rows: 4
        })
    ));
    assert!(!should_page(&"x\n".repeat(49), true, true, None));
    assert!(should_page(&"x\n".repeat(50), true, true, None));
    assert!(!should_page("x\n", false, true, None));
    assert!(!should_page("x\n", true, false, None));
}

#[test]
fn zero_size_adapter_is_unknown_even_though_pure_threshold_accepts_zero_rows() {
    for size in [
        TerminalSize {
            columns: 0,
            rows: 0,
        },
        TerminalSize {
            columns: 80,
            rows: 0,
        },
        TerminalSize {
            columns: 0,
            rows: 24,
        },
    ] {
        assert_eq!(usable_size(Some(size)), None);
    }
    assert_eq!(
        usable_size(Some(TerminalSize {
            columns: 80,
            rows: 24
        })),
        Some(TerminalSize {
            columns: 80,
            rows: 24
        })
    );
}

#[test]
fn pager_command_uses_argv_without_shell_and_exact_fallback_names() {
    assert_eq!(primary_command(None, OsFamily::Unix).program, "less");
    assert_eq!(primary_command(Some(""), OsFamily::Windows).program, "more");
    let command = primary_command(Some(" /usr/bin/less  -R  'literal arg' "), OsFamily::Unix);
    assert_eq!(command.program, "/usr/bin/less");
    assert_eq!(command.args, ["-R", "'literal", "arg'"]);
    assert_eq!(primary_command(Some("  "), OsFamily::Unix).program, "");
    assert_eq!(
        fallback_commands("less", OsFamily::Unix)
            .iter()
            .map(|c| c.program.as_str())
            .collect::<Vec<_>>(),
        ["more", "cat"]
    );
    assert_eq!(
        fallback_commands("/usr/bin/less", OsFamily::Unix)
            .iter()
            .map(|c| c.program.as_str())
            .collect::<Vec<_>>(),
        ["less", "more", "cat"]
    );
    assert_eq!(
        fallback_commands("more", OsFamily::Windows)
            .iter()
            .map(|c| c.program.as_str())
            .collect::<Vec<_>>(),
        ["less"]
    );
}

#[test]
fn close_early_falls_back_and_passes_exact_bytes() {
    let mut runner = FakeRunner {
        outcomes: vec![PagerAttempt::ClosedEarly, PagerAttempt::Completed],
        ..FakeRunner::default()
    };
    let mut stdout = output();
    let shown = page(
        "hello\n",
        PagerCommand {
            program: "less".to_owned(),
            args: vec![],
        },
        OsFamily::Unix,
        &mut runner,
        &mut stdout,
    )
    .unwrap();
    assert!(matches!(shown.delivery, Delivery::Paged(ref command) if command.program == "more"));
    assert!(matches!(shown.failures[0].1, PagerFailure::ClosedEarly));
    assert_eq!(runner.bytes, [b"hello\n".to_vec(), b"hello\n".to_vec()]);
    assert_eq!(stdout.flushes, 1);
    assert!(stdout.bytes.is_empty());
}

#[test]
fn all_fail_prints_once_with_extra_line_feed() {
    let mut runner = FakeRunner {
        outcomes: (0..4)
            .map(|_| PagerAttempt::Failed(PagerFailure::EmptyProgram))
            .collect(),
        ..FakeRunner::default()
    };
    let mut stdout = output();
    let shown = page(
        "hello\n",
        primary_command(Some("  "), OsFamily::Unix),
        OsFamily::Unix,
        &mut runner,
        &mut stdout,
    )
    .unwrap();
    assert!(matches!(shown.delivery, Delivery::Direct(_)));
    assert_eq!(shown.failures.len(), 4);
    assert_eq!(stdout.bytes, b"hello\n\n");
}

#[test]
fn size_failure_at_render_entry_uses_width_and_fallback_threshold() {
    let mut runner = FakeRunner::default();
    let mut stdout = output();
    let shown = render_and_show(
        "---",
        &request(None, None),
        NoColor::Nonempty,
        None,
        HostSource::Fixed("host".to_owned()),
        &mut runner,
        &mut stdout,
    )
    .unwrap();
    assert!(matches!(shown.delivery, Delivery::Direct(_)));
    assert!(stdout.bytes.starts_with("_".repeat(80).as_bytes()));
    assert!(runner.attempts.is_empty());

    let mut runner = FakeRunner {
        outcomes: vec![PagerAttempt::Completed],
        ..FakeRunner::default()
    };
    let mut stdout = output();
    let many = "line\n\n".repeat(51);
    let shown = render_and_show(
        &many,
        &request(None, None),
        NoColor::Nonempty,
        None,
        HostSource::Fixed("host".to_owned()),
        &mut runner,
        &mut stdout,
    )
    .unwrap();
    assert!(matches!(shown.delivery, Delivery::Paged(_)));
    assert!(stdout.bytes.is_empty());
    assert_eq!(runner.bytes[0].last(), Some(&b'\n'));

    let mut runner = FakeRunner::default();
    let mut stdout = output();
    let zero = request(
        Some(TerminalSize {
            columns: 0,
            rows: 0,
        }),
        None,
    );
    let shown = render_and_show(
        "---",
        &zero,
        NoColor::Nonempty,
        None,
        HostSource::Fixed("host".to_owned()),
        &mut runner,
        &mut stdout,
    )
    .unwrap();
    assert!(matches!(shown.delivery, Delivery::Direct(_)));
    assert!(stdout.bytes.starts_with("_".repeat(80).as_bytes()));
}

#[cfg(unix)]
#[test]
fn invalid_utf8_pager_does_not_break_help_but_fails_on_paging() {
    use std::os::unix::ffi::OsStringExt;
    use std::process::Command;
    let invalid = OsString::from_vec(vec![0xff]);
    for arg in ["-V", "--help"] {
        let result = Command::new(env!("CARGO_BIN_EXE_linear"))
            .arg(arg)
            .env("PAGER", &invalid)
            .output()
            .unwrap();
        assert!(result.status.success(), "{arg}: {:?}", result.stderr);
    }
    let req = PagerRequest {
        pager: Some(&invalid),
        ..request(
            Some(TerminalSize {
                columns: 80,
                rows: 1,
            }),
            None,
        )
    };
    let mut runner = FakeRunner::default();
    let mut stdout = output();
    let error = show("content", &req, &mut runner, &mut stdout).unwrap_err();
    assert_eq!(error.kind, linear_cli::error::AppErrorKind::Validation);
    assert_eq!(error.message, "PAGER is not valid UTF-8");
    assert!(runner.attempts.is_empty());
    assert!(stdout.bytes.is_empty());

    let mut short = request(None, None);
    short.pager = Some(&invalid);
    show("short", &short, &mut runner, &mut stdout).unwrap();
    assert_eq!(stdout.bytes, b"short\n");
}

#[test]
fn no_pager_goes_directly_to_console_policy() {
    let mut runner = FakeRunner::default();
    let mut stdout = output();
    let mut req = request(
        Some(TerminalSize {
            columns: 80,
            rows: 2,
        }),
        None,
    );
    req.enabled = false;
    show("text\n", &req, &mut runner, &mut stdout).unwrap();
    assert_eq!(stdout.bytes, b"text\n\n");
    assert!(runner.attempts.is_empty());
}

#[cfg(unix)]
static NEXT_FAKE: AtomicU64 = AtomicU64::new(0);

#[cfg(unix)]
fn fake_script(body: &str) -> std::path::PathBuf {
    let path = std::env::temp_dir().join(format!(
        "linear-c024f2-pager-{}-{}",
        std::process::id(),
        NEXT_FAKE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    path
}

#[cfg(unix)]
#[test]
fn real_child_gets_exact_stdin_argv_and_overlay_while_streams_are_inherited() {
    use linear_cli::platform::pager::PagerRunner;
    let script = fake_script(
        "printf 'pager-out-marker\\n'; printf 'pager-err-marker\\n' >&2; printf '%s\\n' \"$2\" \"$PAGER_PROBE\" > \"$1.args\"; cat > \"$1\"",
    );
    let data = script.with_extension("input");
    let mut base = BTreeMap::<OsString, OsString>::new();
    base.insert("PATH".into(), "/usr/bin:/bin".into());
    let mut runner =
        ProcessPagerRunner::with_test_environment(base, [("PAGER_PROBE", "overlay-value")]);
    let command = PagerCommand {
        program: script.to_string_lossy().into_owned(),
        args: vec![
            data.to_string_lossy().into_owned(),
            "literal;not-shell".to_owned(),
        ],
    };
    assert!(matches!(
        runner.run(&command, b"exact\0bytes\n"),
        PagerAttempt::Completed
    ));
    assert_eq!(fs::read(&data).unwrap(), b"exact\0bytes\n");
    assert_eq!(
        fs::read_to_string(data.with_extension("input.args")).unwrap(),
        "literal;not-shell\noverlay-value\n"
    );
    fs::remove_file(&script).unwrap();
    fs::remove_file(&data).unwrap();
    fs::remove_file(data.with_extension("input.args")).unwrap();
}

#[cfg(unix)]
#[test]
fn real_child_early_quit_nonzero_and_signal_are_distinct() {
    use linear_cli::platform::pager::{ChildExit, PagerRunner};
    let script = fake_script("exit 0");
    let mut runner = ProcessPagerRunner::inheriting([]);
    let command = PagerCommand {
        program: script.to_string_lossy().into_owned(),
        args: vec![],
    };
    assert!(matches!(
        runner.run(&command, &vec![b'x'; 1024 * 1024]),
        PagerAttempt::ClosedEarly
    ));
    fs::write(&script, "#!/bin/sh\nexit 17\n").unwrap();
    assert!(matches!(
        runner.run(&command, b""),
        PagerAttempt::Failed(PagerFailure::Exit(ChildExit::Code(17)))
    ));
    fs::write(&script, "#!/bin/sh\nkill -TERM $$\n").unwrap();
    assert!(matches!(
        runner.run(&command, b""),
        PagerAttempt::Failed(PagerFailure::Exit(ChildExit::Signal(15)))
    ));
    fs::remove_file(&script).unwrap();
}
