alias w := watch-ci

gen-raw:
    curl -X POST "https://api.openai.com/v1/images/generations" \
        -H "Authorization: Bearer $OPENAI_API_KEY" \
        -H "Content-type: application/json" \
        -d '{ "model": "gpt-image-1", "prompt": "A childrens book drawing of a veterinarian using a stethoscope to listen to the heartbeat of a baby otter." }' \
        | jq .

ci:
    cargo clippy --all-targets
    cargo test

watch-ci:
    cargo watch --why --shell "just ci"

# `just release patch`
release *args:
    cargo release --no-publish {{ args }}
