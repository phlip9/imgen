//! Simple multipart form encoding purpose built for the OpenAI API.

use rand::{distr::Alphanumeric, Rng};
use std::fs;
use std::io;
use std::path::Path;

/// Builds a multipart/form-data request body.
#[derive(Debug)]
pub struct MultipartBuilder {
    boundary: String,
    parts: Vec<Part>,
}

impl MultipartBuilder {
    /// Creates a new MultipartBuilder with a random boundary.
    pub fn new() -> Self {
        let boundary = generate_boundary();
        MultipartBuilder {
            boundary,
            parts: Vec::new(),
        }
    }

    /// Creates a new MultipartBuilder with the specified boundary.
    /// Useful for testing.
    #[cfg(test)] // Only include this constructor in test builds
    fn with_boundary(boundary: String) -> Self {
        MultipartBuilder {
            boundary,
            parts: Vec::new(),
        }
    }

    /// Adds a text field to the multipart form.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the form field.
    /// * `value` - The value of the form field.
    pub fn add_text(&mut self, name: &str, value: &str) {
        self.parts.push(Part::Text {
            name: name.to_string(),
            value: value.to_string(),
        });
    }

    /// Adds a file field to the multipart form by reading from the given path.
    ///
    /// The file content is read into memory. The filename is extracted from the path,
    /// and the MIME type is inferred from the filename extension.
    ///
    /// # Arguments
    ///
    /// * `name` - The name of the form field.
    /// * `path` - The path to the file to include.
    ///
    /// # Errors
    ///
    /// Returns an `io::Error` if the file cannot be read.
    pub fn add_file(&mut self, name: &str, path: &str) -> io::Result<()> {
        let path_obj = Path::new(path);
        let filename = path_obj
            .file_name()
            .and_then(|os_str| os_str.to_str())
            .unwrap_or("unknown_file")
            .to_string();

        let content = fs::read(path_obj)?;
        let content_type = mime_from_filename(&filename);

        self.parts.push(Part::File {
            name: name.to_string(),
            filename,
            content_type,
            content,
        });
        Ok(())
    }

    /// Builds the final multipart/form-data body and returns it along with the
    /// `Content-Type` header value (including the boundary).
    ///
    /// # Returns
    ///
    /// A tuple containing:
    /// 1. `Vec<u8>`: The raw bytes of the multipart/form-data body.
    /// 2. `String`: The value for the `Content-Type` header, e.g.,
    ///    `"multipart/form-data; boundary=..."`.
    pub fn build(self) -> (Vec<u8>, String) {
        let mut body = Vec::new();
        let boundary_marker = format!("--{}\r\n", self.boundary);
        let boundary_end = format!("--{}--\r\n", self.boundary);

        for part in self.parts {
            body.extend_from_slice(boundary_marker.as_bytes());

            match part {
                Part::Text { name, value } => {
                    body.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"\r\n\r\n",
                            name
                        )
                        .as_bytes(),
                    );
                    body.extend_from_slice(value.as_bytes());
                    body.extend_from_slice(b"\r\n");
                }
                Part::File {
                    name,
                    filename,
                    content_type,
                    content,
                } => {
                    body.extend_from_slice(
                        format!(
                            "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                            name, filename
                        )
                        .as_bytes(),
                    );
                    body.extend_from_slice(
                        format!("Content-Type: {}\r\n\r\n", content_type)
                            .as_bytes(),
                    );
                    body.extend_from_slice(&content);
                    body.extend_from_slice(b"\r\n");
                }
            }
        }

        body.extend_from_slice(boundary_end.as_bytes());
        let content_type_header =
            format!("multipart/form-data; boundary={}", self.boundary);

        (body, content_type_header)
    }
}

/// Represents a part in a multipart/form-data request.
#[derive(Debug)]
enum Part {
    /// A simple text field.
    Text { name: String, value: String },
    /// A file field, including its content read into memory.
    File {
        name: String,
        filename: String,
        content_type: String,
        content: Vec<u8>,
    },
}

/// Generates a random alphanumeric boundary string of length 30.
fn generate_boundary() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(30)
        .map(char::from)
        .collect()
}

/// Infers a MIME type from a filename extension.
///
/// Supports common image types used by the OpenAI API. Defaults to
/// `application/octet-stream` for unknown extensions.
fn mime_from_filename(filename: &str) -> String {
    match Path::new(filename).extension().and_then(|s| s.to_str()) {
        Some("png") => "image/png".to_string(),
        Some("jpg") | Some("jpeg") => "image/jpeg".to_string(),
        Some("webp") => "image/webp".to_string(),
        // Add other types if needed
        _ => "application/octet-stream".to_string(),
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::NamedTempFile; // Keep this import for later tests

    #[test]
    fn test_build_basic_text() {
        let boundary = "testboundary123".to_string();
        let mut builder = MultipartBuilder::with_boundary(boundary.clone());
        builder.add_text("prompt", "A test prompt");
        builder.add_text("model", "gpt-image-1");

        let (body, content_type) = builder.build();
        let body_str =
            String::from_utf8(body).expect("Body is not valid UTF-8");

        let expected_content_type =
            format!("multipart/form-data; boundary={}", boundary);
        assert_eq!(content_type, expected_content_type);

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

    // #[test]
    // fn test_build_with_file() {
    //     // Create a temporary file with known content
    //     let mut temp_file = tempfile::Builder::new().tempfile().unwrap();
    //     temp_file
    //         .write_all("dummy file content".as_bytes())
    //         .unwrap();
    //     let filename = Path::new(&temp_file.path())
    //         .file_name()
    //         .unwrap()
    //         .to_str()
    //         .unwrap();
    //
    //     // Create a temporary PNG file for MIME type testing
    //     let mut temp_png_file =
    //         tempfile::Builder::new().suffix(".png").tempfile().unwrap();
    //     temp_png_file.write_all("png data".as_bytes()).unwrap();
    //     let png_filename = Path::new(&temp_png_file.path())
    //         .file_name()
    //         .unwrap()
    //         .to_str()
    //         .unwrap();
    //
    //     let mut builder = MultipartBuilder::new();
    //     builder.add_text("model", "gpt-image-1");
    //     builder.add_file("image", &file_path)?; // Test default MIME
    //     builder.add_file("mask", &png_path)?; // Test PNG MIME
    //
    //     let (body, content_type) = builder.build();
    //     // Use lossy conversion as body contains binary data and text parts
    //     let body_str = String::from_utf8_lossy(&body);
    //
    //     assert!(content_type.starts_with("multipart/form-data; boundary="));
    //     let boundary = content_type
    //         .split('=')
    //         .nth(1)
    //         .expect("Boundary not found in Content-Type");
    //
    //     // Construct the expected body string manually
    //     let expected_body = format!(
    //         "--{boundary}\r\n\
    //          Content-Disposition: form-data; name=\"model\"\r\n\r\n\
    //          gpt-image-1\r\n\
    //          --{boundary}\r\n\
    //          Content-Disposition: form-data; name=\"image\"; filename=\"{filename}\"\r\n\
    //          Content-Type: application/octet-stream\r\n\r\n\
    //          {file_content}\r\n\
    //          --{boundary}\r\n\
    //          Content-Disposition: form-data; name=\"mask\"; filename=\"{png_filename}\"\r\n\
    //          Content-Type: image/png\r\n\r\n\
    //          png data\r\n\
    //          --{boundary}--\r\n"
    //     );
    //
    //     assert_eq!(body_str, expected_body);
    //
    //     Ok(())
    // }
    //
    // #[test]
    // fn test_mime_inference() {
    //     assert_eq!(mime_from_filename("image.png"), "image/png");
    //     assert_eq!(mime_from_filename("photo.jpg"), "image/jpeg");
    //     assert_eq!(mime_from_filename("graphic.jpeg"), "image/jpeg");
    //     assert_eq!(mime_from_filename("animation.webp"), "image/webp");
    //     assert_eq!(
    //         mime_from_filename("document.pdf"),
    //         "application/octet-stream"
    //     );
    //     assert_eq!(mime_from_filename("unknown"), "application/octet-stream");
    //     assert_eq!(mime_from_filename("file.with.dots.png"), "image/png");
    //     assert_eq!(
    //         mime_from_filename("noextension"),
    //         "application/octet-stream"
    //     );
    // }
    //
    // #[test]
    // fn test_empty_builder() {
    //     let builder = MultipartBuilder::new();
    //     let (body, content_type) = builder.build();
    //     let body_str =
    //         String::from_utf8(body).expect("Body is not valid UTF-8");
    //
    //     assert!(content_type.starts_with("multipart/form-data; boundary="));
    //     let boundary = content_type
    //         .split('=')
    //         .nth(1)
    //         .expect("Boundary not found in Content-Type");
    //
    //     let expected_body = format!("--{}--\r\n", boundary);
    //     assert_eq!(body_str, expected_body);
    // }
}
