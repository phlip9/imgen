#![allow(dead_code)]

mod api;
mod cli;

use clap::Parser;
use cli::{Cli, Commands};

fn main() {
    // Load environment variables from .env file if present
    dotenvy::dotenv().ok();

    // Parse command line arguments
    let cli = Cli::parse();

    match cli.command {
        Commands::Create(args) => {
            println!("Creating image with prompt: {}", args.prompt);
            // TODO: Implement image creation
        }
        Commands::Edit(args) => {
            println!("Editing image with prompt: {}", args.prompt);
            println!("Using images: {:?}", args.image);
            // TODO: Implement image editing
        }
    }
}
