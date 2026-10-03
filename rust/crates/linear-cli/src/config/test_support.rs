//! A private directory tree for config discovery tests.
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use super::{ConfigInputs, OsFamily};

pub(crate) struct TempTree(tempfile::TempDir);

impl TempTree {
    pub(crate) fn new() -> Self {
        Self(tempfile::tempdir().expect("temp dir"))
    }

    pub(crate) fn path(&self) -> &Path {
        self.0.path()
    }

    pub(crate) fn join(&self, relative: &str) -> PathBuf {
        self.path().join(relative)
    }

    pub(crate) fn write(&self, relative: &str, bytes: &[u8]) {
        let path = self.join(relative);
        fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
        fs::write(path, bytes).expect("write file");
    }

    pub(crate) fn mkdir(&self, relative: &str) {
        fs::create_dir_all(self.join(relative)).expect("create dir");
    }

    /// Unix inputs with the tree as the working directory and no environment.
    pub(crate) fn inputs(&self) -> ConfigInputs {
        ConfigInputs {
            cwd: self.path().to_owned(),
            os: OsFamily::Unix,
            process_env: BTreeMap::new(),
        }
    }
}

/// An absolute fixture path in the host platform's spelling.
pub(crate) fn fixture_path(path: &str) -> String {
    assert!(path.starts_with('/'), "fixture paths must be rooted");
    if cfg!(windows) {
        format!("C:{}", path.replace('/', "\\"))
    } else {
        path.to_owned()
    }
}
