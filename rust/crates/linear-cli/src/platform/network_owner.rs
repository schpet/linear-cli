//! two fixed eager phases, each scoped worker/current-thread runtime.
//! No general executor framework/inbox, detached thread or new Tokio features.
use crate::error::{AppError, AppErrorKind};
use futures_util::future::{AbortHandle, Abortable, join};
use std::{
    future::Future,
    sync::mpsc,
    thread::{self, Scope, ScopedJoinHandle},
};
pub struct Pending<T> {
    receiver: mpsc::Receiver<Result<T, AppError>>,
}
impl<T> Pending<T> {
    pub fn take(self) -> Result<T, AppError> {
        self.receiver.recv().map_err(|error| {
            AppError::new(
                AppErrorKind::Invariant,
                "issue-create phase result channel closed unexpectedly",
            )
            .with_source(error)
        })?
    }
}
pub struct Phase<'scope> {
    worker: Option<ScopedJoinHandle<'scope, ()>>,
    aborts: Vec<AbortHandle>,
}
impl Phase<'_> {
    pub fn close(mut self) -> Result<(), AppError> {
        self.finish()
    }
    fn finish(&mut self) -> Result<(), AppError> {
        for abort in &self.aborts {
            abort.abort()
        }
        if let Some(worker) = self.worker.take() {
            worker.join().map_err(|_| {
                AppError::new(
                    AppErrorKind::Invariant,
                    "issue-create phase worker panicked",
                )
            })?
        }
        Ok(())
    }
}
impl Drop for Phase<'_> {
    fn drop(&mut self) {
        if let Err(error) = self.finish() {
            assert!(thread::panicking(), "{error}");
        }
    }
}
fn ready_runtime() -> Result<tokio::runtime::Runtime, AppError> {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|error| {
            AppError::new(
                AppErrorKind::IoProcess,
                "Could not create issue-create preload runtime",
            )
            .with_source(error)
        })
}
fn task<'a, T: Send + 'a>(
    future: impl Future<Output = Result<T, AppError>> + Send + 'a,
) -> (
    impl Future<Output = ()> + Send + 'a,
    Pending<T>,
    AbortHandle,
) {
    let (sender, receiver) = mpsc::channel();
    let (abort, registration) = AbortHandle::new_pair();
    let future = async move {
        // Each result is sent as soon as its own request completes, even while
        // join keeps driving the other pending request. Main waits outside this
        // runtime at the exact source stage, never mpsc.recv inside polling.
        if let Ok(result) = Abortable::new(future, registration).await {
            let _abandoned = sender.send(result);
        }
    };
    (future, Pending { receiver }, abort)
}
pub fn pair<'scope, 'env, T: Send + 'scope, U: Send + 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    first: impl Future<Output = Result<T, AppError>> + Send + 'scope,
    second: impl Future<Output = Result<U, AppError>> + Send + 'scope,
) -> Result<(Phase<'scope>, Pending<T>, Pending<U>), AppError> {
    let runtime = ready_runtime()?;
    let (first, a, abort_a) = task(first);
    let (second, b, abort_b) = task(second);
    let worker = thread::Builder::new()
        .name("linear-create-team-auto".to_owned())
        .spawn_scoped(scope, move || {
            runtime.block_on(join(first, second));
        })
        .map_err(|error| {
            AppError::new(
                AppErrorKind::IoProcess,
                "Could not start issue-create preload worker",
            )
            .with_source(error)
        })?;
    Ok((
        Phase {
            worker: Some(worker),
            aborts: vec![abort_a, abort_b],
        },
        a,
        b,
    ))
}
pub type Triple<'scope, T, U, V> = (Phase<'scope>, Pending<T>, Pending<U>, Pending<V>);
pub fn triple<'scope, 'env, T: Send + 'scope, U: Send + 'scope, V: Send + 'scope>(
    scope: &'scope Scope<'scope, 'env>,
    first: impl Future<Output = Result<T, AppError>> + Send + 'scope,
    second: impl Future<Output = Result<U, AppError>> + Send + 'scope,
    third: impl Future<Output = Result<V, AppError>> + Send + 'scope,
) -> Result<Triple<'scope, T, U, V>, AppError> {
    let runtime = ready_runtime()?;
    let (first, a, abort_a) = task(first);
    let (second, b, abort_b) = task(second);
    let (third, c, abort_c) = task(third);
    let worker = thread::Builder::new()
        .name("linear-create-team-preloads".to_owned())
        .spawn_scoped(scope, move || {
            runtime.block_on(join(join(first, second), third));
        })
        .map_err(|error| {
            AppError::new(
                AppErrorKind::IoProcess,
                "Could not start issue-create preload worker",
            )
            .with_source(error)
        })?;
    Ok((
        Phase {
            worker: Some(worker),
            aborts: vec![abort_a, abort_b, abort_c],
        },
        a,
        b,
        c,
    ))
}
