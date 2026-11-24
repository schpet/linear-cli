//! Application templates. The evaluator only sees explicit data and an in-memory
//! registry; it never loads files, personal engine config, or process state.
use std::collections::{BTreeMap, BTreeSet};

use clap::CommandFactory;
use minijinja::machinery::{Instruction, get_compiled_template};
use minijinja::{AutoEscape, Environment, Template, UndefinedBehavior};
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
pub(crate) const FORMAT_ISSUE_ID: &str = "format_issue_id";
const BUILTIN_FORMAT_ISSUE_ID: &str = "builtin/format_issue_id";
pub(crate) const EXPERIMENTAL_TEMPLATES_HINT: &str = "Pass `--experimental-templates` or set `templates = true` under `[experimental]` in your global or project config.";
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
    experimental: bool,
    dormant_aliases: BTreeSet<String>,
    slot_fallbacks: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum Layer {
    Builtin,
    Override,
    DisabledPreferences,
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

fn environment(experimental: bool) -> Environment<'static> {
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
    if experimental {
        env.add_function("hyperlink", display::hyperlink);
    }
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

fn experimental_error(source: &str, key: &str, feature: &str) -> Error {
    config_error(
        source,
        key,
        format!("{feature} requires experimental template support"),
    )
    .with_hint(EXPERIMENTAL_TEMPLATES_HINT)
}

fn uses_instruction(
    template: &Template<'_, '_>,
    predicate: impl Fn(&Instruction<'_>) -> bool,
) -> bool {
    let compiled = get_compiled_template(template);
    std::iter::once(&compiled.instructions)
        .chain(compiled.blocks.values())
        .any(|code| (0..).map_while(|index| code.get(index)).any(&predicate))
}

fn uses_native_macros(template: &Template<'_, '_>) -> bool {
    // Only macro declarations and call-block bodies compile to BuildMacro.
    uses_instruction(template, |op| matches!(op, Instruction::BuildMacro(..)))
}

fn uses_hyperlink_helper(template: &Template<'_, '_>, slot: bool) -> bool {
    // A partial can inherit an ordinary variable named hyperlink from its caller.
    template.undeclared_variables(false).contains("hyperlink")
        && (slot
            || uses_instruction(template, |op| {
                matches!(op, Instruction::CallFunction("hyperlink", _))
            }))
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
    let formatted = config_error(source, key, format!("{reason}{line}"));
    if error.kind() == minijinja::ErrorKind::UnknownFunction
        && error.detail() == Some("hyperlink is unknown")
    {
        return formatted.with_hint(EXPERIMENTAL_TEMPLATES_HINT);
    }
    formatted
}

impl TemplateConfig {
    #[cfg(test)]
    pub fn builtin() -> Self {
        Self::builtin_with_experimental(false)
    }

    pub fn builtin_with_experimental(experimental: bool) -> Self {
        Self::builtin_from_source(DEFAULTS, experimental)
    }

    fn builtin_from_source(source: &str, experimental: bool) -> Self {
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
            environment: environment(experimental),
            dormant_aliases: BTreeSet::new(),
            slot_fallbacks: BTreeMap::new(),
            display_environment: Environment::empty(),
            experimental,
        };
        config
            .apply_table(BUILTIN_SOURCE, &table, Layer::Builtin)
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
            .rebuild(false)
            .expect("shipped template registry compiles");
        config
    }

    #[cfg(test)]
    pub fn from_tiers<'a>(tiers: impl IntoIterator<Item = &'a ConfigTier>) -> Result<Self> {
        Self::from_tiers_with_experimental(tiers, false)
    }

    pub fn from_tiers_with_experimental<'a>(
        tiers: impl IntoIterator<Item = &'a ConfigTier>,
        experimental: bool,
    ) -> Result<Self> {
        let mut config = Self::builtin_with_experimental(experimental);
        for tier in tiers {
            config.apply_tier(tier)?;
        }
        Ok(config)
    }

    /// A persistent opt-out suspends saved experimental definitions. Explicit CLI
    /// overlays still go through the usual strict opt-in checks.
    pub fn with_experiment_disabled<'a>(
        tiers: impl IntoIterator<Item = &'a ConfigTier>,
    ) -> Result<Self> {
        let mut config = Self::builtin_with_experimental(false);
        for tier in tiers {
            config.apply_table(
                &tier.path.display().to_string(),
                &tier.table,
                Layer::DisabledPreferences,
            )?;
        }
        Ok(config)
    }

    pub fn experimental_enabled(&self) -> bool {
        self.experimental
    }

    pub fn entries(&self) -> &BTreeMap<String, TemplateEntry> {
        &self.entries
    }

    pub fn validate_alias(&self, name: &str) -> Result<()> {
        if self.entries.get(name).is_some_and(TemplateEntry::is_alias) {
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
        self.apply_table(
            &tier.path.display().to_string(),
            &tier.table,
            Layer::Override,
        )
    }

    fn apply_table(&mut self, source: &str, table: &toml::Table, layer: Layer) -> Result<()> {
        let builtin = layer == Layer::Builtin;
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
                if name == FORMAT_ISSUE_ID {
                    if !builtin && !candidate.experimental && section == "templates" {
                        if layer == Layer::DisabledPreferences {
                            continue;
                        }
                        return Err(experimental_error(
                            source,
                            &key,
                            "format_issue_id overrides",
                        ));
                    }
                    if candidate.experimental && section == "template-aliases" {
                        return Err(config_error(
                            source,
                            &key,
                            "format_issue_id is reserved when experimental templates are enabled",
                        )
                        .with_hint("Rename the alias and update any templates that refer to it."));
                    }
                }
                if section == "template-aliases"
                    && ((candidate.template_names.contains(name) && name != FORMAT_ISSUE_ID)
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
                if layer == Layer::DisabledPreferences {
                    let template = candidate
                        .environment
                        .template_from_named_str(name, body)
                        .map_err(|error| config_error(source, &key, error.kind()))?;
                    if uses_native_macros(&template)
                        || uses_hyperlink_helper(&template, section == "templates")
                    {
                        if section == "template-aliases" {
                            candidate.dormant_aliases.insert(name.clone());
                        }
                        continue;
                    }
                }
                if section == "templates" {
                    if layer == Layer::DisabledPreferences {
                        if let Some(previous) = candidate.entries.get(name) {
                            candidate
                                .slot_fallbacks
                                .entry(name.clone())
                                .or_default()
                                .push(previous.body.clone());
                        }
                    } else {
                        candidate.slot_fallbacks.remove(name);
                    }
                }
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
        candidate.rebuild(!builtin)?;
        *self = candidate;
        Ok(())
    }

    fn rebuild(&mut self, check_user_templates: bool) -> Result<()> {
        let mut env = environment(self.experimental);
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
        // Distinguish a deliberately dormant partial from a misspelled include.
        // MiniJinja resolves includes/imports dynamically, with the caller's data.
        for name in self
            .dormant_aliases
            .iter()
            .filter(|name| !self.entries.contains_key(*name))
        {
            env.add_template_owned(
                name.clone(),
                "{{ fail('experimental partial is disabled') }}".to_owned(),
            )
            .expect("fixed dormant-partial sentinel compiles");
        }
        for (name, entry) in &self.entries {
            for registry in [&mut env, &mut display_env] {
                registry
                    .add_template_owned(name.clone(), entry.body.clone())
                    .map_err(|error| engine_error(error, name, &self.entries))?;
            }
            let check_experimental = check_user_templates
                && !self.experimental
                && self.builtins.get(name) != Some(entry);
            let fields = if entry.is_alias() {
                None
            } else {
                context_fields(name)
            };
            if check_experimental || fields.is_some() {
                let template = env
                    .get_template(name)
                    .map_err(|error| engine_error(error, name, &self.entries))?;
                if check_experimental && uses_native_macros(&template) {
                    return Err(experimental_error(
                        &entry.origin.source,
                        &entry.origin.key,
                        "macros and call blocks",
                    ));
                }
                for variable in template.undeclared_variables(true) {
                    let root = variable.split('.').next().expect("a variable has a root");
                    if check_experimental && fields.is_some() && root == "hyperlink" {
                        return Err(experimental_error(
                            &entry.origin.source,
                            &entry.origin.key,
                            "hyperlink",
                        ));
                    }
                    // Catch direct misspellings even in unselected branches.
                    // Nested partial and indexed lookups are checked on render.
                    let Some(fields) = fields else {
                        continue;
                    };
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
        let mut env = std::borrow::Cow::Borrowed(&self.environment);
        let mut fallbacks = std::borrow::Cow::Borrowed(&self.slot_fallbacks);
        loop {
            let result = env
                .get_template(name)
                .and_then(|template| template.render(minijinja::value::Serde(context)));
            let error = match result {
                Ok(text) => return Ok(text),
                Err(error) => error,
            };
            if self.dormant_aliases.is_empty() {
                return Err(engine_error(error, name, &self.entries));
            }
            // A saved override can import a dormant macro through several partials,
            // or select it by a computed name. Restore the closest enclosing slot
            // and retry, keeping unrelated stable customizations in effect.
            let mut enclosing_slot = None;
            let mut cause = &error;
            while let Some(next) = std::error::Error::source(cause)
                .and_then(|source| source.downcast_ref::<minijinja::Error>())
            {
                if let Some(slot) = cause.name().filter(|slot| {
                    fallbacks
                        .get(*slot)
                        .is_some_and(|bodies| !bodies.is_empty())
                }) {
                    enclosing_slot = Some(slot);
                }
                cause = next;
            }
            if let Some(slot) = enclosing_slot
                && cause.name().is_some_and(|alias| {
                    self.dormant_aliases.contains(alias) && !self.entries.contains_key(alias)
                })
                && let Some(body) = fallbacks.to_mut().get_mut(slot).and_then(Vec::pop)
            {
                env.to_mut()
                    .add_template_owned(slot.to_owned(), body)
                    .expect("previously compiled template");
                continue;
            }
            return Err(engine_error(error, name, &self.entries));
        }
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

        let name = if self.experimental {
            FORMAT_ISSUE_ID
        } else {
            BUILTIN_FORMAT_ISSUE_ID
        };
        let mut output = self.render_display(name, &Context { issue }, styled)?;
        for span in &mut output.spans {
            if let std::borrow::Cow::Owned(text) = terminal_text::single_line(&span.text) {
                span.text = text;
            }
        }
        Ok(output)
    }
}

impl TemplateEntry {
    fn is_alias(&self) -> bool {
        self.origin.key.starts_with("template-aliases.")
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
