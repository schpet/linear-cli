//! `cargo xtask licenses`: gathers the license and notice files of every
//! third-party crate in Cargo.lock into one Markdown file. Release builds run
//! it and ship the result as THIRD_PARTY_LICENSES.md in every archive.
//!
//! Notices come from the files each crate package ships next to its
//! Cargo.toml. A few crates publish no license text in their package; for
//! those, `licenses/supplemental/sources.json` records upstream copies pinned
//! by SHA-256. A crate with neither is an error.

#[cfg(test)]
mod tests;

use std::collections::{BTreeMap, BTreeSet};
use std::env;
use std::error::Error;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};

use serde::Deserialize;
use sha2::{Digest, Sha256};

type Result<T, E = Box<dyn Error>> = std::result::Result<T, E>;

const DEFAULT_OUTPUT: &str = "THIRD_PARTY_LICENSES.md";
const SUPPLEMENTAL_DIR: &str = "licenses/supplemental";

/// Top-level files and directories of a crate package whose names start with
/// one of these, ignoring case, hold its license notices.
const NOTICE_PREFIXES: &[&str] = &["license", "copying", "notice", "copyright"];

/// Notice files that don't follow the naming above, as (crate, file name).
/// r-efi keeps its MIT permission and copyright notice in AUTHORS.
const EXTRA_NOTICES: &[(&str, &str)] = &[("r-efi", "AUTHORS")];

pub fn run(workspace: &Path, output: Option<PathBuf>) -> Result<()> {
    let packages = third_party_packages(cargo_metadata(workspace)?)?;
    let mut supplements = Supplements::load(&workspace.join(SUPPLEMENTAL_DIR))?;
    let crates = packages
        .into_iter()
        .map(|package| {
            let supplemental = supplements.take(&package.identity());
            collect_notices(package, supplemental)
        })
        .collect::<Result<Vec<_>>>()?;
    supplements.ensure_all_used()?;

    let output = output.unwrap_or_else(|| workspace.join(DEFAULT_OUTPUT));
    fs::write(&output, render(&crates))
        .map_err(|error| format!("failed to write {}: {error}", output.display()))?;
    eprintln!(
        "Wrote the license notices of {} crates to {}",
        crates.len(),
        output.display()
    );
    Ok(())
}

/// The parts of `cargo metadata` output this task reads.
#[derive(Deserialize)]
struct Metadata {
    packages: Vec<MetadataPackage>,
    workspace_members: Vec<String>,
}

#[derive(Deserialize)]
struct MetadataPackage {
    id: String,
    name: String,
    version: String,
    /// `None` for workspace members and other path dependencies.
    source: Option<String>,
    license: Option<String>,
    license_file: Option<PathBuf>,
    repository: Option<String>,
    manifest_path: PathBuf,
}

fn cargo_metadata(workspace: &Path) -> Result<Metadata> {
    let cargo = env::var_os("CARGO").ok_or("CARGO is not set; run this with `cargo xtask`")?;
    // All features and, without --filter-platform, all targets, so the
    // packages are exactly those in Cargo.lock.
    let output = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--all-features",
        ])
        .arg("--manifest-path")
        .arg(workspace.join("Cargo.toml"))
        .stderr(Stdio::inherit())
        .output()
        .map_err(|error| format!("failed to run cargo metadata: {error}"))?;
    if !output.status.success() {
        return Err(format!("cargo metadata failed ({})", output.status).into());
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("unexpected cargo metadata output: {error}").into())
}

/// A third-party crate whose notices ship with the binary.
#[derive(Debug)]
struct Package {
    name: String,
    version: String,
    license: String,
    repository: Option<String>,
    /// The directory holding the crate's Cargo.toml.
    root: PathBuf,
    license_file: Option<PathBuf>,
}

impl Package {
    fn identity(&self) -> String {
        format!("{}@{}", self.name, self.version)
    }
}

/// Selects every package Cargo resolved, for any target or feature and
/// including build and dev dependencies, except the workspace's own crates,
/// which ship under the project's LICENSE. Sorted by name, then version.
fn third_party_packages(metadata: Metadata) -> Result<Vec<Package>> {
    let members: BTreeSet<String> = metadata.workspace_members.into_iter().collect();
    let mut seen = BTreeSet::new();
    let mut packages = Vec::new();
    for package in metadata.packages {
        let identity = format!("{}@{}", package.name, package.version);
        if members.contains(&package.id) {
            continue;
        }
        if package.source.is_none() {
            return Err(format!(
                "{identity} is a path dependency outside the workspace; it needs its own notice handling"
            )
            .into());
        }
        if !seen.insert(identity.clone()) {
            return Err(format!("{identity} appears more than once in cargo metadata").into());
        }
        let license = package
            .license
            .filter(|license| !license.trim().is_empty())
            .ok_or_else(|| format!("{identity} declares no license expression"))?;
        let root = package
            .manifest_path
            .parent()
            .ok_or_else(|| format!("{identity} has no manifest directory"))?
            .to_path_buf();
        packages.push(Package {
            name: package.name,
            version: package.version,
            license,
            repository: package.repository,
            root,
            license_file: package.license_file,
        });
    }
    packages.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    Ok(packages)
}

/// A crate and the notices to reproduce for it.
#[derive(Debug)]
struct Crate {
    name: String,
    version: String,
    license: String,
    repository: Option<String>,
    notices: Vec<Notice>,
}

#[derive(Debug, PartialEq)]
struct Notice {
    title: String,
    text: String,
    origin: Origin,
}

#[derive(Debug, PartialEq)]
enum Origin {
    /// A file in the crate package.
    Package,
    /// A text from `licenses/supplemental`.
    Upstream { source_url: String, reason: String },
}

fn collect_notices(package: Package, supplements: Vec<Supplement>) -> Result<Crate> {
    let mut notices = package_notice_files(&package)?
        .into_iter()
        .map(|(title, path)| {
            Ok(Notice {
                title,
                text: read_text(&path)?,
                origin: Origin::Package,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    notices.extend(supplements.into_iter().map(|supplement| Notice {
        title: supplement.file_name,
        text: supplement.text,
        origin: Origin::Upstream {
            source_url: supplement.source_url,
            reason: supplement.reason,
        },
    }));
    if notices.is_empty() {
        return Err(format!(
            "{} ships no license or notice file; add its upstream license text to {SUPPLEMENTAL_DIR} and record it in sources.json there",
            package.identity()
        )
        .into());
    }
    Ok(Crate {
        name: package.name,
        version: package.version,
        license: package.license,
        repository: package.repository,
        notices,
    })
}

/// The license and notice files a crate package ships, as (path within the
/// package, full path), in path order. The manifest's `license-file` comes
/// last unless it is already among them.
fn package_notice_files(package: &Package) -> Result<Vec<(String, PathBuf)>> {
    let root = fs::canonicalize(&package.root)
        .map_err(|error| format!("failed to resolve {}: {error}", package.root.display()))?;
    let mut files = Vec::new();
    for entry in sorted_entries(&root)? {
        let name = entry
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| format!("{} is not a UTF-8 file name", entry.display()))?;
        let lower = name.to_lowercase();
        let is_notice = NOTICE_PREFIXES
            .iter()
            .any(|prefix| lower.starts_with(prefix))
            || EXTRA_NOTICES.contains(&(package.name.as_str(), name));
        if is_notice {
            collect_files(&entry, &mut files)?;
        }
    }
    if let Some(license_file) = &package.license_file {
        let path = fs::canonicalize(root.join(license_file)).map_err(|error| {
            format!(
                "{}: license-file {}: {error}",
                package.identity(),
                license_file.display()
            )
        })?;
        if !path.starts_with(&root) || !path.is_file() {
            return Err(format!(
                "{}: license-file {} is not a file in the package",
                package.identity(),
                license_file.display()
            )
            .into());
        }
        if !files.contains(&path) {
            files.push(path);
        }
    }
    files
        .into_iter()
        .map(|path| Ok((package_relative(&root, &path)?, path)))
        .collect()
}

/// Adds `path` if it is a file, or every file under it if it is a directory.
fn collect_files(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let file_type = fs::symlink_metadata(path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?
        .file_type();
    if file_type.is_symlink() {
        return Err(format!(
            "{} is a symlink; notices must be regular files",
            path.display()
        )
        .into());
    }
    if file_type.is_dir() {
        for entry in sorted_entries(path)? {
            collect_files(&entry, files)?;
        }
    } else if file_type.is_file() {
        files.push(path.to_path_buf());
    } else {
        return Err(format!("{} is neither a file nor a directory", path.display()).into());
    }
    Ok(())
}

fn sorted_entries(dir: &Path) -> Result<Vec<PathBuf>> {
    let mut entries = fs::read_dir(dir)
        .and_then(|entries| {
            entries
                .map(|entry| entry.map(|entry| entry.path()))
                .collect::<std::io::Result<Vec<_>>>()
        })
        .map_err(|error| format!("failed to list {}: {error}", dir.display()))?;
    entries.sort();
    Ok(entries)
}

/// `path` relative to `root`, with `/` separators on every platform.
fn package_relative(root: &Path, path: &Path) -> Result<String> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| format!("{} is outside {}", path.display(), root.display()))?;
    let parts = relative
        .components()
        .map(|component| match component {
            Component::Normal(part) => part.to_str(),
            Component::Prefix(_)
            | Component::RootDir
            | Component::CurDir
            | Component::ParentDir => None,
        })
        .collect::<Option<Vec<_>>>()
        .ok_or_else(|| format!("{} is not a plain UTF-8 path", relative.display()))?;
    Ok(parts.join("/"))
}

/// Reads a notice as UTF-8 with `\n` line endings, rejecting empty files.
fn read_text(path: &Path) -> Result<String> {
    let bytes =
        fs::read(path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    text_from_bytes(path, bytes)
}

fn text_from_bytes(path: &Path, bytes: Vec<u8>) -> Result<String> {
    let text = String::from_utf8(bytes)
        .map_err(|_| format!("{} is not UTF-8", path.display()))?
        .replace("\r\n", "\n");
    if text.trim().is_empty() {
        return Err(format!("{} is empty", path.display()).into());
    }
    Ok(text)
}

/// `licenses/supplemental/sources.json`.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SourcesFile {
    records: Vec<SourceRecord>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
struct SourceRecord {
    /// A file name in `licenses/supplemental`.
    file: String,
    sha256: String,
    /// Where the text was copied from.
    source_url: String,
    /// The `name@version` crates the text applies to.
    packages: Vec<String>,
    /// Why the text applies to those crates.
    reason: String,
}

#[derive(Clone, Debug)]
struct Supplement {
    file_name: String,
    text: String,
    source_url: String,
    reason: String,
}

/// Upstream license texts for crates whose packages ship none, by
/// `name@version`.
#[derive(Debug)]
struct Supplements {
    by_identity: BTreeMap<String, Vec<Supplement>>,
}

impl Supplements {
    fn load(dir: &Path) -> Result<Self> {
        let sources_path = dir.join("sources.json");
        let json = fs::read_to_string(&sources_path)
            .map_err(|error| format!("failed to read {}: {error}", sources_path.display()))?;
        let sources: SourcesFile = serde_json::from_str(&json)
            .map_err(|error| format!("invalid {}: {error}", sources_path.display()))?;
        let mut by_identity: BTreeMap<String, Vec<Supplement>> = BTreeMap::new();
        for record in sources.records {
            let supplement = load_supplement(dir, &record)?;
            if record.packages.is_empty() {
                return Err(format!("supplemental {} names no packages", record.file).into());
            }
            for identity in record.packages {
                let texts = by_identity.entry(identity).or_default();
                if texts
                    .iter()
                    .any(|text| text.file_name == supplement.file_name)
                {
                    return Err(format!(
                        "supplemental {} is listed twice for one package",
                        record.file
                    )
                    .into());
                }
                texts.push(supplement.clone());
            }
        }
        Ok(Self { by_identity })
    }

    fn take(&mut self, identity: &str) -> Vec<Supplement> {
        self.by_identity.remove(identity).unwrap_or_default()
    }

    /// Fails when a record names a crate that is not in Cargo.lock, which
    /// means it is stale (the crate was upgraded or removed).
    fn ensure_all_used(self) -> Result<()> {
        if self.by_identity.is_empty() {
            return Ok(());
        }
        let unused: Vec<&str> = self.by_identity.keys().map(String::as_str).collect();
        Err(format!(
            "{SUPPLEMENTAL_DIR}/sources.json names crates that are not in Cargo.lock: {}",
            unused.join(", ")
        )
        .into())
    }
}

fn load_supplement(dir: &Path, record: &SourceRecord) -> Result<Supplement> {
    let mut components = Path::new(&record.file).components();
    let is_file_name = matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    );
    if !is_file_name {
        return Err(format!(
            "supplemental {:?} must be a file name in {SUPPLEMENTAL_DIR}",
            record.file
        )
        .into());
    }
    let path = dir.join(&record.file);
    let is_regular_file = fs::symlink_metadata(&path)
        .map_err(|error| format!("failed to read {}: {error}", path.display()))?
        .file_type()
        .is_file();
    if !is_regular_file {
        return Err(format!("{} must be a regular file", path.display()).into());
    }
    let bytes =
        fs::read(&path).map_err(|error| format!("failed to read {}: {error}", path.display()))?;
    let sha256 = hex(&Sha256::digest(&bytes));
    if sha256 != record.sha256 {
        return Err(format!(
            "{} has SHA-256 {sha256}, but sources.json records {}",
            path.display(),
            record.sha256
        )
        .into());
    }
    if !record.source_url.starts_with("https://") {
        return Err(format!("supplemental {} needs an https source URL", record.file).into());
    }
    if record.reason.trim().is_empty() {
        return Err(format!("supplemental {} needs a reason", record.file).into());
    }
    Ok(Supplement {
        file_name: record.file.clone(),
        text: text_from_bytes(&path, bytes)?,
        source_url: record.source_url.clone(),
        reason: record.reason.clone(),
    })
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn render(crates: &[Crate]) -> String {
    let mut out = format!(
        "# Third-party licenses\n\n\
         `linear` is built from third-party Rust crates. This file covers all {} \
         crates in its Cargo.lock, for every platform and including build and \
         test dependencies. For each crate it gives the declared license, the \
         repository, and the full text of the license and notice files the \
         crate ships.\n\n\
         Generated by `cargo xtask licenses`.\n",
        crates.len()
    );
    for krate in crates {
        out.push_str(&format!(
            "\n## {} {}\n\n- License: {}\n",
            krate.name, krate.version, krate.license
        ));
        if let Some(repository) = &krate.repository {
            out.push_str(&format!("- Repository: {repository}\n"));
        }
        for notice in &krate.notices {
            out.push_str(&format!("\n### {}\n\n", notice.title));
            match &notice.origin {
                Origin::Package => {}
                Origin::Upstream { source_url, reason } => out.push_str(&format!(
                    "Not included in the crate package; copied from {source_url} ({reason}).\n\n"
                )),
            }
            let fence = fence_for(&notice.text);
            out.push_str(&format!(
                "{fence}text\n{}\n{fence}\n",
                notice.text.trim_end()
            ));
        }
    }
    out
}

/// A backtick fence longer than any run of backticks in `text`.
fn fence_for(text: &str) -> String {
    let longest = text.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    "`".repeat(longest.max(2) + 1)
}
