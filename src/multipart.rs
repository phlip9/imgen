//! Simple multipart form encoding purpose built for the OpenAI API.

use rand::{distr::Alphanumeric, Rng};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

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
    /// * `path` - The path to the file to include. Can be any type that implements `AsRef<Path>`.
    ///
    /// # Errors
    ///
    /// Returns an `io::Error` if the file cannot be read.
    pub fn add_file<P: AsRef<Path>>(
        &mut self,
        name: &str,
        path: P,
    ) -> io::Result<()> {
        let path_ref = path.as_ref();
        // Get OsStr filename, return error if path has no filename component
        let filename_osstr = path_ref.file_name().ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidInput, "Path has no filename")
        })?;
        let filename = PathBuf::from(filename_osstr); // Convert OsStr to PathBuf

        let content = fs::read(path_ref)?;
        let content_type = mime_from_filename(path_ref); // Pass the Path directly

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
                    // Build Content-Disposition header directly
                    body.extend_from_slice(
                        b"Content-Disposition: form-data; name=\"",
                    );
                    body.extend_from_slice(name.as_bytes());
                    body.extend_from_slice(b"\"\r\n\r\n");
                    body.extend_from_slice(value.as_bytes());
                    body.extend_from_slice(b"\r\n");
                }
                Part::File {
                    name,
                    filename,
                    content_type,
                    content,
                } => {
                    // Build Content-Disposition header directly
                    body.extend_from_slice(
                        b"Content-Disposition: form-data; name=\"",
                    );
                    body.extend_from_slice(name.as_bytes());
                    body.extend_from_slice(b"\"; filename=\"");
                    body.extend_from_slice(
                        filename.as_os_str().as_encoded_bytes(),
                    );
                    body.extend_from_slice(b"\"\r\n");

                    // Build Content-Type header directly
                    body.extend_from_slice(b"Content-Type: ");
                    body.extend_from_slice(content_type.as_bytes());
                    body.extend_from_slice(b"\r\n\r\n");

                    // Append file content
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
        filename: PathBuf,
        content_type: &'static str,
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
/// `application/octet-stream` for unknown extensions or non-UTF8 extensions.
fn mime_from_filename<P: AsRef<Path>>(path: P) -> &'static str {
    match path.as_ref().extension().and_then(|s| s.to_str()) {
        Some("png") => "image/png",
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        // Add other types if needed
        _ => "application/octet-stream",
    }
}

// --- Tests ---

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use std::path::PathBuf; // Add PathBuf import
    use tempfile::NamedTempFile;

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

    #[test]
    fn test_build_with_file() {
        // Create a temporary file with known content (no extension)
        let mut temp_file = NamedTempFile::new().unwrap();
        let file_content = "dummy file content";
        temp_file.write_all(file_content.as_bytes()).unwrap();
        let file_path = temp_file.path().to_path_buf();
        // Assume valid UTF-8 for test
        let filename = file_path.file_name().unwrap().to_str().unwrap();

        // Create a temporary PNG file for MIME type testing
        let mut temp_png_file =
            tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        let png_content = "png data";
        temp_png_file.write_all(png_content.as_bytes()).unwrap();
        // Assume valid UTF-8 for test
        let png_filename =
            temp_png_file.path().file_name().unwrap().to_str().unwrap();

        let boundary = "testboundary456".to_string();
        let mut builder = MultipartBuilder::with_boundary(boundary.clone());
        builder.add_text("model", "gpt-image-1");
        builder.add_file("image", temp_file.path()).unwrap();
        builder.add_file("mask", temp_png_file.path()).unwrap();

        let (body, content_type) = builder.build();
        // Use lossy conversion as body contains binary data and text parts
        let body_str = String::from_utf8_lossy(&body);

        let expected_content_type =
            format!("multipart/form-data; boundary={}", boundary);
        assert_eq!(content_type, expected_content_type);

        // Construct the expected body string manually
        let expected_body = format!(
            "--{boundary}\r\n\
             Content-Disposition: form-data; name=\"model\"\r\n\r\n\
             gpt-image-1\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"image\"; filename=\"{filename}\"\r\n\
             Content-Type: application/octet-stream\r\n\r\n\
             {file_content}\r\n\
             --{boundary}\r\n\
             Content-Disposition: form-data; name=\"mask\"; filename=\"{png_filename}\"\r\n\
             Content-Type: image/png\r\n\r\n\
             {png_content}\r\n\
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
        let path_buf = PathBuf::from("another.png");
        assert_eq!(mime_from_filename(&path_buf), "image/png");
    }

    #[test]
    fn test_empty_builder() {
        let boundary = "emptyboundary789".to_string();
        let builder = MultipartBuilder::with_boundary(boundary.clone());
        let (body, content_type) = builder.build();
        let body_str =
            String::from_utf8(body).expect("Body is not valid UTF-8");

        let expected_content_type =
            format!("multipart/form-data; boundary={}", boundary);
        assert_eq!(content_type, expected_content_type);

        let expected_body = format!("--{}--\r\n", boundary);
        assert_eq!(body_str, expected_body);
    }
}
