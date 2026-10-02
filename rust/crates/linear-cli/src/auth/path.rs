use std::path::PathBuf;

use crate::config::{OsFamily, lexical_config_path};

/// Where the credentials file lives; no file is read.
pub fn credentials_path(
    os: OsFamily,
    xdg: Option<&str>,
    home: Option<&str>,
    appdata: Option<&str>,
) -> Option<PathBuf> {
    let base = match os {
        OsFamily::Unix => xdg
            .filter(|value| !value.is_empty())
            .map(|value| PathBuf::from(value).join("linear"))
            .or_else(|| {
                home.filter(|value| !value.is_empty())
                    .map(|value| PathBuf::from(value).join(".config/linear"))
            }),
        OsFamily::Windows => appdata
            .filter(|value| !value.is_empty())
            .map(|value| PathBuf::from(value).join("linear")),
    }?;
    Some(lexical_config_path(&base.join("credentials.toml")))
}
