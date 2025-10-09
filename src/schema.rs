use anyhow::{Context, Result};
use serde::Deserialize;
use serde_json::{Map, Value};
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Base arguments file structure: { "args": ["..."] }
#[derive(Debug, Deserialize)]
pub(crate) struct BaseArgsFile {
    pub args: Vec<String>,
}

impl BaseArgsFile {
    pub fn from_path(path: &Path) -> Result<Self> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read baseArgs file: {}", path.display()))?;
        let parsed: BaseArgsFile = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse baseArgs JSON: {}", path.display()))?;
        Ok(parsed)
    }
}

/// Individual configuration entry with template reference and overrides
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

impl ConfigFile {
    /// Loads and validates configuration entries from a path. Returns one entry per JSON object.
    pub fn from_path(config_path: &Path) -> Result<Vec<Self>> {
        let content = fs::read_to_string(config_path)
            .with_context(|| format!("Failed to read config file: {}", config_path.display()))?;

        let raw: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse config JSON: {}", config_path.display()))?;

        let entries = match raw {
            Value::Array(items) => items,
            Value::Object(_) => {
                anyhow::bail!(
                    "{} must be a JSON array of configuration objects. Legacy single-object configs are no longer supported.",
                    config_path.display()
                );
            }
            other => {
                let type_name = match other {
                    Value::Null => "null",
                    Value::Bool(_) => "boolean",
                    Value::Number(_) => "number",
                    Value::String(_) => "string",
                    Value::Array(_) => unreachable!(),
                    Value::Object(_) => unreachable!(),
                };
                anyhow::bail!(
                    "{} must be a JSON array of configuration objects, found {} instead.",
                    config_path.display(),
                    type_name
                );
            }
        };

        entries
            .into_iter()
            .enumerate()
            .map(|(idx, entry)| -> Result<_> {
                let config: ConfigFile = serde_json::from_value(entry).with_context(|| {
                    format!(
                        "Failed to parse config JSON entry at index {} in {}",
                        idx,
                        config_path.display()
                    )
                })?;

                config.validate_extends(config_path)?;
                Ok(config)
            })
            .collect()
    }

    fn validate_extends(&self, config_path: &Path) -> Result<()> {
        if self.extends.contains('/') || self.extends.contains('\\') {
            anyhow::bail!(
                "Invalid extends value '{}' in {}\nOnly template names are allowed (e.g., 'cpp', 'lldb')",
                self.extends,
                config_path.display()
            );
        }
        Ok(())
    }
}

/// Single template definition parsed from manifest or in-memory JSON
#[derive(Debug, Clone)]
pub(crate) struct Template {
    pub type_field: String,
    pub request: Option<String>,
    pub program: Option<String>,
    pub stop_at_entry: Option<bool>,
    pub rest: Map<String, Value>,
}

impl Template {
    pub fn from_value(template: Value) -> Result<Self> {
        let template_obj = match template {
            Value::Object(obj) => obj,
            _ => anyhow::bail!("Template must be a JSON object"),
        };

        // Disallow 'args' in templates to avoid ambiguity with per-config args/baseArgs
        if template_obj.contains_key("args") {
            anyhow::bail!("Template must not define 'args'; use config files to set args");
        }

        let type_field = template_obj
            .get("type")
            .and_then(|v| v.as_str())
            .ok_or_else(|| anyhow::anyhow!("Template missing required 'type' field"))?
            .to_string();

        let request = template_obj
            .get("request")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let program = template_obj
            .get("program")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let stop_at_entry = template_obj.get("stopAtEntry").and_then(|v| v.as_bool());

        let mut rest: Map<String, Value> = Map::with_capacity(template_obj.len());
        for (k, v) in template_obj.iter() {
            if k == "type" || k == "request" || k == "program" || k == "stopAtEntry" {
                continue;
            }
            rest.insert(k.clone(), v.clone());
        }

        Ok(Self {
            type_field,
            request,
            program,
            stop_at_entry,
            rest,
        })
    }
}

/// Manifest containing multiple templates indexed by name
#[derive(Debug, Clone, Default)]
pub(crate) struct TemplateFile {
    templates: BTreeMap<String, Template>,
}

impl TemplateFile {
    pub fn from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            anyhow::bail!("Templates manifest does not exist: {}", path.display());
        }

        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read templates manifest: {}", path.display()))?;

        let root: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse templates manifest: {}", path.display()))?;

        let templates_value = root.get("templates").ok_or_else(|| {
            anyhow::anyhow!("Templates manifest must contain a 'templates' array")
        })?;

        let templates_array = templates_value
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("'templates' must be an array in {}", path.display()))?;

        let mut templates = BTreeMap::new();
        for (idx, entry) in templates_array.iter().enumerate() {
            let mut object = entry.as_object().cloned().ok_or_else(|| {
                anyhow::anyhow!("Template entry at index {} must be a JSON object", idx)
            })?;

            let name_value = object.remove("name").ok_or_else(|| {
                anyhow::anyhow!(
                    "Template entry at index {} is missing required 'name' field",
                    idx
                )
            })?;

            let name = name_value.as_str().ok_or_else(|| {
                anyhow::anyhow!(
                    "Template entry at index {} must have 'name' as a string",
                    idx
                )
            })?;

            if templates.contains_key(name) {
                anyhow::bail!(
                    "Duplicate template name '{}' found in {}",
                    name,
                    path.display()
                );
            }

            let template = Template::from_value(Value::Object(object))
                .with_context(|| format!("Invalid template '{}'", name))?;

            templates.insert(name.to_string(), template);
        }

        if templates.is_empty() {
            anyhow::bail!(
                "Templates manifest '{}' must define at least one template",
                path.display()
            );
        }

        Ok(Self { templates })
    }

    pub fn get(&self, name: &str) -> Result<&Template> {
        self.templates
            .get(name)
            .ok_or_else(|| anyhow::anyhow!("Template '{}' not found in templates manifest", name))
    }
}
