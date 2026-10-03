//! Adding, removing and migrating stored credentials, and the files written.
use std::cell::RefCell;
use std::collections::BTreeMap;

use super::*;
use crate::auth::CredentialManifest;
use crate::auth::keyring::LookupResult;
use crate::auth::test_support::{hit, manifest, store};

/// A credentials file in a fresh temporary directory.
struct File {
    _dir: tempfile::TempDir,
    path: PathBuf,
}

impl File {
    fn new(contents: Option<&str>) -> Self {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("linear").join("credentials.toml");
        if let Some(contents) = contents {
            std::fs::create_dir_all(path.parent().expect("parent")).expect("create dir");
            std::fs::write(&path, contents).expect("write credentials");
        }
        Self { _dir: dir, path }
    }

    fn read(&self) -> String {
        std::fs::read_to_string(&self.path).expect("read credentials")
    }

    /// The file as the CLI reads it, with `keyring` answering lookups.
    fn store(&self, keyring: &[(&str, &str)]) -> CredentialStore {
        let manifest = match std::fs::read_to_string(&self.path) {
            Ok(text) => manifest(&text).expect("valid credentials"),
            Err(_) => CredentialManifest::empty(),
        };
        let replies: Vec<(&str, LookupResult)> = keyring
            .iter()
            .map(|(workspace, key)| (*workspace, hit(key)))
            .collect();
        store(manifest, &replies)
    }

    fn credentials(&self, store: &CredentialStore) -> Credentials {
        Credentials::new(store, &self.path)
    }
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

impl Keyring for FakeKeyring {
    fn get(&self, _: &str) -> LookupResult {
        unreachable!("keys are read through the credential store")
    }
    fn set(&self, workspace: &str, secret: &ConfigSecret) -> Result<()> {
        self.calls.borrow_mut().push(format!("store {workspace}"));
        if self.fail_store == Some(workspace) {
            return Err(Error::new("store refused"));
        }
        self.entries
            .borrow_mut()
            .insert(workspace.to_owned(), secret.expose().to_owned());
        Ok(())
    }
    fn delete(&self, workspace: &str) -> Result<()> {
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

#[test]
fn first_login_stores_the_key_in_the_keyring_and_lists_the_workspace() {
    let file = File::new(None);
    let store = file.store(&[]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    credentials
        .add("acme", secret("key-acme"), false, &store, &keyring)
        .expect("should succeed");
    assert_eq!(file.read(), "default = \"acme\"\nworkspaces = [\"acme\"]\n");
    assert_eq!(
        keyring
            .entries
            .borrow()
            .get("acme")
            .expect("stored")
            .as_str(),
        "key-acme"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&file.path)
            .expect("should succeed")
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }
}

#[test]
fn plaintext_files_stay_plaintext_and_are_sorted_with_quoted_names() {
    let file = File::new(Some("z = \"key-z\"\n"));
    let store = file.store(&[]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    assert!(credentials.stores_plaintext(false));
    for (name, key) in [("10", "key-10"), ("2", "key-2"), ("中", "key-unicode")] {
        credentials
            .add(name, secret(key), false, &store, &keyring)
            .expect("should succeed");
    }
    assert_eq!(
        file.read(),
        "10 = \"key-10\"\n2 = \"key-2\"\nz = \"key-z\"\n\"中\" = \"key-unicode\"\n"
    );
    assert!(keyring.calls.borrow().is_empty());
    for reserved in ["default", "workspaces"] {
        credentials
            .add(reserved, secret("key"), false, &store, &keyring)
            .expect_err("should fail");
    }
}

#[test]
fn written_files_read_back_the_same() {
    for contents in [INLINE, KEYRING] {
        let file = File::new(Some(contents));
        let store = file.store(&[]);
        let mut credentials = file.credentials(&store);
        credentials.set_default("acme").expect("should succeed");
        let expected = contents.replace("default = \"beta\"", "default = \"acme\"");
        assert_eq!(file.read(), expected);
        let reread = file.store(&[]);
        assert_eq!(reread.default(), Some("acme"));
        assert_eq!(reread.workspaces(), ["acme", "beta"]);
    }
}

#[test]
fn plaintext_key_added_to_a_keyring_file_moves_every_key_into_the_file() {
    let file = File::new(Some(KEYRING));
    let store = file.store(&[("acme", "key-acme"), ("beta", "key-beta")]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    credentials
        .add("gamma", secret("key-gamma"), true, &store, &keyring)
        .expect("should succeed");
    assert_eq!(credentials.format(), CredentialFormat::Inline);
    assert_eq!(
        file.read(),
        "default = \"beta\"\nacme = \"key-acme\"\nbeta = \"key-beta\"\ngamma = \"key-gamma\"\n"
    );
    assert!(keyring.calls.borrow().is_empty());
}

#[test]
fn plaintext_conversion_fails_without_writing_when_a_keyring_key_is_unreadable() {
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
        .expect_err("should fail");
    assert!(error.to_string().contains("\"beta\""), "{error}");
    assert_eq!(file.read(), KEYRING);
}

#[test]
fn keyring_failure_on_add_leaves_the_file_alone() {
    let file = File::new(Some(KEYRING));
    let store = file.store(&[]);
    let keyring = FakeKeyring {
        fail_store: Some("gamma"),
        ..FakeKeyring::default()
    };
    let error = file
        .credentials(&store)
        .add("gamma", secret("key-gamma"), false, &store, &keyring)
        .expect_err("should fail");
    assert!(
        error
            .to_string()
            .starts_with("Failed to store API key in system keyring for workspace \"gamma\""),
        "{error}"
    );
    assert_eq!(file.read(), KEYRING);
}

#[test]
fn removing_the_default_picks_the_next_workspace() {
    let file = File::new(Some(KEYRING));
    let store = file.store(&[]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    credentials
        .remove("beta", &keyring)
        .expect("should succeed");
    assert_eq!(*keyring.calls.borrow(), ["delete beta"]);
    assert_eq!(file.read(), "default = \"acme\"\nworkspaces = [\"acme\"]\n");

    let file = File::new(Some(INLINE));
    let store = file.store(&[]);
    let mut credentials = file.credentials(&store);
    credentials
        .remove("acme", &keyring)
        .expect("should succeed");
    assert_eq!(file.read(), "default = \"beta\"\nbeta = \"key-beta\"\n");
    credentials
        .remove("beta", &keyring)
        .expect("should succeed");
    assert_eq!(file.read(), "");
    assert_eq!(*keyring.calls.borrow(), ["delete beta"]);
}

#[test]
fn keyring_failure_on_remove_leaves_the_file_alone() {
    let file = File::new(Some(KEYRING));
    let store = file.store(&[]);
    let keyring = FakeKeyring {
        fail_delete: Some("acme"),
        ..FakeKeyring::default()
    };
    file.credentials(&store)
        .remove("acme", &keyring)
        .expect_err("should fail");
    assert_eq!(file.read(), KEYRING);
}

#[test]
fn migrate_moves_plaintext_keys_into_the_keyring() {
    let file = File::new(Some(INLINE));
    let store = file.store(&[]);
    let keyring = FakeKeyring::default();
    let mut credentials = file.credentials(&store);
    let migrated = credentials.migrate(&keyring).expect("should succeed");
    assert_eq!(migrated, ["acme", "beta"]);
    assert_eq!(file.read(), KEYRING);
    assert_eq!(
        keyring
            .entries
            .borrow()
            .get("acme")
            .expect("stored")
            .as_str(),
        "key-acme"
    );
    assert_eq!(
        keyring
            .entries
            .borrow()
            .get("beta")
            .expect("stored")
            .as_str(),
        "key-beta"
    );
}

#[test]
fn failed_migration_removes_the_entries_it_wrote() {
    let file = File::new(Some(INLINE));
    let store = file.store(&[]);
    let keyring = FakeKeyring {
        fail_store: Some("beta"),
        ..FakeKeyring::default()
    };
    let error = file
        .credentials(&store)
        .migrate(&keyring)
        .expect_err("should fail");
    assert!(error.to_string().contains("\"beta\""), "{error}");
    assert_eq!(
        *keyring.calls.borrow(),
        ["store acme", "store beta", "delete acme"]
    );
    assert!(keyring.entries.borrow().is_empty());
    assert_eq!(file.read(), INLINE);
}
