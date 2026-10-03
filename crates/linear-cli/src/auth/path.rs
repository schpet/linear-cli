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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn os_bases_are_used_in_order_and_the_join_is_normalized() {
        assert_eq!(
            credentials_path(OsFamily::Unix, Some("/tmp/a/../b/"), Some("/home/x"), None),
            Some(PathBuf::from("/tmp/b/linear/credentials.toml"))
        );
        assert_eq!(
            credentials_path(OsFamily::Unix, Some(""), Some("/home/x"), None),
            Some(PathBuf::from("/home/x/.config/linear/credentials.toml"))
        );
        assert_eq!(
            credentials_path(
                OsFamily::Windows,
                Some("/unused"),
                None,
                Some("C:/Users/x/AppData/Roaming")
            ),
            Some(PathBuf::from(
                "C:/Users/x/AppData/Roaming/linear/credentials.toml"
            ))
        );
        assert_eq!(
            credentials_path(
                OsFamily::Windows,
                Some("/unused"),
                Some("/unused"),
                Some("")
            ),
            None
        );
        assert_eq!(credentials_path(OsFamily::Unix, None, None, None), None);
    }

    #[test]
    fn root_is_clamped_and_unresolved_relative_parents_are_kept() {
        for (base, expected) in [
            ("/..", "/linear/credentials.toml"),
            ("../../", "../../linear/credentials.toml"),
            ("a/../../b", "../b/linear/credentials.toml"),
        ] {
            assert_eq!(
                credentials_path(OsFamily::Unix, Some(base), None, None),
                Some(PathBuf::from(expected)),
                "base {base}"
            );
        }
    }
}
