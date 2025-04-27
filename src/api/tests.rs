use super::*;
use serde_json::json;
use std::path::PathBuf;

#[test]
fn test_parse_response() {
    // Example response from the OpenAI API documentation
    let json_response = r#"{
        "created": 1713833628,
        "data": [
            {
                "b64_json": "base64_encoded_image_data"
            }
        ],
        "usage": {
            "total_tokens": 100,
            "input_tokens": 50,
            "output_tokens": 50,
            "input_tokens_details": {
                "text_tokens": 10,
                "image_tokens": 40
            }
        }
    }"#;

    // Parse the JSON response
    let resp: Response = serde_json::from_str(json_response).unwrap();

    // Verify the parsed data
    assert_eq!(resp.created, 1713833628);
    assert_eq!(resp.data.len(), 1);
    assert_eq!(resp.data[0].b64_json, "base64_encoded_image_data");
    assert_eq!(resp.usage.total_tokens, 100);
    assert_eq!(resp.usage.input_tokens, 50);
    assert_eq!(resp.usage.output_tokens, 50);
    assert_eq!(resp.usage.input_tokens_details.text_tokens, 10);
    assert_eq!(resp.usage.input_tokens_details.image_tokens, 40);
}

#[test]
fn test_create_request_serialization() {
    // Create a request object
    let request = CreateRequest {
        model: "gpt-image-1".to_string(),
        prompt: "A cute baby sea otter".to_string(),
        n: Some(1),
        size: Some("1024x1024".to_string()),
        quality: None,
        background: None,
        moderation: None,
        output_compression: None,
        output_format: None,
    };

    // Serialize to JSON
    let json = serde_json::to_value(&request).unwrap();

    // Expected JSON based on the OpenAI API documentation example
    let expected_json = json!({
      "model": "gpt-image-1",
      "prompt": "A cute baby sea otter",
      "n": 1,
      "size": "1024x1024"
    });

    // Compare the serialized JSON with the expected JSON
    assert_eq!(json, expected_json);
}

#[test]
fn test_decode_response() {
    // Create a simple base64 string (this is "test" encoded in base64)
    let b64_data = "dGVzdA==";

    // Create a response with base64 data
    let response = Response {
        created: 1713833628,
        data: vec![ImageData {
            b64_json: b64_data.to_string(),
        }],
        usage: Usage {
            total_tokens: 100,
            input_tokens: 50,
            output_tokens: 50,
            input_tokens_details: InputTokensDetails {
                text_tokens: 10,
                image_tokens: 40,
            },
        },
    };

    // Convert to decoded response
    let decoded = DecodedResponse::try_from(response).unwrap();

    // Check that the data was decoded correctly
    assert_eq!(decoded.data.len(), 1);
    assert_eq!(decoded.data[0].image_bytes, b"test");
    assert_eq!(decoded.created, 1713833628);
    assert_eq!(decoded.usage.total_tokens, 100);
}

#[test]
fn test_edit_request_build_multipart() {
    let input_image = InputImageData {
        bytes: b"dummy image".to_vec(),
        filename: PathBuf::from("test_image.jpg"),
        content_type: "image/jpeg",
    };

    let input_mask = InputImageData {
        bytes: b"dummy mask".to_vec(),
        filename: PathBuf::from("test_mask.png"),
        content_type: "image/png",
    };

    // Create an EditRequest
    let request = EditRequest {
        images: vec![input_image.clone()],
        prompt: "A test edit prompt".to_string(),
        mask: Some(input_mask.clone()),
        model: "gpt-image-1".to_string(),
        n: Some(2),
        quality: Some("high".to_string()),
        size: Some("1024x1024".to_string()),
    };

    // Build the multipart body
    // We need to know the boundary to compare the body, so we use a fixed one.
    // However, the actual `build_multipart` uses `multipart::Builder::new()` which
    // generates a random boundary. To test this properly, we'd need to either:
    // a) Expose `multipart::Builder::with_boundary` outside of `#[cfg(test)]`
    // b) Parse the boundary from the returned content_type and use it in the expected body.
    // Let's go with option (b) as it tests the production code path more closely.

    let boundary = "----12345";
    let multipart_body = request.build_multipart_inner(boundary.to_owned());

    // Extract the boundary from the content type
    let content_type = multipart_body.content_type;
    assert!(content_type.starts_with("multipart/form-data; boundary="));
    let boundary = content_type
        .split('=')
        .nth(1)
        .expect("Boundary not found in Content-Type");

    // Convert body bytes to string for comparison (lossy for file content)
    let body_str = String::from_utf8_lossy(&multipart_body.body);

    // Construct the expected body string using the extracted boundary
    let image_filename = input_image.filename.display();
    let image_content = String::from_utf8(input_image.bytes).unwrap();
    let mask_filename = input_mask.filename.display();
    let mask_content = String::from_utf8(input_mask.bytes).unwrap();
    let expected_body = format!(
        "--{boundary}\r\n\
         Content-Disposition: form-data; name=\"prompt\"\r\n\r\n\
         A test edit prompt\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"model\"\r\n\r\n\
         gpt-image-1\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"n\"\r\n\r\n\
         2\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"quality\"\r\n\r\n\
         high\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"size\"\r\n\r\n\
         1024x1024\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"image[]\"; filename=\"{image_filename}\"\r\n\
         Content-Type: image/jpeg\r\n\r\n\
         {image_content}\r\n\
         --{boundary}\r\n\
         Content-Disposition: form-data; name=\"mask\"; filename=\"{mask_filename}\"\r\n\
         Content-Type: image/png\r\n\r\n\
         {mask_content}\r\n\
         --{boundary}--\r\n"
    );

    // Compare the generated body with the expected body
    assert_eq!(body_str, expected_body);
}
