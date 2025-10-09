use anyhow::Result;
use mklaunch::Generator;
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use tempfile::TempDir;

fn create_test_files(base_dir: &Path) -> Result<()> {
    let base = base_dir.join(".mklaunch");
    let templates_manifest = base.join("templates.json");
    let configs_dir = base.join("configs");

    fs::create_dir_all(&base)?;
    fs::create_dir_all(&configs_dir)?;

    // Create cpp template
    let cpp_template = json!({
        "name": "cpp",
        "type": "cppdbg",
        "request": "launch",
        "program": "${workspaceFolder}/build/bin/myapp",
        "stopAtEntry": false,
        "cwd": "${workspaceFolder}",
        "environment": [],
        "externalConsole": false,
        "MIMode": "gdb",
        "miDebuggerPath": "/usr/bin/gdb",
        "setupCommands": [
            {
                "description": "Enable pretty-printing for gdb",
                "text": "-enable-pretty-printing",
                "ignoreFailures": true
            }
        ],
        "preLaunchTask": "build"
    });

    // Create lldb template for macOS
    let lldb_template = json!({
        "name": "lldb",
        "type": "lldb",
        "request": "launch",
        "program": "${workspaceFolder}/build/bin/myapp",
        "stopAtEntry": false,
        "cwd": "${workspaceFolder}",
        "environment": [],
        "externalConsole": false,
        "preLaunchTask": "build"
    });

    fs::write(
        &templates_manifest,
        serde_json::to_string_pretty(&json!({
            "templates": [
                cpp_template,
                lldb_template
            ]
        }))?,
    )?;
    // Prepare a baseArgs file
    let base_args_path = base_dir.join("baseargs.json");
    fs::write(
        &base_args_path,
        serde_json::to_string_pretty(&json!({
            "args": ["input.json", "-o", "output.json"]
        }))?,
    )?;

    // Create config files (new schema, sorted alphabetically)
    let configs = vec![
        (
            "01-debug-basic.json",
            json!([
                {
                    "name": "Debug Basic",
                    "extends": "cpp",
                    "enabled": true,
                    "args": []
                }
            ]),
        ),
        (
            "02-debug-with-input.json",
            json!([
                {
                    "name": "Debug with Input",
                    "extends": "cpp",
                    "enabled": true,
                    "baseArgs": base_args_path.to_string_lossy(),
                    "args": ["--verbose"]
                }
            ]),
        ),
        (
            "03-benchmark.json",
            json!([
                {
                    "name": "Benchmark",
                    "extends": "cpp",
                    "enabled": true,
                    "args": ["--benchmark", "--iterations", "1000"]
                }
            ]),
        ),
        (
            "04-lldb-debug.json",
            json!([
                {
                    "name": "LLDB Debug",
                    "extends": "lldb",
                    "enabled": true,
                    "args": ["--debug"]
                }
            ]),
        ),
    ];

    for (filename, config) in configs {
        fs::write(
            configs_dir.join(filename),
            serde_json::to_string_pretty(&config)?,
        )?;
    }

    Ok(())
}

// Test helpers to reduce duplication across cases
fn create_dirs(base_dir: &Path) -> Result<(PathBuf, PathBuf)> {
    let base = base_dir.join(".mklaunch");
    let templates_manifest = base.join("templates.json");
    let configs_dir = base.join("configs");
    fs::create_dir_all(&base)?;
    fs::create_dir_all(&configs_dir)?;
    Ok((templates_manifest, configs_dir))
}

fn write_json<P: AsRef<Path>>(path: P, value: &serde_json::Value) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)?)?;
    Ok(())
}

#[test]
fn test_full_generation_process() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_files(temp_dir.path())?;

    let base = temp_dir.path().join(".mklaunch");
    let generator = Generator::new(base.join("templates.json"), base.join("configs"));

    let launch = generator.generate()?;
    let v: serde_json::Value = serde_json::to_value(&launch)?;
    assert_eq!(v["version"], "0.2.0");
    let configurations = v["configurations"].as_array().unwrap();
    assert_eq!(configurations.len(), 4);

    // Check configurations are in alphabetical order by configuration name
    let names: Vec<&str> = configurations
        .iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();

    println!("Generated names: {:?}", names);
    assert_eq!(
        names,
        vec!["Benchmark", "Debug Basic", "Debug with Input", "LLDB Debug"]
    );

    // Helper to find config by name for assertions below
    let find_by_name = |n: &str| -> &serde_json::Value {
        configurations
            .iter()
            .find(|c| c["name"].as_str().unwrap() == n)
            .unwrap()
    };

    // Check Debug Basic
    let basic_config = find_by_name("Debug Basic");
    assert_eq!(basic_config["args"], json!([]));
    assert_eq!(basic_config["cwd"], "${workspaceFolder}"); // From template

    // Check Debug with Input (baseArgs + args)
    let input_config = find_by_name("Debug with Input");
    assert_eq!(
        input_config["args"],
        json!(["input.json", "-o", "output.json", "--verbose"])
    );

    // Check Benchmark
    let benchmark_config = find_by_name("Benchmark");
    assert_eq!(benchmark_config["type"], "cppdbg");
    assert_eq!(
        benchmark_config["args"],
        json!(["--benchmark", "--iterations", "1000"])
    );

    // Check LLDB Debug
    let lldb_config = find_by_name("LLDB Debug");
    assert_eq!(lldb_config["type"], "lldb"); // Different template
    assert!(lldb_config.get("MIMode").is_none()); // LLDB template doesn't have MIMode

    Ok(())
}

#[test]
fn test_error_missing_template() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let configs_dir = temp_dir.path().join(".mklaunch/configs");
    fs::create_dir_all(&configs_dir)?;

    // Create config that references non-existent template (new schema)
    let config = json!([
        {
            "name": "Test",
            "extends": "nonexistent",
            "enabled": true
        }
    ]);

    write_json(configs_dir.join("test.json"), &config)?;

    let base = temp_dir.path().join(".mklaunch");
    let generator = Generator::new(base.join("templates.json"), base.join("configs"));

    let result = generator.generate();
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    println!("Error message: {}", error_msg);
    assert!(
        error_msg.contains("Templates manifest does not exist") || error_msg.contains("not found")
    );

    Ok(())
}

#[test]
fn test_error_duplicate_names() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let (templates_manifest, configs_dir) = create_dirs(temp_dir.path())?;

    // Create template
    let template = json!({
        "name": "cpp",
        "type": "cppdbg"
    });
    write_json(&templates_manifest, &json!({ "templates": [template] }))?;

    // Create two configs with same name
    let config1 = json!([
        {
            "name": "Duplicate Name",
            "extends": "cpp",
            "enabled": true
        }
    ]);

    let config2 = json!([
        {
            "name": "Duplicate Name",
            "extends": "cpp",
            "enabled": true
        }
    ]);

    write_json(configs_dir.join("config1.json"), &config1)?;
    write_json(configs_dir.join("config2.json"), &config2)?;

    let base = temp_dir.path().join(".mklaunch");
    let generator = Generator::new(base.join("templates.json"), base.join("configs"));

    let result = generator.generate();
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
fn test_multiple_configs_in_single_file() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let (templates_manifest, configs_dir) = create_dirs(temp_dir.path())?;

    // Create template used by all entries
    let template = json!({
        "name": "cpp",
        "type": "cppdbg",
        "program": "${workspaceFolder}/bin/app"
    });
    write_json(&templates_manifest, &json!({ "templates": [template] }))?;

    // Single file providing two enabled configurations
    let multi_config = json!([
        {
            "name": "First Multi",
            "extends": "cpp",
            "enabled": true,
            "args": ["--first"]
        },
        {
            "name": "Second Multi",
            "extends": "cpp",
            "enabled": true,
            "args": ["--second"]
        }
    ]);
    write_json(configs_dir.join("multi.json"), &multi_config)?;

    let base = temp_dir.path().join(".mklaunch");
    let generator = Generator::new(base.join("templates.json"), base.join("configs"));

    let launch = generator.generate()?;
    let v: serde_json::Value = serde_json::to_value(&launch)?;
    let configs = v["configurations"].as_array().unwrap();
    assert_eq!(configs.len(), 2);

    let names: Vec<&str> = configs
        .iter()
        .map(|cfg| cfg["name"].as_str().unwrap())
        .collect();
    assert_eq!(names, vec!["First Multi", "Second Multi"]);

    let first = configs
        .iter()
        .find(|cfg| cfg["name"] == "First Multi")
        .unwrap();
    assert_eq!(first["args"], json!(["--first"]));

    let second = configs
        .iter()
        .find(|cfg| cfg["name"] == "Second Multi")
        .unwrap();
    assert_eq!(second["args"], json!(["--second"]));

    Ok(())
}

#[test]
fn test_error_invalid_extends() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let (_, configs_dir) = create_dirs(temp_dir.path())?;

    // Create config with invalid extends path
    let config = json!([
        {
            "name": "Invalid Test",
            "extends": "../other/template",
            "enabled": true
        }
    ]);

    write_json(configs_dir.join("invalid.json"), &config)?;

    let config_path = configs_dir.join("invalid.json");
    let result = mklaunch::ConfigFile::from_path(&config_path);

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
fn test_empty_configs_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    let (templates_manifest, _configs_dir) = create_dirs(temp_dir.path())?;

    // Create template but no configs
    let template = json!({
        "name": "cpp",
        "type": "cppdbg"
    });
    write_json(&templates_manifest, &json!({ "templates": [template] }))?;

    let base = temp_dir.path().join(".mklaunch");
    let generator = Generator::new(base.join("templates.json"), base.join("configs"));

    let result = generator.generate();
    assert!(result.is_err());
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("No configuration entries found")
    );

    Ok(())
}
