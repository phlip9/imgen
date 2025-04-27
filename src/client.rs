use crate::api::{CreateRequest, EditRequest, Response};
use log::info;
use std::error::Error;
use std::fmt;
use std::io;
use std::time::Duration;
use std::time::Instant;
use ureq::http::{self, HeaderValue};
use ureq::typestate::WithBody;

/// OpenAI API endpoint
static BASE_URL: &str = "https://api.openai.com/v1";

/// Our user agent string. ex: "imgen/0.1.2"
static USER_AGENT: &str =
    concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"));

/// End-to-end timeout for requests.
///
/// Our timeout needs to long to handle OpenAI's glacial image generation time.
const TIMEOUT: Duration = Duration::from_secs(20 * 60); // 20 min

/// Limit responses to at most 100 MiB.
const RESPONSE_BODY_LIMIT: u64 = 100 << 20; // 100 MiB

/// Error type for OpenAI API client operations
#[derive(Debug)]
pub enum ClientError {
    /// Error from the HTTP client (transport level, DNS, timeouts, etc.)
    Http(ureq::Error),
    /// Error parsing the response JSON
    Parse(serde_json::Error),
    /// Error during file I/O for multipart request
    Io(io::Error),
    /// Error reported by the OpenAI API (e.g., invalid request, rate limit)
    ApiError {
        status: http::StatusCode,
        message: String,
    },
}

impl fmt::Display for ClientError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ClientError::Http(err) => write!(f, "HTTP transport error: {err}"),
            ClientError::Parse(err) => write!(f, "JSON parse error: {err}"),
            ClientError::Io(err) => write!(f, "File I/O error: {err}"),
            ClientError::ApiError { status, message } => {
                write!(f, "HTTP error {status}: {message}")
            }
        }
    }
}

impl Error for ClientError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            ClientError::Http(e) => Some(e),
            ClientError::Parse(e) => Some(e),
            ClientError::Io(e) => Some(e),
            // API errors don't wrap another error
            ClientError::ApiError { .. } => None,
        }
    }
}

impl From<ureq::Error> for ClientError {
    fn from(err: ureq::Error) -> Self {
        ClientError::Http(err)
    }
}

impl From<serde_json::Error> for ClientError {
    fn from(err: serde_json::Error) -> Self {
        ClientError::Parse(err)
    }
}

// Add From<io::Error> implementation specifically for file I/O errors
impl From<io::Error> for ClientError {
    fn from(err: io::Error) -> Self {
        ClientError::Io(err)
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
        let config = ureq::config::Config::builder()
            .https_only(true)
            .tls_config(
                ureq::tls::TlsConfig::builder()
                    .provider(ureq::tls::TlsProvider::NativeTls)
                    .root_certs(ureq::tls::RootCerts::PlatformVerifier)
                    .build(),
            )
            .timeout_global(Some(TIMEOUT))
            .user_agent(USER_AGENT)
            .http_status_as_error(false) // Don't treat 4xx/5xx as `Err(_)`
            .build();
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
            .send_json(&request)?
            .read_json()?;

        // Log the request duration
        let duration = start_time.elapsed();
        info!("create_image: done in {duration:?}");

        Ok(response)
    }

    pub fn edit_images(
        &self,
        request: EditRequest,
    ) -> Result<Response, ClientError> {
        // Start timing the request
        let start_time = Instant::now();

        // Build the multipart request body
        let multipart_body = request.build_multipart();

        // Make the API request
        let response = self
            .post(&format!("{BASE_URL}/images/edits"))
            .header(http::header::CONTENT_TYPE, multipart_body.content_type)
            .send(multipart_body.body)?
            .read_json()?;

        // Log the request duration
        let duration = start_time.elapsed();
        info!("edit_images: done in {duration:.2?}");

        Ok(response)
    }
}

trait ResponseExt {
    /// Read the response body as a JSON object.
    fn read_json<T: serde::de::DeserializeOwned>(
        self,
    ) -> Result<T, ClientError>;
}

impl ResponseExt for http::Response<ureq::Body> {
    // In order to give the user good error messages on 4xx/5xx errors, we need
    // to explicitly check the status code and read the body on error.
    fn read_json<T: serde::de::DeserializeOwned>(
        self,
    ) -> Result<T, ClientError> {
        let status = self.status();
        if status.is_success() {
            // Success case (2xx)
            // Read the response body as JSON
            self.into_body()
                .with_config()
                .limit(RESPONSE_BODY_LIMIT)
                .read_json()
                .map_err(ClientError::from)
        } else {
            // Error case
            // Try to read the response body as a string
            let body = self
                .into_body()
                .with_config()
                .limit(RESPONSE_BODY_LIMIT)
                .read_to_vec()?;
            let body_str = match String::from_utf8(body) {
                Ok(s) => s,
                Err(err) => {
                    String::from_utf8_lossy(err.as_bytes()).into_owned()
                }
            };
            Err(ClientError::ApiError {
                status,
                message: body_str,
            })
        }
    }
}
