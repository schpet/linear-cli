//! Running an interactive child (editor, pager) in the foreground.

/// While a shield lives, Ctrl-C reaches only the foreground child; this
/// process keeps running so it can collect the child's result and clean up.
pub struct InterruptShield {
    #[cfg(unix)]
    outside: &'static std::sync::atomic::AtomicBool,
}

impl InterruptShield {
    #[cfg(unix)]
    pub fn raise() -> crate::error::Result<Self> {
        use std::sync::Arc;
        use std::sync::OnceLock;
        use std::sync::atomic::{AtomicBool, Ordering};

        static OUTSIDE: OnceLock<std::result::Result<Arc<AtomicBool>, String>> = OnceLock::new();
        let outside = OUTSIDE
            .get_or_init(|| {
                let outside = Arc::new(AtomicBool::new(true));
                // While `outside` is true SIGINT keeps its default action.
                signal_hook::flag::register_conditional_default(
                    signal_hook::consts::SIGINT,
                    Arc::clone(&outside),
                )
                .map(|_| outside)
                .map_err(|error| error.to_string())
            })
            .as_ref()
            .map_err(|message| {
                crate::error::Error::new(format!("Failed to handle Ctrl-C: {message}"))
            })?;
        let outside: &'static AtomicBool = outside;
        outside.store(false, Ordering::SeqCst);
        Ok(Self { outside })
    }

    #[cfg(not(unix))]
    pub fn raise() -> crate::error::Result<Self> {
        Ok(Self {})
    }
}

#[cfg(unix)]
impl Drop for InterruptShield {
    fn drop(&mut self) {
        self.outside
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }
}
