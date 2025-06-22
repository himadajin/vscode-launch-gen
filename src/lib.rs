pub mod error;
pub mod generator;

// Re-export public APIs
pub use error::GeneratorError;
pub use generator::{ConfigFile, Generator, LaunchJson};

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_generator(temp_dir: &TempDir) -> Generator {
        let config_dir = temp_dir.path().join(".vscode-debug");
        let output_path = temp_dir.path().join(".vscode/launch.json");
        Generator::new(config_dir, output_path)
    }

    fn setup_test_files(temp_dir: &TempDir) -> anyhow::Result<()> {
        let templates_dir = temp_dir.path().join(".vscode-debug/templates");
        let configs_dir = temp_dir.path().join(".vscode-debug/configs");

        fs::create_dir_all(&templates_dir)?;
        fs::create_dir_all(&configs_dir)?;

        // Create template
        let template = json!({
            "type": "cppdbg",
            "request": "launch",
            "program": "${workspaceFolder}/build/bin/myapp",
            "stopAtEntry": false,
            "cwd": "${workspaceFolder}",
            "environment": [],
            "externalConsole": false,
            "MIMode": "gdb"
        });

        fs::write(
            templates_dir.join("cpp.json"),
            serde_json::to_string_pretty(&template)?,
        )?;

        // Create config files
        let config1 = json!({
            "extends": "cpp",
            "name": "Basic Test",
            "args": ["--test"]
        });

        let config2 = json!({
            "extends": "cpp",
            "name": "Test with Input",
            "args": ["--input", "data.txt"],
            "cwd": "${workspaceFolder}/test"
        });

        fs::write(
            configs_dir.join("01-basic.json"),
            serde_json::to_string_pretty(&config1)?,
        )?;

        fs::write(
            configs_dir.join("02-input.json"),
            serde_json::to_string_pretty(&config2)?,
        )?;

        Ok(())
    }

    #[test]
    fn test_load_template() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        setup_test_files(&temp_dir)?;
        let generator = create_test_generator(&temp_dir);

        let template = generator.load_template("cpp")?;
        assert_eq!(template["type"], "cppdbg");
        assert_eq!(template["MIMode"], "gdb");

        Ok(())
    }

    #[test]
    fn test_load_template_not_found() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_files(&temp_dir).unwrap();
        let generator = create_test_generator(&temp_dir);

        let result = generator.load_template("nonexistent");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_load_config() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        setup_test_files(&temp_dir)?;
        let generator = create_test_generator(&temp_dir);

        let config_path = temp_dir.path().join(".vscode-debug/configs/01-basic.json");
        let config = generator.load_config(&config_path)?;

        assert_eq!(config.extends, "cpp");
        assert_eq!(config.name, "Basic Test");
        assert_eq!(config.extra["args"], json!(["--test"]));

        Ok(())
    }

    #[test]
    fn test_load_config_invalid_extends() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let configs_dir = temp_dir.path().join(".vscode-debug/configs");
        fs::create_dir_all(&configs_dir)?;

        let invalid_config = json!({
            "extends": "../other/template",
            "name": "Invalid Test"
        });

        let config_path = configs_dir.join("invalid.json");
        fs::write(&config_path, serde_json::to_string_pretty(&invalid_config)?)?;

        let generator = create_test_generator(&temp_dir);
        let result = generator.load_config(&config_path);

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Invalid extends value")
        );

        Ok(())
    }

    #[test]
    fn test_merge_config() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let generator = create_test_generator(&temp_dir);

        let template = json!({
            "type": "cppdbg",
            "program": "${workspaceFolder}/build/bin/myapp",
            "cwd": "${workspaceFolder}",
            "environment": []
        });

        let config = ConfigFile {
            extends: "cpp".to_string(),
            name: "Test Config".to_string(),
            extra: {
                let mut map = std::collections::BTreeMap::new();
                map.insert("args".to_string(), json!(["--test"]));
                map.insert("cwd".to_string(), json!("${workspaceFolder}/test"));
                map
            },
        };

        let merged = generator.merge_config(template, config)?;

        assert_eq!(merged["name"], "Test Config");
        assert_eq!(merged["type"], "cppdbg");
        assert_eq!(merged["args"], json!(["--test"]));
        assert_eq!(merged["cwd"], "${workspaceFolder}/test"); // Should be overwritten

        Ok(())
    }

    #[test]
    fn test_validate_unique_names() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let generator = create_test_generator(&temp_dir);

        let config1 = ConfigFile {
            extends: "cpp".to_string(),
            name: "Test".to_string(),
            extra: std::collections::BTreeMap::new(),
        };

        let config2 = ConfigFile {
            extends: "cpp".to_string(),
            name: "Test".to_string(), // Duplicate name
            extra: std::collections::BTreeMap::new(),
        };

        let configs = vec![
            (std::path::PathBuf::from("config1.json"), config1),
            (std::path::PathBuf::from("config2.json"), config2),
        ];

        let result = generator.validate_unique_names(&configs);
        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("Duplicate configuration name")
        );

        Ok(())
    }

    #[test]
    fn test_collect_config_files() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        setup_test_files(&temp_dir)?;
        let generator = create_test_generator(&temp_dir);

        let files = generator.collect_config_files()?;
        assert_eq!(files.len(), 2);

        // Should be sorted alphabetically
        assert!(files[0].file_name().unwrap().to_str().unwrap() == "01-basic.json");
        assert!(files[1].file_name().unwrap().to_str().unwrap() == "02-input.json");

        Ok(())
    }

    #[test]
    fn test_generate_full_process() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        setup_test_files(&temp_dir)?;
        let generator = create_test_generator(&temp_dir);

        generator.generate()?;

        let output_path = temp_dir.path().join(".vscode/launch.json");
        assert!(output_path.exists());

        let content = fs::read_to_string(output_path)?;
        let launch_json: LaunchJson = serde_json::from_str(&content)?;

        assert_eq!(launch_json.version, "0.2.0");
        assert_eq!(launch_json.configurations.len(), 2);

        // Check first configuration
        let config1 = &launch_json.configurations[0];
        assert_eq!(config1["name"], "Basic Test");
        assert_eq!(config1["type"], "cppdbg");
        assert_eq!(config1["args"], json!(["--test"]));

        // Check second configuration
        let config2 = &launch_json.configurations[1];
        assert_eq!(config2["name"], "Test with Input");
        assert_eq!(config2["cwd"], "${workspaceFolder}/test");

        Ok(())
    }
}
