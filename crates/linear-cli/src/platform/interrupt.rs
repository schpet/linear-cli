//! Ctrl-C while an interactive child (editor, pager) runs or the terminal
//! needs tidying first.
//!
//! Normally Ctrl-C ends the process at once. While an [`InterruptShield`] or
//! a [`Deferral`] lives it only records that it happened.

/// While a shield lives, Ctrl-C reaches only the foreground child; this
/// process keeps running so it can collect the child's result and clean up.
pub struct InterruptShield {
    #[cfg(unix)]
    hooks: &'static unix::Hooks,
}

impl InterruptShield {
    #[cfg(unix)]
    pub fn raise() -> crate::error::Result<Self> {
        let hooks = unix::hooks()?;
        hooks.hold();
        hooks.forget();
        Ok(Self { hooks })
    }

    #[cfg(not(unix))]
    pub fn raise() -> crate::error::Result<Self> {
        Ok(Self {})
    }
}

#[cfg(unix)]
impl Drop for InterruptShield {
    fn drop(&mut self) {
        // The child had the Ctrl-C; it does not end this process afterwards.
        self.hooks.forget();
        self.hooks.release();
    }
}

/// Holds Ctrl-C back so the terminal can be tidied before the process ends:
/// poll [`Deferral::interrupted`] and call [`Deferral::interrupt_now`]. A
/// Ctrl-C nobody acted on ends the process when the deferral is dropped.
pub struct Deferral {
    #[cfg(unix)]
    hooks: &'static unix::Hooks,
}

impl Deferral {
    /// `None` when Ctrl-C cannot be held back on this platform.
    #[cfg(unix)]
    pub fn start() -> Option<Self> {
        // Without the hooks Ctrl-C keeps its default action, which only
        // costs the tidying.
        let hooks = unix::hooks().ok()?;
        hooks.hold();
        Some(Self { hooks })
    }

    #[cfg(not(unix))]
    pub fn start() -> Option<Self> {
        None
    }

    #[cfg(unix)]
    pub fn interrupted(&self) -> bool {
        self.hooks.received()
    }

    #[cfg(not(unix))]
    pub fn interrupted(&self) -> bool {
        false
    }

    /// Ends the process as Ctrl-C would have.
    #[cfg(unix)]
    pub fn interrupt_now(&self) -> ! {
        self.hooks.interrupt()
    }

    #[cfg(not(unix))]
    pub fn interrupt_now(&self) -> ! {
        std::process::exit(130)
    }
}

impl Drop for Deferral {
    fn drop(&mut self) {
        #[cfg(unix)]
        {
            if self.hooks.received() {
                self.hooks.interrupt();
            }
            self.hooks.release();
        }
    }
}

#[cfg(unix)]
mod unix {
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::{Arc, Mutex, OnceLock};

    use signal_hook::consts::SIGINT;

    use crate::error::{Error, Result};

    pub struct Hooks {
        /// While true, SIGINT keeps its default action.
        default: Arc<AtomicBool>,
        /// Set by every SIGINT.
        received: Arc<AtomicBool>,
        /// How many shields and deferrals are alive.
        holds: Mutex<usize>,
    }

    pub fn hooks() -> Result<&'static Hooks> {
        static HOOKS: OnceLock<std::result::Result<Hooks, String>> = OnceLock::new();
        HOOKS
            .get_or_init(|| {
                let received = Arc::new(AtomicBool::new(false));
                let default = Arc::new(AtomicBool::new(true));
                signal_hook::flag::register(SIGINT, Arc::clone(&received))
                    .and_then(|_| {
                        signal_hook::flag::register_conditional_default(
                            SIGINT,
                            Arc::clone(&default),
                        )
                    })
                    .map(|_| Hooks {
                        default,
                        received,
                        holds: Mutex::new(0),
                    })
                    .map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(|message| Error::new(format!("Failed to handle Ctrl-C: {message}")))
    }

    impl Hooks {
        pub fn hold(&self) {
            let mut holds = self
                .holds
                .lock()
                .expect("no thread panics holding the lock");
            *holds += 1;
            self.default.store(false, Ordering::SeqCst);
        }

        pub fn release(&self) {
            let mut holds = self
                .holds
                .lock()
                .expect("no thread panics holding the lock");
            *holds = holds.checked_sub(1).expect("every release follows a hold");
            if *holds == 0 {
                self.default.store(true, Ordering::SeqCst);
            }
        }

        pub fn received(&self) -> bool {
            self.received.load(Ordering::SeqCst)
        }

        pub fn forget(&self) {
            self.received.store(false, Ordering::SeqCst);
        }

        pub fn interrupt(&self) -> ! {
            self.default.store(true, Ordering::SeqCst);
            // Dying by the signal tells the shell the command was interrupted;
            // exit status 130 says the same when that is impossible.
            let _ignored = signal_hook::low_level::emulate_default_handler(SIGINT);
            std::process::exit(130)
        }
    }
}
