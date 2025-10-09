# mklaunch

A simple command-line tool that generates VSCode's debug configuration file (`launch.json`) from modular template and configuration files.

## What is this tool?

This tool helps you manage complex debug configurations for VSCode by separating common debugger settings (templates) from specific execution conditions (configurations). Instead of maintaining a large, unwieldy `launch.json` file, you can organize your debug configurations into small, focused files and automatically generate the complete launch configuration.

### How it works

1. **Templates** (`templates/*.json`): Define common debugger settings like debugger type, executable path, etc.
2. **Configurations** (`configs/*.json`): Define specific execution conditions like command-line arguments, environment variables, etc.
3. **Generation**: The tool merges templates with configurations to create a complete `launch.json` file.

## Command Line Options

```
mklaunch [OPTIONS]

Options:
      --templates <PATH>  Templates directory path [default: .mklaunch/templates]
      --configs <PATH>    Configs directory path [default: .mklaunch/configs]
  -o, --output <PATH>  Output file path for generated launch.json [default: .vscode/launch.json]
  -v, --verbose        Enable verbose output
  -h, --help           Print help
```

## Basic Usage

1. Create the configuration directory structure:
   ```
   .mklaunch/
   ├── templates/
   │   └── cpp.json
   └── configs/
       ├── basic-test.json
       └── input-test.json
   ```

2. Create a template file (`templates/cpp.json`):
   ```json
   {
     "type": "cppdbg",
     "request": "launch",
     "program": "${workspaceFolder}/build/myapp",
     "MIMode": "gdb"
   }
   ```

3. Create baseArgs file (`launch/test1/args.json`):

   ```json
   {
     "args": ["-v", "-o", "output.txt", "input.txt"]
   }
   ```

4. Create configuration files (`configs/basic-test.json`):

   ```json
   [
     {
       "name": "Basic Test",
       "extends": "cpp",
       "enabled": true,
       "baseArgs": "/path/to/args.json",
       "args": ["--debug-mode"]
     }
   ]
   ```

5. Run the tool:
   ```bash
   mklaunch
   ```

6. The tool generates `.vscode/launch.json`:
   ```json
   {
     "version": "0.2.0",
     "configurations": [
       {
         "name": "Basic Test",
         "type": "cppdbg",
         "request": "launch",
         "program": "${workspaceFolder}/build/myapp",
         "MIMode": "gdb",
         "args": [
           "-v",
           "-o",
           "output.txt",
           "input.txt",
           "--debug-mode"
         ]
       }
     ]
   }
   ```

## Examples

### Basic usage

```bash
mklaunch
```

### Custom configuration directory

```bash
mklaunch --templates ./debug-configs/templates --configs ./debug-configs/configs
```

### Custom output path

```bash
mklaunch --output ./custom/.vscode/launch.json
```

### Verbose output

```bash
mklaunch --verbose
```

## Configuration File Format

Every file inside the `configs/` directory must be a **JSON array** of configuration objects. Even if a file only defines a single configuration, it must still be wrapped in an array. Empty arrays are permitted and simply contribute no configurations.

Each configuration object supports the following fields:

- **`name`** *(required)*: Unique configuration name displayed in VSCode.
- **`extends`** *(required)*: Template name to use (without the `.json` suffix).
- **`enabled`** *(required)*: Boolean flag to enable/disable this configuration.
- **`baseArgs`** *(optional)*: Path to a JSON file containing `{ "args": [...] }`. These arguments are prepended.
- **`args`** *(optional)*: Additional arguments appended after `baseArgs`.

Example with multiple configurations in a single file:

```json
[
  {
    "name": "Debug (fast)",
    "extends": "cpp",
    "enabled": true,
    "args": ["--mode", "fast"]
  },
  {
    "name": "Debug (slow)",
    "extends": "cpp",
    "enabled": false,
    "args": ["--mode", "slow"]
  }
]
```

### Enabling/Disabling Configurations

You can temporarily disable configurations by setting `enabled: false` inside the array entry:

```json
[
  {
    "name": "Disabled Test",
    "extends": "cpp",
    "enabled": false,
    "args": ["--test"]
  }
]
```

Disabled entries are ignored during generation and will not appear in the resulting `launch.json`.

This tool is designed to be simple and focused, making it easy to manage multiple debug configurations for your development workflow.
