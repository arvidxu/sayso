//! Registries of providers, modes and feature entries, and the [`Extension`] trait through
//! which an add-on contributes to them. Loading an extension is all-or-nothing.

mod extension;
mod features;
mod modes;
mod providers;
mod registry;

pub use extension::Extension;
pub use features::FeatureRegistry;
pub use modes::ModeRegistry;
pub use providers::ProviderRegistry;
pub use registry::Registry;
