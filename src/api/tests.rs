use super::*;

#[test]
fn test_parse_create_response() {
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
    let resp: CreateResponse =
        serde_json::from_str(json_response).expect("Failed to parse JSON");

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
