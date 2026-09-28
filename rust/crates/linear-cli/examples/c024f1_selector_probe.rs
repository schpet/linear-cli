//! Synthetic PTY witness for the reusable selector. It never loads credentials.
use std::io::{self, IsTerminal, Write};

use linear_cli::platform::selector::{
    PromptLabels, SelectOption, Selection, interactive_allowed, run,
};

struct FailWriter;

impl Write for FailWriter {
    fn write(&mut self, _buffer: &[u8]) -> io::Result<usize> {
        Err(io::Error::other("synthetic write failure"))
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let ci = std::env::var("CI").ok();
    if !interactive_allowed(stdin.is_terminal(), stdout.is_terminal(), ci.as_deref()) {
        std::process::exit(2);
    }
    let options = [
        SelectOption {
            label: "Alpha  ·  Planned  ·  ARC  ·  alpha".to_owned(),
            value: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".to_owned(),
        },
        SelectOption {
            label: "Zeta  ·  Active  ·  ARC  ·  zeta".to_owned(),
            value: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".to_owned(),
        },
    ];
    let labels = PromptLabels {
        message: "Select a project",
        search_label: "Search projects",
        max_rows: 8,
    };
    let outcome = if std::env::var_os("C024F1_FAIL_WRITE").is_some() {
        run(&options, &labels, ci.as_deref(), &mut FailWriter)
    } else {
        run(&options, &labels, ci.as_deref(), &mut stdout.lock())
    };
    match outcome {
        Ok(Selection::Selected(value)) => println!("SELECTED:{value}"),
        Ok(Selection::Interrupted) => std::process::exit(130),
        Ok(Selection::EndOfInput) => std::process::exit(3),
        Err(_) => std::process::exit(1),
    }
}
