use crate::{
    api::{CreateRequest, DecodedResponse, EditRequest, Response},
    client::Client,
    config::Config,
    multipart,
};
use anyhow::Context;
use clap::Parser;
use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use log::{error, info, warn};
use std::{
    io::Read as _,
    path::{Path, PathBuf},
    str::FromStr,
    time::Duration,
};

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
    pub prompt: Option<InputPrompt>,

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
    pub image: Option<Vec<InputImage>>,

    /// An image whose transparent areas indicate where to edit (edit only).
    ///
    /// Can be a file path or '-' to read from stdin. Use '@<path>' to force
    /// interpretation as a file path.
    #[arg(short, long)]
    pub mask: Option<InputImage>,

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
        // Validate and read input prompt and images
        let prompt_source = self.prompt.context("Missing prompt")?;
        let inputs =
            InputPromptAndImages::new(prompt_source, self.image, self.mask)?;
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
            let images: Vec<InputImageData> = images
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

// --- Prompt and Image Input Handling --- //

/// Parsed prompt and image inputs from the command line. Ensures at most one
/// input uses stdin.
struct InputPromptAndImages {
    prompt: InputPrompt,
    images: Option<Vec<InputImage>>,
    mask: Option<InputImage>,
}

/// Prompts can be a literal string, a file path, or stdin ('-').
#[derive(Clone, Debug)]
pub enum InputPrompt {
    Literal(String),
    File(PathBuf),
    Stdin,
}

/// Image inputs can be a file path or stdin ('-').
#[derive(Clone, Debug)]
pub enum InputImage {
    File(PathBuf),
    Stdin,
}

/// The read image data, including the raw bytes and metadata.
#[cfg_attr(test, derive(Clone))]
pub struct InputImageData {
    pub bytes: Vec<u8>,
    pub filename: PathBuf,
    pub content_type: &'static str,
}

impl InputPromptAndImages {
    fn new(
        prompt: InputPrompt,
        images: Option<Vec<InputImage>>,
        mask: Option<InputImage>,
    ) -> anyhow::Result<Self> {
        let prompt_stdin_count = matches!(prompt, InputPrompt::Stdin) as usize;
        let mask_stdin_count = matches!(mask, Some(InputImage::Stdin)) as usize;

        let images_stdin_count = match images {
            Some(ref imgs) => imgs
                .iter()
                .map(|img| matches!(img, InputImage::Stdin) as usize)
                .sum(),
            None => 0,
        };

        let total_stdin_count =
            prompt_stdin_count + mask_stdin_count + images_stdin_count;
        if total_stdin_count > 1 {
            return Err(anyhow::anyhow!(
                "Only one input prompt or --image can be '-' (stdin) at a time"
            ));
        }

        Ok(Self {
            prompt,
            images,
            mask,
        })
    }
}

impl InputPrompt {
    fn read_prompt(self) -> anyhow::Result<String> {
        match self {
            InputPrompt::Literal(prompt) => Ok(prompt),
            InputPrompt::File(path) => std::fs::read_to_string(&path)
                .with_context(|| {
                    format!(
                        "Failed to read prompt from file: {}",
                        path.display()
                    )
                }),
            InputPrompt::Stdin => {
                let mut input = String::new();
                std::io::stdin()
                    .lock()
                    .read_to_string(&mut input)
                    .context("Failed to read prompt from stdin")?;
                Ok(input)
            }
        }
    }
}

impl FromStr for InputPrompt {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match LiteralOrFileOrStdin::from_str(s)? {
            LiteralOrFileOrStdin::Literal(prompt) => Ok(Self::Literal(prompt)),
            LiteralOrFileOrStdin::File(path) => Ok(Self::File(path)),
            LiteralOrFileOrStdin::Stdin => Ok(Self::Stdin),
        }
    }
}

impl InputImage {
    fn read_image(self) -> anyhow::Result<InputImageData> {
        match self {
            InputImage::File(path) => {
                let bytes = std::fs::read(&path).with_context(|| {
                    format!(
                        "Failed to read image from file: {}",
                        path.display()
                    )
                })?;
                let content_type = multipart::mime_from_filename(&path)?;
                Ok(InputImageData {
                    bytes,
                    filename: path,
                    content_type,
                })
            }
            InputImage::Stdin => {
                let mut bytes = Vec::new();
                std::io::stdin()
                    .lock()
                    .read_to_end(&mut bytes)
                    .context("Failed to read image from stdin")?;

                // Infer the content type from the bytes we read off stdin.
                let content_type = multipart::mime_from_bytes(&bytes);

                // Use fake filename for stdin: "stdin.{png,jpg,webp}"
                let mut filename = PathBuf::from("stdin");
                filename.set_extension(multipart::ext_from_mime(content_type)?);

                Ok(InputImageData {
                    bytes,
                    filename,
                    content_type,
                })
            }
        }
    }
}

impl FromStr for InputImage {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match LiteralOrFileOrStdin::from_str(s)? {
            LiteralOrFileOrStdin::Literal(_) => Err(anyhow::anyhow!(
                "Expected a file path or '-' for stdin for --image input"
            )),
            LiteralOrFileOrStdin::File(path) => Ok(Self::File(path)),
            LiteralOrFileOrStdin::Stdin => Ok(Self::Stdin),
        }
    }
}

enum LiteralOrFileOrStdin {
    Literal(String),
    File(PathBuf),
    Stdin,
}

impl FromStr for LiteralOrFileOrStdin {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Check for stdin input
        if s == "-" {
            return Ok(LiteralOrFileOrStdin::Stdin);
        }

        // Check if the string starts with '@' to indicate that the user
        // explicitly wants only a file path
        let (require_file, path) = if let Some(s) = s.strip_prefix('@') {
            (true, Path::new(s))
        } else {
            (false, Path::new(s))
        };

        if path.exists() {
            Ok(LiteralOrFileOrStdin::File(PathBuf::from(path)))
        } else if !require_file {
            Ok(LiteralOrFileOrStdin::Literal(String::from(s)))
        } else {
            Err(anyhow::anyhow!("File not found: {}", path.display()))
        }
    }
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
