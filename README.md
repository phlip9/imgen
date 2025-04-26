# imgen

`imgen` is a clean and simple Rust cli tool for generating and editing images
using OpenAI's `gpt-image-1` image generation models.

Minimal dependencies. Uses:
* `ureq` for HTTP requests
* `serde` for JSON serialization
* `clap` for command line argument parsing
* `dotenvy` for .env convenience
* `indicatif` for progress spinners
* `indicatif-log-bridge` to integrate logging with spinners
