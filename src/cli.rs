use crate::{
    api::{CreateRequest, DecodedResponse},
    client::Client,
};
use anyhow::Context;
use clap::{Parser, Subcommand};
use log::info;

/// A CLI tool for generating and editing images using OpenAI's latest `gpt-image-1`
/// image generation model.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// OpenAI API key (can also be set via OPENAI_API_KEY environment variable)
    #[arg(short, long, env = "OPENAI_API_KEY", hide_env = true)]
    pub api_key: Option<String>,

    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Create an image given a prompt using gpt-image-1
    Create(CreateArgs),

    /// Create an edited or extended image given one or more source images and a prompt using gpt-image-1
    Edit(EditArgs),
}

#[derive(Parser, Debug)]
pub struct CreateArgs {
    /// A text description of the desired image(s)
    #[arg(required = true)]
    pub prompt: String,

    /// The number of images to generate (1-10)
    #[arg(short, default_value = "1")]
    pub n: u8,

    /// The size of the generated images
    #[arg(long, default_value = "1024x1024")]
    pub size: String,

    /// The quality of the image that will be generated (high, medium, low)
    #[arg(long, default_value = "high")]
    pub quality: String,

    /// Set transparency for the background (transparent, opaque, auto)
    #[arg(long, default_value = "opaque")]
    pub background: String,

    /// Control the content-moderation level (low, auto)
    #[arg(long, default_value = "low")]
    pub moderation: String,

    /// The compression level for generated images (jpeg and webp only) (0-100)
    #[arg(long, default_value = "100")]
    pub output_compression: u8,

    /// The format of the generated images (png, jpeg, webp)
    #[arg(long, default_value = "png")]
    pub output_format: String,
}

#[derive(Parser, Debug)]
pub struct EditArgs {
    /// A text description of the desired image(s)
    #[arg(required = true)]
    pub prompt: String,

    /// The image(s) to edit (path to image files)
    #[arg(short, long, required = true)]
    pub image: Vec<String>,

    /// An additional image whose transparent areas indicate where to edit
    #[arg(short, long)]
    pub mask: Option<String>,

    /// The number of images to generate (1-10)
    #[arg(short, long, default_value = "1")]
    pub n: u8,

    /// The quality of the image that will be generated (high, medium, low)
    #[arg(long, default_value = "low")]
    pub quality: String,

    /// The size of the generated images (1024x1024, 1536x1024, 1024x1536, auto)
    #[arg(long, default_value = "1024x1024")]
    pub size: String,
}

impl Cli {
    pub fn run(self) -> anyhow::Result<()> {
        // Get API key from CLI args or environment
        let api_key = self.api_key.context(
            "Error: API key is required. Provide it with --api-key or set the \
             `OPENAI_API_KEY` environment variable.",
        )?;

        self.command.run(&Client::new(api_key))
    }
}

impl Commands {
    fn run(self, client: &Client) -> anyhow::Result<()> {
        match self {
            Self::Create(args) => args.run(client),
            Self::Edit(args) => args.run(client),
        }
    }
}

impl CreateArgs {
    /// Run the create image command
    fn run(self, client: &Client) -> anyhow::Result<()> {
        info!("Creating image with prompt: {}", self.prompt);

        // Create the request
        let req = CreateRequest {
            model: "gpt-image-1".to_string(),
            prompt: self.prompt.clone(),
            n: if self.n == 1 { None } else { Some(self.n) },
            size: if self.size == "1024x1024" {
                None
            } else {
                Some(self.size)
            },
            quality: if self.quality == "auto" {
                None
            } else {
                Some(self.quality)
            },
            background: if self.background == "auto" {
                None
            } else {
                Some(self.background)
            },
            moderation: if self.moderation == "auto" {
                None
            } else {
                Some(self.moderation)
            },
            output_compression: Some(self.output_compression),
            output_format: Some(self.output_format),
        };

        // Make the API request
        let resp = client.create_images(req)?;

        info!("Image created at: {}", resp.created);
        info!("Generated {} image(s)", resp.data.len());

        // Calculate and display cost information
        let cost = resp.usage.calculate_cost();
        info!(
            "Token usage: {} total tokens ({} input, {} output)",
            resp.usage.total_tokens,
            resp.usage.input_tokens,
            resp.usage.output_tokens
        );
        info!("Estimated cost: ${:.2}", cost);

        // Decode the images from base64
        let decoded_resp = DecodedResponse::try_from(resp)
            .context("Failed to decode base64 image data")?;

        // Create a sanitized prefix from the prompt (first few words)
        let prefix = self
            .prompt
            .split_whitespace()
            .take(5)
            .collect::<Vec<_>>()
            .join("_");

        // Save the images to files
        decoded_resp
            .save_images(&prefix)
            .context("Failed to save images to files")?;

        Ok(())
    }
}

impl EditArgs {
    /// Run the edit image command
    fn run(self, _client: &Client) -> anyhow::Result<()> {
        unimplemented!()
    }
}
