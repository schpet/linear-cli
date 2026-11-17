use super::*;
use serde_json::{Value, json};

struct Tools {
    plan: Value,
    latest: String,
    stale: bool,
    missing_notes: bool,
    calls: Vec<(String, Vec<OsString>)>,
}

impl Tools {
    fn new(version: &str, prerelease: bool) -> Self {
        Self {
            plan: json!({
                "announcement_tag": format!("v{version}"),
                "announcement_title": format!("{version} - 2026-11-17"),
                "announcement_is_prerelease": prerelease,
                "releases": [{"app_version": version}],
            }),
            latest: version.to_owned(),
            stale: false,
            missing_notes: false,
            calls: Vec::new(),
        }
    }
}

impl Runner for Tools {
    fn run(&mut self, root: &Path, program: &str, args: &[OsString]) -> Result<String> {
        self.calls.push((program.to_owned(), args.to_vec()));
        let args: Vec<_> = args
            .iter()
            .map(|arg| arg.to_str().expect("test UTF-8"))
            .collect();
        match (program, args.as_slice()) {
            ("changelog", ["version", "latest"]) => Ok(format!("{}\n", self.latest)),
            ("changelog", ["entry", _]) if self.missing_notes => {
                Err(io::Error::other("Missing versioned notes").into())
            }
            ("changelog", ["entry", _]) => Ok("Notes".to_owned()),
            ("svbump", ["write", version, "workspace.package.version", path]) => {
                let version = if self.stale { "4.0.0" } else { version };
                std::fs::write(
                    root.join(path),
                    format!("[workspace.package]\nversion=\"{version}\"\n"),
                )?;
                Ok(String::new())
            }
            ("svbump", ["read", "workspace.package.version", path]) => {
                let text = std::fs::read_to_string(root.join(path))?;
                let (_, value) = text.split_once('=').expect("test manifest version");
                Ok(format!("{}\n", value.trim().trim_matches('"')))
            }
            ("cargo", ["update", "--workspace"]) => Ok(String::new()),
            ("dist", ["plan", "--tag", _, "--output-format=json"]) => Ok(self.plan.to_string()),
            _ => {
                Err(io::Error::other(format!("Unexpected test command: {program} {args:?}")).into())
            }
        }
    }
}

fn fixture() -> tempfile::TempDir {
    let root = tempfile::tempdir().expect("fixture");
    std::fs::write(
        root.path().join("Cargo.toml"),
        "[workspace.package]\nversion=\"3.1.0\"\n",
    )
    .expect("manifest");
    root
}

#[test]
fn prepares_exact_stable_alpha_and_build_versions() {
    for (version, prerelease) in [
        ("4.0.0-alpha.3", true),
        ("4.0.0", false),
        ("4.0.0+build-test", false),
    ] {
        let root = fixture();
        let mut tools = Tools::new(version, prerelease);
        assert_eq!(
            prepare(root.path(), version, &mut tools).expect("prepare"),
            version
        );
        let manifest = std::fs::read_to_string(root.path().join("Cargo.toml")).expect("manifest");
        assert!(manifest.contains(&format!("version=\"{version}\"")));
        assert!(tools.calls.iter().any(|(program, _)| program == "cargo"));
        assert!(
            tools
                .calls
                .iter()
                .all(|(program, _)| !matches!(program.as_str(), "git" | "jj" | "gh"))
        );
        let (_, probe_args) = tools
            .calls
            .iter()
            .find(|(program, _)| program == "svbump")
            .expect("probe write");
        assert!(
            !Path::new(probe_args.last().expect("probe path")).exists(),
            "probe cleaned up"
        );
    }
}

#[test]
fn latest_resolves_the_changelog_version() {
    let root = fixture();
    let mut tools = Tools::new("4.0.0-alpha.3", true);
    assert_eq!(
        prepare(root.path(), "latest", &mut tools).expect("latest"),
        "4.0.0-alpha.3"
    );
    let (program, args) = tools.calls.first().expect("first call");
    assert_eq!(program, "changelog");
    assert_eq!(args, &[OsString::from("version"), OsString::from("latest")]);
}

#[test]
fn stale_svbump_and_missing_notes_leave_the_workspace_untouched() {
    for stale in [true, false] {
        let root = fixture();
        let manifest = root.path().join("Cargo.toml");
        let before = std::fs::read(&manifest).expect("before");
        let mut tools = Tools::new("4.0.0-alpha.3", true);
        tools.stale = stale;
        tools.missing_notes = !stale;
        let error = prepare(root.path(), "4.0.0-alpha.3", &mut tools).expect_err("reject");
        assert!(error.to_string().contains(if stale {
            "update svbump"
        } else {
            "Missing versioned notes"
        }));
        assert_eq!(std::fs::read(&manifest).expect("after"), before);
        assert!(!tools.calls.iter().any(|(program, _)| program == "cargo"));
    }
}

#[test]
fn rejects_incorrect_announcement_fields_and_empty_releases() {
    for (field, value) in [
        ("announcement_tag", json!("v4.0.0")),
        ("announcement_title", json!("Version 4.0.0-alpha.3")),
        ("announcement_is_prerelease", json!(false)),
        ("releases", json!([])),
        ("releases", json!([{"app_version":"4.0.0"}])),
    ] {
        let root = fixture();
        let mut tools = Tools::new("4.0.0-alpha.3", true);
        *tools.plan.get_mut(field).expect("field") = value;
        assert!(
            prepare(root.path(), "4.0.0-alpha.3", &mut tools)
                .expect_err("bad plan")
                .to_string()
                .contains("cargo-dist release metadata")
        );
    }
}

#[test]
fn rejects_malformed_manifest_shapes() {
    for json in ["not JSON", "{}", r#"{"announcement_tag": 42}"#] {
        assert!(validate_plan("4.0.0-alpha.3", json).is_err());
    }
}

#[test]
fn rejects_an_empty_selected_version_before_running_tools() {
    let root = fixture();
    let mut tools = Tools::new("", false);
    assert!(prepare(root.path(), "", &mut tools).is_err());
    assert!(tools.calls.is_empty());
}

#[test]
fn command_start_errors_name_the_failed_tool() {
    let root = fixture();
    let error = Process
        .run(root.path(), "linear-release-nonexistent-tool", &[])
        .expect_err("missing tool");
    assert!(
        error
            .to_string()
            .contains("Failed to run linear-release-nonexistent-tool")
    );
}

#[test]
fn cli_defaults_to_latest_and_accepts_explicit_alpha_versions() {
    use clap::Parser;
    for (args, expected) in [
        (vec!["xtask", "prepare-release"], "latest"),
        (
            vec!["xtask", "prepare-release", "4.0.0-alpha.3"],
            "4.0.0-alpha.3",
        ),
    ] {
        let cli = crate::Cli::try_parse_from(args).expect("CLI");
        let version = match cli.task {
            crate::Task::PrepareRelease { version } => Some(version),
            crate::Task::Licenses { .. } => None,
        };
        assert_eq!(version.as_deref(), Some(expected));
    }
}
