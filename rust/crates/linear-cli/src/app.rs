use std::error::Error;
use std::ffi::OsString;
use std::future::Future;
use std::io;
use std::io::Write;
use std::path::PathBuf;
use std::time::Duration;

use crate::cli::clap_input::{OptionValue, ParsedAction};
use crate::cli::{self, DispatchAction};
use crate::commands::{auth_list, auth_whoami, client, team_id, team_list};
use crate::config::{NoColor, StartupConfig};
use crate::error::{AppError, AppErrorKind, ExitStatus};
use crate::platform::output::{Output, OutputOutcome, OutputPolicy, Stream, failed_stream};
use crate::startup::{AppStartupReport, render_startup_diagnostic};

/// Lazily run one network action on a current-thread IO runtime. The action
/// owns its inputs and returns before its caller writes to the CLI streams.
pub fn block_on_network<T, F>(future: F) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    block_on_network_with(future, || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
    })
}

fn block_on_network_with<T, F>(
    future: F,
    build: impl FnOnce() -> io::Result<tokio::runtime::Runtime>,
) -> Result<T, AppError>
where
    F: Future<Output = Result<T, AppError>>,
{
    let runtime = build().map_err(|error| {
        AppError::new(AppErrorKind::IoProcess, "could not start network runtime").with_source(error)
    })?;
    let result = runtime.block_on(future);
    runtime.shutdown_timeout(Duration::from_millis(500));
    result
}

pub struct AppContext<'a> {
    pub startup: AppStartupReport,
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
        self.startup
            .result
            .as_ref()
            .map(|loaded| &loaded.config)
            .map_err(|_| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "config was requested after startup failed",
                )
            })
    }

    pub fn credentials(&self) -> Result<&crate::auth::CredentialStore, AppError> {
        self.startup
            .result
            .as_ref()
            .map(|loaded| &loaded.credentials)
            .map_err(|_| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "credentials requested after startup failed",
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
        let rendered = render_startup_diagnostic(&diagnostic, context.help_color());
        write_stderr(context, rendered.as_bytes())?;
    }
    if let Err(error) = &context.startup.result {
        return Err(error.app_error());
    }
    let os_argv = argv.iter().map(OsString::from).collect::<Vec<_>>();
    match cli::clap_input::parse(&os_argv)? {
        cli::clap_input::Invocation::Help { route, long } => {
            let help = cli::render::help(route, context.help_color(), long)?;
            write_stdout(context, help.as_bytes())?;
            Ok(ExitStatus::Success)
        }
        cli::clap_input::Invocation::Version { long: false } => {
            write_stdout(
                context,
                format!("{}\n", env!("CARGO_PKG_VERSION")).as_bytes(),
            )?;
            Ok(ExitStatus::Success)
        }
        cli::clap_input::Invocation::Version { long: true } => {
            write_stdout(
                context,
                cli::render::long_version(context.help_color()).as_bytes(),
            )?;
            Ok(ExitStatus::Success)
        }
        cli::clap_input::Invocation::Action(action) => dispatch(action, context),
    }
}

fn dispatch(
    action: cli::clap_input::ParsedAction,
    context: &mut AppContext<'_>,
) -> Result<ExitStatus, AppError> {
    let route = action.route;
    match route.route.action() {
        DispatchAction::Root => {
            context.write_stdout_with_policy(
                b"Use --help to see available commands\n",
                OutputPolicy::ConsoleLike,
            )?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::AuthList => {
            let config = context.config()?;
            let rows = auth_list::classify(context.credentials()?);
            let output = if rows.is_empty() {
                auth_list::EMPTY_OUTPUT.as_bytes().to_vec()
            } else {
                let prepared = auth_list::prepare_transports(
                    rows,
                    config.options.endpoint().value(),
                    &config.transport_env,
                )
                .map_err(|error| error.with_context(auth_list::CONTEXT))?;
                let listed = block_on_network(auth_list::fetch(prepared))
                    .map_err(|error| error.with_context(auth_list::CONTEXT))?;
                auth_list::render(&listed, context.stdout_tty && !context.no_color())
            };
            context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::AuthWhoami => {
            let config = context.config()?;
            let credentials = context.credentials()?;
            let workspace = action
                .global_workspace
                .as_ref()
                .map(|value| value.value.as_str());
            let transport = auth_whoami::prepare_transport(
                &config.options,
                credentials,
                workspace,
                &config.transport_env,
            )?;
            let output = block_on_network(async move { auth_whoami::run(&transport).await })?;
            write_stdout(context, &output)?;
            Ok(ExitStatus::Success)
        }
        DispatchAction::TeamList => {
            let flags = team_list::Options {
                json: action_switch(&action, "json")?,
                web: action_switch(&action, "web")?,
                app: action_switch(&action, "app")?,
            };
            if flags.web || flags.app {
                let (url, opening) = team_list::web_opening(&context.config()?.options, flags.app)?;
                context.write_stdout_with_policy(&opening, OutputPolicy::ConsoleLike)?;
                team_list::open(&url, flags.app)?;
                return Ok(ExitStatus::Success);
            }
            let spinner = !flags.json
                && context.stdout_tty
                && context.startup.settings.no_color == NoColor::Absent;
            if spinner {
                context.write_stdout_with_policy(
                    team_list::spinner_frame(0).as_bytes(),
                    OutputPolicy::ConsoleLike,
                )?;
            }
            let prepared = (|| {
                let config = context.config()?;
                let credentials = context.credentials()?;
                let workspace = action
                    .global_workspace
                    .as_ref()
                    .map(|value| value.value.as_str());
                client::prepare_transport(
                    &config.options,
                    credentials,
                    workspace,
                    &config.transport_env,
                )
                .map_err(|error| error.with_context("Failed to fetch teams"))
            })();
            let transport = match prepared {
                Ok(transport) => transport,
                Err(error) => {
                    if spinner {
                        context.write_stdout_with_policy(
                            team_list::SPINNER_CLEAR,
                            OutputPolicy::ConsoleLike,
                        )?;
                    }
                    return Err(error);
                }
            };
            let columns = team_list::stdout_columns(context.stdout_tty);
            let color = context.stdout_tty && !context.no_color();
            let output_result = if spinner {
                block_on_network(async {
                    let pending = team_list::run(&transport, flags.json, columns, color);
                    tokio::pin!(pending);
                    let mut ticks = tokio::time::interval(Duration::from_millis(75));
                    ticks.tick().await;
                    let mut frame = 1;
                    loop {
                        tokio::select! {
                            biased;
                            result = &mut pending => break result,
                            _ = ticks.tick() => {
                                context.write_stdout_with_policy(
                                    team_list::spinner_frame(frame).as_bytes(),
                                    OutputPolicy::ConsoleLike,
                                )?;
                                frame = frame.wrapping_add(1);
                            }
                        }
                    }
                })
            } else {
                block_on_network(async {
                    team_list::run(&transport, flags.json, columns, color).await
                })
            };
            if spinner {
                context.write_stdout_with_policy(
                    team_list::SPINNER_CLEAR,
                    OutputPolicy::ConsoleLike,
                )?;
            }
            let output = output_result.map_err(|error| {
                if error.context.is_none() {
                    error.with_context("Failed to fetch teams")
                } else {
                    error
                }
            })?;
            context.write_stdout_with_policy(&output, OutputPolicy::ConsoleLike)?;
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
        DispatchAction::TeamId => {
            let text = team_id::render(context)?;
            context.write_stdout_with_policy(text.as_bytes(), OutputPolicy::ConsoleLike)?;
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

fn action_switch(action: &ParsedAction, name: &str) -> Result<bool, AppError> {
    match action.option(name) {
        None => Ok(false),
        Some(option) => match &option.value.value {
            OptionValue::Switch(value) => Ok(*value),
            _ => Err(AppError::new(
                AppErrorKind::Invariant,
                format!("team list option {name} was not a switch"),
            )),
        },
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
    if context.debug_enabled()
        && matches!(error.kind, AppErrorKind::GraphQl | AppErrorKind::Transport)
    {
        if let Some(detail) = error.debug_detail() {
            write_stderr(context, format!("  debug: {detail}\n").as_bytes())?;
        }
        let mut source = error.source();
        while let Some(cause) = source {
            write_stderr(context, format!("  caused by: {cause}\n").as_bytes())?;
            source = cause.source();
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

#[cfg(test)]
mod network_runtime_tests {
    use super::{block_on_network, block_on_network_with};
    use crate::error::AppErrorKind;
    use std::io;
    use std::sync::mpsc;
    use std::time::{Duration, Instant};

    #[test]
    fn runtime_build_failure_is_a_handled_io_error() {
        let result = block_on_network_with(async { Ok::<(), _>(()) }, || {
            Err(io::Error::other("synthetic runtime failure"))
        });
        assert!(
            matches!(result, Err(ref error) if error.kind == AppErrorKind::IoProcess && error.display_message() == "could not start network runtime")
        );
    }

    #[test]
    fn runtime_shutdown_does_not_wait_forever_for_blocking_work() {
        let (release, blocked) = mpsc::channel::<()>();
        let started = Instant::now();
        let result = block_on_network(async move {
            let (ready, entered) = mpsc::channel();
            tokio::task::spawn_blocking(move || {
                let _ = ready.send(());
                let _ = blocked.recv_timeout(Duration::from_secs(3));
            });
            assert!(entered.recv_timeout(Duration::from_secs(1)).is_ok());
            Ok::<(), crate::error::AppError>(())
        });
        assert!(result.is_ok());
        assert!(started.elapsed() < Duration::from_secs(2));
        assert!(release.send(()).is_ok());
    }
}
