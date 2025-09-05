# vscode-launch-gen

A simple command-line tool that generates VSCode's debug configuration file (`launch.json`) from modular template and configuration files.

## What is this tool?

This tool helps you manage complex debug configurations for VSCode by separating common debugger settings (templates) from specific execution conditions (configurations). Instead of maintaining a large, unwieldy `launch.json` file, you can organize your debug configurations into small, focused files and automatically generate the complete launch configuration.

### How it works

1. **Templates** (`templates/*.json`): Define common debugger settings like debugger type, executable path, etc.
2. **Configurations** (`configs/*.json`): Define specific execution conditions like command-line arguments, environment variables, etc.
3. **Generation**: The tool merges templates with configurations to create a complete `launch.json` file.

## Command Line Options

```
vscode-launch-gen [OPTIONS]

Options:
  -d, --dir <PATH>     Configuration directory path [default: .vscode-debug]
  -o, --output <PATH>  Output file path for generated launch.json [default: .vscode/launch.json]
  -v, --verbose        Enable verbose output
  -h, --help           Print help
```

## Basic Usage

1. Create the configuration directory structure:
   ```
   .vscode-debug/
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
   {
     "name": "Basic Test",
     "extends": "cpp",
     "enabled": true,
     "baseArgs": "/path/to/args.json",
     "args": ["--debug-mode"]
   }
   ```

5. Run the tool:
   ```bash
   vscode-launch-gen
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
         "args": "args": [
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
vscode-launch-gen
```

### Custom configuration directory

```bash
vscode-launch-gen --dir ./debug-configs
```

### Custom output path

```bash
vscode-launch-gen --output ./custom/.vscode/launch.json
```

### Verbose output

```bash
vscode-launch-gen --verbose
```

## Configuration File Format

Each configuration file in the `configs/` directory must include the following required fields:

- **`name`**: Unique configuration name displayed in VSCode
- **`extends`**: Template name to use (without .json extension)
- **`enabled`**: Boolean flag to enable/disable this configuration
- **`config`**: Object containing debug configuration properties that override template values
  - Additional debug properties like `args`, `cwd`, `environment`, etc.

### Enabling/Disabling Configurations

You can temporarily disable configurations by setting `enabled: false`:

```json
{
  "name": "Disabled Test",
  "extends": "cpp",
  "enabled": false,
  "config": {
    "args": ["--test"]
  }
}
```

Disabled configurations will be ignored during generation and won't appear in the final `launch.json` file.

This tool is designed to be simple and focused, making it easy to manage multiple debug configurations for your development workflow.
