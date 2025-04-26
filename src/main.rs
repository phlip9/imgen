#![allow(dead_code)]

mod api;
mod cli;
mod client;

use api::CreateRequest;
use clap::Parser;
use cli::{Cli, Commands};
use client::Client;
use std::process;

fn main() {
    // Load environment variables from .env file if present
    dotenvy::dotenv().ok();

    // Parse command line arguments
    let cli = Cli::parse();

    // Get API key from CLI args or environment
    let api_key = match cli.api_key {
        Some(key) => key,
        None => {
            eprintln!("Error: API key is required. Provide it with --api-key or set the `OPENAI_API_KEY` environment variable.");
            process::exit(1);
        }
    };

    // Create the OpenAI client
    let client = Client::new(api_key);

    match cli.command {
        Commands::Create(args) => {
            eprintln!("Creating image with prompt: {}", args.prompt);

            // Create the request
            let request = CreateRequest {
                model: "gpt-image-1".to_string(),
                prompt: args.prompt,
                n: if args.n == 1 { None } else { Some(args.n) },
                size: if args.size == "1024x1024" {
                    None
                } else {
                    Some(args.size)
                },
                quality: if args.quality == "auto" {
                    None
                } else {
                    Some(args.quality)
                },
                background: if args.background == "auto" {
                    None
                } else {
                    Some(args.background)
                },
                moderation: if args.moderation == "auto" {
                    None
                } else {
                    Some(args.moderation)
                },
                output_compression: Some(args.output_compression),
                output_format: Some(args.output_format),
            };

            // Make the API request
            match client.create_image(request) {
                Ok(response) => {
                    eprintln!("Image created at: {}", response.created);
                    eprintln!("Generated {} image(s)", response.data.len());

                    // TODO: Save the images to files

                    eprintln!(
                        "Token usage: {} total tokens",
                        response.usage.total_tokens
                    );
                }
                Err(err) => {
                    eprintln!("Error creating image: {}", err);
                    process::exit(1);
                }
            }
        }
        Commands::Edit(args) => {
            println!("Editing image with prompt: {}", args.prompt);
            println!("Using images: {:?}", args.image);
            // TODO: Implement image editing
        }
    }
}
