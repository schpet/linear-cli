use linear_cli::auth::write::{CredentialFileWriter, prepare_default_write};
use linear_cli::auth::{CredentialStore, LookupReply, LookupResult, hydrate, parse_credentials};
use linear_cli::config::{RawConfigFile, parse_config_tier};
use linear_cli::error::AppErrorKind;
use std::cell::RefCell;
use std::io;
use std::path::{Path, PathBuf};

fn store(text: &str) -> CredentialStore {
    let tier = parse_config_tier(RawConfigFile {
        path: PathBuf::from("/fake/credentials.toml"),
        bytes: text.as_bytes().to_vec(),
    })
    .unwrap();
    let manifest = parse_credentials(tier).unwrap();
    let replies = manifest
        .lookup_requests()
        .into_iter()
        .map(|workspace| LookupReply {
            workspace: workspace.to_owned(),
            result: LookupResult::Miss,
        })
        .collect();
    hydrate(manifest, replies).unwrap()
}
#[derive(Default)]
struct Writer(RefCell<Vec<(PathBuf, Vec<u8>)>>);
impl CredentialFileWriter for Writer {
    fn write_credentials(&self, path: &Path, bytes: &[u8]) -> io::Result<()> {
        self.0.borrow_mut().push((path.to_owned(), bytes.to_vec()));
        Ok(())
    }
}
fn saved(text: &str, target: &str) -> Vec<u8> {
    let store = store(text);
    let plan = prepare_default_write(
        &store,
        target,
        Some(Path::new("/fake/chosen/credentials.toml")),
    )
    .unwrap();
    let writer = Writer::default();
    plan.save(&writer).unwrap();
    let calls = writer.0.into_inner();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].0, Path::new("/fake/chosen/credentials.toml"));
    calls.into_iter().next().unwrap().1
}
#[test]
fn inline_and_metadata_emit_exact_source_bytes_without_requiring_metadata_keys() {
    assert_eq!(
        saved(
            "default='zeta'\nzeta='lin_api_fake_zeta'\nalpha=''\n",
            "alpha"
        ),
        b"default = \"alpha\"\nalpha = \"\"\nzeta = \"lin_api_fake_zeta\"\n"
    );
    assert_eq!(
        saved("default='zeta'\nworkspaces=['zeta','alpha']\n", "alpha"),
        b"default = \"alpha\"\nworkspaces = [\"alpha\",\"zeta\"]\n"
    );
}
#[test]
fn utf16_sort_and_numeric_property_enumeration_are_not_utf8_or_lexical_numeric() {
    let bytes = saved(
        "default='zeta'\n\"10\"='lin_api_fake_ten'\n\"2\"='lin_api_fake_two'\n\"01\"='lin_api_fake_leading'\nzeta='lin_api_fake_zeta'\n\"😀\"='lin_api_fake_astral'\n\"\"='lin_api_fake_bmp'\n",
        "zeta",
    );
    assert_eq!(
        String::from_utf8(bytes).unwrap(),
        "2 = \"lin_api_fake_two\"\n10 = \"lin_api_fake_ten\"\ndefault = \"zeta\"\n01 = \"lin_api_fake_leading\"\nzeta = \"lin_api_fake_zeta\"\n\"😀\" = \"lin_api_fake_astral\"\n\"\" = \"lin_api_fake_bmp\"\n"
    );
}
#[test]
fn json_escaping_keys_and_constructor_are_preserved_but_source_omitted_property_is_not_created() {
    let text = "default='alpha'\n\"__proto__\"='lin_api_fake_proto'\nconstructor='lin_api_fake_constructor'\nalpha='lin_api_fake_alpha'\n\"space ws\"=\"lin_api_fake_quote\\\"slash\\\\new\\n\"\n";
    assert_eq!(saved(text,"constructor"), b"default = \"constructor\"\nalpha = \"lin_api_fake_alpha\"\nconstructor = \"lin_api_fake_constructor\"\n\"space ws\" = \"lin_api_fake_quote\\\"slash\\\\new\\n\"\n");
}
#[test]
fn prepared_bytes_are_redacted_and_explicit_path_and_writer_errors_are_typed() {
    let store = store("alpha='lin_api_fake_unique_secret'\nzeta='lin_api_fake_zeta'\n");
    assert_eq!(
        prepare_default_write(&store, "alpha", None)
            .unwrap_err()
            .kind,
        AppErrorKind::IoProcess
    );
    assert_eq!(
        prepare_default_write(&store, "missing", Some(Path::new("/fake/path")))
            .unwrap_err()
            .kind,
        AppErrorKind::Invariant
    );
    let plan = prepare_default_write(
        &store,
        "alpha",
        Some(Path::new("/fake/chosen/credentials.toml")),
    )
    .unwrap();
    assert!(!format!("{plan:?}").contains("lin_api_fake_unique_secret"));
    struct Denied;
    impl CredentialFileWriter for Denied {
        fn write_credentials(&self, _path: &Path, _bytes: &[u8]) -> io::Result<()> {
            Err(io::Error::from(io::ErrorKind::PermissionDenied))
        }
    }
    let error = plan.save(&Denied).unwrap_err();
    assert_eq!(error.kind, AppErrorKind::IoProcess);
    assert!(error.message.contains("/fake/chosen/credentials.toml"));
    assert!(!error.message.contains("lin_api_fake_unique_secret"));
}
