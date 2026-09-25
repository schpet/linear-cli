#![cfg(unix)]
use std::fs;
use std::io::ErrorKind;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use linear_cli::auth::file::{
    CredentialFileSource, CredentialReadFailure, RealCredentialFileSource,
};

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

#[test]
fn missing_regular_directory_and_enotdir_have_distinct_results() {
    let sandbox = Sandbox::new();
    let reader = RealCredentialFileSource;
    assert_eq!(
        reader.read_credentials(&sandbox.0.join("missing")).unwrap(),
        None
    );
    fs::write(sandbox.0.join("credentials.toml"), b"default = 'demo'\n").unwrap();
    assert_eq!(
        reader
            .read_credentials(&sandbox.0.join("credentials.toml"))
            .unwrap(),
        Some(b"default = 'demo'\n".to_vec())
    );
    assert_eq!(
        reader.read_credentials(&sandbox.0).unwrap_err(),
        CredentialReadFailure::NotRegular
    );
    assert_eq!(
        reader
            .read_credentials(&sandbox.0.join("credentials.toml/child"))
            .unwrap_err(),
        CredentialReadFailure::Io(ErrorKind::NotADirectory)
    );
}

#[test]
fn bounded_read_rejects_oversize_and_nonregular_fifo_without_blocking() {
    let sandbox = Sandbox::new();
    let reader = RealCredentialFileSource;
    fs::write(sandbox.0.join("big"), vec![b'x'; 1024 * 1024 + 1]).unwrap();
    assert_eq!(
        reader.read_credentials(&sandbox.0.join("big")).unwrap_err(),
        CredentialReadFailure::TooLarge
    );
    let fifo = sandbox.0.join("fifo");
    let status = Command::new("/usr/bin/mkfifo").arg(&fifo).status().unwrap();
    assert!(status.success());
    assert_eq!(
        reader.read_credentials(&fifo).unwrap_err(),
        CredentialReadFailure::NotRegular
    );
}
