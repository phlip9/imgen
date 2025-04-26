use clap::{Parser, Subcommand};

/// A CLI tool for generating and editing images using OpenAI's image generation models
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
pub struct Cli {
    /// OpenAI API key (can also be set via OPENAI_API_KEY environment variable)
    #[arg(short, long, env = "OPENAI_API_KEY")]
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
    #[arg(long, default_value = "low")]
    pub quality: String,

    /// Set transparency for the background (transparent, opaque, auto)
    #[arg(long, default_value = "auto")]
    pub background: String,

    /// Control the content-moderation level (low, auto)
    #[arg(long, default_value = "low")]
    pub moderation: String,

    /// The compression level for generated images (0-100)
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
