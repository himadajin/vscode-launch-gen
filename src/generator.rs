use crate::schema::{BaseArgsFile, ConfigFile, TemplateFile};
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
    #[serde(flatten)]
    rest: Map<String, Value>,
}

#[derive(Debug, Serialize)]
struct WriteLaunchJson {
    version: String,
    configurations: Vec<LaunchConfig>,
}

/// Main generator for creating VSCode launch.json from templates and configs
pub struct Generator {
    config_dir: PathBuf,
    templates_dir: PathBuf,
    configs_dir: PathBuf,
    output_path: PathBuf,
}

impl Generator {
    /// Creates a new generator instance with directory paths
    pub fn new(config_dir: PathBuf, output_path: PathBuf) -> Self {
        let templates_dir = config_dir.join("templates");
        let configs_dir = config_dir.join("configs");

        Self {
            config_dir,
            templates_dir,
            configs_dir,
            output_path,
        }
    }

    /// Merges template and config and returns a JSON value (for tests and intermediate checks)
    pub fn merge_config(&self, template: Value, config: ConfigFile) -> Result<Value> {
        let tmpl = TemplateFile::from_value(template)?;
        let ordered = LaunchConfig::from_template_and_config_with_template(config, tmpl)?;
        Ok(serde_json::to_value(ordered)?)
    }

    /// Collects all JSON config files from configs directory in alphabetical order
    pub fn collect_config_files(&self) -> Result<Vec<PathBuf>> {
        if !self.configs_dir.exists() {
            anyhow::bail!(
                "Config directory does not exist: {}",
                self.configs_dir.display()
            );
        }

        let mut config_files = Vec::new();

        for entry in fs::read_dir(&self.configs_dir).with_context(|| {
            format!(
                "Failed to read configs directory: {}",
                self.configs_dir.display()
            )
        })? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() && path.extension().and_then(|s| s.to_str()) == Some("json") {
                config_files.push(path);
            }
        }

        config_files.sort();
        Ok(config_files)
    }

    /// Validates that all configuration names are unique across files
    pub fn validate_unique_names(&self, configs: &[(PathBuf, ConfigFile)]) -> Result<()> {
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

    /// Ensures the output directory exists, creating it if necessary
    fn ensure_output_dir(&self) -> Result<()> {
        if let Some(parent) = self.output_path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("Failed to create output directory: {}", parent.display())
            })?;
        }
        Ok(())
    }

    /// Main generation process - reads configs, merges with templates, and writes launch.json
    pub fn generate(&self) -> Result<()> {
        if !self.config_dir.exists() {
            anyhow::bail!(
                "Config directory does not exist: {}",
                self.config_dir.display()
            );
        }

        if !self.templates_dir.exists() {
            anyhow::bail!(
                "Templates directory does not exist: {}",
                self.templates_dir.display()
            );
        }

        let config_files = self.collect_config_files()?;

        if config_files.is_empty() {
            anyhow::bail!(
                "No configuration files found in: {}",
                self.configs_dir.display()
            );
        }

        let mut configs = Vec::new();
        for config_path in config_files {
            let config = ConfigFile::from_path(&config_path)?;
            configs.push((config_path, config));
        }

        // Filter out disabled configurations before validation
        let enabled_configs: Vec<_> = configs
            .into_iter()
            .filter(|(_, config)| config.enabled)
            .collect();

        if enabled_configs.is_empty() {
            anyhow::bail!(
                "No enabled configuration files found in: {}",
                self.configs_dir.display()
            );
        }

        self.validate_unique_names(&enabled_configs)?;

        let mut configurations: Vec<LaunchConfig> = Vec::new();

        for (config_path, config) in enabled_configs {
            let merged = LaunchConfig::from_template_and_config(&self.templates_dir, config, None)
                .with_context(|| format!("Error processing config: {}", config_path.display()))?;
            configurations.push(merged);
        }

        let launch_json = WriteLaunchJson {
            version: "0.2.0".to_string(),
            configurations,
        };

        self.ensure_output_dir()?;

        let json_content = serde_json::to_string_pretty(&launch_json)
            .context("Failed to serialize launch.json")?;

        fs::write(&self.output_path, json_content).with_context(|| {
            format!(
                "Failed to write output file: {}",
                self.output_path.display()
            )
        })?;

        Ok(())
    }
}

impl LaunchConfig {
    /// Build a configuration from templates dir and ConfigFile.
    /// If `template_override` is provided, it is used instead of reading from disk.
    pub fn from_template_and_config(
        templates_dir: &Path,
        config: ConfigFile,
        template_override: Option<Value>,
    ) -> Result<Self> {
        let tmpl = match template_override {
            Some(v) => TemplateFile::from_value(v)?,
            None => {
                let template_path = templates_dir.join(format!("{}.json", config.extends));
                TemplateFile::from_path(&template_path)?
            }
        };
        let mut rest = tmpl.rest.clone();

        // Build args: baseArgs (if any) + args (if any)
        if config.args.is_some() || config.base_args.is_some() {
            let mut final_args: Vec<String> = Vec::new();
            if let Some(base_path) = &config.base_args {
                let base = BaseArgsFile::from_path(base_path)?;
                final_args.extend(base.args);
            }
            if let Some(extra) = &config.args {
                final_args.extend(extra.clone());
            }
            rest.insert("args".to_string(), serde_json::json!(final_args));
        }

        Ok(LaunchConfig {
            type_field: tmpl.type_field,
            request: tmpl.request,
            name: config.name,
            program: tmpl.program,
            rest,
        })
    }

    /// Build a configuration from an already-parsed TemplateFile and ConfigFile.
    fn from_template_and_config_with_template(
        config: ConfigFile,
        tmpl: TemplateFile,
    ) -> Result<Self> {
        let mut rest = tmpl.rest.clone();
        if config.args.is_some() || config.base_args.is_some() {
            let mut final_args: Vec<String> = Vec::new();
            if let Some(base_path) = &config.base_args {
                let base = BaseArgsFile::from_path(base_path)?;
                final_args.extend(base.args);
            }
            if let Some(extra) = &config.args {
                final_args.extend(extra.clone());
            }
            rest.insert("args".to_string(), serde_json::json!(final_args));
        }

        Ok(LaunchConfig {
            type_field: tmpl.type_field,
            request: tmpl.request,
            name: config.name,
            program: tmpl.program,
            rest,
        })
    }
}
