//! On-device speech recognition: shared engine logic plus one small backend per engine.

mod backend;
mod engine;
pub mod error_rate;
mod language_names;
pub mod model_facts;
mod session;
#[cfg(feature = "sherpa-onnx")]
pub mod sherpa_onnx;
mod tuning;

use std::sync::Arc;

pub use backend::{ModelBackend, SpeechModel};
pub use engine::LocalAsrEngine;
pub use language_names::LanguageNames;
pub use model_facts::capabilities_of;
pub use tuning::{BUNDLED_CONFIG, RecognitionTuning};

pub fn built_in_backends() -> Vec<Arc<dyn ModelBackend>> {
    vec![
        #[cfg(feature = "sherpa-onnx")]
        Arc::new(sherpa_onnx::SherpaOnnxBackend::new()),
    ]
}
