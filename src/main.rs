#![allow(dead_code)]

mod api;
mod cli;
mod client;
mod multipart;

use clap::Parser;
use cli::Cli;
use log::error;

fn main() {
    // Load environment variables from .env file if present
    dotenvy::dotenv().ok();

    // Build the logger.
    let env_logger = env_logger::Builder::from_env(
        env_logger::Env::default().default_filter_or("info"),
    )
    .build();
    // Wrap the logger so log messages and progress bars don't interfere with
    // each other.
    let progress = indicatif::MultiProgress::new();
    indicatif_log_bridge::LogWrapper::new(progress.clone(), env_logger)
        .try_init()
        .unwrap();

    // Parse command line arguments
    let cli = Cli::parse();

    // Run the CLI application
    if let Err(err) = cli.run(&progress) {
        error!("{}", err);
        std::process::exit(1);
    }
}
