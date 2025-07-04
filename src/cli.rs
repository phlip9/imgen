use std::path::PathBuf;

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
mod sanitize;
mod spinner;

// Default values for CLI options
const DEFAULT_BACKGROUND: &str = "auto";
const DEFAULT_MODERATION: &str = "low";
const DEFAULT_NUM_IMAGES: u8 = 1;
const DEFAULT_OUTPUT_COMPRESSION: u8 = 100;
const DEFAULT_OUTPUT_FORMAT: &str = "png";
const DEFAULT_QUALITY: &str = "auto";
const DEFAULT_SIZE: &str = "1024x1024";

/// imgen
///
/// imgen generates images using OpenAI's `gpt-image-1` image generation model.
///
/// The tool operates in two modes: 'create' mode by default, or 'edit' mode
/// when one or more `--image` inputs are provided. Some options are only
/// applicable in one mode or the other.
///
/// Example usage:
///
/// ```
/// # Save <your_api_key> to the config file (`~/.config/imgen/config.json`)
/// imgen --setup --openai-api-key <your_api_key>
///
/// # Generate a single image from a prompt
/// imgen "A cute cat saying 'hello' on the Moon"
///
/// # Generate images using other images as a reference
/// imgen -i cat.png -i hat.png "A photo of the cat weaing the hat"
///
/// # Edit an image using a mask
/// imgen -i pool.png -m mask.png "A sunlit pool containing a flamingo"
///
/// # Build image generation pipelines using standard unix pipes
/// cat dog.webp | imgen -i - -o - prompt.md | gzip -c | hexyl
/// ```
///
/// The OpenAI API key is sourced in this order:
/// • from the command line with `--openai-api-key`
/// • from the environment variable `OPENAI_API_KEY`
/// • from `OPENAI_API_KEY` in a `.env` file
/// • from the config file `~/.config/imgen/config.json` (--setup to create)
#[derive(Parser, Debug)]
#[command(author, version, about, long_about)]
#[clap(verbatim_doc_comment)]
pub struct Cli {
    /// OpenAI API key (can also be set via `OPENAI_API_KEY` environment variable)
    #[arg(short = 'k', long, env = "OPENAI_API_KEY", hide_env = true)]
    pub openai_api_key: Option<String>,

    /// Store the `--openai-api-key` in the config file and exit.
    #[arg(long)]
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
    /// A text description of the desired image(s) (Required unless --setup)
    ///
    /// Can be a literal string, a path to a text file (if the path exists),
    /// or '-' to read from stdin. Use '@<path>' to force interpretation as a
    /// file path.
    #[arg(verbatim_doc_comment, required_unless_present("setup"))]
    pub prompt: Option<input::PromptArg>,

    /// Input image(s) to edit. Providing at least one input image triggers the
    /// edit operation.
    ///
    /// Can be file paths or '-' to read from stdin. Use '@<path>' to force
    /// interpretation as a file path.
    ///
    /// Supported input image formats:
    /// • png, jpeg, webp
    #[arg(short, long, verbatim_doc_comment)]
    #[arg(help_heading = "Input Options (edit)")]
    pub image: Vec<input::ImageArg>,

    /// An image whose transparent areas indicate where to edit (edit only).
    ///
    /// Can be a file path or '-' to read from stdin. Use '@<path>' to force
    /// interpretation as a file path.
    ///
    /// Supported input mask image formats:
    /// • png, jpeg, webp
    #[arg(short, long, verbatim_doc_comment)]
    #[arg(help_heading = "Input Options (edit)")]
    pub mask: Option<input::ImageArg>,

    /// Save the generated output image to this path (only supported with `-n 1`).
    ///
    /// If not specified, automatically saves to files based on the prompt.
    /// Ex: prompt='A cute cat saying "hello" on the Moon' will save to
    /// "a_cute_cat_saying_hello.<timestamp>.<i>.png" in the current directory.
    ///
    /// Can be a file path or '-' to write to stdout. Use '@<path>' to force
    /// interpretation as a file path.
    ///
    /// Supported output image formats:
    /// • png, jpeg, webp  (no --image inputs)
    /// • png              (with --image inputs)
    #[arg(short, long, verbatim_doc_comment)]
    #[arg(help_heading = "Output Options")]
    pub output: Option<input::OutputArg>,

    /// Open the generated image(s) in the default system viewer after saving.
    ///
    /// Conflicts with `--output -` (stdout).
    #[arg(long)]
    #[arg(help_heading = "Output Options")]
    pub open: bool,

    /// The number of images to generate (1-10)
    #[arg(short, long, default_value_t = DEFAULT_NUM_IMAGES)]
    #[arg(help_heading = "Output Options", verbatim_doc_comment)]
    pub n: u8,

    /// The size of the generated images.
    /// One of: auto, 1024x1024, 1536x1024, 1024x1536, square, landscape, portrait
    #[arg(long, default_value = DEFAULT_SIZE)]
    #[arg(help_heading = "Output Options")]
    pub size: String,

    /// The quality of the image that will be generated (high, medium, low, auto)
    #[arg(long, default_value = DEFAULT_QUALITY)]
    #[arg(help_heading = "Output Options")]
    pub quality: String,

    /// Set the desired background opacity of the generated image (create only)
    /// One of: transparent, opaque, auto
    #[arg(long, default_value = DEFAULT_BACKGROUND)]
    #[arg(help_heading = "Output Options (create)", verbatim_doc_comment)]
    pub background: String,

    /// Control the content-moderation level (low, auto) (create only)
    #[arg(long, default_value = DEFAULT_MODERATION)]
    #[arg(help_heading = "Output Options (create)")]
    pub moderation: String,

    /// The output image compression level (jpeg and webp only) (0-100) (create only)
    #[arg(long, default_value_t = DEFAULT_OUTPUT_COMPRESSION)]
    #[arg(help_heading = "Output Options (create)")]
    pub output_compression: u8,

    /// The output image format (png, jpeg, webp) (create only)
    #[arg(long, default_value = DEFAULT_OUTPUT_FORMAT)]
    #[arg(help_heading = "Output Options (create)")]
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
        // Validate and read input prompt, images, and output target
        let prompt_source = self.prompt.context("Missing prompt")?;
        let inputs = input::InputArgs::new(
            prompt_source,
            self.image,
            self.mask,
            self.output,
            self.n,
            self.open,
        )?;
        let prompt = inputs.prompt.read_prompt()?;
        let uses_edit_api = !inputs.images.is_empty();
        let out_target = inputs.out_target.with_data(
            uses_edit_api,
            &prompt,
            &self.output_format,
        );

        // Determine if we're using the edit API or the create API based on the
        // presence of `--image` options
        let result = if uses_edit_api {
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
            let images: Vec<input::ImageData> = inputs
                .images
                .into_iter()
                .map(|img| img.read_image())
                .collect::<Result<Vec<_>, _>>()?;

            // Read the mask data if provided
            let mask = inputs.mask.map(|img| img.read_image()).transpose()?;

            // Create the EditRequest
            let req = EditRequest {
                images,
                prompt,
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
                prompt,
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

        // Handle the response (logging, decoding, saving/writing, opening)
        let response = result?;
        handle_response(response, out_target, self.open)
    }
}

/// Handles the common logic after receiving an API response.
///
/// Decodes images, calculates cost, saves/writes the output, and optionally opens them.
fn handle_response(
    resp: Response,
    out_target: input::OutputTargetWithData<'_>,
    open_files: bool,
) -> anyhow::Result<()> {
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

    // Handle output based on the target
    let out_paths = decoded_resp.save_images(out_target)?;

    // Open the generated images if requested
    if open_files {
        open_images(&out_paths)?;
    }

    Ok(())
}

/// Open the generated images in the default system viewer.
fn open_images(paths: &[PathBuf]) -> anyhow::Result<()> {
    for path in paths {
        open::that_detached(path).with_context(|| {
            format!("Failed to open image: {}", path.display())
        })?;
    }
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
