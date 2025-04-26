use crate::{
    api::{CreateRequest, DecodedResponse, EditRequest, Response},
    client::Client,
};
use anyhow::Context;
use clap::{Parser, Subcommand};
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::info;
use std::time::Duration;

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
    pub fn run(self, progress: &MultiProgress) -> anyhow::Result<()> {
        // Get API key from CLI args or environment
        let api_key = self.api_key.context(
            "Error: API key is required. Provide it with --api-key or set the \
             `OPENAI_API_KEY` environment variable.",
        )?;

        self.command.run(progress, &Client::new(api_key))
    }
}

impl Commands {
    fn run(
        self,
        progress: &MultiProgress,
        client: &Client,
    ) -> anyhow::Result<()> {
        match self {
            Self::Create(args) => args.run(progress, client),
            Self::Edit(args) => args.run(progress, client),
        }
    }
}

impl CreateArgs {
    /// Run the create image command
    fn run(
        self,
        progress: &MultiProgress,
        client: &Client,
    ) -> anyhow::Result<()> {
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

        // Set up the spinner
        let sp = spinner(progress);
        sp.set_message("Generating image...");

        // Call the image generation API
        let result = client.create_images(req);

        // Update spinner message based on result
        let msg = match result {
            Ok(_) => "✓ Image generation complete.",
            Err(_) => "✗ Image generation failed.",
        };
        sp.finish_with_message(msg);
        progress.remove(&sp);

        // Handle the response (logging, decoding, saving)
        let resp = result?;
        handle_response(resp, &self.prompt, "create")
    }
}

impl EditArgs {
    /// Run the edit image command
    fn run(
        self,
        progress: &MultiProgress,
        client: &Client,
    ) -> anyhow::Result<()> {
        info!("Editing image(s) with prompt: {}", self.prompt);
        info!("Input image(s): {:?}", self.image);
        if let Some(mask) = &self.mask {
            info!("Using mask: {}", mask);
        }

        // Create the request
        let req = EditRequest {
            images: self.image,
            prompt: self.prompt.clone(),
            mask: self.mask,
            model: "gpt-image-1".to_string(),
            n: if self.n == 1 { None } else { Some(self.n) },
            quality: if self.quality == "auto" {
                None
            } else {
                Some(self.quality)
            },
            size: if self.size == "1024x1024" {
                None
            } else {
                Some(self.size)
            },
        };

        // Set up the spinner
        let sp = spinner(progress);
        sp.set_message("Editing image...");

        // Call the image generation API
        let result = client.edit_images(req);

        // Update spinner message based on result
        let msg = match result {
            Ok(_) => "✓ Image editing complete.",
            Err(_) => "✗ Image editing failed.",
        };
        sp.finish_with_message(msg);
        progress.remove(&sp);

        // Handle the response (logging, decoding, saving)
        let resp = result?;
        handle_response(resp, &self.prompt, "edit")
    }
}

/// Handles the common logic after receiving an API response.
fn handle_response(
    resp: Response,
    prompt: &str,
    operation_prefix: &str,
) -> anyhow::Result<()> {
    info!("Operation completed at: {}", resp.created);
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
    let prompt_prefix = prompt
        .split_whitespace()
        .take(5)
        .collect::<Vec<_>>()
        .join("_");
    let file_prefix = format!("{}_{}", operation_prefix, prompt_prefix);

    // Save the images to files
    let saved_files = decoded_resp
        .save_images(&file_prefix)
        .context("Failed to save images to files")?;

    info!("Saved images to: {:?}", saved_files);

    Ok(())
}

/// Create a new "dots" spinner to indicate progress while waiting for the API
/// response.
///
/// For more spinners check out: <https://github.com/sindresorhus/cli-spinners/blob/main/spinners.json>
fn spinner(progress: &MultiProgress) -> ProgressBar {
    let pb = progress.add(ProgressBar::new_spinner());
    pb.enable_steady_tick(Duration::from_millis(80));
    pb.set_style(
        ProgressStyle::with_template("{spinner:.blue} {msg}")
            .unwrap()
            .tick_strings(&["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]),
    );
    pb
}
