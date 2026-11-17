//! Offline, template-only config discovery. Deliberately bypasses dotenv,
//! authentication, client initialization, keyrings and process mutations.
use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::templates::TemplateConfig;

use super::discover::{CandidateTier, ConfigCandidate, discover_config_paths};
use super::parse_config_tier;
use super::source::{ConfigInputs, FileSource, MAX_CONFIG_BYTES, RawConfigFile, repo_root};

pub fn load_template_config(
    inputs: &ConfigInputs,
    files: &impl FileSource,
    extra_files: &[PathBuf],
    snippets: &[String],
) -> Result<TemplateConfig> {
    let root = repo_root(&inputs.cwd, files);
    let paths = discover_config_paths(inputs, root.as_deref());
    let global = super::startup::read_tier(&paths.global, files)?;
    let project = super::startup::read_tier(&paths.project, files)?;
    let mut config = TemplateConfig::from_tiers(global.iter().chain(project.iter()))?;
    let extra_files = extra_files
        .iter()
        .map(|path| {
            if path.is_absolute() {
                path.clone()
            } else {
                inputs.cwd.join(path)
            }
        })
        .collect::<Vec<_>>();
    apply_template_overrides(&mut config, files, &extra_files, snippets)?;
    Ok(config)
}

/// Shared overlay handling for the normal and offline startup paths. Explicit
/// files are mandatory, and files precede snippets regardless of CLI ordering.
pub fn apply_template_overrides(
    config: &mut TemplateConfig,
    files: &impl FileSource,
    extra_files: &[PathBuf],
    snippets: &[String],
) -> Result<()> {
    for path in extra_files {
        let tier = super::startup::read_tier(
            &[ConfigCandidate {
                tier: CandidateTier::Project,
                path: path.clone(),
            }],
            files,
        )?
        .ok_or_else(|| {
            Error::new(format!(
                "template config file does not exist: {}",
                path.display()
            ))
        })?;
        validate_overlay_roots(&tier)?;
        config.apply_tier(&tier)?;
    }
    for (index, snippet) in snippets.iter().enumerate() {
        if !u64::try_from(snippet.len()).is_ok_and(|len| len <= MAX_CONFIG_BYTES) {
            return Err(Error::new("template --config value is too large"));
        }
        let tier = parse_config_tier(RawConfigFile {
            path: PathBuf::from(format!("--config #{}", index + 1)),
            bytes: snippet.as_bytes().to_vec(),
        })
        .map_err(|error| Error::new(format!("invalid template config: {error}")))?;
        validate_overlay_roots(&tier)?;
        config.apply_tier(&tier)?;
    }
    Ok(())
}

fn validate_overlay_roots(tier: &super::ConfigTier) -> Result<()> {
    if tier.table.is_empty()
        || tier
            .table
            .keys()
            .any(|key| !matches!(key.as_str(), "templates" | "template-aliases"))
    {
        return Err(Error::new(format!(
            "template overlay {} accepts only templates and template-aliases tables",
            tier.path.display()
        ))
        .with_hint("Use --config-file for a template-only TOML file, or --config 'templates.pr_title=\"{{ issues[0].title }}\"'."));
    }
    Ok(())
}
