//! Prompt and image input handling

use anyhow::{Context, Result};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::str::FromStr;

use crate::multipart;

/// Parsed prompt and image inputs from the command line. Ensures at most one
/// input uses stdin.
pub struct PromptAndImages {
    pub prompt: Prompt,
    pub images: Option<Vec<Image>>,
    pub mask: Option<Image>,
}

/// Prompts can be a literal string, a file path, or stdin ('-').
#[derive(Clone, Debug)]
pub enum Prompt {
    Literal(String),
    File(PathBuf),
    Stdin,
}

/// Image inputs can be a file path or stdin ('-').
#[derive(Clone, Debug)]
pub enum Image {
    File(PathBuf),
    Stdin,
}

/// The read image data, including the raw bytes and metadata.
#[cfg_attr(test, derive(Clone))]
pub struct ImageData {
    pub bytes: Vec<u8>,
    pub filename: PathBuf,
    pub content_type: &'static str,
}

impl PromptAndImages {
    pub fn new(
        prompt: Prompt,
        images: Option<Vec<Image>>,
        mask: Option<Image>,
    ) -> anyhow::Result<Self> {
        let prompt_stdin_count = matches!(prompt, Prompt::Stdin) as usize;
        let mask_stdin_count = matches!(mask, Some(Image::Stdin)) as usize;

        let images_stdin_count = match images {
            Some(ref imgs) => imgs
                .iter()
                .map(|img| matches!(img, Image::Stdin) as usize)
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

impl Prompt {
    pub fn read_prompt(self) -> anyhow::Result<String> {
        match self {
            Prompt::Literal(prompt) => Ok(prompt),
            Prompt::File(path) => {
                std::fs::read_to_string(&path).with_context(|| {
                    format!(
                        "Failed to read prompt from file: {}",
                        path.display()
                    )
                })
            }
            Prompt::Stdin => {
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

impl FromStr for Prompt {
    type Err = anyhow::Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match LiteralOrFileOrStdin::from_str(s)? {
            LiteralOrFileOrStdin::Literal(prompt) => Ok(Self::Literal(prompt)),
            LiteralOrFileOrStdin::File(path) => Ok(Self::File(path)),
            LiteralOrFileOrStdin::Stdin => Ok(Self::Stdin),
        }
    }
}

impl Image {
    pub fn read_image(self) -> anyhow::Result<ImageData> {
        match self {
            Image::File(path) => {
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
            Image::Stdin => {
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

impl FromStr for Image {
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
