//! Command lines for the keyring tools. Accounts are `service=linear-cli`
//! plus the workspace name.
use crate::config::ConfigSecret;

/// A keyring tool invocation that receives the secret on stdin.
pub struct StoreCommand {
    pub arguments: Vec<String>,
    pub input: Vec<u8>,
}

/// The workspace or secret holds characters `security -i` would need quoted.
#[derive(Debug)]
pub struct UnquotableValue;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReaderFlavor {
    SecretTool,
    MacSecurity,
}
impl ReaderFlavor {
    pub const fn executable(self) -> &'static str {
        match self {
            Self::SecretTool => "secret-tool",
            Self::MacSecurity => "/usr/bin/security",
        }
    }
    pub fn lookup_arguments(self, workspace: &str) -> Vec<String> {
        match self {
            Self::SecretTool => ["lookup", "service", "linear-cli", "account", workspace]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            Self::MacSecurity => [
                "find-generic-password",
                "-a",
                workspace,
                "-s",
                "linear-cli",
                "-w",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }
    }
    pub const fn store_action(self) -> &'static str {
        match self {
            Self::SecretTool => "store",
            Self::MacSecurity => "add-generic-password",
        }
    }
    /// How to store `secret`: the tool's arguments and its stdin. The secret
    /// never appears in the arguments, where other users could see it.
    pub fn store_command(
        self,
        workspace: &str,
        secret: &ConfigSecret,
    ) -> Result<StoreCommand, UnquotableValue> {
        match self {
            Self::SecretTool => Ok(StoreCommand {
                arguments: vec![
                    "store".to_owned(),
                    "--label".to_owned(),
                    format!("linear-cli: {workspace}"),
                    "service".to_owned(),
                    "linear-cli".to_owned(),
                    "account".to_owned(),
                    workspace.to_owned(),
                ],
                input: secret.expose().as_bytes().to_vec(),
            }),
            // `security -i` runs commands read from stdin, splitting each line
            // on whitespace. Values are limited to characters that never need
            // quoting there; API keys and workspace slugs always are.
            Self::MacSecurity => {
                let plain = |value: &str| {
                    !value.is_empty()
                        && value
                            .chars()
                            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
                };
                if !plain(workspace) || !plain(secret.expose()) {
                    return Err(UnquotableValue);
                }
                Ok(StoreCommand {
                    arguments: vec!["-i".to_owned()],
                    input: format!(
                        "add-generic-password -U -a {workspace} -s linear-cli -w {}\n",
                        secret.expose()
                    )
                    .into_bytes(),
                })
            }
        }
    }
    pub const fn delete_action(self) -> &'static str {
        match self {
            Self::SecretTool => "clear",
            Self::MacSecurity => "delete-generic-password",
        }
    }
    pub fn delete_arguments(self, workspace: &str) -> Vec<String> {
        match self {
            Self::SecretTool => ["clear", "service", "linear-cli", "account", workspace]
                .into_iter()
                .map(str::to_owned)
                .collect(),
            Self::MacSecurity => [
                "delete-generic-password",
                "-a",
                workspace,
                "-s",
                "linear-cli",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workspace_names_are_passed_literally() {
        let workspace = "dummy space;'中";
        assert_eq!(
            ReaderFlavor::MacSecurity.lookup_arguments(workspace),
            [
                "find-generic-password",
                "-a",
                workspace,
                "-s",
                "linear-cli",
                "-w"
            ]
        );
        assert_eq!(
            ReaderFlavor::SecretTool.lookup_arguments(workspace),
            ["lookup", "service", "linear-cli", "account", workspace]
        );
        assert_eq!(
            ReaderFlavor::MacSecurity.delete_arguments(workspace),
            [
                "delete-generic-password",
                "-a",
                workspace,
                "-s",
                "linear-cli"
            ]
        );
        assert_eq!(
            ReaderFlavor::SecretTool.delete_arguments(workspace),
            ["clear", "service", "linear-cli", "account", workspace]
        );
    }

    #[test]
    fn secrets_go_to_stdin_never_to_arguments() {
        let workspace = "dummy space;'中";
        let secret = ConfigSecret::new("dummy key\n中".to_owned());
        let secret_tool = ReaderFlavor::SecretTool
            .store_command(workspace, &secret)
            .expect("secret-tool accepts any value");
        assert_eq!(secret_tool.input, "dummy key\n中".as_bytes());
        assert_eq!(
            secret_tool.arguments,
            [
                "store",
                "--label",
                &format!("linear-cli: {workspace}"),
                "service",
                "linear-cli",
                "account",
                workspace
            ]
        );

        // `security -i` would split these on whitespace.
        assert!(
            ReaderFlavor::MacSecurity
                .store_command(workspace, &secret)
                .is_err()
        );
        let plain = ConfigSecret::new("lin_api_ab-1.2".to_owned());
        let mac = ReaderFlavor::MacSecurity
            .store_command("acme-co", &plain)
            .expect("plain values");
        assert_eq!(mac.arguments, ["-i"]);
        assert_eq!(
            mac.input,
            b"add-generic-password -U -a acme-co -s linear-cli -w lin_api_ab-1.2\n"
        );
    }
}
