//! The installable models: data read from the catalog file, never written into code.

use std::env::consts::{ARCH, OS};
use std::fs;
use std::path::Path;

use dictation_types::{AppError, AppResult, ModelDescriptor, ModelId, PlatformTarget};

use crate::catalog_file;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelCatalog {
    entries: Vec<ModelDescriptor>,
}

/// Usable as a folder name on every supported system, including not being a reserved device name.
fn is_folder_safe(id: &str) -> bool {
    let stem = id.split('.').next().unwrap_or(id).to_ascii_uppercase();
    let device = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.ends_with(|ch: char| ch.is_ascii_digit() && ch != '0'));
    !id.is_empty()
        && !id.starts_with('.')
        && !id.ends_with('.')
        && !device
        && id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '.' | '-' | '_'))
}

impl ModelCatalog {
    /// Every entry has to be valid, and no two IDs may differ only in letter case.
    pub fn new(entries: Vec<ModelDescriptor>) -> AppResult<Self> {
        let mut seen: Vec<String> = Vec::new();
        for entry in &entries {
            entry.validate()?;
            let id = entry.id.as_str();
            if !is_folder_safe(id) {
                return Err(AppError::validation(format!(
                    "model id '{id}' may only use letters, digits, '.', '-' and '_', may not \
                     start or end with '.', and may not be a device name such as 'con'"
                )));
            }
            let key = id.to_ascii_lowercase();
            if seen.contains(&key) {
                return Err(AppError::validation(format!(
                    "the catalog lists the model '{id}' twice"
                )));
            }
            seen.push(key);
        }
        Ok(Self { entries })
    }

    pub fn from_json(text: &str) -> AppResult<Self> {
        Self::new(catalog_file::parse(text)?)
    }

    pub fn load(path: &Path) -> AppResult<Self> {
        let text = fs::read_to_string(path).map_err(|err| {
            AppError::storage(format!(
                "failed to read the model catalog '{}': {err}",
                path.display()
            ))
        })?;
        Self::from_json(&text)
    }

    pub fn entries(&self) -> &[ModelDescriptor] {
        &self.entries
    }

    pub fn get(&self, id: &ModelId) -> Option<&ModelDescriptor> {
        self.entries.iter().find(|entry| &entry.id == id)
    }

    pub fn for_platform(&self, platform: PlatformTarget) -> Vec<&ModelDescriptor> {
        self.entries
            .iter()
            .filter(|entry| entry.supported_platforms.contains(&platform))
            .collect()
    }
}

/// `None` for a platform the catalog has no name for.
pub fn current_platform() -> Option<PlatformTarget> {
    let platform = match (OS, ARCH) {
        ("macos", "aarch64") => PlatformTarget::MacosArm64,
        ("macos", "x86_64") => PlatformTarget::MacosX64,
        ("windows", "x86_64") => PlatformTarget::WindowsX64,
        ("windows", "aarch64") => PlatformTarget::WindowsArm64,
        ("linux", "x86_64") => PlatformTarget::LinuxX64,
        ("linux", "aarch64") => PlatformTarget::LinuxArm64,
        ("ios", "aarch64") => PlatformTarget::IosArm64,
        ("android", "aarch64") => PlatformTarget::AndroidArm64,
        _ => return None,
    };
    Some(platform)
}
