use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Individual configuration file structure with template reference and overrides
#[derive(Debug, Deserialize)]
pub struct ConfigFile {
    /// Unique configuration name displayed in VSCode
    pub name: String,
    /// Template name to extend (without .json extension)
    pub extends: String,
    /// Whether this configuration is enabled
    pub enabled: bool,
    /// Optional path to a JSON file containing base args, e.g., { "args": ["..."] }
    #[serde(rename = "baseArgs")]
    pub base_args: Option<PathBuf>,
    /// Additional args to append after base args
    pub args: Option<Vec<String>>,
}

/// VSCode launch.json file structure
#[derive(Debug, Serialize, Deserialize)]
pub struct LaunchJson {
    /// VSCode launch configuration version
    pub version: String,
    /// Array of debug configurations
    pub configurations: Vec<Value>,
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

    /// Loads a template file by name from the templates directory
    pub fn load_template(&self, template_name: &str) -> Result<Value> {
        let template_path = self.templates_dir.join(format!("{}.json", template_name));

        if !template_path.exists() {
            anyhow::bail!(
                "Base template '{}' not found (expected: {})",
                template_name,
                template_path.display()
            );
        }

        let content = fs::read_to_string(&template_path).with_context(|| {
            format!("Failed to read template file: {}", template_path.display())
        })?;

        serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse template JSON: {}", template_path.display()))
    }

    /// Loads and validates a configuration file
    pub fn load_config(&self, config_path: &Path) -> Result<ConfigFile> {
        let content = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let config: ConfigFile = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config JSON: {}", config_path.display()))?;

        if config.extends.contains('/') || config.extends.contains('\\') {
            anyhow::bail!(
                "Invalid extends value '{}' in {}\nOnly template names are allowed (e.g., 'cpp', 'lldb')",
                config.extends,
                config_path.display()
            );
        }

        Ok(config)
    }

    /// Load base args from a JSON file that contains { "args": [ ... ] }
    fn load_base_args(&self, path: &Path) -> Result<Vec<String>> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read baseArgs file: {}", path.display()))?;
        let v: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse baseArgs JSON: {}", path.display()))?;
        let args = v.get("args").and_then(|a| a.as_array()).ok_or_else(|| {
            anyhow::anyhow!(
                "Invalid baseArgs format in {}: missing 'args' array",
                path.display()
            )
        })?;
        let mut out = Vec::with_capacity(args.len());
        for item in args {
            match item.as_str() {
                Some(s) => out.push(s.to_string()),
                None => anyhow::bail!(
                    "Invalid baseArgs format in {}: 'args' must be an array of strings",
                    path.display()
                ),
            }
        }
        Ok(out)
    }

    /// Merges template and config, with config values overriding template values
    pub fn merge_config(&self, template: Value, config: ConfigFile) -> Result<Value> {
        let mut merged = if let Value::Object(template_obj) = template {
            template_obj
        } else {
            anyhow::bail!("Template must be a JSON object");
        };

        // Insert the name field from the top-level config
        merged.insert("name".to_string(), Value::String(config.name));

        // Build args: baseArgs (if any) + args (if any)
        let mut final_args: Vec<String> = Vec::new();
        if let Some(base_path) = &config.base_args {
            let base = self.load_base_args(base_path)?;
            final_args.extend(base);
        }
        if let Some(extra) = &config.args {
            final_args.extend(extra.clone());
        }
        if config.args.is_some() || config.base_args.is_some() {
            merged.insert("args".to_string(), serde_json::json!(final_args));
        }

        Ok(Value::Object(merged))
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
            let config = self.load_config(&config_path)?;
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

        let mut configurations = Vec::new();

        for (config_path, config) in enabled_configs {
            let template = self
                .load_template(&config.extends)
                .with_context(|| format!("Error processing config: {}", config_path.display()))?;

            let merged = self.merge_config(template, config)?;
            configurations.push(merged);
        }

        let launch_json = LaunchJson {
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
