use std::time::Duration;

use dictation_types::{
    AppError, AppResult, AudioCaptureConfig, AudioChunk, AudioDevice, InputLevel,
};

/// One recording in progress, started by [`AudioSource::start_capture`]. Audio is mono at the
/// requested sample rate; each piece is handed out once, by [`read_chunk`](Self::read_chunk)
/// or else by [`stop`](Self::stop). A recording that ends by itself keeps what it recorded.
pub trait AudioCaptureSession: Send {
    /// The next chunk of audio, waiting at most `wait` for one to be ready. `None` when none
    /// became ready in time, or at once when the recording has ended by itself and everything
    /// was read. An error after [`stop`](Self::stop) or [`cancel`](Self::cancel).
    fn read_chunk(&mut self, wait: Duration) -> AppResult<Option<AudioChunk>>;

    /// Returns the audio not read yet, which may be empty. Works after the recording ended by
    /// itself too; an error when the session was already stopped or cancelled.
    fn stop(&mut self) -> AppResult<AudioChunk>;

    /// Discards the audio not read yet. Cancelling a session that has already ended does
    /// nothing.
    fn cancel(&mut self) -> AppResult<()>;

    /// Whether the session is still taking audio: not stopped, cancelled or ended by itself.
    fn is_active(&self) -> bool;

    /// Why the recording ended by itself, if it did.
    fn interruption(&self) -> Option<AppError>;

    /// How many short glitches the device reported, after which recording went on.
    fn dropouts(&self) -> u32;

    /// Readable from anywhere while recording goes on; reads 0 once the recording has ended.
    fn level(&self) -> InputLevel;
}

pub trait AudioSource: Send + Sync {
    fn list_devices(&self) -> AppResult<Vec<AudioDevice>>;

    /// The device the system marks as default; `None` when none is marked.
    fn default_device(&self) -> AppResult<Option<AudioDevice>> {
        Ok(self.list_devices()?.into_iter().find(|d| d.is_default))
    }

    /// The session delivers audio in the format `config` asks for; the source converts from
    /// whatever the device records in.
    fn start_capture(&self, config: AudioCaptureConfig) -> AppResult<Box<dyn AudioCaptureSession>>;
}
