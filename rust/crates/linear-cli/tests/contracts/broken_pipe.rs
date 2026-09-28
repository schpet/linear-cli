use std::error::Error;
use std::io::{self, Write};

use linear_cli::app::{AppContext, finalize, run};
use linear_cli::error::{AppError, AppErrorKind, ExitStatus};
use linear_cli::platform::output::{
    Output, OutputFailure, OutputOutcome, OutputPolicy, Stream, failed_stream,
};

#[derive(Default)]
struct Sink {
    bytes: Vec<u8>,
    writes: usize,
    flushes: usize,
    write_error_at: Option<(usize, io::ErrorKind)>,
    flush_error_at: Option<(usize, io::ErrorKind)>,
    partial: bool,
}

impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.writes += 1;
        if let Some((at, kind)) = self.write_error_at
            && self.writes == at
        {
            return Err(kind.into());
        }
        let count = if self.partial {
            bytes.len().min(1)
        } else {
            bytes.len()
        };
        self.bytes.extend_from_slice(&bytes[..count]);
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flushes += 1;
        if let Some((at, kind)) = self.flush_error_at
            && self.flushes == at
        {
            return Err(kind.into());
        }
        Ok(())
    }
}

fn context<'a>(stdout: &'a mut dyn Write, stderr: &'a mut dyn Write) -> AppContext<'a> {
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

fn failure(error: &AppError, stream: Stream, operation: &str, kind: io::ErrorKind) {
    assert_eq!(error.kind, AppErrorKind::IoProcess);
    assert_eq!(failed_stream(error), Some(stream));
    let typed = error
        .source()
        .and_then(|source| source.downcast_ref::<OutputFailure>())
        .expect("typed output failure");
    assert_eq!(typed.stream, stream);
    assert_eq!(
        typed.to_string(),
        format!("failed to {operation} {stream:?}").to_lowercase()
    );
    let source = typed
        .source()
        .and_then(|source| source.downcast_ref::<io::Error>())
        .expect("underlying io error");
    assert_eq!(source.kind(), kind);
    assert_eq!(typed.is_broken_pipe(), kind == io::ErrorKind::BrokenPipe);
}

#[test]
fn root_first_write_epipe_is_quiet_through_finalization() {
    let mut stdout = Sink {
        write_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        flush_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let mut stderr = Sink::default();
    let mut app = context(&mut stdout, &mut stderr);
    let route = run(&[], &mut app);
    assert_eq!(
        app.stdout_finalization,
        Some((OutputPolicy::ConsoleLike, OutputOutcome::QuietBrokenPipe))
    );
    assert_eq!(finalize(route, &mut app).unwrap(), ExitStatus::Success);
    assert_eq!(app.stdout_finalization, None);
    drop(app);
    assert!(stdout.bytes.is_empty());
    assert_eq!(stdout.writes, 1);
    assert_eq!(stdout.flushes, 1);
    assert_eq!(stderr.writes, 0);
    assert!(stderr.bytes.is_empty());
}

#[test]
fn root_partial_write_epipe_does_not_claim_complete_output() {
    let mut stdout = Sink {
        partial: true,
        write_error_at: Some((2, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let mut stderr = Sink::default();
    let mut app = context(&mut stdout, &mut stderr);
    let route = run(&[], &mut app);
    assert_eq!(finalize(route, &mut app).unwrap(), ExitStatus::Success);
    drop(app);
    assert_eq!(stdout.bytes, b"U");
    assert!(stderr.bytes.is_empty());
}

#[test]
fn root_in_operation_flush_epipe_is_quiet() {
    let mut stdout = Sink {
        flush_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let mut stderr = Sink::default();
    let mut app = context(&mut stdout, &mut stderr);
    let route = run(&[], &mut app);
    assert_eq!(
        app.stdout_finalization,
        Some((OutputPolicy::ConsoleLike, OutputOutcome::QuietBrokenPipe))
    );
    assert_eq!(finalize(route, &mut app).unwrap(), ExitStatus::Success);
    drop(app);
    assert_eq!(stdout.flushes, 2);
    assert!(stderr.bytes.is_empty());
}

#[test]
fn root_write_success_then_final_flush_epipe_is_a_strict_failure() {
    let mut stdout = Sink {
        flush_error_at: Some((2, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let mut stderr = Sink::default();
    let mut app = context(&mut stdout, &mut stderr);
    let route = run(&[], &mut app);
    assert_eq!(
        app.stdout_finalization,
        Some((OutputPolicy::ConsoleLike, OutputOutcome::Written))
    );
    let error = finalize(route, &mut app).expect_err("no quiet write outcome to carry");
    failure(&error, Stream::Stdout, "flush", io::ErrorKind::BrokenPipe);
    drop(app);
    assert_eq!(stdout.flushes, 2);
    assert_eq!(stderr.writes, 1);
    assert_eq!(stderr.bytes, b"\xe2\x9c\x97 failed to flush stdout\n");
}

#[test]
fn root_non_broken_pipe_final_flush_failure_reports_once() {
    let mut stdout = Sink {
        flush_error_at: Some((2, io::ErrorKind::PermissionDenied)),
        ..Sink::default()
    };
    let mut stderr = Sink::default();
    let mut app = context(&mut stdout, &mut stderr);
    let route = run(&[], &mut app);
    let error = finalize(route, &mut app).expect_err("non-EPIPE final flush must fail");
    failure(
        &error,
        Stream::Stdout,
        "flush",
        io::ErrorKind::PermissionDenied,
    );
    drop(app);
    assert_eq!(stderr.writes, 1);
    assert_eq!(stderr.bytes, b"\xe2\x9c\x97 failed to flush stdout\n");
}

#[test]
fn strict_write_epipe_remains_typed_for_command_owner() {
    let mut stdout = Sink {
        write_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let error = Output::new(&mut stdout, Stream::Stdout)
        .write_with_policy(b"json", OutputPolicy::Strict)
        .expect_err("strict write must expose EPIPE");
    failure(&error, Stream::Stdout, "write", io::ErrorKind::BrokenPipe);
    assert_eq!(stdout.flushes, 0);
}

#[test]
fn strict_partial_write_and_flush_epipe_remain_distinct() {
    let mut stdout = Sink {
        partial: true,
        write_error_at: Some((2, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let error = Output::new(&mut stdout, Stream::Stdout)
        .write_with_policy(b"json", OutputPolicy::Strict)
        .expect_err("partial write must expose EPIPE");
    failure(&error, Stream::Stdout, "write", io::ErrorKind::BrokenPipe);
    assert_eq!(stdout.bytes, b"j");

    let mut stdout = Sink {
        flush_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let error = Output::new(&mut stdout, Stream::Stdout)
        .write_with_policy(b"json", OutputPolicy::Strict)
        .expect_err("flush must expose EPIPE");
    failure(&error, Stream::Stdout, "flush", io::ErrorKind::BrokenPipe);
    assert_eq!(stdout.bytes, b"json");
}

#[test]
fn console_policy_keeps_other_failures_and_stderr_epipe() {
    for (stream, kind) in [
        (Stream::Stdout, io::ErrorKind::PermissionDenied),
        (Stream::Stderr, io::ErrorKind::BrokenPipe),
    ] {
        let mut sink = Sink {
            write_error_at: Some((1, kind)),
            ..Sink::default()
        };
        let error = Output::new(&mut sink, stream)
            .write_with_policy(b"message", OutputPolicy::ConsoleLike)
            .expect_err("only stdout EPIPE may be quiet");
        failure(&error, stream, "write", kind);
    }
}

#[test]
fn root_non_broken_pipe_failure_keeps_one_diagnostic() {
    let mut stdout = Sink {
        write_error_at: Some((1, io::ErrorKind::PermissionDenied)),
        ..Sink::default()
    };
    let mut stderr = Sink::default();
    let mut app = context(&mut stdout, &mut stderr);
    let route = run(&[], &mut app);
    let error = finalize(route, &mut app).expect_err("non-EPIPE must fail");
    failure(
        &error,
        Stream::Stdout,
        "write",
        io::ErrorKind::PermissionDenied,
    );
    drop(app);
    assert_eq!(stderr.writes, 1);
    assert_eq!(stderr.bytes, b"\xe2\x9c\x97 failed to write stdout\n");
}

#[test]
fn quiet_root_does_not_hide_stderr_final_flush_failure() {
    let mut stdout = Sink {
        write_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let mut stderr = Sink {
        flush_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let mut app = context(&mut stdout, &mut stderr);
    let route = run(&[], &mut app);
    let error = finalize(route, &mut app).expect_err("stderr EPIPE must win");
    failure(&error, Stream::Stderr, "flush", io::ErrorKind::BrokenPipe);
    drop(app);
    assert_eq!(stderr.writes, 0);
}

#[test]
fn a_later_strict_operation_replaces_quiet_root_state() {
    let mut stdout = Sink {
        write_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        flush_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let mut stderr = Sink::default();
    let mut app = context(&mut stdout, &mut stderr);
    assert_eq!(run(&[], &mut app).unwrap(), ExitStatus::Success);
    assert_eq!(
        app.stdout_finalization,
        Some((OutputPolicy::ConsoleLike, OutputOutcome::QuietBrokenPipe))
    );
    let route = run(&["-V".to_owned()], &mut app);
    assert_eq!(app.stdout_finalization, None);
    let error = finalize(route, &mut app).expect_err("strict version write must fail");
    failure(&error, Stream::Stdout, "flush", io::ErrorKind::BrokenPipe);
    drop(app);
    assert_eq!(stderr.writes, 1);
}

#[test]
fn a_later_route_without_stdout_cannot_inherit_quiet_policy() {
    let mut stdout = Sink {
        write_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        flush_error_at: Some((1, io::ErrorKind::BrokenPipe)),
        ..Sink::default()
    };
    let mut stderr = Sink::default();
    let mut app = context(&mut stdout, &mut stderr);
    assert_eq!(run(&[], &mut app).unwrap(), ExitStatus::Success);
    assert_eq!(
        app.stdout_finalization,
        Some((OutputPolicy::ConsoleLike, OutputOutcome::QuietBrokenPipe))
    );
    // `api` is currently a stable no-stdout Unimplemented route. This checks
    // state lifetime only; the future API command owns its pipe semantics.
    let route = run(&["api".to_owned()], &mut app);
    assert_eq!(app.stdout_finalization, None);
    let error = finalize(route, &mut app).expect_err("later route flush must remain strict");
    failure(&error, Stream::Stdout, "flush", io::ErrorKind::BrokenPipe);
    drop(app);
    assert_eq!(stderr.writes, 2);
    assert!(
        stderr
            .bytes
            .ends_with(b"\xe2\x9c\x97 failed to flush stdout\n")
    );
}
