//! Network-free witness for the full C039 prompt sequence. No credentials load.
use std::io;

use linear_cli::error::AppError;
use linear_cli::platform::prompt::{PlainOption, PlainSelect, PromptOutcome, PromptSession};

fn option(label: &str, value: &str, script_token: &str) -> PlainOption {
    PlainOption {
        label: label.to_owned(),
        value: value.to_owned(),
        script_token: script_token.to_owned(),
    }
}

fn main() {
    let statuses = [
        option("Planned", "Planned", "Planned"),
        option("Active", "Active", "Active"),
        option("Completed", "Completed", "Completed"),
    ];
    let colors = [
        option("Skip (use default)", "__skip__", "skip"),
        option("Red (#EB5757)", "#EB5757", "#EB5757"),
        option("Orange (#F2994A)", "#F2994A", "#F2994A"),
        option("Yellow (#F2C94C)", "#F2C94C", "#F2C94C"),
        option("Green (#27AE60)", "#27AE60", "#27AE60"),
        option("Teal (#0D9488)", "#0D9488", "#0D9488"),
        option("Blue (#2F80ED)", "#2F80ED", "#2F80ED"),
        option("Indigo (#5E6AD2)", "#5E6AD2", "#5E6AD2"),
        option("Purple (#8B5CF6)", "#8B5CF6", "#8B5CF6"),
        option("Pink (#BB6BD9)", "#BB6BD9", "#BB6BD9"),
        option("Gray (#6B6F76)", "#6B6F76", "#6B6F76"),
        option("Custom color", "__custom__", "custom"),
    ];
    let mut stdout = io::stdout().lock();
    let mut session = match PromptSession::stdio(&mut stdout) {
        Ok(session) => session,
        Err(error) => exit_error(error.display_message()),
    };

    let name_result = session.text("Initiative name:", 1, |_| Ok(()));
    let name = answer(&mut session, name_result);
    let description_result = session.text("Description (optional):", 0, |_| Ok(()));
    let description = answer(&mut session, description_result);
    let status_result = session.select(&PlainSelect {
        message: "Status:",
        options: &statuses,
        default_index: 0,
        default_hint: Some("planned"),
    });
    let status = answer(&mut session, status_result);
    let owner_result = session.text("Owner (optional):", 0, |_| Ok(()));
    let owner = answer(&mut session, owner_result);
    let date_result = session.text("Target date (optional):", 0, |_| Ok(()));
    let date = answer(&mut session, date_result);
    let color_result = session.select(&PlainSelect {
        message: "Color (optional):",
        options: &colors,
        default_index: 0,
        default_hint: Some("__skip__"),
    });
    let color_choice = answer(&mut session, color_result);
    let color = if color_choice == "__custom__" {
        let custom_result = session.text("Enter hex color:", 0, |raw| {
            if raw.len() == 7
                && raw.starts_with('#')
                && raw.chars().skip(1).all(|part| part.is_ascii_hexdigit())
            {
                Ok(())
            } else {
                Err("Please enter a valid hex color (e.g., #FF5733)".to_owned())
            }
        });
        answer(&mut session, custom_result)
    } else {
        color_choice
    };
    if let Err(error) = session.close() {
        exit_error(error.display_message());
    }
    println!("SUBMITTED:{name}:{description}:{status}:{owner}:{date}:{color}");
}

fn answer<R: io::Read, W: io::Write>(
    session: &mut PromptSession<R, W>,
    result: Result<PromptOutcome<String>, AppError>,
) -> String {
    match result {
        Ok(PromptOutcome::Submitted(value)) => value,
        other => match session.finish_result(other) {
            Ok(PromptOutcome::Submitted(value)) => value,
            Ok(PromptOutcome::Interrupted) => std::process::exit(130),
            Ok(PromptOutcome::EndOfInput) => std::process::exit(3),
            Err(error) => exit_error(error.display_message()),
        },
    }
}

fn exit_error(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1)
}
