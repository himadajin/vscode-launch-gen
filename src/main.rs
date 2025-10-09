use anyhow::Result;
use clap::Parser;
use mklaunch::Generator;
use serde_json::to_string_pretty;
use std::fs;
use std::io::Write;
use std::path::PathBuf;

/// Command line interface for VSCode launch.json generator
#[derive(Parser)]
#[command(name = "mklaunch")]
#[command(about = "Generate VSCode launch.json from template and config files")]
struct Cli {
    /// Templates directory path
    #[arg(long, default_value = ".mklaunch/templates")]
    templates: PathBuf,

    /// Configs directory path
    #[arg(long, default_value = ".mklaunch/configs")]
    configs: PathBuf,

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

    let generator = Generator::new(cli.templates, cli.configs);

    let launch = generator.generate()?;

    // Ensure output directory exists and write file
    if let Some(parent) = cli.output.parent() {
        fs::create_dir_all(parent)?;
    }
    let mut f = fs::File::create(&cli.output)?;
    f.write_all(to_string_pretty(&launch)?.as_bytes())?;

    if cli.verbose {
        println!(
            "Generated launch.json with {} configurations",
            launch.configurations().len()
        );
    }

    Ok(())
}
