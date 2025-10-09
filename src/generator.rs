use crate::schema::{BaseArgsFile, ConfigFile, Template, TemplateFile};
use anyhow::{Context, Result};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Launch configuration (template + overrides) serialized with ordered keys.
/// Order: type, request, name, program, then other keys.
#[derive(Debug, Serialize)]
pub struct LaunchConfig {
    #[serde(rename = "type")]
    type_field: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    request: Option<String>,
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    program: Option<String>,
    args: Vec<String>,
    #[serde(rename = "stopAtEntry", skip_serializing_if = "Option::is_none")]
    stop_at_entry: Option<bool>,
    #[serde(flatten)]
    rest: Map<String, Value>,
}

impl LaunchConfig {
    /// Backward-compatible helper that delegates to `Resolver`.
    pub fn from_template_and_config(
        templates_manifest: &Path,
        config: ConfigFile,
        template_override: Option<Value>,
    ) -> Result<Self> {
        let resolver = Resolver::new(TemplateFile::from_path(templates_manifest)?);
        resolver.resolve(config, template_override)
    }
}
/// Resolves `ConfigFile` into `LaunchConfig` using templates manifest context.
pub(crate) struct Resolver {
    templates: TemplateFile,
}

impl Resolver {
    pub fn new(templates: TemplateFile) -> Self {
        Self { templates }
    }

    /// Build a configuration from templates dir and ConfigFile.
    /// If `template_override` is provided, it is used instead of reading from disk.
    pub fn resolve(
        &self,
        config: ConfigFile,
        template_override: Option<Value>,
    ) -> Result<LaunchConfig> {
        let tmpl = match template_override {
            Some(v) => Template::from_value(v)?,
            None => self.templates.get(&config.extends)?.clone(),
        };
        Self::build_from_template(config, tmpl)
    }

    fn build_from_template(config: ConfigFile, tmpl: Template) -> Result<LaunchConfig> {
        // Build args: baseArgs (if any) + args (if any). Always present (can be empty)
        let mut args: Vec<String> = Vec::new();
        if let Some(base_path) = &config.base_args {
            let base = BaseArgsFile::from_path(base_path)?;
            args.extend(base.args);
        }
        if let Some(extra) = &config.args {
            args.extend(extra.clone());
        }

        // Sanity check: templates must not provide args (enforced at parse time)
        debug_assert!(
            !tmpl.rest.contains_key("args"),
            "Template rest must not contain 'args'"
        );

        Ok(LaunchConfig {
            type_field: tmpl.type_field,
            request: tmpl.request,
            name: config.name,
            program: tmpl.program,
            args,
            stop_at_entry: tmpl.stop_at_entry,
            rest: tmpl.rest.clone(),
        })
    }
}

#[derive(Debug, Serialize)]
pub struct LaunchJson {
    version: String,
    configurations: Vec<LaunchConfig>,
}

impl LaunchJson {
    pub fn configurations(&self) -> &[LaunchConfig] {
        &self.configurations
    }
}

/// Main generator for creating VSCode launch.json from templates and configs
pub struct Generator {
    templates_path: PathBuf,
    configs_dir: PathBuf,
}

impl Generator {
    /// Creates a new generator instance with explicit templates/configs directories
    pub fn new(templates_path: PathBuf, configs_dir: PathBuf) -> Self {
        Self {
            templates_path,
            configs_dir,
        }
    }

    /// Main generation process - reads configs, merges with templates, and returns LaunchJson
    pub fn generate(&self) -> Result<LaunchJson> {
        if !self.templates_path.exists() {
            anyhow::bail!(
                "Templates manifest does not exist: {}",
                self.templates_path.display()
            );
        }

        let configs = collect_config_files(&self.configs_dir)?;

        if configs.is_empty() {
            anyhow::bail!(
                "No configuration entries found in: {}",
                self.configs_dir.display()
            );
        }

        // Filter out disabled configurations before validation
        let enabled_configs: Vec<_> = configs
            .into_iter()
            .filter(|(_, config)| config.enabled)
            .collect();

        if enabled_configs.is_empty() {
            anyhow::bail!(
                "No enabled configuration entries found in: {}",
                self.configs_dir.display()
            );
        }

        validate_unique_names(&enabled_configs)?;

        let mut configurations: Vec<LaunchConfig> = Vec::new();
        let resolver = Resolver::new(TemplateFile::from_path(&self.templates_path)?);

        for (config_path, config) in enabled_configs {
            let merged = resolver
                .resolve(config, None)
                .with_context(|| format!("Error processing config: {}", config_path.display()))?;
            configurations.push(merged);
        }

        // Sort configurations by display name to stabilize order
        configurations.sort_by(|a, b| a.name.cmp(&b.name));

        let launch_json = LaunchJson {
            version: "0.2.0".to_string(),
            configurations,
        };

        Ok(launch_json)
    }
}

/// Collects all JSON config entries from `configs_dir` in alphabetical order of file path
pub(crate) fn collect_config_files(configs_dir: &Path) -> Result<Vec<(PathBuf, ConfigFile)>> {
    if !configs_dir.exists() {
        anyhow::bail!("Config directory does not exist: {}", configs_dir.display());
    }

    let mut config_files: Vec<PathBuf> = Vec::new();

    for entry in fs::read_dir(configs_dir).with_context(|| {
        format!(
            "Failed to read configs directory: {}",
            configs_dir.display()
        )
    })? {
        let entry = entry?;
        let path = entry.path();

        if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
            config_files.push(path);
        }
    }

    config_files.sort();

    // Load after collecting all paths
    let mut configs: Vec<(PathBuf, ConfigFile)> = Vec::new();
    for config_path in config_files.into_iter() {
        let entries = ConfigFile::from_path(&config_path)?;
        for config in entries {
            configs.push((config_path.clone(), config));
        }
    }
    Ok(configs)
}

/// Validates that all configuration names are unique across files
pub(crate) fn validate_unique_names(configs: &[(PathBuf, ConfigFile)]) -> Result<()> {
    let mut name_to_files: BTreeMap<&str, Vec<&Path>> = BTreeMap::new();

    for (path, config) in configs {
        name_to_files.entry(&config.name).or_default().push(path);
    }

    for (name, files) in name_to_files {
        if files.len() > 1 {
            let file_list: Vec<String> = files
                .iter()
                .map(|p| format!("  - {}", p.display()))
                .collect();

            anyhow::bail!(
                "Duplicate configuration name '{}' found in:\n{}\nEach configuration must have a unique name.",
                name,
                file_list.join("\n")
            );
        }
    }

    Ok(())
}
