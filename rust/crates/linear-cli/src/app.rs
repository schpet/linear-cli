//! The process entry point: load configuration, build the [`Ctx`], run the
//! selected command and report its error.

use std::error::Error as StdError;

use crate::auth::credentials_path;
use crate::cli::{self, Cli, RootCommand};
use crate::commands;
use crate::commands::completions::{self, CompletionShell};
use crate::config::{
    DisplaySettings, OsFamily, ProcessEnvSnapshot, RealFileSource, load_startup, render_diagnostic,
};
use crate::ctx::{Ctx, CtxInit, Terminal};
use crate::error::{Error, ErrorKind, Result};
use crate::platform::output::{self, Stdout};
use crate::platform::style;

/// Runs a parsed command line and returns the process exit status.
pub fn main(cli: Cli) -> u8 {
    let mut settings = DisplaySettings {
        debug: false,
        no_color: false,
    };
    let result = run(cli, &mut settings);
    match result {
        Ok(()) => 0,
        Err(error) => {
            report(&error, settings);
            error.exit_code()
        }
    }
}

fn run(cli: Cli, settings: &mut DisplaySettings) -> Result<()> {
    let Cli { workspace, command } = cli;
    let command = match command {
        RootCommand::Completions(action) => return completions_command(&action),
        RootCommand::Markdown(_) => return markdown(),
        command => command,
    };
    let cwd = std::env::current_dir().map_err(|error| {
        Error::new(format!("Failed to read the working directory: {error}")).with_source(error)
    })?;
    let os = if cfg!(windows) {
        OsFamily::Windows
    } else {
        OsFamily::Unix
    };
    let process = ProcessEnvSnapshot::capture(cwd.clone(), os)
        .map_err(|error| Error::new(format!("Invalid environment: {error}")))?;
    let report = load_startup(&process, &RealFileSource);
    *settings = report.settings;
    let terminal = Terminal::detect(report.settings.no_color);
    for diagnostic in &report.diagnostics {
        output::eprint(render_diagnostic(diagnostic, terminal.stderr_color()).as_bytes())?;
    }
    let config = report.result?;
    let env = |name| process.inputs.env(name);
    let ctx = Ctx::new(CtxInit {
        config,
        workspace,
        cwd,
        terminal,
        credentials_path: credentials_path(os, env("XDG_CONFIG_HOME"), env("HOME"), env("APPDATA")),
    })?;
    let result = dispatch(&ctx, command);
    // Output written before a failure still reaches the reader.
    let flushed = ctx.flush();
    result.and(flushed)
}

/// Prints `error` to stderr: `✗ message`, an optional hint, and under
/// `LINEAR_DEBUG` the debug detail and source chain.
fn report(error: &Error, settings: DisplaySettings) {
    let lines = match error.kind() {
        ErrorKind::Cancelled | ErrorKind::Exit(_) | ErrorKind::BrokenPipe => return,
        ErrorKind::Usage => match error.usage_error() {
            Some(usage) => usage.render().to_string(),
            None => format!("{error}\n"),
        },
        ErrorKind::Other | ErrorKind::Auth | ErrorKind::NotFound => {
            let color = Terminal::detect(settings.no_color).stderr_color();
            let mut lines = format!("{}\n", style::red(&format!("✗ {error}"), color));
            if let Some(hint) = error.hint() {
                lines.push_str(&format!("{}\n", style::gray(&format!("  {hint}"), color)));
            }
            if settings.debug {
                if let Some(detail) = error.debug_detail() {
                    lines.push_str(&format!("  debug: {detail}\n"));
                }
                let mut source = error.source();
                while let Some(cause) = source {
                    lines.push_str(&format!("  caused by: {cause}\n"));
                    source = cause.source();
                }
            }
            lines
        }
    };
    // Nothing is left to report a failure to.
    let _ignored = output::eprint(lines.as_bytes());
}

fn completions_command(action: &cli::completions::Completions) -> Result<()> {
    use cli::completions::CompletionsCommand;
    let output = match &action.command {
        CompletionsCommand::Bash(action) => {
            completions::script(CompletionShell::Bash, action.name.as_deref())?
        }
        CompletionsCommand::Fish(action) => {
            completions::script(CompletionShell::Fish, action.name.as_deref())?
        }
        CompletionsCommand::Zsh(action) => {
            completions::script(CompletionShell::Zsh, action.name.as_deref())?
        }
        CompletionsCommand::Complete(action) => completions::complete(action)?,
    };
    let stdout = Stdout::new();
    stdout.write(&output)?;
    stdout.flush()
}

fn markdown() -> Result<()> {
    let stdout = Stdout::new();
    stdout.write(include_str!("cli/markdown.txt").as_bytes())?;
    stdout.flush()
}

fn dispatch(ctx: &Ctx, command: RootCommand) -> Result<()> {
    match command {
        RootCommand::Auth(args) => commands::auth::run(ctx, &args.command),
        RootCommand::Issue(args) => commands::issue::run(ctx, &args.command),
        RootCommand::Team(args) => commands::team::run(ctx, &args.command),
        RootCommand::User(args) => commands::user::run(ctx, &args.command),
        RootCommand::Project(args) => commands::project::run(ctx, &args.command),
        RootCommand::ProjectUpdate(args) => commands::project_update::run(ctx, &args.command),
        RootCommand::Cycle(args) => commands::cycle::run(ctx, &args.command),
        RootCommand::Milestone(args) => commands::milestone::run(ctx, &args.command),
        RootCommand::Initiative(args) => commands::initiative::run(ctx, &args.command),
        RootCommand::InitiativeUpdate(args) => commands::initiative_update::run(ctx, &args.command),
        RootCommand::Label(args) => commands::label::run(ctx, &args.command),
        RootCommand::Template(args) => commands::template::run(ctx, &args.command),
        RootCommand::Document(args) => commands::document::run(ctx, &args.command),
        RootCommand::Config(_) => commands::config_generate::run(ctx),
        RootCommand::Schema(args) => commands::schema::run(ctx, &args),
        RootCommand::Api(args) => commands::api::run(ctx, &args),
        RootCommand::Completions(_) | RootCommand::Markdown(_) => {
            unreachable!("handled before configuration loads")
        }
    }
}
