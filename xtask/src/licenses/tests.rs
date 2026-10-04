use std::fs;
use std::path::{Path, PathBuf};

use sha2::{Digest, Sha256};
use tempfile::TempDir;

use super::*;

const REGISTRY: &str = "registry+https://github.com/rust-lang/crates.io-index";

fn metadata_package(name: &str, version: &str, source: Option<&str>) -> MetadataPackage {
    MetadataPackage {
        id: format!("{name} {version}"),
        name: name.to_owned(),
        version: version.to_owned(),
        source: source.map(str::to_owned),
        license: Some("MIT".to_owned()),
        license_file: None,
        repository: Some(format!("https://example.test/{name}")),
        manifest_path: PathBuf::from(format!("/registry/{name}-{version}/Cargo.toml")),
    }
}

fn package(name: &str, root: &Path) -> Package {
    Package {
        name: name.to_owned(),
        version: "1.0.0".to_owned(),
        license: "MIT".to_owned(),
        repository: None,
        root: root.to_path_buf(),
        license_file: None,
    }
}

fn write(path: &Path, contents: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, contents).unwrap();
}

fn titles(package: &Package) -> Vec<String> {
    package_notice_files(package)
        .unwrap()
        .into_iter()
        .map(|(title, _)| title)
        .collect()
}

fn error_message<T: std::fmt::Debug>(result: Result<T>) -> String {
    result.unwrap_err().to_string()
}

#[test]
fn selects_third_party_packages_sorted_without_workspace_members() {
    let metadata = Metadata {
        packages: vec![
            metadata_package("zlib", "1.0.0", Some(REGISTRY)),
            metadata_package("linear", "3.0.0", None),
            metadata_package("anyhow", "1.0.0", Some(REGISTRY)),
            metadata_package("anyhow", "0.9.0", Some(REGISTRY)),
        ],
        workspace_members: vec!["linear 3.0.0".to_owned()],
    };
    let packages = third_party_packages(metadata).unwrap();
    let identities: Vec<String> = packages.iter().map(Package::identity).collect();
    assert_eq!(identities, ["anyhow@0.9.0", "anyhow@1.0.0", "zlib@1.0.0"]);
    assert_eq!(packages[0].root, Path::new("/registry/anyhow-0.9.0"));
}

#[test]
fn rejects_duplicate_identities() {
    let mut git = metadata_package("serde", "1.0.0", Some("git+https://example.test/serde"));
    git.id = "serde git".to_owned();
    let metadata = Metadata {
        packages: vec![metadata_package("serde", "1.0.0", Some(REGISTRY)), git],
        workspace_members: Vec::new(),
    };
    assert!(
        error_message(third_party_packages(metadata))
            .contains("serde@1.0.0 appears more than once")
    );
}

#[test]
fn rejects_packages_without_a_license_expression() {
    let mut unlicensed = metadata_package("mystery", "1.0.0", Some(REGISTRY));
    unlicensed.license = None;
    let metadata = Metadata {
        packages: vec![unlicensed],
        workspace_members: Vec::new(),
    };
    assert!(
        error_message(third_party_packages(metadata)).contains("mystery@1.0.0 declares no license")
    );
}

#[test]
fn rejects_path_dependencies_outside_the_workspace() {
    let metadata = Metadata {
        packages: vec![metadata_package("vendored", "1.0.0", None)],
        workspace_members: Vec::new(),
    };
    assert!(
        error_message(third_party_packages(metadata))
            .contains("vendored@1.0.0 is a path dependency")
    );
}

#[test]
fn finds_notice_files_by_name_and_directory() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    for file in [
        "Cargo.toml",
        "COPYING",
        "LICENSE-MIT",
        "license-apache.txt",
        "NOTICE",
        "README.md",
        "AUTHORS",
        "src/license.rs",
        "LICENSES/MIT.txt",
        "LICENSES/extra/Unicode.txt",
    ] {
        write(&root.join(file), "text");
    }
    assert_eq!(
        titles(&package("example", root)),
        [
            "COPYING",
            "LICENSE-MIT",
            "LICENSES/MIT.txt",
            "LICENSES/extra/Unicode.txt",
            "NOTICE",
            "license-apache.txt",
        ]
    );
    assert!(titles(&package("r-efi", root)).contains(&"AUTHORS".to_owned()));
}

#[test]
fn adds_the_manifest_license_file_once() {
    let dir = TempDir::new().unwrap();
    let root = dir.path();
    write(&root.join("LICENSE"), "text");
    write(&root.join("legal/terms.txt"), "text");

    let mut explicit = package("example", root);
    explicit.license_file = Some(PathBuf::from("legal/terms.txt"));
    assert_eq!(titles(&explicit), ["LICENSE", "legal/terms.txt"]);

    explicit.license_file = Some(PathBuf::from("LICENSE"));
    assert_eq!(titles(&explicit), ["LICENSE"]);
}

#[test]
fn rejects_a_license_file_outside_the_package() {
    let dir = TempDir::new().unwrap();
    write(&dir.path().join("outside.txt"), "text");
    write(&dir.path().join("crate/Cargo.toml"), "");
    let mut escaping = package("example", &dir.path().join("crate"));
    escaping.license_file = Some(PathBuf::from("../outside.txt"));
    assert!(
        error_message(package_notice_files(&escaping)).contains("is not a file in the package")
    );
}

#[cfg(unix)]
#[test]
fn rejects_symlinked_notices() {
    let dir = TempDir::new().unwrap();
    write(&dir.path().join("elsewhere"), "text");
    write(&dir.path().join("crate/Cargo.toml"), "");
    std::os::unix::fs::symlink(
        dir.path().join("elsewhere"),
        dir.path().join("crate/LICENSE"),
    )
    .unwrap();
    assert!(
        error_message(package_notice_files(&package(
            "example",
            &dir.path().join("crate")
        )))
        .contains("is a symlink")
    );
}

#[test]
fn fails_when_a_crate_has_no_notice() {
    let dir = TempDir::new().unwrap();
    write(&dir.path().join("Cargo.toml"), "");
    let message = error_message(collect_notices(package("bare", dir.path()), Vec::new()));
    assert!(
        message.contains("bare@1.0.0 ships no license or notice file"),
        "{message}"
    );
    assert!(message.contains("sources.json"), "{message}");
}

#[test]
fn reads_package_notices_then_supplements() {
    let dir = TempDir::new().unwrap();
    write(&dir.path().join("LICENSE"), "package text\r\nline two\r\n");
    let supplement = Supplement {
        file_name: "upstream.txt".to_owned(),
        text: "upstream text".to_owned(),
        source_url: "https://example.test/LICENSE".to_owned(),
        reason: "because".to_owned(),
    };
    let krate = collect_notices(package("example", dir.path()), vec![supplement]).unwrap();
    assert_eq!(
        krate.notices,
        [
            Notice {
                title: "LICENSE".to_owned(),
                text: "package text\nline two\n".to_owned(),
                origin: Origin::Package,
            },
            Notice {
                title: "upstream.txt".to_owned(),
                text: "upstream text".to_owned(),
                origin: Origin::Upstream {
                    source_url: "https://example.test/LICENSE".to_owned(),
                    reason: "because".to_owned(),
                },
            },
        ]
    );
}

#[test]
fn rejects_empty_notices() {
    let dir = TempDir::new().unwrap();
    write(&dir.path().join("LICENSE"), " \n");
    assert!(
        error_message(collect_notices(package("example", dir.path()), Vec::new()))
            .contains("is empty")
    );
}

/// A supplemental directory holding `NOTICE.txt` and a sources.json with
/// `record` as its only record.
fn supplemental_dir(record: serde_json::Value) -> TempDir {
    let dir = TempDir::new().unwrap();
    write(&dir.path().join("NOTICE.txt"), "upstream text");
    write(
        &dir.path().join("sources.json"),
        &serde_json::json!({ "records": [record] }).to_string(),
    );
    dir
}

fn record() -> serde_json::Value {
    serde_json::json!({
        "file": "NOTICE.txt",
        "sha256": hex(&Sha256::digest(b"upstream text")),
        "sourceUrl": "https://example.test/LICENSE",
        "packages": ["first@1.0.0", "second@2.0.0"],
        "reason": "the upstream LICENSE",
    })
}

fn record_with(key: &str, value: serde_json::Value) -> serde_json::Value {
    let mut record = record();
    record[key] = value;
    record
}

#[test]
fn looks_up_supplements_by_identity() {
    let dir = supplemental_dir(record());
    let mut supplements = Supplements::load(dir.path()).unwrap();
    let first = supplements.take("first@1.0.0");
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].file_name, "NOTICE.txt");
    assert_eq!(first[0].text, "upstream text");
    assert!(supplements.take("first@1.0.0").is_empty());
    assert!(supplements.take("unrelated@1.0.0").is_empty());

    let message = error_message(supplements.ensure_all_used());
    assert!(
        message.ends_with("not in Cargo.lock: second@2.0.0"),
        "{message}"
    );
}

#[test]
fn accepts_supplements_once_all_are_used() {
    let dir = supplemental_dir(record());
    let mut supplements = Supplements::load(dir.path()).unwrap();
    supplements.take("first@1.0.0");
    supplements.take("second@2.0.0");
    supplements.ensure_all_used().unwrap();
}

#[test]
fn rejects_invalid_supplemental_records() {
    let cases = [
        (record_with("sha256", "0".repeat(64).into()), "has SHA-256"),
        (
            record_with("sourceUrl", "http://example.test".into()),
            "needs an https source URL",
        ),
        (
            record_with("file", "../NOTICE.txt".into()),
            "must be a file name",
        ),
        (
            record_with("file", "nested/NOTICE.txt".into()),
            "must be a file name",
        ),
        (
            record_with("packages", serde_json::json!([])),
            "names no packages",
        ),
        (record_with("reason", "".into()), "needs a reason"),
        (
            record_with("qualification", "old field".into()),
            "unknown field",
        ),
        (
            record_with(
                "packages",
                serde_json::json!(["first@1.0.0", "first@1.0.0"]),
            ),
            "listed twice",
        ),
    ];
    for (record, expected) in cases {
        let dir = supplemental_dir(record);
        let message = error_message(Supplements::load(dir.path()));
        assert!(
            message.contains(expected),
            "expected {expected:?} in {message:?}"
        );
    }
}

#[test]
fn renders_crates_with_their_notices() {
    let crates = [
        Crate {
            name: "alpha".to_owned(),
            version: "1.2.3".to_owned(),
            license: "MIT OR Apache-2.0".to_owned(),
            repository: Some("https://example.test/alpha".to_owned()),
            notices: vec![Notice {
                title: "LICENSE-MIT".to_owned(),
                text: "MIT text\n\n".to_owned(),
                origin: Origin::Package,
            }],
        },
        Crate {
            name: "beta".to_owned(),
            version: "0.1.0".to_owned(),
            license: "MPL-2.0".to_owned(),
            repository: None,
            notices: vec![Notice {
                title: "beta-MPL-2.0.txt".to_owned(),
                text: "uses ``` fences".to_owned(),
                origin: Origin::Upstream {
                    source_url: "https://example.test/LICENSE".to_owned(),
                    reason: "the upstream LICENSE".to_owned(),
                },
            }],
        },
    ];
    let rendered = render(&crates);
    assert!(
        rendered.starts_with("# Third-party licenses\n\n`linear` is built from third-party Rust crates. This file covers all 2 crates")
    );
    let body = rendered.split_once("\n## ").unwrap().1;
    assert_eq!(
        body,
        "alpha 1.2.3\n\
         \n\
         - License: MIT OR Apache-2.0\n\
         - Repository: https://example.test/alpha\n\
         \n\
         ### LICENSE-MIT\n\
         \n\
         ```text\n\
         MIT text\n\
         ```\n\
         \n\
         ## beta 0.1.0\n\
         \n\
         - License: MPL-2.0\n\
         \n\
         ### beta-MPL-2.0.txt\n\
         \n\
         Not included in the crate package; copied from https://example.test/LICENSE (the upstream LICENSE).\n\
         \n\
         ````text\n\
         uses ``` fences\n\
         ````\n"
    );
}

#[test]
fn fences_outgrow_backtick_runs() {
    assert_eq!(fence_for("plain"), "```");
    assert_eq!(fence_for("`code` and ``more``"), "```");
    assert_eq!(fence_for("````"), "`````");
}
