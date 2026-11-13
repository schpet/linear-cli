//! Carrying out a plan: the only code in `linear skill` that changes files.
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::plan::{Change, Step, Stubs, inspect};
use super::slots::{SKILL_FILE, SKILL_NAME, Slot};
use crate::platform::private_file;

/// The filesystem changes `setup` and `remove` make, so tests can make one
/// fail.
pub trait Files {
    fn create_dir_all(&self, path: &Path) -> io::Result<()>;
    /// Creates one directory, failing if anything is already there.
    fn create_dir(&self, path: &Path) -> io::Result<()>;
    /// Replaces the file at `path` in one step.
    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()>;
    fn rename(&self, from: &Path, to: &Path) -> io::Result<()>;
    fn remove_file(&self, path: &Path) -> io::Result<()>;
    /// Removes an empty directory.
    fn remove_dir(&self, path: &Path) -> io::Result<()>;
}

pub struct RealFiles;

impl Files for RealFiles {
    fn create_dir_all(&self, path: &Path) -> io::Result<()> {
        fs::create_dir_all(path)
    }

    fn create_dir(&self, path: &Path) -> io::Result<()> {
        fs::create_dir(path)
    }

    fn write(&self, path: &Path, contents: &[u8]) -> io::Result<()> {
        private_file::write_atomic_shared(path, contents)
    }

    fn rename(&self, from: &Path, to: &Path) -> io::Result<()> {
        fs::rename(from, to)
    }

    fn remove_file(&self, path: &Path) -> io::Result<()> {
        fs::remove_file(path)
    }

    fn remove_dir(&self, path: &Path) -> io::Result<()> {
        fs::remove_dir(path)
    }
}

/// Applies `steps` in order, recording in each what happened. A slot that no
/// longer looks as it did when the plan was made becomes a conflict. The
/// first filesystem failure is recorded and stops the run: later steps are
/// marked as not run.
pub fn apply(steps: &mut [Step], stubs: Stubs, files: &impl Files) {
    let mut stopped = false;
    for step in steps.iter_mut().filter(|step| step.change.mutates()) {
        // Until its move happens, a step's backup is only where it would go.
        let planned_backup = step.backup.take();
        if stopped {
            step.change = Change::NotRun;
            continue;
        }
        if inspect(&step.slot.path, stubs) != step.seen {
            step.change = Change::Conflict;
            step.note = Some("changed while this command was running".to_owned());
            continue;
        }
        if let Err(failure) = apply_one(step, planned_backup.as_deref(), stubs, files) {
            step.change = failure.change;
            step.note = Some(failure.note);
            stopped = failure.change == Change::Failed;
        }
    }
}

/// Why a step did not happen as planned.
struct Failure {
    change: Change,
    note: String,
}

impl Failure {
    fn io(action: &str, path: &Path, error: &io::Error) -> Self {
        Self {
            change: Change::Failed,
            note: format!("could not {action} {}: {error}", path.display()),
        }
    }
}

/// Carries out `step`. `planned_backup` is where a forced step's skill was
/// planned to go; `step.backup` is set only once it has been moved, to where
/// it really went.
fn apply_one(
    step: &mut Step,
    planned_backup: Option<&Path>,
    stubs: Stubs,
    files: &impl Files,
) -> Result<(), Failure> {
    let slot = step.slot.clone();
    let stub = stubs.current.as_bytes();
    match step.change {
        Change::Created => create(&slot, step.seen.directory, stub, files),
        Change::Updated => write(&slot.skill_file(), stub, files),
        Change::Replaced => {
            let backup = reserve_backup(planned_backup, files)?;
            // Built beside the backup, so the install is one rename.
            let staging = backup.join("new");
            let staged = files
                .create_dir(&staging)
                .map_err(|error| Failure::io("create", &staging, &error))
                .and_then(|()| write(&staging.join(SKILL_FILE), stub, files))
                .and_then(|()| move_aside(step, &backup, files));
            if let Err(failure) = staged {
                // Nothing was moved, so the backup directory holds at most
                // the staged copy. Leaving it would be clutter, not a loss.
                let _ = files.remove_file(&staging.join(SKILL_FILE));
                let _ = files.remove_dir(&staging);
                let _ = files.remove_dir(&backup);
                return Err(failure);
            }
            ensure_skills_directory(&slot, files)?;
            files.rename(&staging, &slot.path).map_err(|error| Failure {
                change: Change::Failed,
                note: format!(
                    "moved the old skill to {} but could not install the new one at {}: {error}",
                    backup.join(SKILL_NAME).display(),
                    slot.path.display()
                ),
            })
        }
        Change::MovedAside => {
            let backup = reserve_backup(planned_backup, files)?;
            move_aside(step, &backup, files).inspect_err(|_| {
                let _ = files.remove_dir(&backup);
            })
        }
        Change::Removed => {
            let file = slot.skill_file();
            files
                .remove_file(&file)
                .map_err(|error| Failure::io("remove", &file, &error))?;
            match files.remove_dir(&slot.path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == io::ErrorKind::DirectoryNotEmpty => {
                    step.note = Some("the directory holds other files and was kept".to_owned());
                    Ok(())
                }
                Err(error) => Err(Failure::io("remove", &slot.path, &error)),
            }
        }
        Change::Unchanged
        | Change::Conflict
        | Change::Absent
        | Change::Kept
        | Change::Failed
        | Change::NotRun => Ok(()),
    }
}

fn ensure_skills_directory(slot: &Slot, files: &impl Files) -> Result<(), Failure> {
    let skills = slot
        .path
        .parent()
        .expect("a slot is a directory inside a skills directory");
    files
        .create_dir_all(skills)
        .map_err(|error| Failure::io("create", skills, &error))
}

fn create(
    slot: &Slot,
    directory_exists: bool,
    stub: &[u8],
    files: &impl Files,
) -> Result<(), Failure> {
    ensure_skills_directory(slot, files)?;
    if !directory_exists {
        match files.create_dir(&slot.path) {
            Ok(()) => {}
            // Someone else installed a skill here since the check above.
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {
                return Err(Failure {
                    change: Change::Conflict,
                    note: "changed while this command was running".to_owned(),
                });
            }
            Err(error) => return Err(Failure::io("create", &slot.path, &error)),
        }
    }
    write(&slot.skill_file(), stub, files)
}

fn write(file: &Path, stub: &[u8], files: &impl Files) -> Result<(), Failure> {
    files
        .write(file, stub)
        .map_err(|error| Failure::io("write", file, &error))
}

/// Creates the backup directory `planned_backup` is in, or the next free
/// name after it. An existing backup is never reused.
fn reserve_backup(planned_backup: Option<&Path>, files: &impl Files) -> Result<PathBuf, Failure> {
    let planned = planned_backup
        .and_then(Path::parent)
        .expect("a forced step was planned with a backup directory")
        .to_path_buf();
    let backups = planned
        .parent()
        .expect("a backup directory is inside the backups directory");
    files
        .create_dir_all(backups)
        .map_err(|error| Failure::io("create", backups, &error))?;
    let stamp = planned
        .file_name()
        .expect("a backup directory has a name")
        .to_string_lossy()
        .into_owned();
    for attempt in 1..=100_u32 {
        let directory = if attempt == 1 {
            planned.clone()
        } else {
            backups.join(format!("{stamp}-{attempt}"))
        };
        match files.create_dir(&directory) {
            Ok(()) => return Ok(directory),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(Failure::io("create", &directory, &error)),
        }
    }
    Err(Failure {
        change: Change::Failed,
        note: format!(
            "could not pick an unused backup name in {}",
            backups.display()
        ),
    })
}

/// Moves the slot itself, a directory or a symlink, into `backup`, and
/// records in `step` where it now is.
fn move_aside(step: &mut Step, backup: &Path, files: &impl Files) -> Result<(), Failure> {
    let destination = backup.join(SKILL_NAME);
    files
        .rename(&step.slot.path, &destination)
        .map_err(|error| Failure {
            change: Change::Failed,
            note: format!(
                "could not move {} to {}: {error}",
                step.slot.path.display(),
                destination.display()
            ),
        })?;
    step.backup = Some(destination);
    Ok(())
}
