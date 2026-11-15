//! Capturing audio, converting its format and finding speech in it. The target sample rate
//! always comes from the caller; nothing here knows about a particular engine.

mod capture_buffer;
pub mod capture_device;
mod config;
mod device_inbox;
mod measure;
mod microphone;
mod microphone_session;
mod mono;
mod resample;
mod segment;
mod speech_scorer;
mod speech_span;
mod stream_resampler;
mod tuning;
mod voice_detection;

pub use capture_buffer::CaptureBuffer;
pub use config::{AudioConfig, BUNDLED_CONFIG};
pub use device_inbox::DeviceInbox;
pub use measure::loudness;
pub use microphone::MicrophoneSource;
pub use microphone_session::MicrophoneSession;
pub use mono::downmix_to_mono;
pub use resample::resample_to_mono;
pub use segment::{AudioSegment, SegmentationConfig, segment_speech};
pub use speech_scorer::NeuralSpeechScorer;
pub use speech_span::{SpeechSpan, keep_speech};
pub use stream_resampler::StreamResampler;
pub use tuning::{CaptureTuning, ResampleTuning, SegmentationTuning};
pub use voice_detection::{FrameScorer, VoiceDetectionConfig, VoiceDetector};
