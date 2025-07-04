//! Prompt and image input handling

use anyhow::{anyhow, Context};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::cli::sanitize;
use crate::multipart;

/// Parsed inputs from the command line. Ensures at most one input uses stdin.
/// Also stores the desired output target.
pub struct InputArgs {
    pub prompt: PromptArg,
    pub images: Vec<ImageArg>,
    pub mask: Option<ImageArg>,
    pub out_target: OutputTarget,
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
pub enum OutputTarget {
    /// Save automatically based on prompt, timestamp, and index.
    Automatic,
    /// Save to a specific file path. Only valid for n=1.
    File(PathBuf),
    /// Write to standard output. Only valid for n=1.
    Stdout,
}

/// [`OutputTarget`] with additional data needed to write the output files.
pub enum OutputTargetWithData<'a> {
    Automatic { prefix: String, extension: &'a str },
    File(&'a Path),
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
        images: Vec<ImageArg>,
        mask: Option<ImageArg>,
        output_arg: Option<OutputArg>,
        n: u8,
        open: bool,
    ) -> anyhow::Result<Self> {
        // Only use stdin once across all inputs
        let prompt_stdin_count = matches!(prompt, PromptArg::Stdin) as usize;
        let mask_stdin_count = matches!(mask, Some(ImageArg::Stdin)) as usize;
        let images_stdin_count = images
            .iter()
            .map(|img| matches!(img, ImageArg::Stdin) as usize)
            .sum::<usize>();

        let total_stdin_count =
            prompt_stdin_count + mask_stdin_count + images_stdin_count;
        if total_stdin_count > 1 {
            return Err(anyhow!(
                "Only one input source (prompt, image, mask) can be '-' (stdin) at a time"
            ));
        }

        // Non-automatic output target must be used with `-n 1`
        let out_target = match output_arg {
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

        // Cannot use `--open` with `--output -` (stdout)
        if open && matches!(out_target, OutputTarget::Stdout) {
            return Err(anyhow!(
                "Cannot use --open flag when writing output to stdout (`--output -`)"
            ));
        }

        Ok(Self {
            prompt,
            images,
            mask,
            out_target,
        })
    }
}

impl PromptArg {
    pub fn read_prompt(self) -> anyhow::Result<String> {
        match self {
            Self::Literal(prompt) => Ok(prompt),
            Self::File(path) => {
                std::fs::read_to_string(&path).with_context(|| {
                    format!(
                        "Failed to read prompt from file: {}",
                        path.display()
                    )
                })
            }
            Self::Stdin => {
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

impl From<String> for OutputArg {
    fn from(s: String) -> Self {
        if s == "-" {
            Self::Stdout
        } else if let Some(s) = s.strip_prefix('@') {
            Self::File(PathBuf::from(s))
        } else {
            Self::File(PathBuf::from(s))
        }
    }
}

impl OutputTarget {
    /// Enrich the output target with additional data we need to actually write
    /// the output.
    pub fn with_data<'a>(
        &'a self,
        uses_edit_api: bool,
        prompt: &str,
        output_format: &'a str,
    ) -> OutputTargetWithData<'a> {
        match self {
            Self::Automatic => {
                let prefix = sanitize::prompt_prefix(prompt);
                let extension = if uses_edit_api {
                    // "edit" API only supports PNG output
                    "png"
                } else {
                    output_format
                };
                OutputTargetWithData::Automatic { prefix, extension }
            }
            Self::File(path) => OutputTargetWithData::File(path),
            Self::Stdout => OutputTargetWithData::Stdout,
        }
    }
}

impl<'a> OutputTargetWithData<'a> {
    pub fn file_path(&self) -> Option<&'a Path> {
        match self {
            Self::File(path) => Some(path),
            Self::Automatic { .. } | Self::Stdout => None,
        }
    }
}
