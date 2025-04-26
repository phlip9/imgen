use serde::{Deserialize, Serialize};

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

/// Detailed information about input tokens
#[derive(Debug, Deserialize)]
pub struct InputTokensDetails {
    /// The number of text tokens in the input prompt
    pub text_tokens: u32,

    /// The number of image tokens in the input prompt
    pub image_tokens: u32,
}
