use crate::{
    api::{CreateRequest, DecodedResponse, EditRequest, Response},
    client::Client,
};
use anyhow::Context;
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{error, info, warn};
use std::time::Duration;

/// A CLI tool for generating and editing images using OpenAI's latest `gpt-image-1`
/// image generation model.
///
/// If --image inputs are provided, the tool will use the image editing API.
/// Otherwise, it will use the image creation API.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// OpenAI API key (can also be set via `OPENAI_API_KEY` environment variable)
    #[arg(short, long, env = "OPENAI_API_KEY", hide_env = true)]
    pub api_key: Option<String>,

    // Embed the unified image generation arguments directly
    #[command(flatten)]
    pub args: GenerateArgs,
}

// Unified arguments struct combining CreateArgs and EditArgs
#[derive(Parser, Debug)]
pub struct GenerateArgs {
    // --- Common Arguments ---
    /// A text description of the desired image(s)
    #[arg(required = true)]
    pub prompt: String,

    /// The number of images to generate (1-10)
    #[arg(short, long, default_value = "1")]
    pub n: u8,

    /// The size of the generated images (one of: 1024x1024, 1536x1024, 1024x1536,
    /// auto, square, landscape, portrait)
    #[arg(long, default_value = "1024x1024")]
    pub size: String,

    /// The quality of the image that will be generated (high, medium, low, auto)
    #[arg(long, default_value = "auto")]
    pub quality: String,

    // --- Edit-Specific Arguments ---
    /// The image(s) to edit (path to image files). Providing this triggers the edit operation.
    #[arg(short, long)]
    pub image: Option<Vec<String>>,

    /// An additional image whose transparent areas indicate where to edit (edit only)
    #[arg(short, long)]
    pub mask: Option<String>,

    // --- Create-Specific Arguments ---
    /// Set transparency for the background (transparent, opaque, auto) (create only)
    #[arg(long, default_value = "opaque")]
    pub background: String,

    /// Control the content-moderation level (low, auto) (create only)
    #[arg(long, default_value = "low")]
    pub moderation: String,

    /// The compression level for generated images (jpeg and webp only) (0-100) (create only)
    #[arg(long, default_value = "100")]
    pub output_compression: u8,

    /// The format of the generated images (png, jpeg, webp) (create only)
    #[arg(long, default_value = "png")]
    pub output_format: String,
}

impl Cli {
    pub fn run(self, progress: &MultiProgress) -> anyhow::Result<()> {
        // Get API key from CLI args or environment
        let api_key = self.api_key.context(
            "Error: API key is required. Provide it with --api-key or set the \
             `OPENAI_API_KEY` environment variable.",
        )?;
        let client = Client::new(api_key);

        // Set up the spinner
        let sp = spinner(progress);
        sp.set_message("Generating image(s)...");

        let result = self.args.run(&client);

        // Update spinner message based on result
        match result {
            Ok(_) => info!("✓ Done"),
            Err(_) => error!("✗ Done"),
        };

        // Clean up the spinner
        sp.finish();
        progress.remove(&sp);

        result
    }
}

impl GenerateArgs {
    /// Run the appropriate image generation or editing command based on args
    fn run(self, client: &Client) -> anyhow::Result<()> {
        // Determine if we're using the edit API or the create API based on the
        // presence of `--image` options
        let result = if let Some(images) = self.image {
            // Warn about create-API-only arguments if they are not default
            if self.background != "opaque" {
                warn!("Ignoring --background option; it is only applicable when generating images without --image inputs.");
            }
            if self.moderation != "low" {
                warn!("Ignoring --moderation option; it is only applicable when generating images without --image inputs.");
            }
            if self.output_compression != 100 {
                warn!("Ignoring --output-compression option; it is only applicable when generating images without --image inputs.");
            }
            if self.output_format != "png" {
                warn!("Ignoring --output-format option; it is only applicable when generating images without --image inputs.");
            }

            // Create the EditRequest
            let req = EditRequest {
                images,
                prompt: self.prompt.clone(),
                mask: self.mask.clone(),
                model: "gpt-image-1".to_string(),
                n: n_canonical(self.n),
                size: size_canonical(self.size.clone()),
                quality: quality_canonical(self.quality.clone()),
            };

            client.edit_images(req)
        } else {
            // Warn about edit-API-only arguments if they are present
            if self.mask.is_some() {
                warn!("Ignoring --mask option; it is only applicable when generating images using --image inputs.");
            }
            // No warning needed for --image itself, as its absence triggers this path.

            // Create the CreateRequest
            let req = CreateRequest {
                model: "gpt-image-1".to_string(),
                prompt: self.prompt.clone(),
                n: n_canonical(self.n),
                size: size_canonical(self.size.clone()),
                quality: quality_canonical(self.quality.clone()),
                background: background_canonical(self.background.clone()),
                moderation: moderation_canonical(self.moderation.clone()),
                output_compression: Some(self.output_compression), // Always send for create
                output_format: Some(self.output_format.clone()), // Always send for create
            };

            client.create_images(req)
        };

        // Handle the response (logging, decoding, saving)
        let response = result?;
        handle_response(response, &self.prompt)
    }
}

/// Handles the common logic after receiving an API response.
fn handle_response(resp: Response, prompt: &str) -> anyhow::Result<()> {
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
        .map(|s| {
            s.chars()
                .filter(|c| c.is_alphanumeric())
                .map(|c| c.to_ascii_lowercase())
                .collect::<String>()
        }) // Sanitize
        .filter(|s| !s.is_empty()) // Remove empty strings resulting from non-alphanumeric words
        .take(5) // Take first 5 words
        .collect::<Vec<_>>()
        .join("_");

    // Handle cases where the prompt might be empty or only contain non-alphanumeric chars
    let safe_prompt_prefix = if prompt_prefix.is_empty() {
        "imgen" // Default if prompt yields no usable prefix
    } else {
        &prompt_prefix
    };

    let file_prefix = safe_prompt_prefix;

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

// --- CLI option canonicalization functions ---

fn n_canonical(n: u8) -> Option<u8> {
    if n == 1 {
        None // API default is 1, so don't send if it's 1
    } else {
        Some(n)
    }
}

fn size_canonical(size: String) -> Option<String> {
    match size.to_lowercase().as_str() {
        "auto" => None, // Let API decide default
        "square" => Some("1024x1024".to_string()),
        "landscape" => Some("1536x1024".to_string()),
        "portrait" => Some("1024x1536".to_string()),
        _ => Some(size), // Pass through custom sizes like "1024x1024"
    }
}

fn quality_canonical(quality: String) -> Option<String> {
    match quality.to_lowercase().as_str() {
        "auto" => None, // Let API decide default
        _ => Some(quality),
    }
}

fn background_canonical(background: String) -> Option<String> {
    match background.to_lowercase().as_str() {
        "auto" => None, // Let API decide default
        _ => Some(background),
    }
}

fn moderation_canonical(moderation: String) -> Option<String> {
    match moderation.to_lowercase().as_str() {
        "auto" => None, // Let API decide default
        _ => Some(moderation),
    }
}
