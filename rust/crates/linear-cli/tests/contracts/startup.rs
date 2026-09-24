use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use linear_cli::config::{
    FileKind, FileSource, GitProbeResult, GitRootProbe, OsFamily, ProcessEnvSnapshot,
    StartupReport, load_startup,
};

struct EmptyFiles;

impl FileSource for EmptyFiles {
    fn kind(&self, _path: &Path) -> io::Result<Option<FileKind>> {
        Ok(None)
    }

    fn read_bounded(&self, _path: &Path, _max_bytes: u64) -> io::Result<Vec<u8>> {
        Err(io::Error::from(io::ErrorKind::NotFound))
    }
}

struct NoGit;

impl GitRootProbe for NoGit {
    fn probe(&self) -> GitProbeResult {
        GitProbeResult::SpawnFailure
    }
}

pub fn empty_startup(cwd: PathBuf, env: &[(&str, &str)]) -> StartupReport {
    let os = if cfg!(windows) {
        OsFamily::Windows
    } else {
        OsFamily::Unix
    };
    let variables = env
        .iter()
        .map(|(name, value)| (OsString::from(name), OsString::from(value)));
    let process = ProcessEnvSnapshot::from_vars_os(cwd, os, variables)
        .expect("synthetic process environment is valid");
    let startup = load_startup(&process, &EmptyFiles, &NoGit);
    assert!(startup.result.is_ok(), "synthetic config must load");
    startup
}

static NEXT_SANDBOX: AtomicU64 = AtomicU64::new(0);

pub struct BinarySandbox {
    root: PathBuf,
}

impl BinarySandbox {
    pub fn new() -> Self {
        let tick = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let number = NEXT_SANDBOX.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "linear-contract-{}-{tick}-{number}",
            std::process::id()
        ));
        for name in ["cwd", "home", "bin"] {
            std::fs::create_dir_all(root.join(name)).expect("create private binary test directory");
        }
        Self { root }
    }

    pub fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_linear"));
        command
            .env_clear()
            .current_dir(self.root.join("cwd"))
            .env("HOME", self.root.join("home"))
            .env("XDG_CONFIG_HOME", self.root.join("home"))
            .env("APPDATA", self.root.join("home"))
            .env("PATH", self.root.join("bin"))
            .env("LINEAR_IGNORE_ENV_FILE", "1");
        command
    }
}

impl Drop for BinarySandbox {
    fn drop(&mut self) {
        if let Err(error) = std::fs::remove_dir_all(&self.root) {
            eprintln!(
                "failed to remove binary test sandbox {}: {error}",
                self.root.display()
            );
        }
    }
}
