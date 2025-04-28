//! Prompt and image input handling

use anyhow::{anyhow, Context};
use std::ffi::{OsStr, OsString};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::api::DecodedImageData;
use crate::multipart;

/// Parsed prompt and image inputs from the command line. Ensures at most one
/// input uses stdin. Also stores the desired output target.
pub struct InputArgs {
    pub prompt: PromptArg,
    pub images: Option<Vec<ImageArg>>,
    pub mask: Option<ImageArg>,
    pub output: OutputTarget,
}

/// Prompts can be a literal string, a file path, or stdin ('-').
#[derive(Clone, Debug)]
pub enum PromptArg {
    Literal(String),
    File(PathBuf),
    Stdin,
}

/// Image inputs can be a file path or stdin ('-').
#[derive(Clone, Debug)]
pub enum ImageArg {
    File(PathBuf),
    Stdin,
}

/// Represents the parsed value of the `--output` argument *before* validation
/// against other arguments like `-n`.
#[derive(Clone, Debug)]
pub enum OutputArg {
    File(PathBuf),
    Stdout,
}

/// Represents the validated output destination for the generated image(s).
#[derive(Clone, Debug)]
pub enum OutputTarget {
    /// Save automatically based on prompt, timestamp, and index.
    Automatic,
    /// Save to a specific file path. Only valid for n=1.
    File(PathBuf),
    /// Write to standard output. Only valid for n=1.
    Stdout,
}

/// The read image data, including the raw bytes and metadata.
#[cfg_attr(test, derive(Clone))]
pub struct ImageData {
    pub bytes: Vec<u8>,
    pub filename: PathBuf,
    pub content_type: &'static str,
}

impl InputArgs {
    /// Creates a new `InputArgs` instance, validating input combinations.
    ///
    /// # Errors
    ///
    /// * More than one input source uses stdin (`-`).
    /// * `--output` is specified (not automatic) but `n` is not 1.
    pub fn new(
        prompt: PromptArg,
        images: Option<Vec<ImageArg>>,
        mask: Option<ImageArg>,
        output_arg: Option<OutputArg>,
        n: u8,
    ) -> anyhow::Result<Self> {
        // Only use stdin once across all inputs
        let prompt_stdin_count = matches!(prompt, PromptArg::Stdin) as usize;
        let mask_stdin_count = matches!(mask, Some(ImageArg::Stdin)) as usize;
        let images_stdin_count = match &images {
            Some(ref imgs) => imgs
                .iter()
                .map(|img| matches!(img, ImageArg::Stdin) as usize)
                .sum(),
            None => 0,
        };

        let total_stdin_count =
            prompt_stdin_count + mask_stdin_count + images_stdin_count;
        if total_stdin_count > 1 {
            return Err(anyhow!(
                "Only one input source (prompt, image, mask) can be '-' (stdin) at a time"
            ));
        }

        // Non-automatic output target must be used with `-n 1`
        let output = match output_arg {
            // Default to automatic naming
            None => OutputTarget::Automatic,
            Some(OutputArg::File(path)) => {
                if n != 1 {
                    return Err(anyhow!(
                        "Cannot use --output <file> when generating more than one image (n={n})"
                    ));
                }
                OutputTarget::File(path)
            }
            Some(OutputArg::Stdout) => {
                if n != 1 {
                    return Err(anyhow!(
                        "Cannot use --output - (stdout) when generating more than one image (n={n})"
                    ));
                }
                OutputTarget::Stdout
            }
        };

        Ok(Self {
            prompt,
            images,
            mask,
            output,
        })
    }
}

impl PromptArg {
    pub fn read_prompt(self) -> anyhow::Result<String> {
        match self {
            PromptArg::Literal(prompt) => Ok(prompt),
            PromptArg::File(path) => std::fs::read_to_string(&path)
                .with_context(|| {
                    format!(
                        "Failed to read prompt from file: {}",
                        path.display()
                    )
                }),
            PromptArg::Stdin => {
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

impl FromStr for PromptArg {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match LiteralOrFileOrStdin::from_str(s)? {
            LiteralOrFileOrStdin::Literal(prompt) => Ok(Self::Literal(prompt)),
            LiteralOrFileOrStdin::File(path) => Ok(Self::File(path)),
            LiteralOrFileOrStdin::Stdin => Ok(Self::Stdin),
        }
    }
}

impl ImageArg {
    pub fn read_image(self) -> anyhow::Result<ImageData> {
        match self {
            ImageArg::File(path) => {
                let bytes = std::fs::read(&path).with_context(|| {
                    format!(
                        "Failed to read image from file: {}",
                        path.display()
                    )
                })?;
                let content_type = multipart::mime_from_filename(&path)?;
                Ok(ImageData {
                    bytes,
                    filename: path,
                    content_type,
                })
            }
            ImageArg::Stdin => {
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

                Ok(ImageData {
                    bytes,
                    filename,
                    content_type,
                })
            }
        }
    }
}

impl FromStr for ImageArg {
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

impl From<OsString> for OutputArg {
    fn from(s: OsString) -> Self {
        if s == OsStr::new("-") {
            OutputArg::Stdout
        } else {
            OutputArg::File(PathBuf::from(s))
        }
    }
}

impl OutputTarget {
    /// Writes the image data to the specified target.
    pub fn write_image(
        &self,
        image_data: &DecodedImageData,
    ) -> anyhow::Result<()> {
        match self {
            OutputTarget::Automatic => {
                // This case should be handled by the caller using save_images
                unreachable!(
                    "Automatic output target should be handled separately"
                );
            }
            OutputTarget::File(path) => {
                image_data
                    .save_to_file(path.to_str().unwrap_or("output.image"))?;
            }
            OutputTarget::Stdout => {
                let mut handle = io::stdout().lock();
                handle.write_all(&image_data.image_bytes)?;
                handle.flush()?;
            }
        }
        Ok(())
    }
}
