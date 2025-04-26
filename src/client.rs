use crate::api::{CreateRequest, EditRequest, Response};
use crate::multipart::MultipartBuilder; // Add this line
use log::info;
use std::error::Error;
use std::fmt;
use std::io; // Add this line
use std::time::Instant;
use ureq::http::{self, HeaderValue};
use ureq::typestate::WithBody;

static BASE_URL: &str = "https://api.openai.com/v1";

/// Error type for OpenAI API client operations
#[derive(Debug)]
pub enum ClientError {
    /// Error from the HTTP client
    HttpError(ureq::Error),
    /// Error parsing the response
    ParseError(serde_json::Error),
    /// Error during file I/O for multipart request
    IoError(io::Error), // Add this variant
    /// Other errors
    Other(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::HttpError(e) => write!(f, "HTTP error: {}", e),
            ClientError::ParseError(e) => write!(f, "Parse error: {}", e),
            ClientError::IoError(e) => write!(f, "File I/O error: {}", e), // Add this line
            ClientError::Other(s) => write!(f, "Error: {}", s),
        }
    }
}

impl Error for ClientError {}

impl From<ureq::Error> for ClientError {
    fn from(err: ureq::Error) -> Self {
        ClientError::HttpError(err)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(err: serde_json::Error) -> Self {
        ClientError::ParseError(err)
    }
}

// Add From<io::Error> implementation
impl From<io::Error> for ClientError {
    fn from(err: io::Error) -> Self {
        ClientError::IoError(err)
    }
}

/// Client for the OpenAI API
pub struct Client {
    /// HTTP agent for making requests
    agent: ureq::Agent,
    /// Authorization header value
    auth: HeaderValue,
}

impl Client {
    /// Create a new client with the given API key
    pub fn new(api_key: String) -> Self {
        let auth = HeaderValue::try_from(format!("Bearer {}", api_key))
            .expect("Invalid API key format");
        let config = ureq::config::Config::builder().https_only(true).build();
        let agent = ureq::Agent::new_with_config(config);
        Self { agent, auth }
    }

    fn post(&self, uri: &str) -> ureq::RequestBuilder<WithBody> {
        self.agent
            .post(uri)
            .header(http::header::AUTHORIZATION, self.auth.clone())
    }

    /// Create an image using the OpenAI API
    pub fn create_images(
        &self,
        request: CreateRequest,
    ) -> Result<Response, ClientError> {
        // Start timing the request
        let start_time = Instant::now();

        // Make the API request
        let response = self
            .post(&format!("{BASE_URL}/images/generations"))
            .send_json(&request)?;

        // Get the response body as bytes to measure size
        let mut body = response.into_body();
        let response_size = body.content_length().unwrap_or(0);

        // Calculate the duration
        let duration = start_time.elapsed();

        // Log the request duration and response size
        info!(
            "create_image: request completed in {duration:?} with response size \
             of {response_size} bytes",
        );

        let resp = body
            .with_config()
            .limit(100 << 20) // 100 MiB
            .read_json()?;
        Ok(resp)
    }

    pub fn edit_images(
        &self,
        request: EditRequest,
    ) -> Result<Response, ClientError> {
        // Start timing the request
        let start_time = Instant::now();

        // Build the multipart request body
        let mut builder = MultipartBuilder::new();

        // Add text fields
        builder.add_text("prompt", &request.prompt);
        builder.add_text("model", &request.model);
        if let Some(n) = request.n {
            builder.add_text("n", &n.to_string());
        }
        if let Some(quality) = &request.quality {
            builder.add_text("quality", quality);
        }
        if let Some(size) = &request.size {
            builder.add_text("size", size);
        }

        // Add image files (note the field name "image[]" for multiple files)
        for image_path in &request.images {
            // Use "image[]" as the field name if multiple images are allowed by the API
            // If only one image is allowed by the specific model/endpoint variant,
            // just use "image". The current API doc implies multiple are possible
            // for gpt-image-1 edits, using `-F "image[]=@file1.png" -F "image[]=@file2.png"`
            // However, the text description says "For dall-e-2, you can only provide one image".
            // Let's assume "image[]" is correct for gpt-image-1 based on the curl example.
            // If the API expects just "image" even for gpt-image-1, this needs adjustment.
            builder.add_file("image[]", image_path)?;
        }

        // Add optional mask file
        if let Some(mask_path) = &request.mask {
            builder.add_file("mask", mask_path)?;
        }

        // Build the final body and content type
        let multipart_body = builder.build();

        // Make the API request
        let response = self
            .post(&format!("{BASE_URL}/images/edits"))
            .header(http::header::CONTENT_TYPE, multipart_body.content_type)
            .send(multipart_body.body)?;

        // Get the response body as bytes to measure size
        let mut response_body = response.into_body();
        let response_size = response_body.content_length().unwrap_or(0);

        // Calculate the duration
        let duration = start_time.elapsed();

        // Log the request duration and response size
        info!(
            "edit_images: request completed in {duration:?} with response size \
             of {response_size} bytes",
        );

        // Parse the JSON response
        let resp = response_body
            .with_config()
            .limit(100 << 20) // 100 MiB limit
            .read_json()?;

        Ok(resp)
    }
}
