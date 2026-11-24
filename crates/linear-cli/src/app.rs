//! The process entry point: load configuration, build the [`Ctx`], run the
//! selected command and report its error.

use std::error::Error as StdError;

use crate::auth::credentials_path;
use crate::cli::config::ConfigCommand;
use crate::cli::{self, Cli, RootCommand};
use crate::commands;
use crate::commands::completions;
use crate::config::{
    ConfigInputs, DisplaySettings, OsFamily, ProcessEnvSnapshot, RealFileSource,
    apply_template_overrides, load_startup, load_template_config, render_diagnostic,
};
use crate::ctx::{Ctx, CtxInit, Terminal};
use crate::error::{Error, ErrorKind, Result};
use crate::platform::output::{self, Stdout};
use crate::platform::{style, terminal_text};

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
    let Cli { global, command } = cli;
    let command = match command {
        RootCommand::Completions(action) => return completions_command(&action),
        RootCommand::Markdown(_) => return markdown(),
        RootCommand::Skill(args) => {
            let templates = match &args.command {
                cli::skill::SkillCommand::Show(show) => {
                    Some(offline_templates(show.builtin, &global).map_err(skill_recovery_hint)?)
                }
                cli::skill::SkillCommand::Setup(_)
                | cli::skill::SkillCommand::Status(_)
                | cli::skill::SkillCommand::Remove(_)
                | cli::skill::SkillCommand::SessionStart(_) => None,
            };
            return commands::skill::run(&args.command, templates.as_ref());
        }
        RootCommand::Config(args)
            if args.command.is_some() && !matches!(args.command, Some(ConfigCommand::Check)) =>
        {
            let command = args.command.as_ref().expect("inspection command selected");
            let templates = offline_templates(command.defaults(), &global)?;
            return commands::config_inspect::run(command, &templates);
        }
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
    let report = load_startup(&process, &RealFileSource, global.experimental_templates);
    *settings = report.settings;
    let terminal = Terminal::detect(report.settings.no_color);
    for diagnostic in &report.diagnostics {
        output::eprint(render_diagnostic(diagnostic, terminal.stderr_color()).as_bytes())?;
    }
    let mut config = report.result?;
    let extra_files = global
        .config_file
        .iter()
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                cwd.join(path)
            }
        })
        .collect::<Vec<_>>();
    apply_template_overrides(
        &mut config.templates,
        &RealFileSource,
        &extra_files,
        &global.config,
    )?;
    if matches!(&command, RootCommand::Config(args) if matches!(args.command, Some(ConfigCommand::Check)))
    {
        return commands::config_check::run(&config, terminal.stderr_color());
    }
    let env = |name| process.inputs.env(name);
    let ctx = Ctx::new(CtxInit {
        config,
        workspace: global.workspace,
        no_input: global.no_input,
        cwd,
        terminal,
        credentials_path: credentials_path(os, env("XDG_CONFIG_HOME"), env("HOME"), env("APPDATA")),
    })?;
    let result = dispatch(&ctx, command);
    // Output written before a failure still reaches the reader.
    let flushed = ctx.flush();
    result.and(flushed)
}

/// Capture only config discovery paths: unrelated environment, dotenv and auth
/// must not affect offline guide rendering or preference inspection.
fn offline_templates(
    defaults: bool,
    global: &cli::GlobalArgs,
) -> Result<crate::templates::TemplateConfig> {
    if defaults {
        return Ok(crate::templates::TemplateConfig::builtin_with_experimental(
            global.experimental_templates,
        ));
    }
    let cwd = std::env::current_dir().map_err(|error| {
        Error::new(format!("Failed to read the working directory: {error}")).with_source(error)
    })?;
    let os = if cfg!(windows) {
        OsFamily::Windows
    } else {
        OsFamily::Unix
    };
    let mut process_env = std::collections::BTreeMap::new();
    let names: &[&str] = match os {
        OsFamily::Unix => &["HOME", "XDG_CONFIG_HOME"],
        OsFamily::Windows => &["APPDATA"],
    };
    for &name in names {
        match std::env::var(name) {
            Ok(value) => {
                process_env.insert(name.to_owned(), value);
            }
            Err(std::env::VarError::NotPresent) => {}
            Err(error) => return Err(Error::new(format!("Invalid config path {name}: {error}"))),
        }
    }
    load_template_config(
        &ConfigInputs {
            cwd,
            os,
            process_env,
        },
        &RealFileSource,
        &global.config_file,
        &global.config,
        global.experimental_templates,
    )
}

fn skill_recovery_hint(error: Error) -> Error {
    let recovery = "Run `linear skill show --builtin` for the shipped guide.";
    let hint = match error.hint() {
        Some(hint) if hint.contains("--builtin") => return error,
        Some(hint) => format!("{hint} {recovery}"),
        None => recovery.to_owned(),
    };
    error.with_hint(hint)
}

/// Prints `error` to stderr: `✗ message`, an optional hint, and under
/// `LINEAR_DEBUG` the debug detail and source chain.
fn report(error: &Error, settings: DisplaySettings) {
    let lines = match error.kind() {
        ErrorKind::Reported(_) | ErrorKind::Exit(_) | ErrorKind::BrokenPipe => return,
        // The same word as declining a confirmation, though the status differs.
        ErrorKind::Cancelled => "Canceled.\n".to_owned(),
        ErrorKind::Usage(usage) => usage.render().to_string(),
        ErrorKind::Failed(_) | ErrorKind::Invalid => {
            let terminal = Terminal::detect(settings.no_color);
            let color = terminal.stderr_color();
            // On a terminal, lines wrap at spaces with their indent kept.
            let columns = terminal
                .stderr_tty
                .then(|| crossterm::terminal::size().ok())
                .flatten()
                .map(|(columns, _)| usize::from(columns))
                .filter(|&columns| columns > 0);
            let fit = |text: &str, first: &str| match columns {
                Some(columns) => terminal_text::wrap(text, columns, first, "  "),
                None => format!("{first}{text}"),
            };
            let mut lines = format!("{}\n", style::red(&fit(&error.to_string(), "✗ "), color));
            if let Some(hint) = error.hint() {
                lines.push_str(&format!("{}\n", style::gray(&fit(hint, "  "), color)));
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

fn completions_command(args: &cli::completions::Completions) -> Result<()> {
    let stdout = Stdout::new();
    stdout.write(&completions::script(args)?)?;
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
        RootCommand::Config(args) => commands::config_generate::run(ctx, &args),
        RootCommand::Schema(args) => commands::schema::run(ctx, &args),
        RootCommand::Api(args) => commands::api::run(ctx, &args),
        RootCommand::Completions(_) | RootCommand::Markdown(_) | RootCommand::Skill(_) => {
            unreachable!("handled before configuration loads")
        }
    }
}
