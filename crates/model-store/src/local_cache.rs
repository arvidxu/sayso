//! Model files already on this machine, found by checksum and still verified before use.

use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use dictation_types::{AppError, AppResult, check_relative_path};

const CHECKSUM_PLACEHOLDER: &str = "{sha256}";

const ANY_FOLDER: &str = "*";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCacheLocation {
    /// `None` starts from the home folder. A variable that is not set makes the location not apply.
    pub env_var: Option<String>,
    /// Empty when the starting folder is the cache.
    pub sub_path: String,
    /// Folders separated by `/`; `*` is any one folder, `{sha256}` the lowercase hex checksum.
    pub file_pattern: String,
}

impl LocalCacheLocation {
    pub fn validate(&self) -> AppResult<()> {
        if self
            .env_var
            .as_deref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return Err(AppError::validation(
                "a cache location's environment variable must have a name",
            ));
        }
        if !self.sub_path.is_empty() {
            check_relative_path(&self.sub_path, "cache sub path")?;
        }
        if self.file_pattern.matches(CHECKSUM_PLACEHOLDER).count() != 1 {
            return Err(AppError::validation(format!(
                "a cache file pattern must contain {CHECKSUM_PLACEHOLDER} exactly once"
            )));
        }
        for part in self.file_pattern.split('/') {
            if part != ANY_FOLDER {
                let plain = part.replace(CHECKSUM_PLACEHOLDER, "checksum");
                check_relative_path(&plain, "cache file pattern")?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalCache {
    pub dir: PathBuf,
    pub file_pattern: String,
}

/// Locations whose variable is not set or whose folder does not exist are left out.
pub fn resolve_local_caches(locations: &[LocalCacheLocation]) -> Vec<LocalCache> {
    let mut caches: Vec<LocalCache> = Vec::new();
    for location in locations {
        let base = match &location.env_var {
            Some(name) => env::var_os(name).map(PathBuf::from),
            None => env::home_dir(),
        };
        let Some(base) = base.filter(|base| !base.as_os_str().is_empty()) else {
            continue;
        };
        let cache = LocalCache {
            dir: base.join(&location.sub_path),
            file_pattern: location.file_pattern.clone(),
        };
        if cache.dir.is_dir() && !caches.contains(&cache) {
            caches.push(cache);
        }
    }
    caches
}

/// The content still has to be verified: the name only says what it should be.
pub fn find_by_checksum(caches: &[LocalCache], sha256: &str) -> Option<PathBuf> {
    let checksum = sha256.trim().to_ascii_lowercase();
    if checksum.is_empty() || !checksum.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    caches.iter().find_map(|cache| {
        let parts: Vec<String> = cache
            .file_pattern
            .split('/')
            .map(|part| part.replace(CHECKSUM_PLACEHOLDER, &checksum))
            .collect();
        find_in(&cache.dir, &parts)
    })
}

fn find_in(dir: &Path, parts: &[String]) -> Option<PathBuf> {
    let Some((first, rest)) = parts.split_first() else {
        return dir.is_file().then(|| dir.to_path_buf());
    };
    if first != ANY_FOLDER {
        return find_in(&dir.join(first), rest);
    }
    fs::read_dir(dir)
        .ok()?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.is_dir())
        .find_map(|path| find_in(&path, rest))
}
