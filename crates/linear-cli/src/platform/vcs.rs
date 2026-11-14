//! Telling whether the working directory is inside a git or jj repository,
//! for the commands that prepare a branch, a change or a pull request.
use std::path::Path;

use crate::error::Error;

/// The kind of repository a command works in. Always named by the command
/// itself (`issue git …`, `issue jj …`, `@git`, `@jj`), never by a setting.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepositoryKind {
    Git,
    Jj,
}

/// Whether `cwd` is inside a repository of the `kind`: a `.git` (a
/// directory, or the file a worktree has) or a `.jj` directory there or in a
/// parent. Only used to explain a failed git or jj command, so setups such
/// as `GIT_DIR` that work without one are unaffected.
pub fn in_repository(kind: RepositoryKind, cwd: &Path) -> bool {
    cwd.ancestors().any(|dir| match kind {
        RepositoryKind::Git => dir.join(".git").exists(),
        RepositoryKind::Jj => dir.join(".jj").is_dir(),
    })
}

/// The error for a command that needs a repository run outside one.
pub fn not_in_repository(kind: RepositoryKind) -> Error {
    let name = match kind {
        RepositoryKind::Git => "git",
        RepositoryKind::Jj => "jj",
    };
    Error::new(format!("Not in a {name} repository"))
}
