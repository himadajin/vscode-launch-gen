use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
use std::fs;
use std::io::Write;
use serde_json::to_string_pretty;
use vscode_launch_gen::Generator;

/// Command line interface for VSCode launch.json generator
#[derive(Parser)]
#[command(name = "vscode-launch-gen")]
#[command(about = "Generate VSCode launch.json from template and config files")]
struct Cli {
    /// Configuration directory path containing templates and configs
    #[arg(short, long, default_value = ".vscode-debug")]
    dir: PathBuf,

    /// Output file path for generated launch.json
    #[arg(short, long, default_value = ".vscode/launch.json")]
    output: PathBuf,

    /// Enable verbose output
    #[arg(short, long)]
    verbose: bool,
}

/// Main entry point - parses CLI arguments and generates launch.json
fn main() -> Result<()> {
    let cli = Cli::parse();

    let output_path = cli.output.clone();
    let generator = Generator::new(cli.dir, output_path.clone());

    let launch = generator.generate()?;

    // Ensure output directory exists and write file
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::File::create(&output_path)?;
    f.write_all(to_string_pretty(&launch)?.as_bytes())?;

    if cli.verbose {
        println!(
            "Generated launch.json with {} configurations",
            launch.configurations().len()
        );
    }

    Ok(())
}
