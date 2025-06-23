use anyhow::Result;
use serde_json::json;
use std::fs;
use std::path::Path;
use tempfile::TempDir;
use vscode_launch_gen::{Generator, LaunchJson};

fn create_test_files(base_dir: &Path) -> Result<()> {
    let templates_dir = base_dir.join(".vscode-debug/templates");
    let configs_dir = base_dir.join(".vscode-debug/configs");
    
    fs::create_dir_all(&templates_dir)?;
    fs::create_dir_all(&configs_dir)?;
    
    // Create cpp template
    let cpp_template = json!({
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
    
    fs::write(
        templates_dir.join("cpp.json"),
        serde_json::to_string_pretty(&cpp_template)?
    )?;
    
    // Create lldb template for macOS
    let lldb_template = json!({
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
        templates_dir.join("lldb.json"),
        serde_json::to_string_pretty(&lldb_template)?
    )?;
    
    // Create config files (will be sorted alphabetically)
    let configs = vec![
        ("01-debug-basic.json", json!({
            "extends": "cpp",
            "name": "Debug Basic",
            "enabled": true,
            "args": []
        })),
        ("02-debug-with-input.json", json!({
            "extends": "cpp",
            "name": "Debug with Input",
            "enabled": true,
            "args": ["--input", "test/data/sample.txt", "--verbose"],
            "cwd": "${workspaceFolder}/test",
            "environment": [
                {"name": "DEBUG_LEVEL", "value": "3"}
            ],
            "console": "integratedTerminal"
        })),
        ("03-benchmark.json", json!({
            "extends": "cpp",
            "name": "Benchmark",
            "enabled": true,
            "args": ["--benchmark", "--iterations", "1000"],
            "environment": [
                {"name": "BENCHMARK_MODE", "value": "1"}
            ]
        })),
        ("04-lldb-debug.json", json!({
            "extends": "lldb",
            "name": "LLDB Debug",
            "enabled": true,
            "args": ["--debug"]
        }))
    ];
    
    for (filename, config) in configs {
        fs::write(
            configs_dir.join(filename),
            serde_json::to_string_pretty(&config)?
        )?;
    }
    
    Ok(())
}

#[test]
fn test_full_generation_process() -> Result<()> {
    let temp_dir = TempDir::new()?;
    create_test_files(temp_dir.path())?;
    
    let generator = Generator::new(
        temp_dir.path().join(".vscode-debug"),
        temp_dir.path().join(".vscode/launch.json")
    );
    
    generator.generate()?;
    
    let output_path = temp_dir.path().join(".vscode/launch.json");
    assert!(output_path.exists());
    
    let content = fs::read_to_string(output_path)?;
    let launch_json: LaunchJson = serde_json::from_str(&content)?;
    
    assert_eq!(launch_json.version, "0.2.0");
    assert_eq!(launch_json.configurations.len(), 4);
    
    // Check configurations are in alphabetical order by filename
    let names: Vec<&str> = launch_json.configurations.iter()
        .map(|c| c["name"].as_str().unwrap())
        .collect();
    
    println!("Generated names: {:?}", names);
    assert_eq!(names, vec!["Debug Basic", "Debug with Input", "Benchmark", "LLDB Debug"]);
    
    // Check first configuration (Debug Basic - from 01-debug-basic.json)
    let basic_config = &launch_json.configurations[0];
    assert_eq!(basic_config["name"], "Debug Basic");
    assert_eq!(basic_config["args"], json!([]));
    assert_eq!(basic_config["cwd"], "${workspaceFolder}"); // From template
    
    // Check second configuration (Debug with Input - from 02-debug-with-input.json)
    let input_config = &launch_json.configurations[1];
    assert_eq!(input_config["name"], "Debug with Input");
    assert_eq!(input_config["cwd"], "${workspaceFolder}/test"); // Overridden
    assert_eq!(input_config["console"], "integratedTerminal");
    assert_eq!(input_config["environment"], json!([{"name": "DEBUG_LEVEL", "value": "3"}]));
    
    // Check third configuration (Benchmark - from 03-benchmark.json)
    let benchmark_config = &launch_json.configurations[2];
    assert_eq!(benchmark_config["name"], "Benchmark");
    assert_eq!(benchmark_config["type"], "cppdbg");
    assert_eq!(benchmark_config["args"], json!(["--benchmark", "--iterations", "1000"]));
    assert_eq!(benchmark_config["environment"], json!([{"name": "BENCHMARK_MODE", "value": "1"}]));
    
    // Check fourth configuration (LLDB Debug - from 04-lldb-debug.json)
    let lldb_config = &launch_json.configurations[3];
    assert_eq!(lldb_config["name"], "LLDB Debug");
    assert_eq!(lldb_config["type"], "lldb"); // Different template
    assert!(lldb_config.get("MIMode").is_none()); // LLDB template doesn't have MIMode
    
    Ok(())
}


#[test]
fn test_error_missing_template() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    let configs_dir = temp_dir.path().join(".vscode-debug/configs");
    fs::create_dir_all(&configs_dir)?;
    
    // Create config that references non-existent template
    let config = json!({
        "extends": "nonexistent",
        "name": "Test"
    });
    
    fs::write(
        configs_dir.join("test.json"),
        serde_json::to_string_pretty(&config)?
    )?;
    
    let generator = Generator::new(
        temp_dir.path().join(".vscode-debug"),
        temp_dir.path().join(".vscode/launch.json")
    );
    
    let result = generator.generate();
    assert!(result.is_err());
    let error_msg = result.unwrap_err().to_string();
    println!("Error message: {}", error_msg);
    assert!(error_msg.contains("Templates directory does not exist") || error_msg.contains("not found"));
    
    Ok(())
}

#[test]
fn test_error_duplicate_names() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    let templates_dir = temp_dir.path().join(".vscode-debug/templates");
    let configs_dir = temp_dir.path().join(".vscode-debug/configs");
    fs::create_dir_all(&templates_dir)?;
    fs::create_dir_all(&configs_dir)?;
    
    // Create template
    let template = json!({"type": "cppdbg"});
    fs::write(
        templates_dir.join("cpp.json"),
        serde_json::to_string_pretty(&template)?
    )?;
    
    // Create two configs with same name
    let config1 = json!({
        "extends": "cpp",
        "name": "Duplicate Name",
        "enabled": true
    });
    
    let config2 = json!({
        "extends": "cpp", 
        "name": "Duplicate Name",
        "enabled": true
    });
    
    fs::write(
        configs_dir.join("config1.json"),
        serde_json::to_string_pretty(&config1)?
    )?;
    
    fs::write(
        configs_dir.join("config2.json"),
        serde_json::to_string_pretty(&config2)?
    )?;
    
    let generator = Generator::new(
        temp_dir.path().join(".vscode-debug"),
        temp_dir.path().join(".vscode/launch.json")
    );
    
    let result = generator.generate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Duplicate configuration name"));
    
    Ok(())
}

#[test]
fn test_error_invalid_extends() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    let configs_dir = temp_dir.path().join(".vscode-debug/configs");
    fs::create_dir_all(&configs_dir)?;
    
    // Create config with invalid extends path
    let config = json!({
        "extends": "../other/template",
        "name": "Invalid Test",
        "enabled": true
    });
    
    fs::write(
        configs_dir.join("invalid.json"),
        serde_json::to_string_pretty(&config)?
    )?;
    
    let generator = Generator::new(
        temp_dir.path().join(".vscode-debug"),
        temp_dir.path().join(".vscode/launch.json")
    );
    
    let config_path = configs_dir.join("invalid.json");
    let result = generator.load_config(&config_path);
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("Invalid extends value"));
    
    Ok(())
}

#[test]
fn test_empty_configs_directory() -> Result<()> {
    let temp_dir = TempDir::new()?;
    
    let templates_dir = temp_dir.path().join(".vscode-debug/templates");
    let configs_dir = temp_dir.path().join(".vscode-debug/configs");
    fs::create_dir_all(&templates_dir)?;
    fs::create_dir_all(&configs_dir)?;
    
    // Create template but no configs
    let template = json!({"type": "cppdbg"});
    fs::write(
        templates_dir.join("cpp.json"),
        serde_json::to_string_pretty(&template)?
    )?;
    
    let generator = Generator::new(
        temp_dir.path().join(".vscode-debug"),
        temp_dir.path().join(".vscode/launch.json")
    );
    
    let result = generator.generate();
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("No configuration files found"));
    
    Ok(())
}