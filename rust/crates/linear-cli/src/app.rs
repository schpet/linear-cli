use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use crate::cli::{self, DispatchAction, RouteMeta};
use crate::error::{AppError, AppErrorKind, ExitStatus};

pub struct AppContext<'a> {
    pub env: BTreeMap<String, String>,
    pub cwd: PathBuf,
    pub stdout: &'a mut dyn Write,
    pub stderr: &'a mut dyn Write,
    pub stdout_tty: bool,
    pub stderr_tty: bool,
    pub startup_diagnostics: Vec<String>,
}

impl AppContext<'_> {
    pub fn debug_enabled(&self) -> bool {
        matches!(
            self.env.get("LINEAR_DEBUG").map(String::as_str),
            Some("1" | "true")
        )
    }

    pub fn no_color(&self) -> bool {
        self.env
            .get("NO_COLOR")
            .is_some_and(|value| !value.is_empty())
    }

    pub fn handled_color(&self) -> bool {
        self.stderr_tty && !self.no_color()
    }

    pub fn help_color(&self) -> bool {
        !self.no_color()
    }
}

fn write_stdout(context: &mut AppContext<'_>, bytes: &[u8]) -> Result<(), AppError> {
    context.stdout.write_all(bytes).map_err(|error| {
        AppError::new(AppErrorKind::IoProcess, "failed to write stdout").with_source(error)
    })
}

fn write_stderr(context: &mut AppContext<'_>, bytes: &[u8]) -> Result<(), AppError> {
    context.stderr.write_all(bytes).map_err(|error| {
        AppError::new(AppErrorKind::IoProcess, "failed to write stderr").with_source(error)
    })
}

pub fn run(argv: &[String], context: &mut AppContext<'_>) -> Result<ExitStatus, AppError> {
    for diagnostic in context.startup_diagnostics.clone() {
        write_stderr(context, diagnostic.as_bytes())?;
    }
    let mut route = cli::root().ok_or_else(|| {
        AppError::new(
            AppErrorKind::Invariant,
            "generated route inventory is empty",
        )
    })?;
    let mut position = 0;
    while let Some(arg) = argv.get(position) {
        if route.children.is_empty() {
            break;
        }
        if arg.starts_with('-') {
            break;
        }
        match cli::resolve_child(route, arg) {
            Some(child) => route = child,
            None => {
                return Err(AppError::new(
                    AppErrorKind::Usage,
                    format!("unknown command: {arg}"),
                ));
            }
        }
        position += 1;
    }
    let remaining = argv.get(position..).ok_or_else(|| {
        AppError::new(
            AppErrorKind::Invariant,
            "argument position exceeded argv length",
        )
    })?;
    dispatch(route, remaining, context)
}

fn dispatch(
    route: &RouteMeta,
    args: &[String],
    context: &mut AppContext<'_>,
) -> Result<ExitStatus, AppError> {
    match route.route.action() {
        DispatchAction::Root if args.is_empty() => {
            write_stdout(context, b"Use --help to see available commands\n")?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::Document if args.is_empty() => {
            write_stdout(context, b"Use --help to see available subcommands\n")?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::Root if args == ["-V"] => {
            write_stdout(
                context,
                format!("{}\n", env!("CARGO_PKG_VERSION")).as_bytes(),
            )?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::Root
        | DispatchAction::Document
        | DispatchAction::ParentPending
        | DispatchAction::Unimplemented => Err(AppError::new(
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
    if error.kind == AppErrorKind::Usage {
        return write_usage_error(context, error);
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
) -> Result<ExitStatus, AppError> {
    // The failing route's stdout help belongs to the pending F01 renderer.
    // Keep this path separate so domain Validation errors cannot acquire status 2.
    let line = format!("  error: {}", error.display_message());
    if context.help_color() {
        write_stderr(context, format!("\x1b[31m{line}\x1b[39m\n").as_bytes())?;
    } else {
        write_stderr(context, format!("{line}\n").as_bytes())?;
    }
    Ok(ExitStatus::UsageFailure)
}
