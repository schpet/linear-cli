//! Application templates. The evaluator only sees explicit data and an in-memory
//! registry; it never loads files, personal engine config, or process state.
use std::collections::{BTreeMap, BTreeSet};

use clap::CommandFactory;
use minijinja::{AutoEscape, Environment, UndefinedBehavior};
use serde::Serialize;

use crate::config::ConfigTier;
use crate::error::{Error, Result};
use crate::platform::terminal_text;
use crate::refs::IssueReference;

mod display;
#[cfg(test)]
pub use display::DisplaySpan;
pub use display::DisplayText;

const DEFAULTS: &str = include_str!("defaults.toml");
const BUILTIN_SOURCE: &str = "compiled defaults";
const FORMAT_ISSUE_ID: &str = "format_issue_id";
// Empty hooks still expose the inheritance block used by shipped templates.
const EMPTY_SKILL_HOOK: &str = "{% block content %}{% endblock %}";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Slot {
    Guide,
    GuideTargeted,
    CommitDescription,
    PrTitle,
    PrBody,
}

impl Slot {
    pub const fn name(self) -> &'static str {
        match self {
            Self::Guide => "guide",
            Self::GuideTargeted => "guide_targeted",
            Self::CommitDescription => "commit_description",
            Self::PrTitle => "pr_title",
            Self::PrBody => "pr_body",
        }
    }
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct SkillContext {
    pub version: String,
    pub commands: String,
    pub command_guidance: String,
    pub command_help: String,
    pub markdown_rules: String,
    pub exit_status: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct IssueData {
    pub identifier: String,
    pub title: String,
    pub url: String,
    pub description: String,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct CommitContext {
    pub issue: IssueData,
    pub linkage: String,
    pub references: bool,
}

#[derive(Clone, Debug, Default, Serialize)]
pub struct PullRequestContext {
    pub issues: Vec<IssueData>,
    pub description: String,
    pub linkage: String,
    pub references: bool,
    pub issue_clause: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateOrigin {
    pub source: String,
    pub key: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TemplateEntry {
    pub body: String,
    pub origin: TemplateOrigin,
}

#[derive(Clone)]
pub struct TemplateConfig {
    entries: BTreeMap<String, TemplateEntry>,
    builtins: BTreeMap<String, TemplateEntry>,
    template_names: BTreeSet<String>,
    environment: Environment<'static>,
    display_environment: Environment<'static>,
}

impl std::fmt::Debug for TemplateConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TemplateConfig(<redacted>)")
    }
}

/// The canonical command hook name. CLI aliases resolve to a canonical path
/// before calling this function; hyphens within command names are preserved.
pub fn skill_hook(path: &[&str]) -> String {
    format!("skill_{}", path.join("_"))
}

fn command_hooks() -> BTreeSet<String> {
    fn walk(command: &clap::Command, path: &mut Vec<String>, names: &mut BTreeSet<String>) {
        for child in command
            .get_subcommands()
            .filter(|child| !child.is_hide_set())
        {
            path.push(child.get_name().to_owned());
            let borrowed: Vec<_> = path.iter().map(String::as_str).collect();
            names.insert(skill_hook(&borrowed));
            walk(child, path, names);
            path.pop();
        }
    }
    let mut names = BTreeSet::new();
    walk(&crate::cli::Cli::command(), &mut Vec::new(), &mut names);
    names
}

fn environment() -> Environment<'static> {
    let mut env = Environment::empty();
    env.set_undefined_behavior(UndefinedBehavior::Strict);
    env.set_recursion_limit(50);
    env.set_fuel(Some(1_000_000));
    env.set_syntax(
        minijinja::syntax::SyntaxConfig::builder()
            .keep_trailing_newline(true)
            .build()
            .expect("fixed template syntax settings"),
    );
    env.add_filter("trim", minijinja::filters::trim);
    env.add_filter("upper", minijinja::filters::upper);
    env.add_filter("lower", minijinja::filters::lower);
    env.add_filter("join", minijinja::filters::join);
    env.add_filter("length", minijinja::filters::length);
    env.add_filter("indent", display::indent);
    env.add_filter("default", minijinja::filters::default);
    env.add_test("defined", minijinja::tests::is_defined);
    env.add_function("hyperlink", display::hyperlink);
    env.add_function(
        "fail",
        |message: String| -> std::result::Result<String, minijinja::Error> {
            Err(minijinja::Error::new(
                minijinja::ErrorKind::InvalidOperation,
                message,
            ))
        },
    );
    env
}

fn config_error(source: &str, key: &str, reason: impl std::fmt::Display) -> Error {
    let message = format!("invalid template configuration in {source}, {key}: {reason}");
    Error::new(terminal_text::single_line(&message).into_owned())
        .with_hint("Fix the template configuration, or run `linear skill show --builtin` for the shipped guide.")
}

fn engine_error(
    error: minijinja::Error,
    fallback: &str,
    entries: &BTreeMap<String, TemplateEntry>,
) -> Error {
    let mut error = &error;
    while let Some(cause) = std::error::Error::source(error)
        .and_then(|source| source.downcast_ref::<minijinja::Error>())
    {
        error = cause;
    }
    let name = error.name().unwrap_or(fallback);
    let normal = name.strip_prefix("builtin/").unwrap_or(name);
    let origin = entries
        .get(name)
        .or_else(|| entries.get(normal))
        .map(|entry| &entry.origin);
    let source = if name.starts_with("builtin/") {
        BUILTIN_SOURCE
    } else {
        origin.map_or("template registry", |origin| origin.source.as_str())
    };
    let key = origin.map_or(name, |origin| origin.key.as_str());
    let line = error
        .line()
        .map_or(String::new(), |line| format!(" at line {line}"));
    // Display/Debug of the complete engine error may contain source snippets.
    let reason = if error.kind() == minijinja::ErrorKind::SyntaxError {
        "invalid template syntax".to_owned()
    } else {
        error
            .detail()
            .map_or_else(|| error.kind().to_string(), str::to_owned)
    };
    config_error(source, key, format!("{reason}{line}"))
}

impl TemplateConfig {
    pub fn builtin() -> Self {
        Self::builtin_from_source(DEFAULTS)
    }

    fn builtin_from_source(source: &str) -> Self {
        // Checkouts may use CRLF. Keep shipped literals portable without
        // changing explicit user templates or interpolated data.
        let table = source
            .replace("\r\n", "\n")
            .parse::<toml::Table>()
            .expect("shipped templates are valid TOML");
        let template_names = table
            .get("templates")
            .and_then(toml::Value::as_table)
            .expect("shipped templates have a templates table")
            .keys()
            .cloned()
            .chain(command_hooks())
            .collect();
        let mut config = Self {
            entries: BTreeMap::new(),
            builtins: BTreeMap::new(),
            template_names,
            environment: environment(),
            display_environment: Environment::empty(),
        };
        config
            .apply_table(BUILTIN_SOURCE, &table)
            .expect("shipped templates compile");
        for name in config.template_names.clone() {
            config
                .entries
                .entry(name.clone())
                .or_insert_with(|| TemplateEntry {
                    body: EMPTY_SKILL_HOOK.to_owned(),
                    origin: TemplateOrigin {
                        source: BUILTIN_SOURCE.to_owned(),
                        key: format!("templates.{name}"),
                    },
                });
        }
        config.builtins = config.entries.clone();
        config
            .rebuild()
            .expect("shipped template registry compiles");
        config
    }

    pub fn from_tiers<'a>(tiers: impl IntoIterator<Item = &'a ConfigTier>) -> Result<Self> {
        let mut config = Self::builtin();
        for tier in tiers {
            config.apply_tier(tier)?;
        }
        Ok(config)
    }

    pub fn entries(&self) -> &BTreeMap<String, TemplateEntry> {
        &self.entries
    }

    pub fn validate_alias(&self, name: &str) -> Result<()> {
        if self.entries.contains_key(name) && !self.template_names.contains(name) {
            Ok(())
        } else {
            Err(Error::invalid(format!(
                "No template alias named {:?}",
                terminal_text::single_line(name)
            ))
            .with_hint("Define the name in [template-aliases], or choose a name shown by `linear config list`."))
        }
    }

    /// Whether an output still uses the shipped definition. A layer that
    /// repeats the same body retains the same behavior regardless of its source.
    pub fn is_builtin(&self, slot: Slot) -> bool {
        let effective = self
            .entries
            .get(slot.name())
            .expect("all output slots are registered");
        let builtin = self
            .builtins
            .get(slot.name())
            .expect("all output slots have a shipped definition");
        effective.body == builtin.body
    }

    pub fn apply_tier(&mut self, tier: &ConfigTier) -> Result<()> {
        self.apply_table(&tier.path.display().to_string(), &tier.table)
    }

    fn apply_table(&mut self, source: &str, table: &toml::Table) -> Result<()> {
        // Build a candidate so a failed override cannot leave a half-applied registry.
        let mut candidate = self.clone();
        for section in ["templates", "template-aliases"] {
            let Some(value) = table.get(section) else {
                continue;
            };
            let values = value
                .as_table()
                .ok_or_else(|| config_error(source, section, "expected a table"))?;
            for (name, value) in values {
                let key = format!("{section}.{name}");
                if !valid_name(name) || name.starts_with("builtin/") {
                    return Err(config_error(
                        source,
                        &key,
                        "invalid or reserved template name",
                    ));
                }
                if section == "templates" && !candidate.template_names.contains(name) {
                    return Err(config_error(
                        source,
                        &key,
                        "unknown template name; use template-aliases for a named partial",
                    ));
                }
                if section == "template-aliases"
                    && (candidate.template_names.contains(name)
                        || name.starts_with("skill_")
                        || name.starts_with("guide_"))
                {
                    return Err(config_error(
                        source,
                        &key,
                        "partial name collides with a reserved template namespace",
                    ));
                }
                let body = value
                    .as_str()
                    .ok_or_else(|| config_error(source, &key, "expected a template string"))?;
                let entry = TemplateEntry {
                    body: body.to_owned(),
                    origin: TemplateOrigin {
                        source: source.to_owned(),
                        key,
                    },
                };
                candidate.entries.insert(name.clone(), entry);
            }
        }
        candidate.rebuild()?;
        *self = candidate;
        Ok(())
    }

    fn rebuild(&mut self) -> Result<()> {
        let mut env = environment();
        let mut display_env = env.clone();
        // The display formatter distinguishes macro captures from raw values.
        // Document templates retain MiniJinja's normal explicit autoescape.
        display_env.set_auto_escape_callback(|_| AutoEscape::Custom("linear".into()));
        display_env.set_formatter(display::format_value);
        for registry in [&mut env, &mut display_env] {
            for (name, entry) in &self.builtins {
                registry
                    .add_template_owned(format!("builtin/{name}"), entry.body.clone())
                    .map_err(|error| engine_error(error, name, &self.builtins))?;
            }
        }
        for (name, entry) in &self.entries {
            for registry in [&mut env, &mut display_env] {
                registry
                    .add_template_owned(name.clone(), entry.body.clone())
                    .map_err(|error| engine_error(error, name, &self.entries))?;
            }
            // This stable metadata check catches direct misspellings even in
            // unselected branches. Includes and indexed/loop-local attributes
            // remain the strict evaluator's responsibility.
            if let Some(fields) = context_fields(name) {
                let template = env
                    .get_template(name)
                    .map_err(|error| engine_error(error, name, &self.entries))?;
                for variable in template.undeclared_variables(true) {
                    let root = variable.split('.').next().expect("a variable has a root");
                    if !matches!(root, "fail" | "hyperlink") && !fields.contains(&root) {
                        return Err(config_error(
                            &entry.origin.source,
                            &entry.origin.key,
                            format!("unknown context field {variable}"),
                        ));
                    }
                    if !known_issue_field(name, root, &variable) {
                        return Err(config_error(
                            &entry.origin.source,
                            &entry.origin.key,
                            format!("unknown context field {variable}"),
                        ));
                    }
                }
            }
        }
        self.environment = env;
        self.display_environment = display_env;
        Ok(())
    }

    pub fn render(&self, slot: Slot, context: &impl Serialize) -> Result<String> {
        self.render_named(slot.name(), context)
    }

    pub fn render_named(&self, name: &str, context: &impl Serialize) -> Result<String> {
        self.environment
            .get_template(name)
            .and_then(|template| template.render(minijinja::value::Serde(context)))
            .map_err(|error| engine_error(error, name, &self.entries))
    }

    pub fn render_display(
        &self,
        name: &str,
        context: &impl Serialize,
        styled: bool,
    ) -> Result<DisplayText> {
        self.display_environment
            .get_template(name)
            .and_then(|template| display::render(&template, context, styled))
            .map_err(|error| engine_error(error, name, &self.entries))
    }

    pub fn format_issue_id(&self, issue: &IssueReference<'_>, styled: bool) -> Result<DisplayText> {
        #[derive(Serialize)]
        struct Context<'a, 'reference> {
            issue: &'a IssueReference<'reference>,
        }

        let mut output = self.render_display(FORMAT_ISSUE_ID, &Context { issue }, styled)?;
        for span in &mut output.spans {
            if let std::borrow::Cow::Owned(text) = terminal_text::single_line(&span.text) {
                span.text = text;
            }
        }
        Ok(output)
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'/' | b'.'))
}

fn context_fields(name: &str) -> Option<&'static [&'static str]> {
    match name {
        FORMAT_ISSUE_ID => Some(&["issue"]),
        "commit_description" => Some(&["issue", "linkage", "references"]),
        "pr_title" | "pr_body" | "pr_issue_links" => Some(&[
            "issues",
            "description",
            "linkage",
            "references",
            "issue_clause",
        ]),
        name if name == "guide" || name.starts_with("guide_") || name.starts_with("skill_") => {
            Some(&[
                "version",
                "commands",
                "command_guidance",
                "command_help",
                "markdown_rules",
                "exit_status",
            ])
        }
        _ => None,
    }
}

fn known_issue_field(template: &str, root: &str, variable: &str) -> bool {
    if root != "issue" || !variable.contains('.') {
        return true;
    }
    match template {
        FORMAT_ISSUE_ID => matches!(
            variable,
            "issue.identifier" | "issue.url" | "issue.workspace_url_key"
        ),
        "commit_description" => matches!(
            variable,
            "issue.identifier" | "issue.title" | "issue.url" | "issue.description"
        ),
        _ => true,
    }
}

#[cfg(test)]
mod tests;
