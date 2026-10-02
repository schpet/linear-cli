//! Credential add, remove and migrate effects.
use crate::{LookupReply, hydrate};
use linear_cli::error::ErrorKind;
use linear_cli::{
    auth::{
        CredentialFormat, LookupResult,
        mutation::{
            CredentialMutationBackend, CredentialMutationFileWriter, CredentialMutationState,
            MutationFailure,
        },
        parse_credentials,
    },
    commands::{auth_login, auth_logout},
    config::{ConfigSecret, RawConfigFile, parse_config_tier},
    error::Error,
};
use std::{
    cell::RefCell,
    collections::BTreeMap,
    io,
    path::{Path, PathBuf},
};
fn state(text: &str, keys: &[(&str, &str)]) -> CredentialMutationState {
    let manifest = parse_credentials(
        parse_config_tier(RawConfigFile {
            path: PathBuf::from("/private/dummy/credentials.toml"),
            bytes: text.as_bytes().to_vec(),
        })
        .unwrap(),
    )
    .unwrap();
    let replies = keys
        .iter()
        .map(|(name, key)| LookupReply {
            workspace: (*name).to_owned(),
            result: LookupResult::Hit(ConfigSecret::new((*key).to_owned())),
        })
        .collect();
    CredentialMutationState::from_store(&hydrate(manifest, replies).unwrap())
}
struct FakeBackend {
    events: RefCell<Vec<String>>,
    entries: RefCell<BTreeMap<String, String>>,
    fail_store: Option<&'static str>,
    fail_delete: Option<&'static str>,
}
impl FakeBackend {
    fn new() -> Self {
        Self {
            events: RefCell::new(Vec::new()),
            entries: RefCell::new(BTreeMap::new()),
            fail_store: None,
            fail_delete: None,
        }
    }
}
impl CredentialMutationBackend for FakeBackend {
    async fn available(&self) -> bool {
        self.events.borrow_mut().push("available".into());
        true
    }
    async fn store(&self, name: &str, key: &ConfigSecret) -> Result<(), MutationFailure> {
        self.events.borrow_mut().push(format!("store:{name}"));
        if self.fail_store == Some(name) {
            return Err(MutationFailure::Ordinary("dummy store401fault".into()));
        }
        self.entries
            .borrow_mut()
            .insert(name.to_owned(), key.expose().to_owned());
        Ok(())
    }
    async fn delete(&self, name: &str) -> Result<(), MutationFailure> {
        self.events.borrow_mut().push(format!("delete:{name}"));
        if self.fail_delete == Some(name) {
            return Err(MutationFailure::Ordinary("dummy delete fault".into()));
        }
        self.entries.borrow_mut().remove(name);
        Ok(())
    }
}
struct FakeFile {
    events: RefCell<Vec<&'static str>>,
    contents: RefCell<Vec<u8>>,
    fail_write: bool,
}
impl FakeFile {
    fn new() -> Self {
        Self {
            events: RefCell::new(Vec::new()),
            contents: RefCell::new(Vec::new()),
            fail_write: false,
        }
    }
}
impl CredentialMutationFileWriter for FakeFile {
    fn prepare_directory(&self, _: &Path) -> io::Result<()> {
        self.events.borrow_mut().push("mkdir");
        Ok(())
    }
    fn write_file(&self, _: &Path, bytes: &[u8]) -> io::Result<()> {
        self.events.borrow_mut().push("write");
        if self.fail_write {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "dummy write denied",
            ));
        }
        *self.contents.borrow_mut() = bytes.to_vec();
        Ok(())
    }
}
fn secret(value: &str) -> ConfigSecret {
    ConfigSecret::new(value.to_owned())
}
fn path() -> Option<&'static Path> {
    Some(Path::new("/private/dummy/credentials.toml"))
}

#[tokio::test(flavor = "current_thread")]
async fn metadata_to_plaintext_preserves_old_keys_and_stale_in_memory_format_no_offer() {
    let mut store = state(
        "default='alpha'\nworkspaces=['alpha']",
        &[("alpha", "dummy_old")],
    );
    let backend = FakeBackend::new();
    let file = FakeFile::new();
    store
        .add(
            "beta",
            secret("dummy_new"),
            Some(true),
            path(),
            &backend,
            &file,
        )
        .await
        .unwrap_or_else(|_| panic!("add"));
    assert_eq!(store.format(), CredentialFormat::Metadata);
    assert_eq!(
        *file.contents.borrow(),
        b"default = \"alpha\"\nalpha = \"dummy_old\"\nbeta = \"dummy_new\"\n"
    );
    assert!(!auth_login::offer_migration(&store, &backend).await);
    assert!(backend.events.borrow().is_empty());
}
#[tokio::test(flavor = "current_thread")]
async fn default_absence_none_plaintext_preserves_inline_and_last_removal_writes_empty_file() {
    let mut store = state("alpha='dummy_old'", &[]);
    let backend = FakeBackend::new();
    let file = FakeFile::new();
    store
        .add("alpha", secret("dummy_new"), None, path(), &backend, &file)
        .await
        .unwrap_or_else(|_| panic!("add"));
    assert_eq!(store.default(), None);
    assert_eq!(store.format(), CredentialFormat::Inline);
    assert!(backend.events.borrow().is_empty());
    store
        .remove("alpha", path(), &backend, &file)
        .await
        .unwrap_or_else(|_| panic!("remove"));
    assert!(file.contents.borrow().is_empty());
}
#[tokio::test(flavor = "current_thread")]
async fn failed_inline_conversion_prepares_directory_before_missing_cache_without_store_or_write() {
    let manifest = parse_credentials(
        parse_config_tier(RawConfigFile {
            path: PathBuf::from("/dummy"),
            bytes: b"workspaces=['alpha']".to_vec(),
        })
        .unwrap(),
    )
    .unwrap();
    let mut store = CredentialMutationState::from_store(
        &hydrate(
            manifest,
            vec![LookupReply {
                workspace: "alpha".into(),
                result: LookupResult::Miss,
            }],
        )
        .unwrap(),
    );
    let backend = FakeBackend::new();
    let file = FakeFile::new();
    let error = store
        .add(
            "beta",
            secret("dummy_new"),
            Some(true),
            path(),
            &backend,
            &file,
        )
        .await
        .err()
        .unwrap()
        .outer();
    assert!(error.message().contains("alpha"));
    assert_eq!(*file.events.borrow(), ["mkdir"]);
    assert!(backend.events.borrow().is_empty());
}
#[tokio::test(flavor = "current_thread")]
async fn rollback_is_forward_best_effort_deletes_overwritten_entry_and_retains_failed_cleanup() {
    let mut store = state("a='dummy_a'\nb='dummy_b'\nc='dummy_c'", &[]);
    let mut backend = FakeBackend::new();
    backend.fail_store = Some("c");
    backend.fail_delete = Some("b");
    backend
        .entries
        .borrow_mut()
        .insert("a".into(), "dummy_preexisting".into());
    let file = FakeFile::new();
    let error = store
        .migrate(path(), &backend, &file)
        .await
        .err()
        .unwrap()
        .outer();
    assert_eq!(
        *backend.events.borrow(),
        ["store:a", "store:b", "store:c", "delete:a", "delete:b"]
    );
    assert!(!backend.entries.borrow().contains_key("a"));
    assert_eq!(backend.entries.borrow().get("b").unwrap(), "dummy_b");
    assert!(
        error
            .message()
            .ends_with("Rolled back 2 already-written entries.")
    );
    assert!(file.events.borrow().is_empty());
}
#[tokio::test(flavor = "current_thread")]
async fn successful_migration_keeps_stale_default_and_poststore_write_failure_does_not_rollback() {
    let mut store = state("default='stale'\nb='dummy_b'\na='dummy_a'", &[]);
    let backend = FakeBackend::new();
    let mut file = FakeFile::new();
    file.fail_write = true;
    assert!(store.migrate(path(), &backend, &file).await.is_err());
    assert_eq!(*backend.events.borrow(), ["store:b", "store:a"]);
    assert_eq!(backend.entries.borrow().len(), 2);
    assert_eq!(store.default(), Some("stale"));
    assert_eq!(store.format(), CredentialFormat::Metadata);
}
#[tokio::test(flavor = "current_thread")]
async fn metadata_logout_clear_precedes_write_failure_and_force_never_bypasses_picker() {
    let mut store = state(
        "default='b'\nworkspaces=['a','b']",
        &[("a", "dummy_a"), ("b", "dummy_b")],
    );
    assert!(matches!(
        auth_logout::prepare(&store, None).unwrap(),
        auth_logout::LogoutTarget::Select(_)
    ));
    let backend = FakeBackend::new();
    backend
        .entries
        .borrow_mut()
        .insert("b".into(), "dummy_b".into());
    let mut file = FakeFile::new();
    file.fail_write = true;
    *file.contents.borrow_mut() = b"old metadata".to_vec();
    assert!(store.remove("b", path(), &backend, &file).await.is_err());
    assert_eq!(*backend.events.borrow(), ["delete:b"]);
    assert!(!backend.entries.borrow().contains_key("b"));
    assert_eq!(*file.contents.borrow(), b"old metadata");
    assert_eq!(store.default(), Some("a"));
}
#[test]
fn login_rejects_keys_that_clean_to_empty_and_whole_inner_401_custom_bypass() {
    let error = auth_login::clean_key(secret(" \u{feff}!!!\u{feff} ")).unwrap_err();
    assert_eq!(error.message(), "No API key provided");
    let error = MutationFailure::Ordinary("post-save backend401".into()).login();
    assert_eq!(error.kind(), ErrorKind::Auth);
    let error = MutationFailure::Typed(Error::new("native prompt401")).login();
    assert_eq!(error.kind(), ErrorKind::Other);
    assert_eq!(error.message(), "native prompt401");
}
#[test]
fn windows_owned_layout_and_decoder_bind_safe_library_boundary_without_os_calls() {
    use linear_cli::auth::LookupFailureCategory;
    use linear_cli::auth::keyring::windows_spec::{
        WindowsCredentialSpec, WindowsReadFailure, classify_windows_lookup, decode_windows_secret,
    };
    let spec = WindowsCredentialSpec::new("dummy中").unwrap();
    assert_eq!(spec.username, "dummy中");
    assert_eq!(spec.target_name, "linear-cli:dummy中");
    assert_eq!(spec.comment, "");
    assert_eq!(spec.target_alias, "");
    assert!(WindowsCredentialSpec::new(&"a".repeat(513)).is_ok());
    assert!(WindowsCredentialSpec::new(&"a".repeat(514)).is_err());
    assert!(WindowsCredentialSpec::new("a\0b").is_err());
    assert!(matches!(
        classify_windows_lookup(Err(WindowsReadFailure::NoEntry)),
        LookupResult::Miss
    ));
    assert!(matches!(
        classify_windows_lookup(Ok(vec![])),
        LookupResult::Miss
    ));
    assert!(matches!(
        classify_windows_lookup(Err(WindowsReadFailure::NativeFailure)),
        LookupResult::Failed(LookupFailureCategory::Other)
    ));
    for bytes in [vec![1], vec![0, 0xd8]] {
        assert!(decode_windows_secret(bytes.clone()).is_err());
        assert!(matches!(
            classify_windows_lookup(Ok(bytes)),
            LookupResult::Failed(LookupFailureCategory::Other)
        ));
    }
    let bytes = "dummy中"
        .encode_utf16()
        .flat_map(u16::to_le_bytes)
        .collect();
    assert_eq!(
        decode_windows_secret(bytes).unwrap().unwrap().expose(),
        "dummy中"
    );
}
#[tokio::test(flavor = "current_thread")]
async fn inline_names_are_sorted_quoted_when_needed_and_reserved_names_rejected() {
    let mut store = state("z='dummy_z'", &[]);
    let backend = FakeBackend::new();
    let file = FakeFile::new();
    for (name, value) in [("10", "dummy10"), ("2", "dummy2"), ("中", "dummy_unicode")] {
        store
            .add(name, secret(value), None, path(), &backend, &file)
            .await
            .unwrap_or_else(|_| panic!("add"));
    }
    assert_eq!(
        std::str::from_utf8(&file.contents.borrow()).unwrap(),
        "10 = \"dummy10\"\n2 = \"dummy2\"\nz = \"dummy_z\"\n\"中\" = \"dummy_unicode\"\n"
    );
    for reserved in ["default", "workspaces"] {
        let Err(MutationFailure::Typed(error)) = store
            .add(reserved, secret("dummy"), None, path(), &backend, &file)
            .await
        else {
            panic!("{reserved} must be rejected");
        };
        assert_eq!(error.kind(), ErrorKind::Other);
    }
    assert!(backend.events.borrow().is_empty());
}
