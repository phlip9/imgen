#![allow(dead_code)]

mod api;
mod cli;
mod client;

use clap::Parser;
use cli::Cli;

fn main() {
    // Load environment variables from .env file if present
    dotenvy::dotenv().ok();

    // Parse command line arguments
    let cli = Cli::parse();

    // Run the CLI application
    if let Err(err) = cli.run() {
        eprintln!("Error: {}", err);
        std::process::exit(1);
    }
}
