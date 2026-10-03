//! Files only their owner can read: credentials and downloaded attachments.
use std::fs::{self, File, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

/// Replaces `path` with `contents` atomically. The bytes go to a new
/// owner-only file in the same directory, which is synced and renamed over
/// `path`, so an interrupted write never leaves a truncated file and the
/// result is never readable by others, whatever the old file's mode was. A
/// symlink at `path` is replaced, not followed.
pub fn write_atomic(path: &Path, contents: &[u8]) -> io::Result<()> {
    let directory = match path.parent() {
        Some(parent) if !parent.as_os_str().is_empty() => parent,
        _ => Path::new("."),
    };
    let name = path
        .file_name()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "path has no file name"))?;
    let (temp, mut file) = create_temp(directory, &name.to_string_lossy())?;
    let written = file
        .write_all(contents)
        .and_then(|()| file.sync_all())
        .and_then(|()| {
            drop(file);
            fs::rename(&temp, path)
        })
        .and_then(|()| sync_directory(directory));
    if written.is_err() {
        // The rename did not happen (or the file is already in place);
        // either way the temporary name is not needed.
        let _ = fs::remove_file(&temp);
    }
    written
}

/// A new, empty, owner-only file named after `name` in `directory`.
fn create_temp(directory: &Path, name: &str) -> io::Result<(PathBuf, File)> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let process = std::process::id();
    for attempt in 0..100_u32 {
        let temp = directory.join(format!(".{name}.{process}.{attempt}.tmp"));
        match options.open(&temp) {
            Ok(file) => return Ok((temp, file)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        format!(
            "could not pick a temporary file name in {}",
            directory.display()
        ),
    ))
}

/// Makes a rename in `directory` durable.
#[cfg(unix)]
fn sync_directory(directory: &Path) -> io::Result<()> {
    File::open(directory)?.sync_all()
}

#[cfg(not(unix))]
fn sync_directory(_: &Path) -> io::Result<()> {
    Ok(())
}

/// Creates `directory` and any missing parents, each new one accessible
/// only by its owner. Existing directories are left as they are.
pub fn create_dir_all(directory: &Path) -> io::Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(directory)
}

/// Creates `directory` (see [`create_dir_all`]) and checks that it is a
/// real directory, not a symlink, owned by this user, and closes it to
/// others if it was open. Anything written inside then cannot be swapped or
/// read by another user.
pub fn private_dir(directory: &Path) -> io::Result<()> {
    create_dir_all(directory)?;
    let metadata = fs::symlink_metadata(directory)?;
    if !metadata.is_dir() {
        return Err(io::Error::other(format!(
            "{} is not a directory",
            directory.display()
        )));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        if metadata.uid() != current_user(directory)? {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                format!("{} belongs to another user", directory.display()),
            ));
        }
        if metadata.mode() & 0o077 != 0 {
            fs::set_permissions(directory, fs::Permissions::from_mode(0o700))?;
        }
    }
    Ok(())
}

/// This process's user id: the owner of a file it creates in `directory`.
#[cfg(unix)]
fn current_user(directory: &Path) -> io::Result<u32> {
    use std::os::unix::fs::MetadataExt;
    let (probe, file) = create_temp(directory, "owner")?;
    let user = file.metadata().map(|metadata| metadata.uid());
    drop(file);
    fs::remove_file(&probe)?;
    user
}

/// Whether `path` is a regular file; a symlink is not followed.
pub fn is_file(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_file())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_replace_the_file_and_leave_no_temporary_files() {
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("file");
        write_atomic(&path, b"one").expect("write");
        write_atomic(&path, b"two").expect("rewrite");
        assert_eq!(fs::read(&path).expect("read"), b"two");
        let names: Vec<_> = fs::read_dir(dir.path())
            .expect("list")
            .map(|entry| entry.expect("entry").file_name())
            .collect();
        assert_eq!(names, ["file"]);
    }

    #[cfg(unix)]
    #[test]
    fn written_files_are_owner_only_even_when_the_old_one_was_not() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temp dir");
        let path = dir.path().join("file");
        fs::write(&path, b"old").expect("write");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).expect("chmod");
        write_atomic(&path, b"new").expect("rewrite");
        let mode = fs::metadata(&path).expect("stat").permissions().mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_in_place_of_the_file_is_replaced_not_followed() {
        let dir = tempfile::tempdir().expect("temp dir");
        let target = dir.path().join("target");
        fs::write(&target, b"target").expect("write");
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&target, &link).expect("symlink");
        assert!(!is_file(&link));
        write_atomic(&link, b"new").expect("write");
        assert_eq!(fs::read(&target).expect("read"), b"target");
        assert!(is_file(&link));
    }

    #[cfg(unix)]
    #[test]
    fn private_dirs_are_created_closed_and_symlinks_refused() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("temp dir");
        let private = dir.path().join("a/b");
        private_dir(&private).expect("create");
        let mode = |path: &Path| fs::metadata(path).expect("stat").permissions().mode() & 0o777;
        assert_eq!(mode(&private), 0o700);
        assert_eq!(mode(&dir.path().join("a")), 0o700);
        fs::set_permissions(&private, fs::Permissions::from_mode(0o777)).expect("chmod");
        private_dir(&private).expect("existing");
        assert_eq!(mode(&private), 0o700);
        let link = dir.path().join("link");
        std::os::unix::fs::symlink(&private, &link).expect("symlink");
        private_dir(&link).expect_err("symlink");
    }
}
