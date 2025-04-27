use indicatif::{MultiProgress, ProgressBar, ProgressStyle};
use std::time::Duration;

/// A RAII struct that automatically finishes the spinner when dropped.
pub struct Spinner<'a> {
    /// The global progress bar collection that's integrated with the logger.
    global_progress: &'a MultiProgress,
    /// The progress bar for this spinner.
    spinner: ProgressBar,
}

impl<'a> Spinner<'a> {
    /// Create a new "dots" spinner to indicate progress while waiting for the
    /// API response. Hooked into the global progress bar collection, which is
    /// integrated with the logger.
    ///
    /// For more spinners check out: <https://github.com/sindresorhus/cli-spinners/blob/main/spinners.json>
    pub fn new(global_progress: &'a MultiProgress) -> Self {
        let spinner = global_progress.add(ProgressBar::new_spinner());
        spinner.enable_steady_tick(Duration::from_millis(80));
        spinner.set_style(
            ProgressStyle::with_template("{spinner:.blue} {msg}")
                .unwrap()
                .tick_strings(&[
                    "⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏",
                ]),
        );
        Self {
            global_progress,
            spinner,
        }
    }

    pub fn set_message(&self, message: &'static str) {
        self.spinner.set_message(message);
    }
}

impl Drop for Spinner<'_> {
    fn drop(&mut self) {
        // Clean up the spinner
        self.spinner.finish();
        self.global_progress.remove(&self.spinner);
    }
}
