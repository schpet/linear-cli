#![cfg(unix)]
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use linear_cli::auth::file::load;
use linear_cli::auth::keyring::UnsupportedKeyringReader;
use linear_cli::auth::{CredentialStore, LookupFailureCategory};
use linear_cli::error::Result;

static NEXT: AtomicU64 = AtomicU64::new(0);

struct Sandbox(PathBuf);

impl Sandbox {
    fn new() -> Self {
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = std::env::temp_dir().join(format!(
            "linear-credential-file-{}-{number}",
            std::process::id()
        ));
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }
}
impl Drop for Sandbox {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

fn read(path: &std::path::Path) -> Result<CredentialStore> {
    load(Some(path), Box::new(UnsupportedKeyringReader))
}

fn failure(path: &std::path::Path) -> String {
    read(path).unwrap_err().to_string()
}

#[test]
fn missing_file_is_an_empty_store_and_other_paths_fail() {
    let sandbox = Sandbox::new();
    assert!(
        read(&sandbox.0.join("missing"))
            .unwrap()
            .workspaces()
            .is_empty()
    );
    fs::write(sandbox.0.join("credentials.toml"), b"default = 'demo'\n").unwrap();
    assert!(read(&sandbox.0.join("credentials.toml")).is_ok());
    assert!(failure(&sandbox.0).contains("not a regular file"));
    assert!(failure(&sandbox.0.join("credentials.toml/child")).contains("read failed"));
}

#[test]
fn bounded_read_rejects_oversize_and_nonregular_fifo_without_blocking() {
    let sandbox = Sandbox::new();
    fs::write(sandbox.0.join("big"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
    assert!(failure(&sandbox.0.join("big")).contains("too large"));
    let fifo = sandbox.0.join("fifo");
    let status = Command::new("/usr/bin/mkfifo").arg(&fifo).status().unwrap();
    assert!(status.success());
    assert!(failure(&fifo).contains("not a regular file"));
}

#[test]
fn keyring_entries_are_read_only_when_a_key_is_needed() {
    let sandbox = Sandbox::new();
    let path = sandbox.0.join("credentials.toml");
    fs::write(&path, b"default = 'a'\nworkspaces = ['a', 'b']\n").unwrap();
    let store = read(&path).unwrap();
    assert!(store.take_warnings().is_empty());
    assert!(store.key("b").is_none());
    assert_eq!(
        store.take_warnings(),
        [linear_cli::auth::CredentialWarning::LookupFailed {
            workspace: "b".to_owned(),
            category: LookupFailureCategory::UnsupportedPlatform,
        }]
    );
}
