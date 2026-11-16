//! Installing, verifying and tracking models on this device; which models exist is catalog data.

pub mod archive;
mod catalog;
mod catalog_file;
mod checksum;
mod config;
mod disk;
mod install;
mod layout;
pub mod local_cache;
mod store;
mod transfer;
mod tuning;

pub use catalog::{ModelCatalog, current_platform};
pub use config::{BUNDLED_CONFIG, ModelsConfig};
pub use local_cache::{LocalCache, LocalCacheLocation, resolve_local_caches};
pub use store::LocalModelStore;
pub use tuning::DownloadTuning;
