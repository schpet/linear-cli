//! `linear completions <shell>`: the script that registers `linear` with a
//! shell's completion system. The script asks the binary for candidates on
//! each completion (`COMPLETE=<shell> linear -- <words>`, handled in `main`),
//! so completions follow the installed version at every command depth.
use crate::cli::completions::Completions;
use crate::error::{Error, Result};
use clap_complete::env::{EnvCompleter, Shells};

pub const DEFAULT_COMMAND_NAME: &str = "linear";

/// The environment variable that switches the binary into completion mode.
const COMPLETE_VAR: &str = "COMPLETE";

pub fn script(args: &Completions) -> Result<Vec<u8>> {
    let shell_name = args.shell.to_string();
    let shells = Shells::builtins();
    let shell: &dyn EnvCompleter = shells
        .completer(&shell_name)
        .expect("every clap_complete shell has a dynamic completer");
    let bin = args.name.as_deref().unwrap_or(DEFAULT_COMMAND_NAME);
    let completer = completer()?;
    let mut output = Vec::new();
    shell
        .write_registration(
            COMPLETE_VAR,
            DEFAULT_COMMAND_NAME,
            bin,
            &completer,
            &mut output,
        )
        .expect("writing to a Vec cannot fail");
    Ok(output)
}

/// How the shell should invoke this binary for candidates: the name it was
/// run as when that is a bare command found on PATH, otherwise its absolute
/// path.
fn completer() -> Result<String> {
    let argv0 = std::env::args_os()
        .next()
        .ok_or_else(|| Error::new("The program was started without a name"))?;
    let path = std::path::PathBuf::from(&argv0);
    let path = if path.components().count() > 1 && path.is_relative() {
        std::env::current_dir()
            .map_err(|error| {
                Error::new(format!("Could not read the current directory: {error}"))
                    .with_source(error)
            })?
            .join(path)
    } else {
        path
    };
    path.into_os_string()
        .into_string()
        .map_err(|_| Error::new("The program path is not valid UTF-8"))
}
