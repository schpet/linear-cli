//! A progress spinner on stderr, drawn by a background thread while a guard lives.
use std::io::{self, Write};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const TICK: Duration = Duration::from_millis(80);
/// Commands that finish quickly never show a frame.
const DELAY: Duration = Duration::from_millis(150);
const CLEAR_LINE: &[u8] = b"\r\x1b[2K";

/// Stops the spinner and clears its line when dropped.
pub struct Spinner {
    running: Option<(Sender<()>, JoinHandle<()>)>,
}

impl Spinner {
    /// A spinner that draws nothing.
    pub fn hidden() -> Self {
        Self { running: None }
    }

    pub fn start() -> Self {
        let (stop, stopped) = mpsc::channel::<()>();
        let handle = thread::spawn(move || {
            if !matches!(stopped.recv_timeout(DELAY), Err(RecvTimeoutError::Timeout)) {
                return;
            }
            let mut drawn = false;
            for frame in FRAMES.iter().cycle() {
                let mut stderr = io::stderr().lock();
                // A spinner frame that cannot be drawn is not worth reporting.
                let _ignored = write!(stderr, "\r{frame} ").and_then(|()| stderr.flush());
                drawn = true;
                drop(stderr);
                if !matches!(stopped.recv_timeout(TICK), Err(RecvTimeoutError::Timeout)) {
                    break;
                }
            }
            if drawn {
                let mut stderr = io::stderr().lock();
                let _ignored = stderr.write_all(CLEAR_LINE).and_then(|()| stderr.flush());
            }
        });
        Self {
            running: Some((stop, handle)),
        }
    }
}

impl Drop for Spinner {
    fn drop(&mut self) {
        if let Some((stop, handle)) = self.running.take() {
            // The thread may already have exited; either way it stops drawing.
            let _ignored = stop.send(());
            let _ignored = handle.join();
        }
    }
}
