use anyhow::Result;
use clap::Parser;
use std::path::PathBuf;
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

    let generator = Generator::new(cli.dir, cli.output);

    match generator.generate() {
        Ok(()) => {
            if cli.verbose {
                let config_count = generator.collect_config_files()?.len();
                println!("Generated launch.json with {} configurations", config_count);
            }
            Ok(())
        }
        Err(e) => Err(e),
    }
}
