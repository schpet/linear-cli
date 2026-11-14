//! Preparing a pull request head in a jj repository: which commit it is,
//! which bookmark names it, and pushing that bookmark. gh is then told the
//! repository and the head, so it never needs git's HEAD or a checkout.
use std::path::Path;
use std::process::Command;

use serde::Deserialize;

use crate::config::ChildEnvOverlay;
use crate::error::{Error, Result};
use crate::platform::process;
use crate::platform::vcs::{self, RepositoryKind};

/// One commit's `json(self)`, then whether it is empty and conflicted.
const COMMIT: &str = r#""[" ++ json(self) ++ "," ++ json(empty) ++ "," ++ json(conflict) ++ "]\n""#;
const BOOKMARK: &str = r#"json(self) ++ "\n""#;
const CONFIG: &str = r#""[" ++ json(name) ++ "," ++ json(value) ++ "]\n""#;

#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
struct CommitFields {
    commit_id: String,
    parents: Vec<String>,
    description: String,
}

/// A commit as far as choosing a pull request head needs to know it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Commit {
    pub id: String,
    pub parents: Vec<String>,
    pub described: bool,
    pub empty: bool,
    pub conflict: bool,
}

/// A bookmark record of `jj bookmark list`: local when `remote` is absent,
/// and pointing at one commit unless it is deleted or conflicted.
#[derive(Clone, Debug, PartialEq, Eq, Deserialize)]
pub struct Bookmark {
    pub name: String,
    #[serde(default)]
    pub remote: Option<String>,
    pub target: Vec<Option<String>>,
}

impl Bookmark {
    /// The one commit a local bookmark points at.
    fn commit(&self) -> Option<&str> {
        match self.target.as_slice() {
            [Some(id)] if self.remote.is_none() => Some(id),
            _ => None,
        }
    }
}

/// The git remote a bookmark is pushed to, which is also the repository the
/// pull request is opened in.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Remote {
    pub name: String,
    pub url: String,
}

/// Where the pull request head comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    /// The local bookmark `--head` names.
    Bookmark(String),
    /// The working-copy commit.
    WorkingCopy,
    /// The working copy's only parent, under an empty undescribed scratch change.
    Parent(String),
}

/// Everything settled before the issue is fetched.
#[derive(Clone, Debug)]
pub struct Survey {
    pub remote: Remote,
    head: Commit,
    bookmarks: Vec<Bookmark>,
    /// The `--head` value, when given.
    explicit: Option<String>,
}

/// The bookmark the pull request is opened from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Head {
    pub name: String,
    /// Whether the bookmark has yet to be created at the head commit.
    pub create: bool,
}

/// jj run in the working directory with the child environment.
pub struct Jj<'a> {
    cwd: &'a Path,
    env: &'a ChildEnvOverlay,
}

impl<'a> Jj<'a> {
    pub fn new(cwd: &'a Path, env: &'a ChildEnvOverlay) -> Self {
        Self { cwd, env }
    }

    /// A jj command that reads the repository as it was last snapshotted.
    fn quiet(&self) -> Command {
        let mut command = process::command("jj", self.cwd, self.env);
        command.args(["--ignore-working-copy", "--no-pager", "--color", "never"]);
        command
    }

    /// Snapshots the working copy and settles the remote, the head commit
    /// and the bookmarks there are, without changing anything else.
    pub fn survey(&self, explicit: Option<&str>) -> Result<Survey> {
        if let Some(head) = explicit
            && head.contains(':')
        {
            return Err(Error::invalid(format!(
                "Cannot open a pull request from {head} with jj"
            ))
            .with_hint(
                "With jj, --head names a bookmark in this repository. For a pull request from another repository, use `gh pr create` directly.",
            ));
        }
        // The one command that snapshots: what is on disk decides whether
        // `@` is an empty scratch change.
        let mut snapshot = process::command("jj", self.cwd, self.env);
        snapshot.args(["--no-pager", "--color", "never"]);
        snapshot.args(["log", "-r", "@", "--no-graph", "-T", COMMIT]);
        let output = process::checked_output(&mut snapshot).map_err(|error| {
            if vcs::in_repository(RepositoryKind::Jj, self.cwd) {
                error.context("Failed to inspect the jj working copy")
            } else {
                vcs::not_in_repository(RepositoryKind::Jj).with_hint(
                    "Run it from your jj repository. In a git checkout, use `linear issue gh pr` instead.",
                )
            }
        })?;
        let working_copy = parse_commit(&output.stdout)?;
        let remote = self.remote()?;
        let bookmarks = self.bookmarks()?;
        let head = match source(explicit, &working_copy, &bookmarks)? {
            Source::WorkingCopy => working_copy,
            Source::Bookmark(id) | Source::Parent(id) => self.commit(&id)?,
        };
        if head.conflict {
            return Err(Error::new(format!(
                "The pull request revision {} has conflicts",
                short(&head.id)
            ))
            .with_hint("Resolve the conflicts before opening a pull request."));
        }
        // Immutability is not checked: it is jj's rewrite policy, and a tag
        // or a remote bookmark can make unmerged work immutable. jj decides
        // what it pushes, and gh whether there is anything to merge into the
        // base.
        if !head.described {
            // jj refuses to push it; saying so now leaves no bookmark behind.
            return Err(Error::invalid(format!(
                "The pull request revision {} has no description",
                short(&head.id)
            ))
            .with_hint("Describe it with `jj describe`, then try again."));
        }
        self.pushable(&remote, &head)?;
        Ok(Survey {
            remote,
            head,
            bookmarks,
            explicit: explicit.map(str::to_owned),
        })
    }

    fn commit(&self, id: &str) -> Result<Commit> {
        let output = process::checked_output(self.quiet().args([
            "log",
            "-r",
            id,
            "--no-graph",
            "-T",
            COMMIT,
        ]))
        .map_err(|error| error.context("Failed to inspect the pull request revision"))?;
        parse_commit(&output.stdout)
    }

    /// Fails when the push would be refused for a change under the head
    /// that the remote does not have yet, before a bookmark is created.
    fn pushable(&self, remote: &Remote, head: &Commit) -> Result<()> {
        let revset = format!(
            "(remote_bookmarks(remote=exact:{}).. & ::{}) & (description(exact:\"\") | conflicts())",
            serde_json::Value::from(remote.name.as_str()),
            head.id
        );
        let output = process::checked_output(self.quiet().args([
            "log",
            "-r",
            &revset,
            "--no-graph",
            "-T",
            "commit_id.short() ++ \"\\n\"",
        ]))
        .map_err(|error| error.context("Failed to inspect the changes to push"))?;
        let refused: Vec<&str> = std::str::from_utf8(&output.stdout)
            .map_err(|error| malformed("jj log", error))?
            .lines()
            .collect();
        if refused.is_empty() {
            return Ok(());
        }
        Err(Error::invalid(format!(
            "jj will not push the pull request: {} under it has no description or has conflicts",
            refused.join(", ")
        ))
        .with_hint("Describe those changes with `jj describe -r <revision>` and resolve their conflicts, then try again."))
    }

    fn bookmarks(&self) -> Result<Vec<Bookmark>> {
        let output =
            process::checked_output(self.quiet().args(["bookmark", "list", "-T", BOOKMARK]))
                .map_err(|error| error.context("Failed to list jj bookmarks"))?;
        parse_lines(&output.stdout, "jj bookmark list")
    }

    /// The remote `jj git push` uses by default: `git.push` when set, else
    /// the only remote, else `origin`.
    fn remote(&self) -> Result<Remote> {
        let output = process::checked_output(
            self.quiet()
                .args(["config", "list", "git.push", "-T", CONFIG]),
        )
        .map_err(|error| error.context("Failed to read jj's git.push setting"))?;
        let settings: Vec<(String, serde_json::Value)> =
            parse_lines(&output.stdout, "jj config list")?;
        let configured = settings
            .into_iter()
            .find(|(name, _)| name == "git.push")
            .map(|(_, value)| match value {
                serde_json::Value::String(name) if !name.trim().is_empty() => Ok(name),
                _ => Err(Error::new("jj's git.push setting is not a remote name")
                    .with_hint("Set it with `jj config set --repo git.push <remote>`.")),
            })
            .transpose()?;
        let output = process::checked_output(self.quiet().args(["git", "remote", "list"]))
            .map_err(|error| error.context("Failed to list the jj repository's git remotes"))?;
        let remotes = parse_remotes(&String::from_utf8_lossy(&output.stdout))?;
        let remote = choose_remote(configured.as_deref(), remotes)?;
        if !is_hosted(&remote.url) {
            return Err(Error::new(format!(
                "Cannot tell which GitHub repository the jj remote {} is",
                remote.name
            ))
            .with_hint(
                "Its URL must be an https:// or ssh:// URL, or the git@host:owner/repo form.",
            ));
        }
        Ok(remote)
    }

    /// Creates the bookmark when it does not exist yet, checks that it is
    /// still at the head, and pushes it. Returns what jj reported.
    pub fn publish(&self, survey: &Survey, head: &Head) -> Result<String> {
        let name = head.name.as_str();
        let mut reported = String::new();
        if head.create {
            let output = process::checked_output(self.quiet().args([
                "bookmark",
                "create",
                "-r",
                &survey.head.id,
                "--",
                name,
            ]))
            .map_err(|error| error.context(format!("Failed to create the jj bookmark {name}")))?;
            reported.push_str(&String::from_utf8_lossy(&output.stderr));
        }
        let now = self.bookmarks()?;
        let still = now
            .iter()
            .filter(|bookmark| bookmark.remote.is_none() && bookmark.name == name)
            .any(|bookmark| bookmark.commit() == Some(survey.head.id.as_str()));
        if !still {
            return Err(Error::new(format!(
                "The jj bookmark {name} changed while the pull request was being prepared"
            ))
            .with_hint(
                "Check where the bookmark points with `jj bookmark list`, then try again.",
            ));
        }
        let pattern = format!("exact:{name}");
        let output = process::checked_output(self.quiet().args([
            "git",
            "push",
            "--remote",
            &survey.remote.name,
            "--bookmark",
            &pattern,
        ]))
        .map_err(|error| {
            error
                .context(format!(
                    "Failed to push the jj bookmark {name} to {}",
                    survey.remote.name
                ))
                .with_hint(format!(
                    "No pull request was opened. Fix the push, or push the bookmark yourself and pass --head {name}."
                ))
        })?;
        reported.push_str(&String::from_utf8_lossy(&output.stderr));
        Ok(reported)
    }
}

impl Survey {
    /// The bookmark to open the pull request from: `--head` when given, else
    /// a bookmark already at the head commit, else a new one with the name
    /// Linear `suggested` for the issue. A bookmark is never moved.
    pub fn head(&self, suggested: &str) -> Result<Head> {
        choose_bookmark(
            self.explicit.as_deref(),
            suggested,
            &self.head.id,
            &self.bookmarks,
        )
    }
}

fn short(id: &str) -> &str {
    id.get(..12).unwrap_or(id)
}

fn malformed(what: &str, error: impl std::fmt::Display) -> Error {
    Error::new(format!("Unexpected output from `{what}`: {error}"))
        .with_hint("This needs jj 0.38 or newer; check `jj --version`.")
}

fn parse_lines<T: serde::de::DeserializeOwned>(stdout: &[u8], what: &str) -> Result<Vec<T>> {
    let text = std::str::from_utf8(stdout).map_err(|error| malformed(what, error))?;
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).map_err(|error| malformed(what, error)))
        .collect()
}

fn parse_commit(stdout: &[u8]) -> Result<Commit> {
    let mut records: Vec<(CommitFields, bool, bool)> = parse_lines(stdout, "jj log")?;
    let (Some((fields, empty, conflict)), true) = (records.pop(), records.is_empty()) else {
        return Err(malformed("jj log", "expected exactly one revision"));
    };
    Ok(Commit {
        id: fields.commit_id,
        parents: fields.parents,
        described: !fields.description.trim().is_empty(),
        empty,
        conflict,
    })
}

/// Which commit the pull request is opened from. An existing bookmark named
/// by `--head` decides it. Otherwise it is the working-copy commit, or its
/// parent when the working copy is the empty, undescribed scratch change of
/// the squash workflow.
fn source(explicit: Option<&str>, working_copy: &Commit, bookmarks: &[Bookmark]) -> Result<Source> {
    if let Some(name) = explicit
        && let Some(bookmark) = local(bookmarks, name)
    {
        return bookmark
            .commit()
            .map(|id| Source::Bookmark(id.to_owned()))
            .ok_or_else(|| unusable(name));
    }
    if !working_copy.empty || working_copy.described {
        return Ok(Source::WorkingCopy);
    }
    match working_copy.parents.as_slice() {
        [parent] => Ok(Source::Parent(parent.clone())),
        parents => Err(Error::invalid(format!(
            "Cannot tell which revision to open a pull request from: the working copy is empty and has {} parents",
            parents.len()
        ))
        .with_hint("Pass --head with the bookmark to open the pull request from.")),
    }
}

fn local<'a>(bookmarks: &'a [Bookmark], name: &str) -> Option<&'a Bookmark> {
    bookmarks
        .iter()
        .find(|bookmark| bookmark.remote.is_none() && bookmark.name == name)
}

fn unusable(name: &str) -> Error {
    Error::new(format!("The jj bookmark {name} is deleted or conflicted"))
        .with_hint("Resolve it with `jj bookmark`, or pass --head with another bookmark name.")
}

fn choose_bookmark(
    explicit: Option<&str>,
    suggested: &str,
    head: &str,
    bookmarks: &[Bookmark],
) -> Result<Head> {
    if let Some(name) = explicit {
        // An existing one is where the head came from; a new name is created there.
        return Ok(Head {
            name: name.to_owned(),
            create: local(bookmarks, name).is_none(),
        });
    }
    let mut here: Vec<&str> = bookmarks
        .iter()
        .filter(|bookmark| bookmark.commit() == Some(head))
        .map(|bookmark| bookmark.name.as_str())
        .collect();
    here.sort_unstable();
    here.dedup();
    let existing = match here.as_slice() {
        [] => None,
        [only] => Some(*only),
        several if several.contains(&suggested) => Some(suggested),
        several => {
            return Err(Error::invalid(format!(
                "Several jj bookmarks point at the pull request revision: {}",
                several.join(", ")
            ))
            .with_hint("Pass --head <bookmark> to choose one."));
        }
    };
    if let Some(name) = existing {
        return Ok(Head {
            name: name.to_owned(),
            create: false,
        });
    }
    match local(bookmarks, suggested) {
        None => Ok(Head {
            name: suggested.to_owned(),
            create: true,
        }),
        Some(bookmark) if bookmark.commit().is_none() => Err(unusable(suggested)),
        Some(_) => Err(Error::invalid(format!(
            "The jj bookmark {suggested} already exists at another revision"
        ))
        .with_hint(format!(
            "Move it yourself with `jj bookmark move {suggested} --to <revision>`, or pass --head with a new bookmark name."
        ))),
    }
}

/// Each line names a remote and its fetch URL; a push URL that differs
/// follows in parentheses and is left out.
fn parse_remotes(text: &str) -> Result<Vec<Remote>> {
    text.lines()
        .filter(|line| !line.trim().is_empty())
        .map(|line| {
            let mut words = line.split_whitespace();
            match (words.next(), words.next()) {
                (Some(name), Some(url)) => Ok(Remote {
                    name: name.to_owned(),
                    url: url.to_owned(),
                }),
                _ => Err(malformed("jj git remote list", "a remote without a URL")),
            }
        })
        .collect()
}

fn choose_remote(configured: Option<&str>, remotes: Vec<Remote>) -> Result<Remote> {
    let named = |name: &str| remotes.iter().find(|remote| remote.name == name).cloned();
    if let Some(name) = configured {
        return named(name).ok_or_else(|| {
            Error::new(format!(
                "jj's git.push setting names the remote {name}, which does not exist"
            ))
            .with_hint("Fix it with `jj config set --repo git.push <remote>`, or add the remote with `jj git remote add`.")
        });
    }
    match remotes.as_slice() {
        [] => Err(
            Error::new("This jj repository has no git remote to push to")
                .with_hint("Add one with `jj git remote add origin <url>`."),
        ),
        [only] => Ok(only.clone()),
        several => named("origin").ok_or_else(|| {
            let names: Vec<&str> = several.iter().map(|remote| remote.name.as_str()).collect();
            Error::new(format!(
                "Cannot tell which remote to push to: {}",
                names.join(", ")
            ))
            .with_hint("Choose one with `jj config set --repo git.push <remote>`.")
        }),
    }
}

/// Whether `url` is a form gh reads a repository from: an HTTPS or SSH URL,
/// or `user@host:owner/repo`. A local path is not.
fn is_hosted(url: &str) -> bool {
    if url.starts_with("https://") || url.starts_with("ssh://") {
        return true;
    }
    match url.split_once(':') {
        Some((host, path)) => {
            !url.contains("://")
                && !host.is_empty()
                && !host.contains(['/', '\\'])
                && host.len() > 1
                && !path.is_empty()
                && !path.starts_with('/')
        }
        None => false,
    }
}

#[cfg(test)]
mod tests;
