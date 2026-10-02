//! Public extraction, exact serializer and cache contracts.
use linear_cli::{
    error::{AppError, AppErrorKind},
    platform::{
        markdown_assets::{self, Asset},
        markdown_ast, markdown_serializer,
    },
};
use serde::Deserialize;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Image {
    url: String,
    alt: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Link {
    url: String,
    text: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Contract {
    id: String,
    content: String,
    replacements: Vec<(String, String)>,
    images: Vec<Image>,
    links: Vec<Link>,
    rewritten: String,
}
fn contracts() -> Vec<Contract> {
    let mut corpus = Vec::new();
    for (source, expected) in [
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../parity/runner/c050-c051-helper-contracts/markdown-source-observations-expanded.json"
            )),
            18,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../parity/runner/c050-c051-helper-contracts/markdown-source-design-observations.json"
            )),
            6,
        ),
        (
            include_str!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../parity/runner/c050-c051-helper-contracts/markdown-source-adjacent-task-observations.json"
            )),
            1,
        ),
    ] {
        let rows: Vec<Contract> =
            serde_json::from_str(source).expect("actual interpreted helper contracts");
        assert_eq!(rows.len(), expected);
        corpus.extend(rows);
    }
    corpus
}
#[test]
fn complete_gfm_extraction_distinguishes_inline_reference_and_definition_nodes() {
    let corpus = contracts();
    assert_eq!(corpus.len(), 25);
    for contract in corpus {
        let (images, links) = markdown_ast::extract(&contract.content).unwrap();
        assert_eq!(
            images,
            contract
                .images
                .into_iter()
                .map(|row| Asset {
                    url: row.url,
                    alt: row.alt
                })
                .collect::<Vec<_>>(),
            "images: {}",
            contract.id
        );
        assert_eq!(
            links,
            contract
                .links
                .into_iter()
                .map(|row| Asset {
                    url: row.url,
                    alt: row.text
                })
                .collect::<Vec<_>>(),
            "links: {}",
            contract.id
        );
    }
}
#[test]
fn full_gfm_serializer_matches_source_bytes_including_definitions_crlf_and_escaping() {
    for contract in contracts() {
        let paths: HashMap<_, _> = contract.replacements.into_iter().collect();
        let output =
            markdown_ast::rewrite_with(&contract.content, &paths, markdown_serializer::serialize)
                .unwrap();
        assert_eq!(output, contract.rewritten, "source bytes: {}", contract.id);
    }
}
#[test]
fn cache_names_match_sanitize_filename_and_hash_original_url() {
    for (alt, expected) in [
        (None, "image"),
        (Some(""), "image"),
        (Some("CON.txt"), ""),
        (Some(".."), ""),
        (Some("a<>:/\\|?*\u{1}\u{80}b. "), "ab"),
    ] {
        assert_eq!(markdown_assets::sanitized_filename(alt), expected);
    }
    let long = "界".repeat(100);
    assert_eq!(
        markdown_assets::sanitized_filename(Some(&long)),
        "界".repeat(85)
    );
    let asset = Asset {
        url: "https://uploads.linear.app/private.png?token=fake".to_owned(),
        alt: Some("cache space (a)".to_owned()),
    };
    assert_eq!(
        markdown_assets::cache_path(std::path::Path::new("/tmp/linear-cli-images"), &asset)
            .unwrap(),
        PathBuf::from("/tmp/linear-cli-images/57c89d19aa713f0e/cache space (a)")
    );
}
static NEXT: AtomicU64 = AtomicU64::new(0);
struct Tree(PathBuf);
impl Tree {
    fn new() -> Self {
        let normalized = markdown_assets::posix_join(&[std::env::temp_dir().to_str().unwrap()]);
        let path = PathBuf::from(normalized).join(format!(
            "linear-md-cache-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Tree {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).expect("remove owned test tree");
    }
}
#[tokio::test]
async fn images_precede_links_first_alt_wins_and_cache_hits_skip_fetch() {
    let tree = Tree::new();
    let content = "[link](https://uploads.linear.app/a.png)\n\n![first](https://uploads.linear.app/a.png) ![second](https://uploads.linear.app/a.png)";
    let mut requests = Vec::new();
    let result = markdown_assets::download_with(
        content,
        &tree.0,
        |url| {
            requests.push(url);
            async { Ok(vec![0, 255, 7]) }
        },
        |_| panic!("no failure expected"),
    )
    .await
    .unwrap();
    assert_eq!(requests, ["https://uploads.linear.app/a.png"]);
    let path = result
        .paths
        .get("https://uploads.linear.app/a.png")
        .unwrap();
    assert!(path.ends_with("/first"));
    assert_eq!(std::fs::read(path).unwrap(), [0, 255, 7]);
    let mut called = false;
    let cached = markdown_assets::download_with(
        content,
        &tree.0,
        |_| {
            called = true;
            async { Ok(Vec::new()) }
        },
        |_| panic!("no failure expected"),
    )
    .await
    .unwrap();
    assert!(!called);
    assert_eq!(cached.paths, result.paths);
}
#[tokio::test]
async fn sanitized_empty_name_reuses_created_directory_without_fetch() {
    let tree = Tree::new();
    let mut called = false;
    let result = markdown_assets::download_with(
        "![CON.txt](data:text/plain,not-fetched)",
        &tree.0,
        |_| {
            called = true;
            async { Ok(Vec::new()) }
        },
        |_| panic!("directory hit must succeed"),
    )
    .await
    .unwrap();
    assert!(!called);
    let path = result.paths.get("data:text/plain,not-fetched").unwrap();
    // Hash/path grounded in c051-data-file-filename-effects source observation.
    assert_eq!(
        path,
        &format!("{}/8dc4d4db56e2d549", tree.0.to_str().unwrap())
    );
    assert!(!path.ends_with('/'));
    assert!(std::fs::metadata(path).unwrap().is_dir());
}
#[tokio::test]
async fn individual_failures_emit_before_next_fetch_and_zero_success_does_not_authorize_rewrite() {
    let tree = Tree::new();
    let content = "![a](https://example.com/a) ![b](https://example.com/b)";
    let events = std::cell::RefCell::new(Vec::new());
    let failed = markdown_assets::download_with(
        content,
        &tree.0,
        |url| {
            events.borrow_mut().push(format!("fetch {url}"));
            async {
                Err(AppError::new(
                    AppErrorKind::Transport,
                    "Failed to download image: 500 Internal Server Error",
                ))
            }
        },
        |bytes| {
            events
                .borrow_mut()
                .push(String::from_utf8(bytes.to_vec()).unwrap());
            Ok(())
        },
    )
    .await
    .unwrap();
    assert!(failed.paths.is_empty());
    assert_eq!(
        events.into_inner(),
        [
            "fetch https://example.com/a",
            "Failed to download https://example.com/a: Failed to download image: 500 Internal Server Error\n",
            "fetch https://example.com/b",
            "Failed to download https://example.com/b: Failed to download image: 500 Internal Server Error\n"
        ]
    );
    // Dispatch returns original bytes when paths.is_empty(); frozen raw case
    // c051-all-downloads-fail-original verifies the complete command surface.
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[serde(rename_all = "camelCase")]
struct JoinedRoot {
    root: String,
    cache_root: String,
    hash_directory: String,
    empty_filename: String,
    filename: String,
}
#[test]
fn posix_cache_join_matches_actual_deno_normalization_and_relative_roots() {
    let value: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../parity/runner/c050-c051-helper-contracts/markdown-cache-boundary-observations.json"
    )))
    .unwrap();
    let rows: Vec<JoinedRoot> = serde_json::from_value(value["paths"].clone()).unwrap();
    assert_eq!(rows.len(), 5);
    for row in rows {
        assert_eq!(
            markdown_assets::cache_root(Some(&row.root), None, None)
                .to_str()
                .unwrap(),
            row.cache_root
        );
        assert_eq!(
            markdown_assets::posix_join(&[&row.root, "linear-cli-images", "hash"]),
            row.hash_directory
        );
        assert_eq!(
            markdown_assets::posix_join(&[&row.root, "linear-cli-images", "hash", ""]),
            row.empty_filename
        );
        assert_eq!(
            markdown_assets::posix_join(&[&row.root, "linear-cli-images", "hash", "x"]),
            row.filename
        );
    }
}
#[tokio::test]
async fn invalid_relative_url_creates_hash_directory_before_transport_and_preserves_exact_error() {
    let tree = Tree::new();
    let directory = tree.0.join("d3036d20a653e1a7");
    let mut stderr = Vec::new();
    let failed = markdown_assets::download_with(
        "![x](foo.png)",
        &tree.0,
        |url| {
            assert_eq!(url, "foo.png");
            assert!(directory.is_dir());
            async {
                Err(AppError::new(
                    AppErrorKind::Transport,
                    "Invalid URL: 'foo.png'",
                ))
            }
        },
        |bytes| {
            stderr.extend_from_slice(bytes);
            Ok(())
        },
    )
    .await
    .unwrap();
    assert!(failed.paths.is_empty());
    assert!(directory.is_dir());
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 0);
    assert_eq!(
        String::from_utf8(stderr).unwrap(),
        "Failed to download foo.png: Invalid URL: 'foo.png'\n"
    );
    // Actual Fetch TypeError/effects are frozen by the interpreted helper.
    // The injected transport error does not claim generic GET is implemented.
}
