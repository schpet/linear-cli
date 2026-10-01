//! Pull-request-only child policy: inherit all three descriptors, no status forwarding.
//! Git/jj capture/inference policies remain in vcs_script and are unchanged.
use crate::{
    config::ChildEnvOverlay,
    error::{AppError, AppErrorKind},
};
use std::{
    io,
    path::Path,
    process::{Child, Command, Stdio},
};
pub trait GhRunner {
    fn create(
        &mut self,
        args: &[String],
        cwd: &Path,
        env: &ChildEnvOverlay,
    ) -> Result<bool, AppError>;
}
pub struct NativeGhRunner;
struct OwnedGh {
    child: Child,
    reaped: bool,
}
impl Drop for OwnedGh {
    fn drop(&mut self) {
        if !self.reaped {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
fn failure(stage: &str, error: io::Error) -> AppError {
    AppError::new(
        AppErrorKind::IoProcess,
        format!("Failed to {stage}: {error}"),
    )
    .with_source(error)
}
impl GhRunner for NativeGhRunner {
    fn create(
        &mut self,
        args: &[String],
        cwd: &Path,
        env: &ChildEnvOverlay,
    ) -> Result<bool, AppError> {
        let mut command = Command::new("gh");
        command
            .args(args)
            .current_dir(cwd)
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit());
        // Inherit ambient environment, adding only explicitly applied dotenv values.
        for (name, value) in env.iter() {
            command.env(name, value);
        }
        let child = command.spawn().map_err(|error| {
            if error.kind() == io::ErrorKind::NotFound {
                AppError::new(
                    AppErrorKind::IoProcess,
                    "Failed to spawn 'gh': entity not found",
                )
                .with_source(error)
            } else {
                failure("spawn gh", error)
            }
        })?;
        let mut owner = OwnedGh {
            child,
            reaped: false,
        };
        match owner.child.wait() {
            Ok(status) => {
                owner.reaped = true;
                Ok(status.success())
            }
            Err(error) => {
                let error = failure("wait for gh", error);
                let kill = owner.child.kill();
                let wait = owner.child.wait();
                if wait.is_ok() {
                    owner.reaped = true;
                }
                Err(error.with_debug_detail(format!("gh cleanup kill={kill:?}; wait={wait:?}; a remote pull request may already exist")))
            }
        }
    }
}
