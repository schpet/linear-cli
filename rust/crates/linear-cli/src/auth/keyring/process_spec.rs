//! Pure process credential argv shared by native selection and confined tests.
use crate::config::ConfigSecret;

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
    pub fn store_arguments(self, workspace: &str, secret: &ConfigSecret) -> Vec<String> {
        match self {
            Self::SecretTool => vec![
                "store".to_owned(),
                "--label".to_owned(),
                format!("linear-cli: {workspace}"),
                "service".to_owned(),
                "linear-cli".to_owned(),
                "account".to_owned(),
                workspace.to_owned(),
            ],
            Self::MacSecurity => [
                "add-generic-password",
                "-a",
                workspace,
                "-s",
                "linear-cli",
                "-w",
                secret.expose(),
                "-U",
            ]
            .into_iter()
            .map(str::to_owned)
            .collect(),
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
