//! Adding, removing and migrating stored credentials, and the files written.
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use linear_cli::auth::mutation::{Credentials, KeyringBackend};
use linear_cli::auth::{
    CredentialFormat, CredentialManifest, CredentialStore, LookupResult, parse_credentials,
};
use linear_cli::config::{ConfigSecret, RawConfigFile, parse_config_tier};
use linear_cli::error::{Error, Result};

use crate::{LookupReply, hydrate};

static NEXT: AtomicU64 = AtomicU64::new(0);

/// A credentials file in a fresh temporary directory.
struct File {
    dir: PathBuf,
    path: PathBuf,
}

impl File {
    fn new(contents: Option<&str>) -> Self {
        let dir = std::env::temp_dir().join(format!(
            "linear-credentials-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let path = dir.join("linear").join("credentials.toml");
        if let Some(contents) = contents {
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, contents).unwrap();
        }
        Self { dir, path }
    }

    fn read(&self) -> String {
        std::fs::read_to_string(&self.path).unwrap()
    }

    /// The file as the CLI reads it, with `keyring` answering lookups.
    fn store(&self, keyring: &[(&str, &str)]) -> CredentialStore {
        let manifest = manifest(&self.path);
        let replies = keyring
            .iter()
            .map(|(workspace, key)| LookupReply {
                workspace: (*workspace).to_owned(),
                result: LookupResult::Hit(secret(key)),
            })
            .collect();
        hydrate(manifest, replies).unwrap()
    }

    fn credentials(&self, store: &CredentialStore) -> Credentials {
        Credentials::new(store, &self.path)
    }
}

impl Drop for File {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn manifest(path: &Path) -> CredentialManifest {
    let Ok(bytes) = std::fs::read(path) else {
        return CredentialManifest::empty();
    };
    let tier = parse_config_tier(RawConfigFile {
        path: path.to_owned(),
        bytes,
    })
    .unwrap();
    parse_credentials(tier).unwrap()
}

fn secret(value: &str) -> ConfigSecret {
    ConfigSecret::new(value.to_owned())
}

/// A keyring that records calls and can fail stores and deletes for one workspace.
#[derive(Default)]
struct FakeKeyring {
    calls: RefCell<Vec<String>>,
    entries: RefCell<BTreeMap<String, String>>,
    fail_store: Option<&'static str>,
    fail_delete: Option<&'static str>,
}

impl KeyringBackend for FakeKeyring {
    async fn available(&self) -> bool {
        true
    }
    async fn store(&self, workspace: &str, secret: &ConfigSecret) -> Result<()> {
        self.calls.borrow_mut().push(format!("store {workspace}"));
        if self.fail_store == Some(workspace) {
            return Err(Error::new("store refused"));
        }
        self.entries
            .borrow_mut()
            .insert(workspace.to_owned(), secret.expose().to_owned());
        Ok(())
    }
    async fn delete(&self, workspace: &str) -> Result<()> {
        self.calls.borrow_mut().push(format!("delete {workspace}"));
        if self.fail_delete == Some(workspace) {
            return Err(Error::new("delete refused"));
        }
        self.entries.borrow_mut().remove(workspace);
        Ok(())
    }
}

const INLINE: &str = "default = \"beta\"\nacme = \"key-acme\"\nbeta = \"key-beta\"\n";
const KEYRING: &str = "default = \"beta\"\nworkspaces = [\"acme\", \"beta\"]\n";

#[tokio::test(flavor = "current_thread")]
async fn first_login_stores_the_key_in_the_keyring_and_lists_the_workspace() {
    let file = File::new(None);
    let store = file.store(&[]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    credentials
        .add("acme", secret("key-acme"), false, &store, &keyring)
        .await
        .unwrap();
    assert_eq!(file.read(), "default = \"acme\"\nworkspaces = [\"acme\"]\n");
    assert_eq!(keyring.entries.borrow()["acme"], "key-acme");
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file.path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn plaintext_files_stay_plaintext_and_are_sorted_with_quoted_names() {
    let file = File::new(Some("z = \"key-z\"\n"));
    let store = file.store(&[]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    assert!(credentials.stores_plaintext(false));
    for (name, key) in [("10", "key-10"), ("2", "key-2"), ("中", "key-unicode")] {
        credentials
            .add(name, secret(key), false, &store, &keyring)
            .await
            .unwrap();
    }
    assert_eq!(
        file.read(),
        "10 = \"key-10\"\n2 = \"key-2\"\nz = \"key-z\"\n\"中\" = \"key-unicode\"\n"
    );
    assert!(keyring.calls.borrow().is_empty());
    for reserved in ["default", "workspaces"] {
        credentials
            .add(reserved, secret("key"), false, &store, &keyring)
            .await
            .unwrap_err();
    }
}

#[tokio::test(flavor = "current_thread")]
async fn written_files_read_back_the_same() {
    for contents in [INLINE, KEYRING] {
        let file = File::new(Some(contents));
        let store = file.store(&[]);
        let mut credentials = file.credentials(&store);
        credentials.set_default("acme").unwrap();
        let expected = contents.replace("default = \"beta\"", "default = \"acme\"");
        assert_eq!(file.read(), expected);
        let reread = file.store(&[]);
        assert_eq!(reread.default(), Some("acme"));
        assert_eq!(reread.workspaces(), ["acme", "beta"]);
    }
}

#[tokio::test(flavor = "current_thread")]
async fn plaintext_key_added_to_a_keyring_file_moves_every_key_into_the_file() {
    let file = File::new(Some(KEYRING));
    let store = file.store(&[("acme", "key-acme"), ("beta", "key-beta")]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    credentials
        .add("gamma", secret("key-gamma"), true, &store, &keyring)
        .await
        .unwrap();
    assert_eq!(credentials.format(), CredentialFormat::Inline);
    assert_eq!(
        file.read(),
        "default = \"beta\"\nacme = \"key-acme\"\nbeta = \"key-beta\"\ngamma = \"key-gamma\"\n"
    );
    assert!(keyring.calls.borrow().is_empty());
}

#[tokio::test(flavor = "current_thread")]
async fn plaintext_conversion_fails_without_writing_when_a_keyring_key_is_unreadable() {
    let file = File::new(Some(KEYRING));
    let store = file.store(&[("acme", "key-acme")]);
    let mut credentials = file.credentials(&store);
    let error = credentials
        .add(
            "gamma",
            secret("key-gamma"),
            true,
            &store,
            &FakeKeyring::default(),
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("\"beta\""), "{error}");
    assert_eq!(file.read(), KEYRING);
}

#[tokio::test(flavor = "current_thread")]
async fn keyring_failure_on_add_leaves_the_file_alone() {
    let file = File::new(Some(KEYRING));
    let store = file.store(&[]);
    let keyring = FakeKeyring {
        fail_store: Some("gamma"),
        ..FakeKeyring::default()
    };
    let error = file
        .credentials(&store)
        .add("gamma", secret("key-gamma"), false, &store, &keyring)
        .await
        .unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("Failed to store API key in system keyring for workspace \"gamma\""),
        "{error}"
    );
    assert_eq!(file.read(), KEYRING);
}

#[tokio::test(flavor = "current_thread")]
async fn removing_the_default_picks_the_next_workspace() {
    let file = File::new(Some(KEYRING));
    let store = file.store(&[]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    credentials.remove("beta", &keyring).await.unwrap();
    assert_eq!(*keyring.calls.borrow(), ["delete beta"]);
    assert_eq!(file.read(), "default = \"acme\"\nworkspaces = [\"acme\"]\n");

    let file = File::new(Some(INLINE));
    let store = file.store(&[]);
    let mut credentials = file.credentials(&store);
    credentials.remove("acme", &keyring).await.unwrap();
    assert_eq!(file.read(), "default = \"beta\"\nbeta = \"key-beta\"\n");
    credentials.remove("beta", &keyring).await.unwrap();
    assert_eq!(file.read(), "");
    assert_eq!(*keyring.calls.borrow(), ["delete beta"]);
}

#[tokio::test(flavor = "current_thread")]
async fn keyring_failure_on_remove_leaves_the_file_alone() {
    let file = File::new(Some(KEYRING));
    let store = file.store(&[]);
    let keyring = FakeKeyring {
        fail_delete: Some("acme"),
        ..FakeKeyring::default()
    };
    file.credentials(&store)
        .remove("acme", &keyring)
        .await
        .unwrap_err();
    assert_eq!(file.read(), KEYRING);
}

#[tokio::test(flavor = "current_thread")]
async fn migrate_moves_plaintext_keys_into_the_keyring() {
    let file = File::new(Some(INLINE));
    let store = file.store(&[]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    let migrated = credentials.migrate(&keyring).await.unwrap();
    assert_eq!(migrated, ["acme", "beta"]);
    assert_eq!(file.read(), KEYRING);
    assert_eq!(keyring.entries.borrow()["acme"], "key-acme");
    assert_eq!(keyring.entries.borrow()["beta"], "key-beta");
}

#[tokio::test(flavor = "current_thread")]
async fn failed_migration_removes_the_entries_it_wrote() {
    let file = File::new(Some(INLINE));
    let store = file.store(&[]);
    let keyring = FakeKeyring {
        fail_store: Some("beta"),
        ..FakeKeyring::default()
    };
    let error = file
        .credentials(&store)
        .migrate(&keyring)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("\"beta\""), "{error}");
    assert_eq!(
        *keyring.calls.borrow(),
        ["store acme", "store beta", "delete acme"]
    );
    assert!(keyring.entries.borrow().is_empty());
    assert_eq!(file.read(), INLINE);
}
