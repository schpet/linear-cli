//! Telling whether the working directory is inside a git or jj repository,
//! for the commands that prepare a branch, a change or a pull request.
use std::path::Path;

use crate::config::Vcs;
use crate::error::Error;

/// Whether `cwd` is inside a repository of the `vcs` kind: a `.git` (a
/// directory, or the file a worktree has) or a `.jj` directory there or in a
/// parent. Only used to explain a failed git or jj command, so setups such
/// as `GIT_DIR` that work without one are unaffected.
pub fn in_repository(vcs: Vcs, cwd: &Path) -> bool {
    cwd.ancestors().any(|dir| match vcs {
        Vcs::Git => dir.join(".git").exists(),
        Vcs::Jj => dir.join(".jj").is_dir(),
    })
}

/// The error for a command that needs a repository run outside one.
pub fn not_in_repository(vcs: Vcs) -> Error {
    let name = match vcs {
        Vcs::Git => "git",
        Vcs::Jj => "jj",
    };
    Error::new(format!("Not in a {name} repository"))
}
