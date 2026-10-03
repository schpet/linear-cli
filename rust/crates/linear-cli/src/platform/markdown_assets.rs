//! Downloading files a Markdown body references into a local cache, and
//! pointing the Markdown at the local copies.
//!
//! Only files hosted by Linear (or by the configured API endpoint's host) are
//! fetched: downloading arbitrary image URLs would reveal the reader's IP
//! address to whoever wrote the Markdown.
use crate::client::LinearClient;
use crate::config::OsFamily;
use crate::error::{Error, Result};
use crate::platform::private_file;
use pulldown_cmark::{Event, LinkType, Options, Parser, Tag, TagEnd};
use reqwest::Url;
use sha2::{Digest, Sha256};
use std::{
    collections::{HashMap, HashSet},
    ops::Range,
    path::{Path, PathBuf},
};

/// Hosts Linear serves uploaded files from.
const LINEAR_UPLOAD_HOSTS: [&str; 2] = ["uploads.linear.app", "public.linear.app"];

/// A downloadable file referenced by an inline image or link.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Asset {
    pub url: String,
    /// Image alt text or link text, used as the cached file's name.
    pub label: String,
}

/// An inline image or link destination in the source text.
#[derive(Debug)]
struct Reference {
    asset: Asset,
    /// Byte range to replace: the destination of `[..](dest)`, or the whole
    /// `<url>` of an autolink.
    range: Range<usize>,
    autolink: bool,
}

fn parser(content: &str) -> Parser<'_> {
    Parser::new_ext(
        content,
        Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS,
    )
}

/// Inline images and links (not reference-style ones) in document order.
fn references(content: &str) -> Vec<Reference> {
    struct Open {
        url: String,
        label: String,
        start: usize,
        end: usize,
        /// End of the label text: one past the opening bracket(s) until a
        /// child event extends it.
        label_end: usize,
        autolink: bool,
        /// Reference-style and email links are tracked only to keep the
        /// stack balanced.
        tracked: bool,
    }
    let mut open: Vec<Open> = Vec::new();
    let mut found = Vec::new();
    for (event, range) in parser(content).into_offset_iter() {
        match event {
            Event::Start(Tag::Image {
                link_type,
                dest_url,
                ..
            }) => open.push(Open {
                url: dest_url.into_string(),
                label: String::new(),
                start: range.start,
                end: range.end,
                label_end: range.start + "![".len(),
                autolink: false,
                tracked: link_type == LinkType::Inline,
            }),
            Event::Start(Tag::Link {
                link_type,
                dest_url,
                ..
            }) => open.push(Open {
                url: dest_url.into_string(),
                label: String::new(),
                start: range.start,
                end: range.end,
                label_end: range.start + "[".len(),
                autolink: link_type == LinkType::Autolink,
                tracked: matches!(link_type, LinkType::Inline | LinkType::Autolink),
            }),
            Event::End(TagEnd::Image | TagEnd::Link) => {
                let link = open
                    .pop()
                    .expect("pulldown-cmark closes only links and images it opened");
                if link.tracked {
                    let range = if link.autolink {
                        link.start..link.end
                    } else {
                        destination(content, link.label_end, link.end)
                            .expect("an inline link's source ends with `](destination)`")
                    };
                    found.push(Reference {
                        asset: Asset {
                            url: link.url,
                            label: link.label,
                        },
                        range,
                        autolink: link.autolink,
                    });
                }
            }
            Event::Text(text) | Event::Code(text) => {
                for link in &mut open {
                    link.label.push_str(&text);
                }
            }
            _ => {}
        }
        // Everything inside a link is part of its label; the label of each
        // still-open link extends at least to the end of this event.
        for link in &mut open {
            if range.start > link.start || range.end < link.end {
                link.label_end = link.label_end.max(range.end);
            }
        }
    }
    found.sort_by_key(|reference| reference.range.start);
    found
}

/// The byte range of the destination in `](dest "title")`, searching
/// `content[label_end..end]`.
fn destination(content: &str, label_end: usize, end: usize) -> Option<Range<usize>> {
    let tail = content.get(label_end..end)?;
    let open = tail.find("](")? + "](".len();
    let rest = tail.get(open..)?;
    let skipped = rest.len() - rest.trim_start().len();
    let start = label_end + open + skipped;
    let rest = rest.trim_start();
    if rest.starts_with('<') {
        let close = rest.find('>')?;
        return Some(start..start + close + 1);
    }
    let mut depth = 0usize;
    let mut escaped = false;
    for (index, character) in rest.char_indices() {
        match character {
            _ if escaped => escaped = false,
            '\\' => escaped = true,
            '(' => depth += 1,
            ')' if depth == 0 => return Some(start..start + index),
            ')' => depth -= 1,
            c if c.is_whitespace() => return Some(start..start + index),
            _ => {}
        }
    }
    None
}

/// Whether `url` is a file Linear hosts. `endpoint_host` also qualifies so a
/// self-hosted or proxied endpoint can serve its own uploads.
fn is_upload(url: &str, endpoint_host: Option<&str>) -> bool {
    let Ok(url) = Url::parse(url) else {
        return false;
    };
    matches!(url.scheme(), "http" | "https")
        && url
            .host_str()
            .is_some_and(|host| LINEAR_UPLOAD_HOSTS.contains(&host) || Some(host) == endpoint_host)
}

/// The uploads referenced by `bodies`, in order, each URL once (its first
/// label wins).
pub fn uploads(bodies: &[&str], endpoint_host: Option<&str>) -> Vec<Asset> {
    let mut seen = HashSet::new();
    bodies
        .iter()
        .flat_map(|body| references(body))
        .map(|reference| reference.asset)
        .filter(|asset| is_upload(&asset.url, endpoint_host) && seen.insert(asset.url.clone()))
        .collect()
}

/// `content` with each image or link destination found in `paths` replaced by
/// its local path. Everything else is left byte-for-byte unchanged.
pub fn rewrite(content: &str, paths: &HashMap<String, String>) -> String {
    let mut out = String::with_capacity(content.len());
    let mut copied = 0;
    for reference in references(content) {
        let Some(path) = paths.get(&reference.asset.url) else {
            continue;
        };
        out.push_str(source(content, copied..reference.range.start));
        let destination = if path.contains(|c: char| c.is_whitespace() || "()<>".contains(c)) {
            format!("<{path}>")
        } else {
            path.clone()
        };
        if reference.autolink {
            out.push_str(&format!("[{}]({destination})", reference.asset.url));
        } else {
            out.push_str(&destination);
        }
        copied = reference.range.end;
    }
    out.push_str(source(content, copied..content.len()));
    out
}

fn source(content: &str, range: Range<usize>) -> &str {
    content
        .get(range)
        .expect("parser offsets fall on character boundaries")
}

/// Downloads the uploads `bodies` reference into `root`, a private cache
/// directory, skipping files already cached, and returns each downloaded
/// URL's local path. A failed download is reported through `report` and its
/// URL is left out.
pub async fn download(
    client: &LinearClient,
    root: Option<&Path>,
    bodies: &[&str],
    mut report: impl FnMut(String) -> Result<()>,
) -> Result<HashMap<String, String>> {
    let endpoint_host = client.endpoint().url().host_str();
    let assets = uploads(bodies, endpoint_host);
    if assets.is_empty() {
        return Ok(HashMap::new());
    }
    let root = private_cache(root)?;
    let mut paths = HashMap::new();
    for asset in assets {
        let path = cache_path(root, &asset.url, &asset.label, "image");
        let fetched = fetch_cached(&path, || client.download_markdown_image(&asset.url)).await;
        match fetched {
            Ok(path) => {
                paths.insert(asset.url, path);
            }
            Err(error) => report(format!(
                "Failed to download {}: {}\n",
                asset.url,
                error.message()
            ))?,
        }
    }
    Ok(paths)
}

/// The file at `path`, downloaded with `fetch` unless it is already there,
/// as a UTF-8 path. The file is written atomically and readable only by its
/// owner.
pub async fn fetch_cached<F>(path: &Path, fetch: impl FnOnce() -> F) -> Result<String>
where
    F: std::future::Future<Output = Result<Vec<u8>>>,
{
    let directory = path.parent().expect("a cache path is inside its directory");
    private_file::create_dir_all(directory).map_err(|error| cache_error(directory, error))?;
    if !private_file::is_file(path) {
        let body = fetch().await?;
        private_file::write_atomic(path, &body).map_err(|error| cache_error(path, error))?;
    }
    path.to_str()
        .map(str::to_owned)
        .ok_or_else(|| Error::new(format!("Cache path {} is not valid UTF-8", path.display())))
}

/// `<root>/<url hash>/<label>`: the hash of the full URL is the file's
/// identity (Linear upload URLs name one file, in one workspace, forever),
/// and the label keeps the file name recognizable.
pub fn cache_path(root: &Path, url: &str, label: &str, fallback: &str) -> PathBuf {
    let digest = Sha256::digest(url.as_bytes());
    let directory: String = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    root.join(directory)
        .join(sanitized_filename(label, fallback))
}

/// The per-user cache directory: `$XDG_CACHE_HOME/linear-cli` or
/// `~/.cache/linear-cli`, or `%LOCALAPPDATA%\linear-cli` on Windows. `None`
/// when none of those is set; a shared temporary directory is never used.
pub fn cache_root(
    os: OsFamily,
    xdg_cache_home: Option<&str>,
    home: Option<&str>,
    local_app_data: Option<&str>,
) -> Option<PathBuf> {
    let present = |value: &&str| !value.is_empty();
    let base = match os {
        OsFamily::Unix => xdg_cache_home
            .filter(present)
            .map(PathBuf::from)
            .or_else(|| {
                home.filter(present)
                    .map(|home| Path::new(home).join(".cache"))
            }),
        OsFamily::Windows => local_app_data.filter(present).map(PathBuf::from),
    }?;
    Some(base.join("linear-cli"))
}

/// `root`, created and checked to be private (see
/// [`private_file::private_dir`]); `None` means no cache directory exists.
pub fn private_cache(root: Option<&Path>) -> Result<&Path> {
    let root = root.ok_or_else(|| {
        Error::new("Could not find a cache directory for downloads")
            .with_hint("Set XDG_CACHE_HOME or HOME, or pass --no-download.")
    })?;
    private_file::private_dir(root).map_err(|error| cache_error(root, error))?;
    Ok(root)
}

fn cache_error(path: &Path, error: std::io::Error) -> Error {
    Error::new(format!("Cannot use cache {}: {error}", path.display())).with_source(error)
}

/// `name` made safe as a file name on every platform, or `fallback` when
/// nothing usable is left.
pub fn sanitized_filename(name: &str, fallback: &str) -> String {
    let mut value: String = name
        .chars()
        .filter(|ch| !matches!(ch, '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*'))
        .filter(|ch| !ch.is_control())
        .collect();
    while value.len() > 255 {
        value.pop();
    }
    value.truncate(value.trim_end_matches(['.', ' ']).len());
    let stem = value.split('.').next().unwrap_or("").to_ascii_lowercase();
    let port_number = stem
        .strip_prefix("com")
        .or_else(|| stem.strip_prefix("lpt"));
    let reserved = matches!(stem.as_str(), "con" | "prn" | "aux" | "nul")
        || matches!(
            port_number,
            Some("0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "¹" | "²" | "³")
        );
    if reserved || value.is_empty() {
        fallback.to_owned()
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assets(content: &str) -> Vec<(String, String)> {
        references(content)
            .into_iter()
            .map(|reference| (reference.asset.url, reference.asset.label))
            .collect()
    }

    fn paths(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(url, path)| ((*url).to_owned(), (*path).to_owned()))
            .collect()
    }

    #[test]
    fn finds_inline_images_and_links_in_document_order() {
        let content = "[doc](https://uploads.linear.app/a.pdf) and ![shot *1*](https://uploads.linear.app/b.png)\n\n- ![](https://uploads.linear.app/c.png)";
        assert_eq!(
            assets(content),
            [
                ("https://uploads.linear.app/a.pdf".into(), "doc".into()),
                ("https://uploads.linear.app/b.png".into(), "shot 1".into()),
                ("https://uploads.linear.app/c.png".into(), String::new()),
            ]
        );
    }

    #[test]
    fn ignores_reference_style_links_and_code() {
        let content = "![a][img] `![b](https://uploads.linear.app/b.png)`\n\n    ![c](https://uploads.linear.app/c.png)\n\n[img]: https://uploads.linear.app/a.png";
        assert!(assets(content).is_empty());
    }

    #[test]
    fn only_linear_and_endpoint_hosts_are_uploads() {
        let content = "![a](https://uploads.linear.app/a.png) ![b](https://tracker.example/b.png) ![c](http://127.0.0.1:9/c.png) ![d](file:///etc/passwd) ![e](relative.png) [f](https://public.linear.app/f.png)";
        let urls = |endpoint| {
            uploads(&[content], endpoint)
                .into_iter()
                .map(|asset| asset.url)
                .collect::<Vec<_>>()
        };
        assert_eq!(
            urls(None),
            [
                "https://uploads.linear.app/a.png",
                "https://public.linear.app/f.png"
            ]
        );
        assert_eq!(
            urls(Some("127.0.0.1")),
            [
                "https://uploads.linear.app/a.png",
                "http://127.0.0.1:9/c.png",
                "https://public.linear.app/f.png"
            ]
        );
    }

    #[test]
    fn uploads_are_deduplicated_across_bodies_keeping_the_first_label() {
        let first = "![first](https://uploads.linear.app/a.png)";
        let second =
            "![second](https://uploads.linear.app/a.png) ![b](https://uploads.linear.app/b.png)";
        let found = uploads(&[first, second], None);
        assert_eq!(
            found
                .iter()
                .map(|asset| asset.label.as_str())
                .collect::<Vec<_>>(),
            ["first", "b"]
        );
    }

    #[test]
    fn rewrite_splices_destinations_and_keeps_everything_else() {
        let content = "# Title\n\n* item ![shot](https://uploads.linear.app/a.png \"Title\") and [a](<https://uploads.linear.app/a.png>)\n\n| x | ![t](https://uploads.linear.app/t.png) |\n|---|---|\n\n![keep](https://example.com/k.png)\n";
        let rewritten = rewrite(
            content,
            &paths(&[
                ("https://uploads.linear.app/a.png", "/tmp/c/shot"),
                ("https://uploads.linear.app/t.png", "/tmp/c/my file (1)"),
            ]),
        );
        assert_eq!(
            rewritten,
            "# Title\n\n* item ![shot](/tmp/c/shot \"Title\") and [a](/tmp/c/shot)\n\n| x | ![t](</tmp/c/my file (1)>) |\n|---|---|\n\n![keep](https://example.com/k.png)\n"
        );
    }

    #[test]
    fn rewrite_handles_nested_brackets_parens_and_autolinks() {
        let content = "[![a [b\\]](https://uploads.linear.app/i.png)](https://uploads.linear.app/l(1).pdf)\r\n<https://uploads.linear.app/auto.pdf>\r\n![x](\n  https://uploads.linear.app/i.png\n)";
        let rewritten = rewrite(
            content,
            &paths(&[
                ("https://uploads.linear.app/i.png", "/c/i"),
                ("https://uploads.linear.app/l(1).pdf", "/c/l"),
                ("https://uploads.linear.app/auto.pdf", "/c/auto"),
            ]),
        );
        assert_eq!(
            rewritten,
            "[![a [b\\]](/c/i)](/c/l)\r\n[https://uploads.linear.app/auto.pdf](/c/auto)\r\n![x](\n  /c/i\n)"
        );
    }

    #[test]
    fn rewrite_without_matches_returns_the_input() {
        let content = "Some *text* with ![img](https://uploads.linear.app/a.png).";
        assert_eq!(rewrite(content, &HashMap::new()), content);
    }

    #[test]
    fn file_names_are_sanitized_with_a_fallback() {
        for (name, expected) in [
            ("", "image"),
            ("CON.txt", "image"),
            ("con.", "image"),
            ("com0.txt", "image"),
            ("LPT9", "image"),
            ("com10", "com10"),
            ("com⁴", "com⁴"),
            ("com", "com"),
            ("..", "image"),
            ("a<>:/\\|?*\u{1}\u{80}b. ", "ab"),
            ("cache space (a)", "cache space (a)"),
        ] {
            assert_eq!(sanitized_filename(name, "image"), expected, "{name:?}");
        }
        assert_eq!(sanitized_filename(&"界".repeat(100), "x"), "界".repeat(85));
    }

    #[test]
    fn file_names_reject_reserved_names_created_by_cleanup() {
        for name in ["nul ", "con .", "AUX  ..", "lpt9 ", "COM0 "] {
            assert_eq!(sanitized_filename(name, "image"), "image", "{name:?}");
        }
        assert_eq!(
            sanitized_filename(&format!("aux{}x", " ".repeat(300)), "attachment"),
            "attachment"
        );
    }

    #[test]
    fn file_names_trim_dots_and_spaces_created_by_truncation() {
        for (name, expected) in [
            (format!("{}. b", "a".repeat(254)), "a".repeat(254)),
            (format!("{} b", "a".repeat(254)), "a".repeat(254)),
            (format!("{}.界", "a".repeat(253)), "a".repeat(253)),
        ] {
            assert_eq!(sanitized_filename(&name, "image"), expected);
        }
    }

    #[test]
    fn file_names_reject_reserved_superscript_device_names() {
        for name in ["COM¹", "com².txt", "CoM³", "LPT¹.txt", "lpt²", "LpT³.foo"] {
            assert_eq!(sanitized_filename(name, "image"), "image", "{name:?}");
        }
    }

    #[test]
    fn cache_path_hashes_the_url() {
        assert_eq!(
            cache_path(
                Path::new("/cache"),
                "https://uploads.linear.app/private.png?token=fake",
                "cache space (a)",
                "image"
            ),
            PathBuf::from("/cache/57c89d19aa713f0e/cache space (a)")
        );
        // Files with the same name but different URLs never share a path.
        let path = |url| cache_path(Path::new("/cache"), url, "notes.pdf", "attachment");
        assert_ne!(
            path("https://uploads.linear.app/org-a/1/notes.pdf"),
            path("https://uploads.linear.app/org-b/1/notes.pdf")
        );
    }

    #[test]
    fn the_cache_is_per_user_and_never_a_shared_temporary_directory() {
        let unix = |xdg, home| cache_root(OsFamily::Unix, xdg, home, Some("C:/unused"));
        assert_eq!(
            unix(Some("/x/cache"), Some("/home/u")),
            Some(PathBuf::from("/x/cache/linear-cli"))
        );
        assert_eq!(
            unix(Some(""), Some("/home/u")),
            Some(PathBuf::from("/home/u/.cache/linear-cli"))
        );
        assert_eq!(unix(None, None), None);
        assert_eq!(
            cache_root(
                OsFamily::Windows,
                Some("/x"),
                Some("/home/u"),
                Some("C:/Local")
            ),
            Some(PathBuf::from("C:/Local/linear-cli"))
        );
        assert_eq!(
            cache_root(OsFamily::Windows, None, Some("/home/u"), None),
            None
        );
    }
}
