//! Editing text in the user's editor.
//!
//! The editor is `VISUAL`, then `EDITOR`, then git's `core.editor`. Like git,
//! the value is run by the shell, so `EDITOR="code --wait"` works.
use std::ffi::OsString;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};

use crate::commands::text_input::read_file;
use crate::config::ChildEnvOverlay;
use crate::error::{Error, Result};
use crate::platform::interrupt::InterruptShield;

/// Opens `initial` in the editor and returns the saved text.
pub fn edit(initial: &str, env: &ChildEnvOverlay) -> Result<String> {
    let editor = configured(env).ok_or_else(|| {
        Error::new("No editor configured")
            .with_hint("Set the EDITOR environment variable, or git's core.editor.")
    })?;
    let file = TempFile::create(&std::env::temp_dir())?;
    fs::write(&file.0, initial).map_err(|error| {
        Error::new(format!("Failed to write editor file: {error}")).with_source(error)
    })?;
    let status = {
        let _shield = InterruptShield::raise()?;
        editor_command(&editor, &file.0)
            .envs(env.iter())
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .map_err(|error| {
                Error::new(format!("Failed to open editor: {error}")).with_source(error)
            })?
    };
    if interrupted(status) {
        return Err(Error::cancelled());
    }
    if !status.success() {
        return Err(Error::new(format!(
            "Editor exited with an error ({status})"
        )));
    }
    read_file(&file.0).map_err(|error| {
        Error::new(format!("Failed to read editor file: {error}")).with_source(error)
    })
}

#[cfg(unix)]
fn interrupted(status: std::process::ExitStatus) -> bool {
    use std::os::unix::process::ExitStatusExt;
    status.signal() == Some(signal_hook::consts::SIGINT)
}

#[cfg(not(unix))]
fn interrupted(_status: std::process::ExitStatus) -> bool {
    false
}

/// The configured editor command, if any.
pub fn configured(env: &ChildEnvOverlay) -> Option<OsString> {
    ["VISUAL", "EDITOR"]
        .into_iter()
        .find_map(|name| std::env::var_os(name).filter(|value| !value.is_empty()))
        .or_else(|| git_editor(env))
}

/// The configured editor's program name (`vim` for `/usr/bin/vim -f`), for
/// prompts that offer to open it.
pub fn configured_name(env: &ChildEnvOverlay) -> Option<String> {
    program_name(&configured(env)?.to_string_lossy())
}

fn program_name(command: &str) -> Option<String> {
    let program = command.split_whitespace().next()?;
    Path::new(program)
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
}

fn git_editor(env: &ChildEnvOverlay) -> Option<OsString> {
    let output = Command::new("git")
        .args(["config", "core.editor"])
        .envs(env.iter())
        .stdin(Stdio::null())
        .stderr(Stdio::null())
        .output()
        .ok()
        .filter(|output| output.status.success())?;
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| OsString::from(value))
}

#[cfg(unix)]
fn editor_command(editor: &OsString, file: &Path) -> Command {
    let mut script = editor.clone();
    script.push(" \"$@\"");
    let mut command = Command::new("/bin/sh");
    command.arg("-c").arg(script).arg(editor).arg(file);
    command
}

#[cfg(not(unix))]
fn editor_command(editor: &OsString, file: &Path) -> Command {
    let mut command = Command::new("cmd");
    command.arg("/C").arg(editor).arg(file);
    command
}

/// A private file removed when dropped.
struct TempFile(PathBuf);

impl TempFile {
    fn create(root: &Path) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        loop {
            let path = root.join(format!(
                "linear-{}-{}.md",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            match options.open(&path) {
                Ok(mut file) => {
                    file.flush().map_err(temp_error)?;
                    return Ok(Self(path));
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(error) => return Err(temp_error(error)),
            }
        }
    }
}

fn temp_error(error: std::io::Error) -> Error {
    Error::new(format!("Failed to create editor file: {error}")).with_source(error)
}

impl Drop for TempFile {
    fn drop(&mut self) {
        // Best effort: the file lives in the system temporary directory.
        let _ignored = fs::remove_file(&self.0);
    }
}

#[cfg(test)]
mod tests {
    use super::program_name;

    #[test]
    fn program_name_is_the_first_word_without_its_directory() {
        assert_eq!(program_name("/usr/bin/vim").as_deref(), Some("vim"));
        assert_eq!(program_name("code --wait").as_deref(), Some("code"));
        assert_eq!(program_name("  "), None);
    }
}
