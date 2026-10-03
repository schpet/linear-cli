//! The macOS login keychain through `/usr/bin/security`. Going through that
//! tool keeps existing keychain items readable without a new access prompt,
//! because their access lists already trust it.
use super::process::{Tool, printed_key};
use super::{Keyring, LookupFailureCategory, LookupResult};
use crate::config::ConfigSecret;
use crate::error::{Error, Result};

const HINT: &str = "Set LINEAR_API_KEY instead.";

/// `security` exits with this status when no matching item exists.
const NOT_FOUND: i32 = 44;

pub struct Security {
    tool: Tool,
}

impl Security {
    #[cfg(target_os = "macos")]
    pub fn new(overlay: crate::config::ChildEnvOverlay) -> Self {
        Self {
            tool: Tool::new("security", "/usr/bin/security", overlay),
        }
    }

    #[cfg(test)]
    pub fn with_tool(tool: Tool) -> Self {
        Self { tool }
    }
}

fn arguments(action: &str, workspace: &str) -> Vec<String> {
    [action, "-a", workspace, "-s", "linear-cli"]
        .into_iter()
        .map(str::to_owned)
        .collect()
}

/// Whether `value` needs no quoting in a `security -i` command line, which
/// is split on whitespace. API keys and workspace slugs never do.
fn plain(value: &str) -> bool {
    !value.is_empty()
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
}

impl Keyring for Security {
    fn get(&self, workspace: &str) -> LookupResult {
        let mut args = arguments("find-generic-password", workspace);
        args.push("-w".to_owned());
        match self.tool.run(&args, None) {
            Ok(output) if output.status.success() => printed_key(output.stdout),
            Ok(output) if output.status.code() == Some(NOT_FOUND) => LookupResult::Miss,
            Ok(_) => LookupResult::Failed(LookupFailureCategory::Other),
            Err(error) => LookupResult::Failed(error.category()),
        }
    }

    /// `security -i` reads the command from stdin, so the secret never
    /// appears in the arguments, where other users could see it.
    fn set(&self, workspace: &str, secret: &ConfigSecret) -> Result<()> {
        if !plain(workspace) || !plain(secret.expose()) {
            return Err(Error::new(
                "The API key or workspace name has characters the keychain tool cannot take",
            )
            .with_hint(
                "Linear API keys and workspace names use only letters, digits, '_', '-' and '.'.",
            ));
        }
        let input = format!(
            "add-generic-password -U -a {workspace} -s linear-cli -w {}\n",
            secret.expose()
        );
        let output = self
            .tool
            .run_change(&["-i".to_owned()], Some(input.as_bytes()), HINT)?;
        self.tool.check(&output, "add-generic-password", &[0])
    }

    fn delete(&self, workspace: &str) -> Result<()> {
        let output =
            self.tool
                .run_change(&arguments("delete-generic-password", workspace), None, HINT)?;
        self.tool
            .check(&output, "delete-generic-password", &[0, NOT_FOUND])
    }
}
