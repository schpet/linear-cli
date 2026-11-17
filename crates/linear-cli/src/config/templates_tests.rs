use std::cell::Cell;
use std::io;
use std::path::Path;

use super::templates::*;
use super::test_support::TempTree;
use super::{FileKind, FileSource, ProcessEnvSnapshot, RealFileSource, load_startup};
use crate::templates::{CommitContext, IssueData, Slot};

fn context() -> CommitContext {
    CommitContext {
        issue: IssueData {
            identifier: "ENG-1".to_owned(),
            ..IssueData::default()
        },
        ..CommitContext::default()
    }
}

#[test]
fn offline_layer_order_is_global_project_files_then_snippets() {
    let tree = TempTree::new();
    tree.write("config/linear/linear.toml", b"api_key = 'secret'\n[templates]\ncommit_description = 'global'\npr_title = 'global title'");
    tree.write(
        "linear.toml",
        b"[templates]\ncommit_description = 'project'",
    );
    tree.write(
        "extra-one.toml",
        b"[templates]\ncommit_description = 'first extra'",
    );
    tree.write(
        "extra-two.toml",
        b"[templates]\ncommit_description = 'second extra'",
    );
    let mut inputs = tree.inputs();
    inputs.process_env.insert(
        "XDG_CONFIG_HOME".to_owned(),
        tree.join("config").to_string_lossy().into_owned(),
    );
    let config = load_template_config(
        &inputs,
        &RealFileSource,
        &["extra-one.toml".into(), "extra-two.toml".into()],
        &[
            "templates.commit_description='first snippet'".to_owned(),
            "templates.commit_description='{{ issue.identifier }} snippet'".to_owned(),
        ],
    )
    .expect("offline config");
    assert_eq!(
        config
            .render(Slot::CommitDescription, &context())
            .expect("render"),
        "ENG-1 snippet"
    );
    assert_eq!(
        config.entries().get("pr_title").expect("entry").body,
        "global title"
    );
    assert_eq!(
        config
            .entries()
            .get("commit_description")
            .expect("entry")
            .origin
            .source,
        "--config #2"
    );
}

#[test]
fn offline_loading_does_not_read_dotenv() {
    let tree = TempTree::new();
    tree.write(".env", b"\xff\xfeinvalid");
    tree.write(
        "linear.toml",
        b"[templates]\ncommit_description = 'works offline'",
    );
    let config =
        load_template_config(&tree.inputs(), &RealFileSource, &[], &[]).expect("no dotenv");
    assert_eq!(
        config
            .render(Slot::CommitDescription, &context())
            .expect("render"),
        "works offline"
    );
}

#[test]
fn overlays_reject_unrelated_roots_and_require_explicit_files() {
    let tree = TempTree::new();
    tree.write("extra.toml", b"api_key = 'secret'");
    for (files, snippets) in [
        (vec![tree.join("missing.toml")], vec![]),
        (vec![tree.join("extra.toml")], vec![]),
        (vec![], vec!["api_key='secret'".to_owned()]),
    ] {
        let error = load_template_config(&tree.inputs(), &RealFileSource, &files, &snippets)
            .expect_err("invalid overlay");
        assert!(!format!("{error:?}").contains("secret"), "{error:?}");
    }
}

#[test]
fn startup_reads_selected_config_once_and_checks_lower_template_tiers() {
    struct Counted<'a> {
        config: &'a Path,
        reads: Cell<usize>,
    }
    impl FileSource for Counted<'_> {
        fn kind(&self, path: &Path) -> io::Result<Option<FileKind>> {
            RealFileSource.kind(path)
        }
        fn read_bounded(&self, path: &Path, max_bytes: u64) -> io::Result<Vec<u8>> {
            if path == self.config {
                self.reads.set(self.reads.get() + 1);
            }
            RealFileSource.read_bounded(path, max_bytes)
        }
    }
    let tree = TempTree::new();
    tree.write("linear.toml", b"[templates]\ncommit_description = 'once'");
    let path = tree.join("linear.toml");
    let files = Counted {
        config: &path,
        reads: Cell::new(0),
    };
    let process = ProcessEnvSnapshot {
        inputs: tree.inputs(),
        pager: None,
    };
    let config = load_startup(&process, &files).result.expect("startup");
    assert_eq!(files.reads.get(), 1);
    assert_eq!(
        config
            .templates
            .render(Slot::CommitDescription, &context())
            .expect("render"),
        "once"
    );

    tree.write(
        "config/linear/linear.toml",
        b"[templates]\ncommit_description = 4",
    );
    let mut inputs = tree.inputs();
    inputs.process_env.insert(
        "XDG_CONFIG_HOME".to_owned(),
        tree.join("config").to_string_lossy().into_owned(),
    );
    let process = ProcessEnvSnapshot {
        inputs,
        pager: None,
    };
    let error = load_startup(&process, &RealFileSource)
        .result
        .expect_err("bad lower layer");
    assert!(
        error.to_string().contains("expected a template string"),
        "{error}"
    );
}
