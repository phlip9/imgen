#![allow(dead_code)]

mod api;
mod cli;
mod client;

use clap::Parser;
use cli::Cli;
use log::error;

fn main() {
    // Load environment variables from .env file if present
    dotenvy::dotenv().ok();

    // Initialize the logger
    env_logger::init();

    // Parse command line arguments
    let cli = Cli::parse();

    // Run the CLI application
    if let Err(err) = cli.run() {
        error!("{}", err);
        std::process::exit(1);
    }
}
