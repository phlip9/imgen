use ureq::http::{self, HeaderValue};

use crate::api::{CreateRequest, Response};
use std::error::Error;
use std::fmt;

static BASE_URL: &str = "https://api.openai.com/v1";

/// Error type for OpenAI API client operations
#[derive(Debug)]
pub enum ClientError {
    /// Error from the HTTP client
    HttpError(ureq::Error),
    /// Error parsing the response
    ParseError(serde_json::Error),
    /// Other errors
    Other(String),
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::HttpError(e) => write!(f, "HTTP error: {}", e),
            ClientError::ParseError(e) => write!(f, "Parse error: {}", e),
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

    /// Create an image using the OpenAI API
    pub fn create_image(
        &self,
        request: CreateRequest,
    ) -> Result<Response, ClientError> {
        let resp = self
            .agent
            .post(&format!("{BASE_URL}/images/generations"))
            .header(http::header::AUTHORIZATION, self.auth.clone())
            .send_json(&request)?
            .into_body()
            .read_json::<Response>()?;
        Ok(resp)
    }
}
