use base64::{prelude::BASE64_STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::io;

#[cfg(test)]
mod tests;

/// Request body for the OpenAI image generation API
#[derive(Debug, Serialize)]
pub struct CreateRequest {
    /// The model to use for image generation (always gpt-image-1 for this app)
    pub model: String,

    /// A text description of the desired image(s)
    pub prompt: String,

    /// The number of images to generate (1-10)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub n: Option<u8>,

    /// The size of the generated images (1024x1024, 1536x1024, 1024x1536, auto)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub size: Option<String>,

    /// The quality of the image that will be generated (high, medium, low)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub quality: Option<String>,

    /// Set transparency for the background (transparent, opaque, auto)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub background: Option<String>,

    /// Control the content-moderation level (low, auto)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub moderation: Option<String>,

    /// The compression level for generated images (0-100)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_compression: Option<u8>,

    /// The format of the generated images (png, jpeg, webp)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_format: Option<String>,
}

/// Request for the OpenAI image edit API
/// Note: This is not Serialize because it needs to be multipart-form-encoded
#[derive(Debug)]
pub struct EditRequest {
    /// The image(s) to edit (paths to image files)
    pub images: Vec<String>,

    /// A text description of the desired image(s)
    pub prompt: String,

    /// An additional image whose transparent areas indicate where to edit
    pub mask: Option<String>,

    /// The model to use for image generation (always gpt-image-1 for this app)
    pub model: String,

    /// The number of images to generate (1-10)
    pub n: Option<u8>,

    /// The quality of the image that will be generated (high, medium, low)
    pub quality: Option<String>,

    /// The size of the generated images (1024x1024, 1536x1024, 1024x1536, auto)
    pub size: Option<String>,
}

/// Response from the OpenAI image generation API
#[derive(Debug, Deserialize)]
pub struct Response {
    /// The Unix timestamp (in seconds) of when the image was created
    pub created: u64,

    /// The list of generated images
    pub data: Vec<ImageData>,

    /// Token usage information for the image generation
    pub usage: Usage,
}

/// Image data returned in the response
#[derive(Debug, Deserialize)]
pub struct ImageData {
    /// The base64-encoded JSON of the generated image
    pub b64_json: String,
}

/// Token usage information
#[derive(Debug, Deserialize)]
pub struct Usage {
    /// The total number of tokens used for the image generation
    pub total_tokens: u32,

    /// The number of tokens in the input prompt
    pub input_tokens: u32,

    /// The number of tokens in the output image
    pub output_tokens: u32,

    /// Detailed information about input tokens
    pub input_tokens_details: InputTokensDetails,
}

impl Usage {
    /// Calculate the total cost in USD based on token usage.
    ///
    /// `gpt-image-1` costs are:
    /// * Input tokens cost $10.00 per 1M tokens
    /// * Output tokens cost $40.00 per 1M tokens
    pub fn calculate_cost(&self) -> f64 {
        const INPUT_COST_PER_MILLION: f64 = 10.0;
        const OUTPUT_COST_PER_MILLION: f64 = 40.0;

        let input_cost =
            (self.input_tokens as f64 / 1_000_000.0) * INPUT_COST_PER_MILLION;
        let output_cost =
            (self.output_tokens as f64 / 1_000_000.0) * OUTPUT_COST_PER_MILLION;

        input_cost + output_cost
    }
}

/// Detailed information about input tokens
#[derive(Debug, Deserialize)]
pub struct InputTokensDetails {
    /// The number of text tokens in the input prompt
    pub text_tokens: u32,

    /// The number of image tokens in the input prompt
    pub image_tokens: u32,
}

/// Decoded image data with raw bytes instead of base64
#[derive(Debug)]
pub struct DecodedImageData {
    /// The raw image bytes decoded from base64
    pub image_bytes: Vec<u8>,
}

/// Decoded response with raw image bytes instead of base64
#[derive(Debug)]
pub struct DecodedResponse {
    /// The Unix timestamp (in seconds) of when the image was created
    pub created: u64,

    /// The list of decoded images
    pub data: Vec<DecodedImageData>,

    /// Token usage information for the image generation
    pub usage: Usage,
}

impl TryFrom<ImageData> for DecodedImageData {
    type Error = base64::DecodeError;

    fn try_from(image_data: ImageData) -> Result<Self, Self::Error> {
        // Decode the base64 string to bytes
        let image_bytes = BASE64_STANDARD.decode(image_data.b64_json)?;
        Ok(DecodedImageData { image_bytes })
    }
}

impl TryFrom<Response> for DecodedResponse {
    type Error = base64::DecodeError;

    fn try_from(response: Response) -> Result<Self, Self::Error> {
        // Convert each ImageData to DecodedImageData
        let mut decoded_data = Vec::with_capacity(response.data.len());
        for image_data in response.data {
            decoded_data.push(DecodedImageData::try_from(image_data)?);
        }

        Ok(DecodedResponse {
            created: response.created,
            data: decoded_data,
            usage: response.usage,
        })
    }
}

impl DecodedImageData {
    /// Save the image to a file
    pub fn save_to_file(&self, path: &str) -> io::Result<()> {
        std::fs::write(path, &self.image_bytes)
    }
}

impl DecodedResponse {
    /// Save all images to files with the given prefix
    pub fn save_images(&self, prefix: &str) -> io::Result<Vec<String>> {
        let mut paths = Vec::with_capacity(self.data.len());

        for (i, image) in self.data.iter().enumerate() {
            let path = format!("{}.{}.{}.png", prefix, self.created, i + 1);
            image.save_to_file(&path)?;
            paths.push(path);
        }

        Ok(paths)
    }
}
