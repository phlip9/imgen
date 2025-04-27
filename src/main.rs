mod api;
mod cli;
mod client;
mod config;
mod multipart;

use clap::Parser;
use cli::Cli;
use log::error;

fn main() {
    // Load environment variables from .env file if present
    let _ = dotenvy::dotenv();

    // Parse command line arguments
    let cli = Cli::parse();

    // Build the stderr logger.
    let env_logger = env_logger::Builder::new()
        .filter_level(cli.verbose.log_level_filter())
        .format_file(false)
        .format_target(false)
        .format_timestamp(None)
        .build();

    // Wrap the logger so log messages and progress bars don't interfere with
    // each other.
    let progress = indicatif::MultiProgress::new();
    indicatif_log_bridge::LogWrapper::new(progress.clone(), env_logger)
        .try_init()
        .unwrap();

    // Run the CLI application
    if let Err(err) = cli.run(&progress) {
        error!("{}", err);
        std::process::exit(1);
    }
}
