pub mod generator;
mod schema;

// Re-export public APIs
pub use generator::{Generator, LaunchConfig};
pub use schema::ConfigFile;

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use std::path::Path;
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

        write_json(templates_dir.join("cpp.json"), &template)?;

        // Create config files (new schema with top-level args)
        let config1 = json!({
            "name": "Basic Test",
            "extends": "cpp",
            "enabled": true,
            "args": ["--test"]
        });

        let config2 = json!({
            "name": "Test with Input",
            "extends": "cpp",
            "enabled": true,
            "args": ["--input", "data.txt"]
        });

        write_json(configs_dir.join("01-basic.json"), &config1)?;
        write_json(configs_dir.join("02-input.json"), &config2)?;

        Ok(())
    }

    fn write_json<P: AsRef<Path>>(path: P, value: &serde_json::Value) -> anyhow::Result<()> {
        fs::write(path, serde_json::to_string_pretty(value)?)?;
        Ok(())
    }

    #[test]
    fn test_load_template() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        setup_test_files(&temp_dir)?;
        let templates_dir = temp_dir.path().join(".vscode-debug/templates");
        let config = ConfigFile {
            name: "Dummy".to_string(),
            extends: "cpp".to_string(),
            enabled: true,
            base_args: None,
            args: None,
        };
        let doc = LaunchConfig::from_template_and_config(&templates_dir, config, None)?;
        let v = serde_json::to_value(doc)?;
        assert_eq!(v["type"], "cppdbg");
        assert_eq!(v["MIMode"], "gdb");

        Ok(())
    }

    #[test]
    fn test_load_template_not_found() {
        let temp_dir = TempDir::new().unwrap();
        setup_test_files(&temp_dir).unwrap();
        let templates_dir = temp_dir.path().join(".vscode-debug/templates");
        let config = ConfigFile {
            name: "Dummy".to_string(),
            extends: "nonexistent".to_string(),
            enabled: true,
            base_args: None,
            args: None,
        };
        let result = LaunchConfig::from_template_and_config(&templates_dir, config, None);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not found"));
    }

    #[test]
    fn test_load_config() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        setup_test_files(&temp_dir)?;
        let config_path = temp_dir.path().join(".vscode-debug/configs/01-basic.json");
        let config = ConfigFile::from_path(&config_path)?;

        assert_eq!(config.extends, "cpp");
        assert_eq!(config.name, "Basic Test");
        assert_eq!(config.args.as_ref().unwrap(), &vec!["--test".to_string()]);

        Ok(())
    }

    #[test]
    fn test_load_config_invalid_extends() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let configs_dir = temp_dir.path().join(".vscode-debug/configs");
        fs::create_dir_all(&configs_dir)?;

        let invalid_config = json!({
            "name": "Invalid Test",
            "extends": "../other/template",
            "enabled": true
        });

        let config_path = configs_dir.join("invalid.json");
        write_json(&config_path, &invalid_config)?;

        let result = ConfigFile::from_path(&config_path);

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
        let _generator = create_test_generator(&temp_dir);

        let template = json!({
            "type": "cppdbg",
            "program": "${workspaceFolder}/build/bin/myapp",
            "cwd": "${workspaceFolder}",
            "environment": []
        });

        let config = ConfigFile {
            name: "Test Config".to_string(),
            extends: "cpp".to_string(),
            enabled: true,
            base_args: None,
            args: Some(vec!["--test".to_string()]),
        };

        // Local helper: resolve using Resolver with in-memory template
        let resolver =
            crate::generator::Resolver::new(temp_dir.path().join(".vscode-debug/templates"));
        let ordered = resolver.resolve(config, Some(template))?;
        let merged = serde_json::to_value(ordered)?;

        assert_eq!(merged["name"], "Test Config");
        assert_eq!(merged["type"], "cppdbg");
        assert_eq!(merged["args"], json!(["--test"]));

        Ok(())
    }

    #[test]
    fn test_validate_unique_names() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let generator = create_test_generator(&temp_dir);

        let config1 = ConfigFile {
            name: "Test".to_string(),
            extends: "cpp".to_string(),
            enabled: true,
            base_args: None,
            args: None,
        };

        let config2 = ConfigFile {
            name: "Test".to_string(), // Duplicate name
            extends: "cpp".to_string(),
            enabled: true,
            base_args: None,
            args: None,
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
        let v: serde_json::Value = serde_json::from_str(&content)?;

        assert_eq!(v["version"], "0.2.0");
        let configs = v["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 2);

        // Check first configuration
        let config1 = &configs[0];
        assert_eq!(config1["name"], "Basic Test");
        assert_eq!(config1["type"], "cppdbg");
        assert_eq!(config1["args"], json!(["--test"]));

        // Check second configuration
        let config2 = &configs[1];
        assert_eq!(config2["name"], "Test with Input");
        assert_eq!(config2["args"], json!(["--input", "data.txt"]));

        Ok(())
    }

    #[test]
    fn test_configuration_key_ordering() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        setup_test_files(&temp_dir)?;
        let generator = create_test_generator(&temp_dir);

        generator.generate()?;

        let output_path = temp_dir.path().join(".vscode/launch.json");
        let content = fs::read_to_string(output_path)?;

        // Find positions of the keys within the first configuration block
        // This is a pragmatic check to ensure ordering in serialized output
        let idx_type = content.find("\"type\"").unwrap();
        let idx_request = content.find("\"request\"").unwrap();
        let idx_name = content.find("\"name\"").unwrap();
        let idx_program = content.find("\"program\"").unwrap();

        assert!(
            idx_type < idx_request,
            "'type' should come before 'request'"
        );
        assert!(
            idx_request < idx_name,
            "'request' should come before 'name'"
        );
        assert!(
            idx_name < idx_program,
            "'name' should come before 'program'"
        );

        Ok(())
    }

    #[test]
    fn test_disabled_config_excluded() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let templates_dir = temp_dir.path().join(".vscode-debug/templates");
        let configs_dir = temp_dir.path().join(".vscode-debug/configs");

        fs::create_dir_all(&templates_dir)?;
        fs::create_dir_all(&configs_dir)?;

        // Create template
        let template = json!({
            "type": "cppdbg",
            "program": "${workspaceFolder}/build/myapp"
        });
        write_json(templates_dir.join("cpp.json"), &template)?;

        // Create enabled config
        let enabled_config = json!({
            "name": "Enabled Config",
            "extends": "cpp",
            "enabled": true,
            "args": ["--enabled"]
        });
        write_json(configs_dir.join("enabled.json"), &enabled_config)?;

        // Create disabled config
        let disabled_config = json!({
            "name": "Disabled Config",
            "extends": "cpp",
            "enabled": false,
            "args": ["--disabled"]
        });
        write_json(configs_dir.join("disabled.json"), &disabled_config)?;

        let generator = create_test_generator(&temp_dir);
        generator.generate()?;

        let output_path = temp_dir.path().join(".vscode/launch.json");
        let content = fs::read_to_string(output_path)?;
        let v: serde_json::Value = serde_json::from_str(&content)?;
        let configs = v["configurations"].as_array().unwrap();
        assert_eq!(configs.len(), 1);
        assert_eq!(configs[0]["name"], "Enabled Config");

        Ok(())
    }

    #[test]
    fn test_all_configs_disabled_error() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let templates_dir = temp_dir.path().join(".vscode-debug/templates");
        let configs_dir = temp_dir.path().join(".vscode-debug/configs");

        fs::create_dir_all(&templates_dir)?;
        fs::create_dir_all(&configs_dir)?;

        // Create template
        let template = json!({
            "type": "cppdbg",
            "program": "${workspaceFolder}/build/myapp"
        });
        write_json(templates_dir.join("cpp.json"), &template)?;

        // Create only disabled config
        let disabled_config = json!({
            "name": "Disabled Config",
            "extends": "cpp",
            "enabled": false,
            "args": ["--disabled"]
        });
        write_json(configs_dir.join("disabled.json"), &disabled_config)?;

        let generator = create_test_generator(&temp_dir);
        let result = generator.generate();

        assert!(result.is_err());
        assert!(
            result
                .unwrap_err()
                .to_string()
                .contains("No enabled configuration files found")
        );

        Ok(())
    }

    #[test]
    fn test_template_with_args_is_error() -> anyhow::Result<()> {
        let temp_dir = TempDir::new()?;
        let templates_dir = temp_dir.path().join(".vscode-debug/templates");
        let configs_dir = temp_dir.path().join(".vscode-debug/configs");

        fs::create_dir_all(&templates_dir)?;
        fs::create_dir_all(&configs_dir)?;

        // Template that wrongly includes args
        let bad_template = json!({
            "type": "cppdbg",
            "program": "${workspaceFolder}/build/myapp",
            "args": ["--should-not-be-here"]
        });
        write_json(templates_dir.join("cpp.json"), &bad_template)?;

        // Minimal config
        let config = json!({
            "name": "Bad",
            "extends": "cpp",
            "enabled": true
        });
        write_json(configs_dir.join("bad.json"), &config)?;

        let generator = create_test_generator(&temp_dir);
        let result = generator.generate();

        assert!(result.is_err());
        assert!(result.is_err());

        Ok(())
    }
}
