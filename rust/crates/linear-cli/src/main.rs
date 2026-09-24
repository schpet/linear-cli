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

use std::collections::BTreeMap;
use std::env;
use std::ffi::OsString;
use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::process::ExitCode;

use linear_cli::app::{AppContext, finalize, report_bootstrap_error, run};
use linear_cli::error::{AppError, AppErrorKind, ExitStatus};

type ProcessInputs = (PathBuf, BTreeMap<String, String>, Vec<String>);

fn unicode_argument(value: OsString, position: usize) -> Result<String, AppError> {
    value.into_string().map_err(|_| {
        AppError::new(
            AppErrorKind::IoProcess,
            format!("argument {position} is not valid UTF-8"),
        )
    })
}

fn unicode_environment(value: OsString, name: &str) -> Result<String, AppError> {
    value.into_string().map_err(|_| {
        AppError::new(
            AppErrorKind::IoProcess,
            format!("environment variable {name} is not valid UTF-8"),
        )
    })
}

fn relevant_environment(
    variables: impl IntoIterator<Item = (OsString, OsString)>,
) -> Result<BTreeMap<String, String>, AppError> {
    let mut relevant = BTreeMap::new();
    for (name, value) in variables {
        match name.to_str() {
            Some("NO_COLOR") => {
                relevant.insert(
                    "NO_COLOR".to_owned(),
                    unicode_environment(value, "NO_COLOR")?,
                );
            }
            Some("LINEAR_DEBUG") => {
                relevant.insert(
                    "LINEAR_DEBUG".to_owned(),
                    unicode_environment(value, "LINEAR_DEBUG")?,
                );
            }
            Some(_) | None => {}
        }
    }
    Ok(relevant)
}

fn process_inputs() -> Result<ProcessInputs, AppError> {
    let cwd = env::current_dir().map_err(|error| {
        AppError::new(AppErrorKind::IoProcess, "failed to read working directory")
            .with_source(error)
    })?;
    let environment = relevant_environment(env::vars_os())?;
    let argv = env::args_os()
        .skip(1)
        .enumerate()
        .map(|(index, value)| unicode_argument(value, index + 1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok((cwd, environment, argv))
}

fn main() -> ExitCode {
    let stdout = io::stdout();
    let stderr = io::stderr();
    let mut out = stdout.lock();
    let mut err = stderr.lock();
    let (cwd, environment, argv) = match process_inputs() {
        Ok(inputs) => inputs,
        Err(error) => {
            let _ = report_bootstrap_error(&mut err, &error);
            return ExitCode::FAILURE;
        }
    };
    let mut context = AppContext {
        env: environment,
        cwd,
        stdout: &mut out,
        stderr: &mut err,
        stdout_tty: stdout.is_terminal(),
        stderr_tty: stderr.is_terminal(),
        startup_diagnostics: Vec::new(),
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
    use super::{relevant_environment, unicode_argument, unicode_environment};
    use std::ffi::OsString;

    #[test]
    fn unicode_process_inputs_are_accepted() {
        assert_eq!(
            unicode_argument(OsString::from("issue"), 1).unwrap(),
            "issue"
        );
        assert_eq!(
            unicode_environment(OsString::from("1"), "NO_COLOR").unwrap(),
            "1"
        );
    }

    #[cfg(unix)]
    #[test]
    fn non_unicode_process_inputs_report_typed_errors() {
        use std::os::unix::ffi::OsStringExt;

        let invalid = || OsString::from_vec(vec![0xff]);
        let argument = unicode_argument(invalid(), 2).unwrap_err();
        assert_eq!(argument.display_message(), "argument 2 is not valid UTF-8");
        let environment = unicode_environment(invalid(), "NO_COLOR").unwrap_err();
        assert_eq!(
            environment.display_message(),
            "environment variable NO_COLOR is not valid UTF-8"
        );
        let relevant = relevant_environment([
            (OsString::from("JUNK"), invalid()),
            (OsString::from("NO_COLOR"), OsString::from("")),
        ])
        .unwrap();
        assert_eq!(relevant.get("NO_COLOR"), Some(&String::new()));
        assert!(relevant_environment([(OsString::from("NO_COLOR"), invalid())]).is_err());
    }
}
