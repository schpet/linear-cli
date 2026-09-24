use std::io::Write;
use std::path::PathBuf;

use crate::cli::{self, DispatchAction, RouteMeta};
use crate::config::{StartupConfig, StartupReport, render_diagnostic};
use crate::error::{AppError, AppErrorKind, ExitStatus};
use crate::platform::output::{Output, OutputOutcome, OutputPolicy, Stream, failed_stream};

pub struct AppContext<'a> {
    pub startup: StartupReport,
    pub cwd: PathBuf,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    pub stdout_tty: bool,
    pub stderr_tty: bool,
    pub stdout_finalization: Option<(OutputPolicy, OutputOutcome)>,
}

impl AppContext<'_> {
    pub fn debug_enabled(&self) -> bool {
        self.startup.settings.debug
    }

    pub fn no_color(&self) -> bool {
        self.startup.settings.no_color()
    }

    pub fn handled_color(&self) -> bool {
        self.stderr_tty && !self.no_color()
    }

    pub fn help_color(&self) -> bool {
        self.startup.settings.help_color()
    }

    pub fn config(&self) -> Result<&StartupConfig, AppError> {
        self.startup.result.as_ref().map_err(|_| {
            AppError::new(
                AppErrorKind::Invariant,
                "config was requested after startup failed",
            )
        })
    }

    fn write_stdout(&mut self, bytes: &[u8]) -> Result<(), AppError> {
        self.write_stdout_with_policy(bytes, OutputPolicy::Strict)
    }

    fn write_stdout_with_policy(
        &mut self,
        bytes: &[u8],
        policy: OutputPolicy,
    ) -> Result<(), AppError> {
        self.stdout_finalization = None;
        let outcome =
            Output::new(&mut *self.stdout, Stream::Stdout).write_with_policy(bytes, policy)?;
        self.stdout_finalization = Some((policy, outcome));
        Ok(())
    }

    fn write_stderr(&mut self, bytes: &[u8]) -> Result<(), AppError> {
        Output::new(&mut *self.stderr, Stream::Stderr).write(bytes)
    }

    fn flush_all(&mut self) -> Result<(), AppError> {
        let policy = match self.stdout_finalization.take() {
            Some((OutputPolicy::ConsoleLike, OutputOutcome::QuietBrokenPipe)) => {
                OutputPolicy::ConsoleLike
            }
            Some(_) | None => OutputPolicy::Strict,
        };
        Output::new(&mut *self.stdout, Stream::Stdout).flush_with_policy(policy)?;
        Output::new(&mut *self.stderr, Stream::Stderr).flush()
    }
}

fn write_stdout(context: &mut AppContext<'_>, bytes: &[u8]) -> Result<(), AppError> {
    context.write_stdout(bytes)
}

fn write_stderr(context: &mut AppContext<'_>, bytes: &[u8]) -> Result<(), AppError> {
    context.write_stderr(bytes)
}

/// Emit one bootstrap diagnostic, including its flush. The caller returns status 1
/// even when this diagnostic cannot be written.
pub fn report_bootstrap_error(stderr: &mut dyn Write, error: &AppError) -> Result<(), AppError> {
    Output::new(stderr, Stream::Stderr).write(format!("✗ {error}\n").as_bytes())
}

fn report_output_failure(context: &mut AppContext<'_>, error: &AppError) {
    if failed_stream(error) == Some(Stream::Stdout) {
        let _ = report_bootstrap_error(context.stderr, error);
    }
}

/// Resolve route output and final stream flushes before the process chooses an exit code.
/// A write or flush failure always wins over the route status, including usage/child codes.
pub fn finalize(
    result: Result<ExitStatus, AppError>,
    context: &mut AppContext<'_>,
) -> Result<ExitStatus, AppError> {
    let (status, route_io_error, output_diagnostic_attempted) = match result {
        Ok(status) => (status, None, false),
        Err(error) => {
            // A failed stderr write already was the diagnostic attempt.
            if failed_stream(&error) == Some(Stream::Stderr) {
                return Err(error);
            }
            match write_final_error(context, &error) {
                Ok(status) => {
                    let output_diagnostic_attempted = failed_stream(&error) == Some(Stream::Stdout);
                    let io_error = (error.kind == AppErrorKind::IoProcess).then_some(error);
                    (status, io_error, output_diagnostic_attempted)
                }
                Err(write_error) => {
                    report_output_failure(context, &write_error);
                    return Err(write_error);
                }
            }
        }
    };
    if let Err(flush_error) = context.flush_all() {
        if !output_diagnostic_attempted {
            report_output_failure(context, &flush_error);
        }
        return Err(flush_error);
    }
    match route_io_error {
        Some(error) => Err(error),
        None => Ok(status),
    }
}

pub fn run(argv: &[String], context: &mut AppContext<'_>) -> Result<ExitStatus, AppError> {
    context.stdout_finalization = None;
    for diagnostic in context.startup.diagnostics.clone() {
        let rendered = render_diagnostic(&diagnostic, context.help_color());
        write_stderr(context, rendered.as_bytes())?;
    }
    if let Err(error) = &context.startup.result {
        return Err(error.app_error());
    }
    match cli::parser::parse(argv)? {
        cli::parser::ParseOutcome::Help { route, long } => {
            let help = cli::render::help(route, context.help_color(), long)?;
            write_stdout(context, help.as_bytes())?;
            Ok(ExitStatus::Success)
        }
        cli::parser::ParseOutcome::Version { long: false } => {
            write_stdout(
                context,
                format!("{}\n", env!("CARGO_PKG_VERSION")).as_bytes(),
            )?;
            Ok(ExitStatus::Success)
        }
        cli::parser::ParseOutcome::Version { long: true } => {
            write_stdout(
                context,
                cli::render::long_version(context.help_color()).as_bytes(),
            )?;
            Ok(ExitStatus::Success)
        }
        cli::parser::ParseOutcome::Action { route, .. } => dispatch(route, context),
    }
}

fn dispatch(route: &RouteMeta, context: &mut AppContext<'_>) -> Result<ExitStatus, AppError> {
    match route.route.action() {
        DispatchAction::Root => {
            context.write_stdout_with_policy(
                b"Use --help to see available commands\n",
                OutputPolicy::ConsoleLike,
            )?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::Document => {
            write_stdout(context, b"Use --help to see available subcommands\n")?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::ParentPending => {
            write_stdout(
                context,
                cli::render::help(route, context.help_color(), false)?.as_bytes(),
            )?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::Markdown => {
            context.write_stdout_with_policy(
                format!("{}\n", route.description).as_bytes(),
                OutputPolicy::ConsoleLike,
            )?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::Unimplemented => Err(AppError::new(
            AppErrorKind::Unimplemented,
            format!(
                "{} is registered, but this action is not implemented yet",
                route.path
            ),
        )),
    }
}

pub fn write_final_error(
    context: &mut AppContext<'_>,
    error: &AppError,
) -> Result<ExitStatus, AppError> {
    if let AppErrorKind::Usage { route } = error.kind {
        return write_usage_error(context, error, route);
    }
    let color = context.handled_color();
    let line = format!("✗ {}", error.display_message());
    if color {
        write_stderr(context, format!("\x1b[31m{line}\x1b[39m\n").as_bytes())?;
    } else {
        write_stderr(context, format!("{line}\n").as_bytes())?;
    }
    if let Some(suggestion) = &error.suggestion {
        if color {
            write_stderr(
                context,
                format!("\x1b[90m  {suggestion}\x1b[39m\n").as_bytes(),
            )?;
        } else {
            write_stderr(context, format!("  {suggestion}\n").as_bytes())?;
        }
    }
    Ok(ExitStatus::HandledFailure)
}

fn write_usage_error(
    context: &mut AppContext<'_>,
    error: &AppError,
    route: cli::Route,
) -> Result<ExitStatus, AppError> {
    let metadata = cli::ROUTES
        .iter()
        .find(|candidate| candidate.route == route)
        .ok_or_else(|| {
            AppError::new(
                AppErrorKind::Invariant,
                "usage error route missing from inventory",
            )
        })?;
    let help = cli::render::help(metadata, context.help_color(), false)?;
    write_stdout(context, help.as_bytes())?;
    if context.help_color() {
        write_stderr(
            context,
            format!(
                "\x1b[31m  \x1b[1merror\x1b[22m: {}\n\x1b[39m\n",
                error.display_message()
            )
            .as_bytes(),
        )?;
    } else {
        write_stderr(
            context,
            format!("  error: {}\n\n", error.display_message()).as_bytes(),
        )?;
    }
    Ok(ExitStatus::UsageFailure)
}
