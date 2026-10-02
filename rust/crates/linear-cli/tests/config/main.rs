mod discover;
mod dotenv;
mod options;
mod parse;
mod runtime;
mod startup;
mod transport;

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use linear_cli::config::{ConfigInputs, OsFamily};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// A private temporary directory, removed on drop.
pub(crate) struct TempTree(pub PathBuf);

impl TempTree {
    pub fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "linear-cli-config-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    pub fn write(&self, suffix: &str, bytes: &[u8]) {
        let path = self.0.join(suffix);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, bytes).unwrap();
    }

    pub fn mkdir(&self, suffix: &str) {
        fs::create_dir_all(self.0.join(suffix)).unwrap();
    }

    pub fn inputs(&self) -> ConfigInputs {
        ConfigInputs {
            cwd: self.0.clone(),
            os: OsFamily::Unix,
            process_env: BTreeMap::new(),
        }
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
