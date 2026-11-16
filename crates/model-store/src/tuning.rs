//! None of these values is built in: they come from the shipped configuration or the caller.

use dictation_types::{AppError, AppResult};

#[derive(Debug, Clone, PartialEq)]
pub struct DownloadTuning {
    /// How long to wait for a connection to a download address before that try fails; failed
    /// tries count towards moving on to the next address.
    pub connect_timeout_ms: u64,
    /// Applies to each read, not the whole download, so a large file can take as long as it needs.
    pub read_timeout_ms: u64,
    /// Progress is reported and cancellation checked once per read.
    pub buffer_bytes: usize,
    /// Failed tries in a row that bring no new bytes before moving on; a try with new bytes resets
    /// the count, so a server that keeps dropping connections still delivers. At least one.
    pub attempts_per_address: u32,
    /// How long to wait before trying the same address again.
    pub retry_delay_ms: u64,
    /// Unpacked bytes per archive byte for the room check; should err high, since model weights
    /// barely compress. Above zero.
    pub unpacked_size_ratio: f64,
}

impl DownloadTuning {
    pub fn validate(&self) -> AppResult<()> {
        for (name, value) in [
            ("connect_timeout_ms", self.connect_timeout_ms),
            ("read_timeout_ms", self.read_timeout_ms),
            ("buffer_bytes", self.buffer_bytes as u64),
            ("attempts_per_address", u64::from(self.attempts_per_address)),
        ] {
            if value == 0 {
                return Err(AppError::validation(format!(
                    "{name} must be greater than zero"
                )));
            }
        }
        if !(self.unpacked_size_ratio.is_finite() && self.unpacked_size_ratio > 0.0) {
            return Err(AppError::validation(
                "unpacked_size_ratio must be a number above zero",
            ));
        }
        Ok(())
    }

    pub fn unpacked_bytes(&self, archive_bytes: u64) -> u64 {
        // Rounded up and saturating, so the estimate is never short.
        (archive_bytes as f64 * self.unpacked_size_ratio).ceil() as u64
    }
}
