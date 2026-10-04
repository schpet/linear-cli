//! A progress spinner on stderr, drawn by a background thread while a guard lives.
use std::io::{self, Write};
use std::sync::Mutex;
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use crate::platform::interrupt::Deferral;

const FRAMES: [char; 10] = ['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const TICK: Duration = Duration::from_millis(80);
/// Commands that finish quickly never show a frame.
const DELAY: Duration = Duration::from_millis(150);
/// How often the thread checks for Ctrl-C between frames.
const POLL: Duration = Duration::from_millis(20);
const CLEAR_LINE: &[u8] = b"\r\x1b[2K";

/// The spinner line on stderr, shared so a prompt can hide it.
struct Line {
    /// Live [`Hold`]s; no frame is drawn while there are any.
    holds: usize,
    /// Whether a frame is on screen.
    drawn: bool,
}

static LINE: Mutex<Line> = Mutex::new(Line {
    holds: 0,
    drawn: false,
});

fn line() -> std::sync::MutexGuard<'static, Line> {
    LINE.lock()
        .expect("no thread panics holding the spinner line")
}

/// Clears any spinner frame and keeps spinners from drawing while it lives,
/// so a prompt can use the terminal while a command's spinner runs.
pub struct Hold(());

pub fn hold() -> Hold {
    let mut line = line();
    if line.drawn {
        clear();
        line.drawn = false;
    }
    line.holds += 1;
    Hold(())
}

impl Drop for Hold {
    fn drop(&mut self) {
        let mut line = line();
        line.holds = line
            .holds
            .checked_sub(1)
            .expect("every hold was counted when taken");
    }
}

/// Stops the spinner and clears its line when dropped.
pub struct Spinner {
    running: Option<(Sender<()>, JoinHandle<()>)>,
}

impl Spinner {
    /// A spinner that draws nothing.
    pub fn hidden() -> Self {
        Self { running: None }
    }

    /// Draws frames followed by `message` until dropped. Ctrl-C clears the
    /// line before it ends the process.
    pub fn start(message: &str) -> Self {
        let message = message.to_owned();
        let (stop, stopped) = mpsc::channel::<()>();
        let deferral = Deferral::start();
        let handle = thread::spawn(move || {
            let mut frames = FRAMES.iter().cycle();
            let mut next_frame = Instant::now() + DELAY;
            while matches!(stopped.recv_timeout(POLL), Err(RecvTimeoutError::Timeout)) {
                if let Some(deferral) = deferral.as_ref().filter(|d| d.interrupted()) {
                    erase();
                    deferral.interrupt_now();
                }
                if Instant::now() < next_frame {
                    continue;
                }
                next_frame += TICK;
                let mut line = line();
                if line.holds > 0 {
                    continue;
                }
                let frame = frames.next().expect("the frames cycle forever");
                let mut stderr = io::stderr().lock();
                // A spinner frame that cannot be drawn is not worth reporting.
                let _ignored = write!(stderr, "\r{frame} {message}").and_then(|()| stderr.flush());
                line.drawn = true;
            }
            erase();
            // Dropping the deferral here, before the guard's join returns,
            // ends the process if Ctrl-C came after the last check.
            drop(deferral);
        });
        Self {
            running: Some((stop, handle)),
        }
    }
}

/// Clears the frame on screen, if any.
fn erase() {
    let mut line = line();
    if line.drawn {
        clear();
        line.drawn = false;
    }
}

fn clear() {
    let mut stderr = io::stderr().lock();
    let _ignored = stderr.write_all(CLEAR_LINE).and_then(|()| stderr.flush());
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
