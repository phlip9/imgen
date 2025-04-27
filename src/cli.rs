use crate::{
    api::{CreateRequest, DecodedResponse, EditRequest, Response},
    cli::spinner::Spinner,
    client::Client,
    config::Config,
};
use anyhow::Context;
use clap::Parser;
use clap_verbosity_flag::{InfoLevel, Verbosity};
use indicatif::MultiProgress;
use log::{error, info, warn};

pub mod input;
mod spinner;

// Default values for CLI options
const DEFAULT_BACKGROUND: &str = "opaque";
const DEFAULT_MODERATION: &str = "low";
const DEFAULT_NUM_IMAGES: u8 = 1;
const DEFAULT_OUTPUT_COMPRESSION: u8 = 100;
const DEFAULT_OUTPUT_FORMAT: &str = "png";
const DEFAULT_QUALITY: &str = "auto";
const DEFAULT_SIZE: &str = "1024x1024";

/// A CLI tool for generating and editing images using OpenAI's `gpt-image-1`
/// image generation model.
///
/// If --image inputs are provided, the tool will use the image editing API.
/// Otherwise, it will use the image creation API.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// OpenAI API key (can also be set via `OPENAI_API_KEY` environment variable)
    #[arg(short = 'k', long, env = "OPENAI_API_KEY", hide_env = true)]
    pub openai_api_key: Option<String>,

    /// Store the `--openai-api-key` in the config file and exit.
    #[arg(long, default_value_t = false)]
    pub setup: bool,

    // Embed the unified image generation arguments directly
    #[command(flatten)]
    pub args: GenerateArgs,

    // Parse --verbose and --quiet flags. Default to INFO log level.
    #[command(flatten)]
    pub verbose: Verbosity<InfoLevel>,
}

// Unified arguments struct combining CreateArgs and EditArgs
#[derive(Parser, Debug)]
pub struct GenerateArgs {
    // --- Common Arguments ---
    /// A text description of the desired image(s).
    ///
    /// Can be a literal string, a path to a text file (if the path exists),
    /// or '-' to read from stdin. Use '@<path>' to force interpretation as a
    /// file path.
    #[arg(required_unless_present("setup"))] // Required if not doing setup
    pub prompt: Option<input::Prompt>,

    /// The number of images to generate (1-10)
    #[arg(short, default_value_t = DEFAULT_NUM_IMAGES)]
    pub n: u8,

    /// The size of the generated images.
    ///
    /// One of: auto, 1024x1024, 1536x1024, 1024x1536, square, landscape, portrait
    #[arg(long, default_value = DEFAULT_SIZE)]
    pub size: String,

    /// The quality of the image that will be generated (high, medium, low, auto)
    #[arg(long, default_value = DEFAULT_QUALITY)]
    pub quality: String,

    // --- Edit-Specific Arguments ---
    /// The image(s) to edit. Providing at least one input image triggers the
    /// edit operation.
    ///
    /// Can be file paths or '-' to read from stdin. Use '@<path>' to force
    /// interpretation as a file path.
    #[arg(short, long)]
    pub image: Option<Vec<input::Image>>,

    /// An image whose transparent areas indicate where to edit (edit only).
    ///
    /// Can be a file path or '-' to read from stdin. Use '@<path>' to force
    /// interpretation as a file path.
    #[arg(short, long)]
    pub mask: Option<input::Image>,

    // --- Create-Specific Arguments ---
    /// Set the generated image background opacity (transparent, opaque, auto) (create only)
    #[arg(long, default_value = DEFAULT_BACKGROUND)]
    pub background: String,

    /// Control the content-moderation level (low, auto) (create only)
    #[arg(long, default_value = DEFAULT_MODERATION)]
    pub moderation: String,

    /// The output image compression level (jpeg and webp only) (0-100) (create only)
    #[arg(long, default_value_t = DEFAULT_OUTPUT_COMPRESSION)]
    pub output_compression: u8,

    /// The output image format (png, jpeg, webp) (create only)
    #[arg(long, default_value = DEFAULT_OUTPUT_FORMAT)]
    pub output_format: String,
}

impl Cli {
    pub fn run(self, progress: &MultiProgress) -> anyhow::Result<()> {
        // Load the configuration file
        let config = Config::load();

        // Get API key from CLI > environment variable > config file
        let api_key = self.openai_api_key.or(config.openai_api_key).context(
            "API key is required. Provide it with --openai-api-key or set the \
             `OPENAI_API_KEY` environment variable.",
        )?;

        // If --setup is provided, store the API key in the config file
        if self.setup {
            let config = Config {
                openai_api_key: Some(api_key.clone()),
            };
            config.save()?;
            return Ok(());
        }

        // Setup the OpenAI API client
        let client = Client::new(api_key);

        // Set up the spinner
        let sp = Spinner::new(progress);
        sp.set_message("Generating image(s)...");

        let result = self.args.run(&client);
        match result {
            Ok(_) => info!("✓ Done"),
            Err(_) => error!("✗ Done"),
        };

        result
    }
}

impl GenerateArgs {
    /// Run the appropriate image generation or editing command based on args
    fn run(self, client: &Client) -> anyhow::Result<()> {
        // Validate and read input prompt and images
        let prompt_source = self.prompt.context("Missing prompt")?;
        let inputs =
            input::PromptAndImages::new(prompt_source, self.image, self.mask)?;
        let prompt = inputs.prompt.read_prompt()?;

        // Determine if we're using the edit API or the create API based on the
        // presence of `--image` options
        let result = if let Some(images) = inputs.images {
            // Warn about create-API-only arguments if they are not default
            if self.background != DEFAULT_BACKGROUND {
                warn!("Ignoring --background option; it is only applicable when generating images without --image inputs.");
            }
            if self.moderation != DEFAULT_MODERATION {
                warn!("Ignoring --moderation option; it is only applicable when generating images without --image inputs.");
            }
            if self.output_compression != DEFAULT_OUTPUT_COMPRESSION {
                warn!("Ignoring --output-compression option; it is only applicable when generating images without --image inputs.");
            }
            if self.output_format != DEFAULT_OUTPUT_FORMAT {
                warn!("Ignoring --output-format option; it is only applicable when generating images without --image inputs.");
            }

            // Read the image data
            let images: Vec<input::ImageData> = images
                .into_iter()
                .map(|img| img.read_image())
                .collect::<Result<Vec<_>, _>>()?;

            // Read the mask data if provided
            let mask = inputs.mask.map(|img| img.read_image()).transpose()?;

            // Create the EditRequest
            let req = EditRequest {
                images,
                prompt: prompt.clone(), // Use the validated prompt
                mask,
                model: "gpt-image-1".to_string(),
                n: n_canonical(self.n),
                size: size_canonical(self.size.clone()),
                quality: quality_canonical(self.quality.clone()),
            };

            // Call the edit API
            client.edit_images(req)
        } else {
            // Warn about edit-API-only arguments if they are present
            if inputs.mask.is_some() {
                warn!("Ignoring --mask option; it is only applicable when generating images using --image inputs.");
            }
            // No warning needed for --image itself, as its absence triggers this path.

            // Create the CreateRequest
            let req = CreateRequest {
                model: "gpt-image-1".to_string(),
                prompt: prompt.clone(),
                n: n_canonical(self.n),
                size: size_canonical(self.size.clone()),
                quality: quality_canonical(self.quality.clone()),
                background: background_canonical(self.background.clone()),
                moderation: moderation_canonical(self.moderation.clone()),
                output_compression: Some(self.output_compression), // Always send for create
                output_format: Some(self.output_format.clone()), // Always send for create
            };

            // Call the create API
            client.create_images(req)
        };

        // Handle the response (logging, decoding, saving)
        let response = result?;
        handle_response(response, &prompt)
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
    info!("Estimated cost: ${:.2}", cost); // Show more precision for cost

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
        .save_images(file_prefix)
        .context("Failed to save images to files")?;

    info!("Saved images to: {}", saved_files.join(", "));

    Ok(())
}

// --- Avoid passing CLI arguments that match the API default values ---

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
