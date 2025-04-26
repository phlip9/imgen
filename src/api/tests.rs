use super::*;
use serde_json::json;

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
