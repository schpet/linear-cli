//! Console spinner bytes shared by read commands.

use std::time::Duration;

const FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub const CLEAR: &[u8] = b"\r\x1b[K";
pub const TICK_INTERVAL: Duration = Duration::from_millis(75);

/// The spinner belongs to non-JSON terminal stdout only when `NO_COLOR` is
/// absent. An empty `NO_COLOR` still suppresses it, matching the Deno CLI.
pub fn enabled(json: bool, stdout_tty: bool, no_color_absent: bool) -> bool {
    !json && stdout_tty && no_color_absent
}

pub fn frame(index: usize) -> String {
    let symbol = FRAMES.get(index % FRAMES.len()).copied().unwrap_or("⠋");
    format!("\r\x1b[K{symbol}\x1b[0m ")
}
