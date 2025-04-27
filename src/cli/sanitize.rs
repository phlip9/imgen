/// Sanitize the prompt to create a prefix for the output files
pub fn prompt_prefix(prompt: &str) -> String {
    // Sanitize only a small prefix
    let (prefix, _) = prompt.split_at_floor_char_boundary(32);

    // Create a sanitized prefix from the prompt (first few words)
    let sanitized = prefix
        .split_whitespace()
        .map(|s| {
            s.chars()
                // ASCII: only alphanumeric chars (command case)
                // Other: passthru (handle other languages)
                .filter(|c| !c.is_ascii() || c.is_alphanumeric())
                .map(|c| c.to_ascii_lowercase())
                .collect::<String>()
        })
        .filter(|s| !s.is_empty())
        .take(5) // Take first 5 words
        .collect::<Vec<_>>()
        .join("_");

    // Ensure the prefix is not empty
    if sanitized.is_empty() {
        "imgen".to_string()
    } else {
        sanitized
    }
}

trait StrExt {
    /// Safely splits the string at `mid` (or the last valid char boundary).
    /// Unlike `std::str::split_at`, this will never panic.
    fn split_at_floor_char_boundary(&self, mid: usize) -> (&str, &str);

    /// Finds the largest `i <= index` such that `self.is_char_boundary(i)`.
    //
    // TODO(phlip9): remove when `std::str::floor_char_boundary` is stable
    fn vendored_floor_char_boundary(&self, index: usize) -> usize;
}

impl StrExt for str {
    fn split_at_floor_char_boundary(&self, mid: usize) -> (&str, &str) {
        let floor_mid = self.vendored_floor_char_boundary(mid);
        self.split_at(floor_mid)
    }

    fn vendored_floor_char_boundary(&self, index: usize) -> usize {
        if index >= self.len() {
            return self.len();
        }

        // UTF-8 code points are 1-4 bytes
        let lower_bound = index.saturating_sub(3);
        (lower_bound..=index)
            .rev()
            .find(|idx| self.is_char_boundary(*idx))
            .unwrap_or(index)
    }
}
