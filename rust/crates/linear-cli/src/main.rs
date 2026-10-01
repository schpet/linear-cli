#![forbid(unsafe_code)]
#![cfg_attr(
    not(test),
    deny(
        clippy::as_conversions,
        clippy::unwrap_used,
        clippy::expect_used,
        clippy::panic,
        clippy::indexing_slicing
    )
)]

use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal};
use std::process::ExitCode;

use linear_cli::app::{AppContext, finalize, report_bootstrap_error, run};
use linear_cli::auth::file::RealCredentialFileSource;
use linear_cli::auth::keyring::NativeKeyringReader;
use linear_cli::config::{
    OsFamily, ProcessEnvError, ProcessEnvSnapshot, RealFileSource, RealGitRootProbe,
};
use linear_cli::error::{AppError, AppErrorKind, ExitStatus};
use linear_cli::startup::load;

type ProcessInputs = (ProcessEnvSnapshot, Vec<String>);

fn unicode_argument(value: OsString, position: usize) -> Result<String, AppError> {
    value.into_string().map_err(|_| {
        AppError::new(
            AppErrorKind::IoProcess,
            format!("argument {position} is not valid UTF-8"),
        )
    })
}

fn process_env_error(error: ProcessEnvError) -> AppError {
    let message = match &error {
        ProcessEnvError::InvalidName => {
            "a relevant environment variable name is not UTF-8".to_owned()
        }
        ProcessEnvError::InvalidValue { name } => {
            format!("environment variable {name} is not valid UTF-8")
        }
        ProcessEnvError::DuplicateName { name } => {
            format!("duplicate environment variable {name}")
        }
    };
    AppError::new(AppErrorKind::IoProcess, message).with_source(error)
}

fn os_family() -> OsFamily {
    if cfg!(windows) {
        OsFamily::Windows
    } else {
        OsFamily::Unix
    }
}

fn process_inputs() -> Result<ProcessInputs, AppError> {
    let cwd = env::current_dir().map_err(|error| {
        AppError::new(AppErrorKind::IoProcess, "failed to read working directory")
            .with_source(error)
    })?;
    let environment = ProcessEnvSnapshot::capture(cwd, os_family()).map_err(process_env_error)?;
    let argv = env::args_os()
        .skip(1)
        .enumerate()
        .map(|(index, value)| unicode_argument(value, index + 1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((environment, argv))
}

fn main() -> ExitCode {
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    let (environment, argv) = match process_inputs() {
        Ok(inputs) => inputs,
        Err(error) => {
            let _ = report_bootstrap_error(&mut err, &error);
            return ExitCode::FAILURE;
        }
    };
    let cwd = environment.inputs.cwd.clone();
    let git = RealGitRootProbe::new(cwd.clone());
    // Parse only for existing command-local startup sort/template timing policies.
    let parsed = linear_cli::cli::parse(&argv.iter().map(OsString::from).collect::<Vec<_>>());
    let keyring = NativeKeyringReader;
    let defer_sort = matches!(
        parsed.as_ref().ok().and_then(|cli| cli.command.as_ref()),
        Some(linear_cli::cli::RootCommand::Issue(
            linear_cli::cli::issue::Issue {
                command: Some(
                    linear_cli::cli::issue::IssueCommand::Mine(_)
                        | linear_cli::cli::issue::IssueCommand::Query(_)
                        | linear_cli::cli::issue::IssueCommand::Start(_)
                )
            }
        ))
    );
    let defer_template = matches!(
        parsed.as_ref().ok().and_then(|cli| cli.command.as_ref()),
        Some(linear_cli::cli::RootCommand::Issue(
            linear_cli::cli::issue::Issue {
                command: Some(linear_cli::cli::issue::IssueCommand::PullRequest(_))
            }
        ))
    );
    let startup_loader = if defer_template {
        linear_cli::startup::load_for_pull_request
    } else if defer_sort {
        linear_cli::startup::load_for_issue_reads
    } else {
        load
    };
    let startup = startup_loader(
        &environment,
        &RealFileSource,
        &git,
        &RealCredentialFileSource,
        &keyring,
    );
    let mut context = AppContext {
        startup,
        cwd,
        stdout: &mut out,
        stderr: &mut err,
        stdin_tty: io::stdin().is_terminal(),
        stdout_tty: stdout.is_terminal(),
        stderr_tty: stderr.is_terminal(),
        stdout_finalization: None,
    };
    let status = match finalize(run(&argv, &mut context), &mut context) {
        Ok(status) => status,
        Err(_) => ExitStatus::HandledFailure,
    };
    ExitCode::from(status.code())
}

#[cfg(test)]
mod tests {
    use super::{process_env_error, unicode_argument};
    use linear_cli::config::{OsFamily, ProcessEnvError, ProcessEnvSnapshot};
    use std::ffi::OsString;

    #[test]
    fn unicode_process_inputs_are_accepted() {
        assert_eq!(
            unicode_argument(OsString::from("issue"), 1).unwrap(),
            "issue"
        );
        let cwd = std::env::current_dir().unwrap();
        let captured = ProcessEnvSnapshot::from_vars_os(
            cwd,
            OsFamily::Unix,
            [(OsString::from("NO_COLOR"), OsString::from("1"))],
        )
        .unwrap();
        assert_eq!(captured.inputs.env("NO_COLOR"), Some("1"));
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_process_inputs_report_typed_errors() {
        use std::os::unix::ffi::OsStringExt;

        let invalid = || OsString::from_vec(vec![0xff]);
        let argument = unicode_argument(invalid(), 2).unwrap_err();
        assert_eq!(argument.display_message(), "argument 2 is not valid UTF-8");
        let environment = process_env_error(ProcessEnvError::InvalidValue {
            name: "NO_COLOR".to_owned(),
        });
        assert_eq!(
            environment.display_message(),
            "environment variable NO_COLOR is not valid UTF-8"
        );
        let relevant = ProcessEnvSnapshot::from_vars_os(
            std::env::current_dir().unwrap(),
            OsFamily::Unix,
            [
                (OsString::from("JUNK"), invalid()),
                (OsString::from("NO_COLOR"), OsString::from("")),
            ],
        )
        .unwrap();
        assert_eq!(relevant.inputs.env("NO_COLOR"), Some(""));
        assert!(
            ProcessEnvSnapshot::from_vars_os(
                std::env::current_dir().unwrap(),
                OsFamily::Unix,
                [(OsString::from("NO_COLOR"), invalid())]
            )
            .is_err()
        );
    }
}
