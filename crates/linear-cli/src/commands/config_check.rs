//! Offline checks use the same startup validation as normal commands.
use crate::config::{StartupConfig, render_retired_setting};
use crate::error::Result;
use crate::platform::output::{self, Stdout};

pub fn run(config: &StartupConfig, color: bool) -> Result<()> {
    for setting in &config.retired_settings {
        output::eprint(render_retired_setting(setting, color).as_bytes())?;
    }
    let stdout = Stdout::new();
    stdout.write("✓ Configuration is valid.\n".as_bytes())?;
    stdout.flush()
}
