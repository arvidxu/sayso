use crate::relative_path::check_relative_path;
use crate::{AppError, AppResult, Language, ModelId, ProviderId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ModelKind {
    Asr,
    Llm,
    Vad,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum PlatformTarget {
    MacosArm64,
    MacosX64,
    WindowsX64,
    WindowsArm64,
    LinuxX64,
    LinuxArm64,
    IosArm64,
    AndroidArm64,
}

named_values!(PlatformTarget, "platform target", {
    MacosArm64 => "macos-arm64",
    MacosX64 => "macos-x64",
    WindowsX64 => "windows-x64",
    WindowsArm64 => "windows-arm64",
    LinuxX64 => "linux-x64",
    LinuxArm64 => "linux-arm64",
    IosArm64 => "ios-arm64",
    AndroidArm64 => "android-arm64",
});

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ArchiveFormat {
    TarBz2,
}

named_values!(ArchiveFormat, "archive format", {
    TarBz2 => "tar.bz2",
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelFile {
    /// Relative to the model's folder, without `..`.
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
    /// In order of preference; later ones are fallbacks such as mirrors.
    pub download_urls: Vec<String>,
    /// If set, the archive is unpacked into the model's folder after verification, then deleted.
    pub unpack: Option<ArchiveFormat>,
}

impl ModelFile {
    pub fn validate(&self) -> AppResult<()> {
        check_relative_path(&self.path, "model file path")?;
        if self.size_bytes == 0 {
            return Err(AppError::validation(
                "model file size_bytes must be greater than zero",
            ));
        }
        if !is_valid_sha256_hex(&self.sha256) {
            return Err(AppError::validation(
                "model file sha256 must be a 64-character hexadecimal string",
            ));
        }
        let is_url = |u: &String| {
            u.split_once("://")
                .is_some_and(|(scheme, rest)| !scheme.is_empty() && !rest.is_empty())
                && !u.chars().any(char::is_whitespace)
        };
        if self.download_urls.is_empty() || !self.download_urls.iter().all(is_url) {
            return Err(AppError::validation(
                "model file must have at least one download URL, each of the form scheme://address",
            ));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelDescriptor {
    pub id: ModelId,
    pub name: String,
    pub kind: ModelKind,
    /// Files are in this engine's format, so one model packed for two engines is two entries.
    pub engine: ProviderId,
    /// A presentation group chosen by the catalog, for example by speed or accuracy.
    pub group: Option<String>,
    /// Empty only for a system-provided model.
    pub files: Vec<ModelFile>,
    pub provided_by_system: bool,
    /// Paths in the installed folder the engine needs, keyed by names the engine knows.
    pub named_paths: Vec<(String, String)>,
    /// Model facts only the engine understands and checks, by name.
    pub engine_settings: Vec<(String, String)>,
    pub supported_platforms: Vec<PlatformTarget>,
    pub supported_languages: Vec<Language>,
}

impl ModelDescriptor {
    /// Relative to the installed model's folder.
    pub fn named_path(&self, name: &str) -> Option<&str> {
        self.named_paths
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, path)| path.as_str())
    }

    pub fn engine_setting(&self, name: &str) -> Option<&str> {
        self.engine_settings
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, value)| value.as_str())
    }

    /// The total download size.
    pub fn size_bytes(&self) -> u64 {
        self.files
            .iter()
            .fold(0u64, |sum, file| sum.saturating_add(file.size_bytes))
    }

    /// Needs at least one platform, none repeated, and either valid files whose paths differ by
    /// more than case, or the system-provided flag with no files.
    pub fn validate(&self) -> AppResult<()> {
        if self.id.is_empty() {
            return Err(AppError::validation(
                "model descriptor id must not be empty",
            ));
        }
        if self.name.trim().is_empty() {
            return Err(AppError::validation(
                "model descriptor name must not be empty",
            ));
        }
        if self.engine.is_empty() {
            return Err(AppError::validation(
                "model descriptor must name the engine that runs it",
            ));
        }
        if self.supported_platforms.is_empty() {
            return Err(AppError::validation(
                "model descriptor must specify at least one supported platform",
            ));
        }
        if self.files.is_empty() && !self.provided_by_system {
            return Err(AppError::validation(
                "a downloadable model must list at least one file",
            ));
        }
        if self.provided_by_system && !self.files.is_empty() {
            return Err(AppError::validation(
                "a model provided by the system must not list files to download",
            ));
        }
        let mut platforms = self.supported_platforms.clone();
        platforms.sort();
        platforms.dedup();
        if platforms.len() != self.supported_platforms.len() {
            return Err(AppError::validation(
                "model descriptor lists a platform twice",
            ));
        }
        for (position, (name, path)) in self.named_paths.iter().enumerate() {
            if name.trim().is_empty() || name != name.trim() {
                return Err(AppError::validation(
                    "a named path must have a name without surrounding spaces",
                ));
            }
            if self.named_paths[..position].iter().any(|(n, _)| n == name) {
                return Err(AppError::validation(format!(
                    "model names the path '{name}' twice"
                )));
            }
            check_relative_path(path, "named path")?;
        }
        for (position, (name, _)) in self.engine_settings.iter().enumerate() {
            if name.trim().is_empty() || name != name.trim() {
                return Err(AppError::validation(
                    "an engine setting must have a name without surrounding spaces",
                ));
            }
            if self.engine_settings[..position]
                .iter()
                .any(|(n, _)| n == name)
            {
                return Err(AppError::validation(format!(
                    "model gives the engine setting '{name}' twice"
                )));
            }
        }
        // Case-insensitive file systems would let such files overwrite each other, and a path
        // cannot be both a file and another's folder.
        let mut paths: Vec<String> = Vec::new();
        for file in &self.files {
            file.validate()?;
            let key = file.path.to_lowercase();
            let nested = |outer: &str, inner: &str| {
                inner
                    .strip_prefix(outer)
                    .is_some_and(|rest| rest.starts_with('/'))
            };
            if paths
                .iter()
                .any(|path| *path == key || nested(path, &key) || nested(&key, path))
            {
                return Err(AppError::validation(format!(
                    "model lists the file '{}' twice, or as the folder of another file",
                    file.path
                )));
            }
            paths.push(key);
        }
        Ok(())
    }
}

pub fn is_valid_sha256_hex(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit())
}

/// Whether a model is on disk and in memory.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ModelInstallState {
    #[default]
    NotInstalled,
    Downloading {
        downloaded_bytes: u64,
        total_bytes: u64,
    },
    Installed {
        local_path: String,
        loaded_in_memory: bool,
    },
    Corrupted {
        reason: String,
    },
}

impl ModelInstallState {
    pub fn is_installed(&self) -> bool {
        matches!(self, Self::Installed { .. })
    }

    pub fn is_loaded(&self) -> bool {
        matches!(
            self,
            Self::Installed {
                loaded_in_memory: true,
                ..
            }
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InstallPhase {
    Downloading,
    Verifying,
    Unpacking,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadProgress {
    pub model_id: ModelId,
    pub phase: InstallPhase,
    /// As listed in the catalog.
    pub file_path: String,
    /// Of the whole model, including bytes present before a resumed download.
    pub downloaded_bytes: u64,
    /// Of the whole model.
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelStatus {
    pub descriptor: ModelDescriptor,
    pub state: ModelInstallState,
}
