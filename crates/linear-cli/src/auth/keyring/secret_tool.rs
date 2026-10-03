//! The Secret Service keyring through libsecret's `secret-tool`.
use super::process::{Tool, printed_key};
use super::{Keyring, LookupFailureCategory, LookupResult};
use crate::config::ConfigSecret;
use crate::error::Result;

const INSTALL_HINT: &str = "Install libsecret (e.g. `apt install libsecret-tools` or `pacman -S libsecret`), or set LINEAR_API_KEY.";

pub struct SecretTool {
    tool: Tool,
}

impl SecretTool {
    #[cfg(target_os = "linux")]
    pub fn new(overlay: crate::config::ChildEnvOverlay) -> Self {
        Self {
            tool: Tool::new("secret-tool", "secret-tool", overlay),
        }
    }

    #[cfg(test)]
    pub fn with_tool(tool: Tool) -> Self {
        Self { tool }
    }
}

fn attributes(workspace: &str) -> [String; 4] {
    [
        "service".to_owned(),
        "linear-cli".to_owned(),
        "account".to_owned(),
        workspace.to_owned(),
    ]
}

fn arguments(action: &str, workspace: &str) -> Vec<String> {
    std::iter::once(action.to_owned())
        .chain(attributes(workspace))
        .collect()
}

impl Keyring for SecretTool {
    fn get(&self, workspace: &str) -> LookupResult {
        let output = match self.tool.run(&arguments("lookup", workspace), None) {
            Ok(output) => output,
            Err(error) => return LookupResult::Failed(error.category()),
        };
        if output.status.success() {
            return printed_key(output.stdout);
        }
        // secret-tool exits 1 without a message when nothing matches.
        let missing = output.status.code() == Some(1)
            && String::from_utf8(output.stderr).is_ok_and(|stderr| stderr.trim().is_empty());
        if missing {
            LookupResult::Miss
        } else {
            LookupResult::Failed(LookupFailureCategory::Other)
        }
    }

    /// The secret goes to stdin, never to the arguments, where other users
    /// could see it.
    fn set(&self, workspace: &str, secret: &ConfigSecret) -> Result<()> {
        let args: Vec<String> = ["store".to_owned(), "--label".to_owned()]
            .into_iter()
            .chain([format!("linear-cli: {workspace}")])
            .chain(attributes(workspace))
            .collect();
        let output = self
            .tool
            .run_change(&args, Some(secret.expose().as_bytes()), INSTALL_HINT)?;
        self.tool.check(&output, "store", &[0])
    }

    fn delete(&self, workspace: &str) -> Result<()> {
        let output = self
            .tool
            .run_change(&arguments("clear", workspace), None, INSTALL_HINT)?;
        self.tool.check(&output, "clear", &[0])
    }

    /// Whether `secret-tool` runs at all; without arguments it only prints
    /// its usage.
    fn available(&self) -> bool {
        self.tool.run(&[], None).is_ok()
    }
}
