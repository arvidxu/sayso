//! The traits between the dictation flow and the parts that do the work. The test doubles in
//! [`fakes`] need the `fakes` feature; shipped code must not enable it.

mod audio;
mod clock;
mod context;
mod dictionary;
mod history;
mod hotkeys;
mod injection;
mod llm;
mod models;
mod permissions;
mod secrets;
mod settings;
mod speech;

#[cfg(feature = "fakes")]
pub mod fakes;

pub use audio::{AudioCaptureSession, AudioSource};
pub use clock::Clock;
pub use context::ContextProvider;
pub use dictionary::DictionaryStore;
pub use history::HistoryStore;
pub use hotkeys::{HotkeySource, ShortcutListener};
pub use injection::TextInjector;
pub use llm::{BoxStream, LlmEngine};
pub use models::ModelStore;
pub use permissions::PermissionProvider;
pub use secrets::SecretStore;
pub use settings::SettingsStore;
pub use speech::{AsrEngine, AsrSession};
