//! Template preferences only: never expose scalar settings or credentials.
use crate::cli::config::ConfigCommand;
use crate::error::{Error, Result};
use crate::platform::output::Stdout;
use crate::templates::TemplateConfig;

pub fn run(command: &ConfigCommand, templates: &TemplateConfig) -> Result<()> {
    let rendered = match command {
        ConfigCommand::Check => unreachable!("config check uses full startup configuration"),
        ConfigCommand::Get { key, .. } => {
            let (namespace, name) = key.split_once('.').ok_or_else(|| unsupported(key))?;
            if !matches!(namespace, "templates" | "template-aliases") {
                return Err(unsupported(key));
            }
            templates
                .entries()
                .get(name)
                .filter(|entry| entry.origin.key == *key)
                .map(|entry| entry.body.clone())
                .ok_or_else(|| {
                    Error::invalid(format!("Unknown template preference: {key}")).with_hint(
                        "Run `linear config list` to see available template preferences.",
                    )
                })?
        }
        ConfigCommand::List { show_origin, .. } => {
            let mut result = String::new();
            for namespace in ["templates", "template-aliases"] {
                result.push_str(&format!("[{namespace}]\n"));
                for (name, entry) in templates.entries() {
                    if entry.origin.key != format!("{namespace}.{name}") {
                        continue;
                    }
                    if *show_origin {
                        for line in entry.origin.source.lines() {
                            result.push_str(&format!("# Source: {line}\n"));
                        }
                    }
                    let mut table = toml::Table::new();
                    table.insert(name.clone(), toml::Value::String(entry.body.clone()));
                    result.push_str(&toml::to_string(&table).map_err(|error| {
                        Error::new(format!("Failed to format template preferences: {error}"))
                    })?);
                }
                result.push('\n');
            }
            result
        }
    };
    let stdout = Stdout::new();
    stdout.write(rendered.as_bytes())?;
    stdout.flush()
}

fn unsupported(key: &str) -> Error {
    Error::invalid(format!("Unsupported config key: {key}"))
        .with_hint("Inspection supports only templates.NAME and template-aliases.NAME; run `linear config list`.")
}
