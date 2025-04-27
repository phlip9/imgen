//! Simple multipart form encoding purpose built for the OpenAI API.

use rand::{distr::Alphanumeric, Rng};
use std::path::Path;

/// Builds a multipart/form-data request body.
#[derive(Debug)]
pub struct Builder<'a> {
    boundary: String,
    parts: Vec<Part<'a>>,
}

impl<'a> Builder<'a> {
    /// Creates a new MultipartBuilder with a random boundary.
    #[allow(dead_code)]
    pub fn new() -> Self {
        let boundary = generate_boundary();
        Builder {
            boundary,
            parts: Vec::new(),
        }
    }

    /// Creates a new MultipartBuilder with the specified boundary.
    /// Useful for testing.
    pub fn with_boundary(boundary: String) -> Self {
        Builder {
            boundary,
            parts: Vec::new(),
        }
    }

    /// Adds a text field to the multipart form.
    pub fn add_text(&mut self, name: &'a str, value: &'a str) {
        self.parts.push(Part::Text { name, value });
    }

    /// Adds a file field from in-memory bytes.
    pub fn add_file_bytes(
        &mut self,
        name: &'a str,
        filename: &'a Path,
        content_type: &'a str,
        content: &'a [u8],
    ) {
        self.parts.push(Part::FileBytes {
            name,
            filename,
            content_type,
            content,
        });
    }

    /// Builds the final multipart/form-data body and returns it along with the
    /// `Content-Type` header value (including the boundary).
    ///
    /// # Returns
    ///
    /// A `MultipartBody` struct containing the raw body bytes and the
    /// `Content-Type` header value.
    pub fn build(self) -> Body {
        let mut body_bytes = Vec::new();
        let boundary_marker = format!("--{}\r\n", self.boundary);
        let boundary_end = format!("--{}--\r\n", self.boundary);

        for part in self.parts {
            body_bytes.extend_from_slice(boundary_marker.as_bytes());

            match part {
                Part::Text { name, value } => {
                    // Build Content-Disposition header directly
                    body_bytes.extend_from_slice(
                        b"Content-Disposition: form-data; name=\"",
                    );
                    body_bytes.extend_from_slice(name.as_bytes());
                    body_bytes.extend_from_slice(b"\"\r\n\r\n");
                    body_bytes.extend_from_slice(value.as_bytes());
                    body_bytes.extend_from_slice(b"\r\n");
                }
                Part::FileBytes {
                    name,
                    filename,
                    content_type,
                    content,
                } => {
                    // Build Content-Disposition header directly
                    body_bytes.extend_from_slice(
                        b"Content-Disposition: form-data; name=\"",
                    );
                    body_bytes.extend_from_slice(name.as_bytes());
                    body_bytes.extend_from_slice(b"\"; filename=\"");
                    body_bytes.extend_from_slice(
                        filename.as_os_str().as_encoded_bytes(),
                    );
                    body_bytes.extend_from_slice(b"\"\r\n");

                    // Build Content-Type header directly
                    body_bytes.extend_from_slice(b"Content-Type: ");
                    body_bytes.extend_from_slice(content_type.as_bytes());
                    body_bytes.extend_from_slice(b"\r\n\r\n");

                    // Append file content
                    body_bytes.extend_from_slice(content);
                    body_bytes.extend_from_slice(b"\r\n");
                }
            }
        }

        body_bytes.extend_from_slice(boundary_end.as_bytes());
        let content_type_header =
            format!("multipart/form-data; boundary={}", self.boundary);

        Body {
            body: body_bytes,
            content_type: content_type_header,
        }
    }
}

/// Represents the built multipart body and its associated Content-Type header.
#[derive(Debug)]
pub struct Body {
    /// The raw bytes of the multipart/form-data body.
    pub body: Vec<u8>,
    /// The value for the `Content-Type` header, e.g., `"multipart/form-data; boundary=..."`.
    pub content_type: String,
}

/// Represents a part in a multipart/form-data request.
#[derive(Debug)]
enum Part<'a> {
    /// A simple text field.
    Text { name: &'a str, value: &'a str },
    /// A file field provided as raw bytes.
    FileBytes {
        name: &'a str,
        filename: &'a Path,
        content_type: &'a str,
        content: &'a [u8],
    },
}

/// Generates a random alphanumeric boundary string of length 30.
pub fn generate_boundary() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}

/// Infers a MIME type from a filename extension.
///
/// Supports common image types used by the OpenAI API. Defaults to
/// `application/octet-stream` for unknown extensions or non-UTF8 extensions.
pub fn mime_from_filename<P: AsRef<Path>>(path: P) -> &'static str {
    match path.as_ref().extension().and_then(|s| s.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        // Add other types if needed
        _ => "application/octet-stream",
    }
}

/// Detects the MIME type of a byte slice.
///
/// Supports PNG, WebP, and JPEG. Defaults to `application/octet-stream` if
/// the signature is not recognized or the slice is too short.
pub fn mime_from_bytes(bytes: &[u8]) -> &'static str {
    // png
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return "image/png";
    }

    // webp
    if bytes.len() >= 12
        && bytes.starts_with(b"RIFF")
        && bytes[8..12] == *b"WEBP"
    {
        return "image/webp";
    }

    // Check for JPEG (3 bytes) - Check after others as it's shorter
    if bytes.starts_with(b"\xff\xd8") {
        return "image/jpeg";
    }

    // Fallback if no signature matches or too few bytes
    "application/octet-stream"
}

pub fn ext_from_mime(mime: &str) -> Option<&'static str> {
    match mime {
        "image/png" => Some("png"),
        "image/jpeg" => Some("jpg"),
        "image/webp" => Some("webp"),
        _ => None,
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_basic_text() {
        let boundary = "testboundary123".to_string();
        let mut builder = Builder::with_boundary(boundary.clone());
        builder.add_text("prompt", "A test prompt");
        builder.add_text("model", "gpt-image-1");

        let result = builder.build();
        let body_str =
            String::from_utf8(result.body).expect("Body is not valid UTF-8");

        let expected_content_type =
            format!("multipart/form-data; boundary={}", boundary);
        assert_eq!(result.content_type, expected_content_type);

        let expected_body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"prompt\"\r\n\r\n\
             A test prompt\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"model\"\r\n\r\n\
             gpt-image-1\r\n\
             --{boundary}--\r\n"
        );

        assert_eq!(body_str, expected_body);
    }

    #[test]
    fn test_mime_inference() {
        assert_eq!(mime_from_filename(Path::new("image.png")), "image/png");
        assert_eq!(mime_from_filename(Path::new("photo.jpg")), "image/jpeg");
        assert_eq!(mime_from_filename(Path::new("graphic.jpeg")), "image/jpeg");
        assert_eq!(
            mime_from_filename(Path::new("animation.webp")),
            "image/webp"
        );
        assert_eq!(
            mime_from_filename(Path::new("document.pdf")),
            "application/octet-stream"
        );
        assert_eq!(
            mime_from_filename(Path::new("unknown")),
            "application/octet-stream"
        );
        assert_eq!(
            mime_from_filename(Path::new("file.with.dots.png")),
            "image/png"
        );
        assert_eq!(
            mime_from_filename(Path::new("noextension")),
            "application/octet-stream"
        );
        // Test with PathBuf
        let path_buf = Path::new("another.png");
        assert_eq!(mime_from_filename(path_buf), "image/png");
    }

    #[test]
    fn test_empty_builder() {
        let boundary = "emptyboundary789".to_string();
        let builder = Builder::with_boundary(boundary.clone());
        let result = builder.build();
        let body_str =
            String::from_utf8(result.body).expect("Body is not valid UTF-8");

        let expected_content_type =
            format!("multipart/form-data; boundary={}", boundary);
        assert_eq!(result.content_type, expected_content_type);

        let expected_body = format!("--{}--\r\n", boundary);
        assert_eq!(body_str, expected_body);
    }
}
