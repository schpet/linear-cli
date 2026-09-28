use std::error::Error;
use std::io::{self, Write};
use std::num::NonZeroU8;

use linear_cli::app::{AppContext, finalize, report_bootstrap_error, run};
use linear_cli::error::{AppError, AppErrorKind, ExitStatus};
use linear_cli::platform::output::{Output, OutputFailure, Stream, failed_stream};

#[derive(Default)]
struct Probe {
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
    fail_write_at: Option<usize>,
    fail_flush_at: Option<usize>,
    fail_every_flush: bool,
    chunk_size: Option<usize>,
}

impl Write for Probe {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if self.fail_write_at == Some(self.writes) {
            return Err(io::Error::from(io::ErrorKind::PermissionDenied));
        }
        let count = self.chunk_size.unwrap_or(bytes.len()).min(bytes.len());
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if self.fail_every_flush || self.fail_flush_at == Some(self.flushes) {
            Err(io::Error::from(io::ErrorKind::Other))
        } else {
            Ok(())
        }
    }
}

fn make_context<'a>(stdout: &'a mut dyn Write, stderr: &'a mut dyn Write) -> AppContext<'a> {
    AppContext {
        startup: super::startup::empty_startup(std::env::temp_dir(), &[]),
        cwd: std::env::temp_dir(),
        stdout,
        stderr,
        stdin_tty: false,
        stdout_tty: false,
        stderr_tty: false,
        stdout_finalization: None,
    }
}

fn assert_io(error: &AppError, stream: Stream) {
    assert_eq!(error.kind, AppErrorKind::IoProcess);
    assert_eq!(failed_stream(error), Some(stream));
    let source = error.source().expect("output source");
    let failure = source
        .downcast_ref::<OutputFailure>()
        .expect("typed failure");
    assert_eq!(failure.stream, stream);
    assert!(failure.source().expect("io source").is::<io::Error>());
}

#[test]
fn no_newline_flush_failure_is_typed() {
    let mut stdout = Probe {
        fail_flush_at: Some(1),
        ..Probe::default()
    };
    let error = Output::new(&mut stdout, Stream::Stdout)
        .write(b"raw bytes without newline")
        .expect_err("flush must fail");
    assert_io(&error, Stream::Stdout);
    assert_eq!(error.message, "failed to flush stdout");
    assert_eq!(stdout.bytes, b"raw bytes without newline");
    assert_eq!(stdout.flushes, 1);
}

#[test]
fn partial_write_failure_reports_once_on_stderr() {
    let mut stdout = Probe {
        fail_write_at: Some(2),
        chunk_size: Some(1),
        ..Probe::default()
    };
    let mut stderr = Probe::default();
    let mut context = make_context(&mut stdout, &mut stderr);
    let argv = vec!["-V".to_owned()];
    let route_result = run(&argv, &mut context);
    let error = finalize(route_result, &mut context).expect_err("write failure wins");
    assert_io(&error, Stream::Stdout);
    drop(context);
    assert_eq!(stdout.bytes, b"3");
    assert_eq!(stderr.writes, 1);
    assert_eq!(stderr.bytes, b"\xe2\x9c\x97 failed to write stdout\n");
}

#[test]
fn final_flush_failure_supersedes_success_usage_and_child_code() {
    let child = ExitStatus::ChildCode(NonZeroU8::new(19).expect("nonzero"));
    for original in [ExitStatus::Success, ExitStatus::UsageFailure, child] {
        let mut stdout = Probe {
            fail_flush_at: Some(1),
            ..Probe::default()
        };
        let mut stderr = Probe::default();
        let mut context = make_context(&mut stdout, &mut stderr);
        let error = finalize(Ok(original), &mut context).expect_err("flush failure wins");
        assert_io(&error, Stream::Stdout);
        drop(context);
        assert_eq!(stderr.bytes, b"\xe2\x9c\x97 failed to flush stdout\n");
        assert_eq!(stderr.writes, 1);
    }
}

#[test]
fn stderr_write_failure_is_not_reported_recursively() {
    let mut stdout = Probe::default();
    let mut stderr = Probe {
        fail_write_at: Some(1),
        ..Probe::default()
    };
    let mut context = make_context(&mut stdout, &mut stderr);
    context.startup.diagnostics = vec![linear_cli::startup::AppStartupDiagnostic::Config(
        linear_cli::config::ConfigDiagnostic {
            path: std::env::temp_dir().join(".env"),
            reason: linear_cli::config::DiagnosticReason::SkippedExpansion(vec![
                "LINEAR_TEAM_ID".to_owned(),
            ]),
        },
    )];
    let route_result = run(&["-V".to_owned()], &mut context);
    let error = finalize(route_result, &mut context).expect_err("stderr failure wins");
    assert_io(&error, Stream::Stderr);
    drop(context);
    assert_eq!(stderr.writes, 1);
    assert!(stderr.bytes.is_empty());
    assert!(stdout.bytes.is_empty());
}

#[test]
fn stderr_final_flush_failure_overrides_route_status() {
    let mut stdout = Probe::default();
    let mut stderr = Probe {
        fail_flush_at: Some(1),
        ..Probe::default()
    };
    let mut context = make_context(&mut stdout, &mut stderr);
    let error = finalize(Ok(ExitStatus::UsageFailure), &mut context)
        .expect_err("stderr flush failure wins");
    assert_io(&error, Stream::Stderr);
    drop(context);
    assert_eq!(stderr.flushes, 1);
    assert_eq!(stderr.writes, 0);
}

#[test]
fn bootstrap_reporter_flushes_and_does_not_retry() {
    let bootstrap = AppError::new(AppErrorKind::IoProcess, "invalid argument encoding");
    let mut stderr = Probe {
        fail_flush_at: Some(1),
        ..Probe::default()
    };
    let error = report_bootstrap_error(&mut stderr, &bootstrap).expect_err("flush failure");
    assert_io(&error, Stream::Stderr);
    assert_eq!(stderr.writes, 1);
    assert_eq!(stderr.flushes, 1);
    assert_eq!(stderr.bytes, b"\xe2\x9c\x97 invalid argument encoding\n");
}

#[test]
fn ordinary_version_output_and_usage_keep_their_status_and_bytes() {
    let mut stdout = Probe::default();
    let mut stderr = Probe::default();
    let mut context = make_context(&mut stdout, &mut stderr);
    let result = run(&["-V".to_owned()], &mut context);
    assert_eq!(finalize(result, &mut context).unwrap(), ExitStatus::Success);
    drop(context);
    assert_eq!(stdout.bytes, b"3.0.0-alpha.1\n");
    assert!(stderr.bytes.is_empty());

    let mut stdout = Probe::default();
    let mut stderr = Probe::default();
    let mut context = make_context(&mut stdout, &mut stderr);
    let result = run(&["frobnicate".to_owned()], &mut context);
    assert_eq!(
        finalize(result, &mut context).unwrap(),
        ExitStatus::UsageFailure
    );
    drop(context);
    let expected_help =
        linear_cli::cli::render::help(linear_cli::cli::root().expect("root route"), true, false)
            .expect("root help");
    assert_eq!(stdout.bytes, expected_help.as_bytes());
    assert_eq!(
        stderr.bytes,
        b"\x1b[31m  \x1b[1merror\x1b[22m: Unknown command \"frobnicate\". Did you mean command \"project\"?\n\x1b[39m\n"
    );
}

#[test]
fn failed_stdout_and_stderr_stop_after_one_diagnostic_attempt() {
    let mut stdout = Probe {
        fail_write_at: Some(1),
        ..Probe::default()
    };
    let mut stderr = Probe {
        fail_write_at: Some(1),
        ..Probe::default()
    };
    let mut context = make_context(&mut stdout, &mut stderr);
    let result = run(&["-V".to_owned()], &mut context);
    let error = finalize(result, &mut context).expect_err("both streams fail");
    assert_io(&error, Stream::Stderr);
    drop(context);
    assert_eq!(stdout.writes, 1);
    assert_eq!(stderr.writes, 1);
    assert!(stderr.bytes.is_empty());
}

#[test]
fn version_broken_pipe_remains_a_strict_io_failure() {
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _bytes: &[u8]) -> io::Result<usize> {
            Err(io::Error::from(io::ErrorKind::BrokenPipe))
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut stdout = Broken;
    let mut stderr = Probe::default();
    let mut context = make_context(&mut stdout, &mut stderr);
    let result = run(&["-V".to_owned()], &mut context);
    let error = finalize(result, &mut context).expect_err("version output remains strict");
    assert_io(&error, Stream::Stdout);
    assert_eq!(error.message, "failed to write stdout");
}

#[test]
fn repeated_stdout_flush_failure_has_only_one_diagnostic() {
    let mut stdout = Probe {
        fail_every_flush: true,
        ..Probe::default()
    };
    let mut stderr = Probe::default();
    let mut context = make_context(&mut stdout, &mut stderr);
    let result = run(&["-V".to_owned()], &mut context);
    let error = finalize(result, &mut context).expect_err("persistent flush failure");
    assert_io(&error, Stream::Stdout);
    drop(context);
    assert_eq!(stdout.flushes, 2);
    assert_eq!(stderr.writes, 1);
    assert_eq!(stderr.bytes, b"\xe2\x9c\x97 failed to flush stdout\n");
}

#[test]
fn real_usage_error_then_second_stdout_flush_failure_reports_io_once() {
    let mut stdout = Probe {
        fail_flush_at: Some(2),
        ..Probe::default()
    };
    let mut stderr = Probe::default();
    let mut context = make_context(&mut stdout, &mut stderr);
    let result = run(&["frobnicate".to_owned()], &mut context);
    let error = finalize(result, &mut context).expect_err("final flush must supersede usage");
    assert_io(&error, Stream::Stdout);
    assert_eq!(error.message, "failed to flush stdout");
    drop(context);
    assert_eq!(stdout.flushes, 2);
    assert_eq!(stderr.writes, 2);
    assert!(stderr.bytes.starts_with(b"\x1b[31m  \x1b[1merror"));
    assert!(
        stderr
            .bytes
            .ends_with(b"\xe2\x9c\x97 failed to flush stdout\n")
    );
}
