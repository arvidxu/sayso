#![allow(dead_code)]

pub mod archive;
pub mod server;

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use dictation_types::{CancelToken, DownloadProgress, ModelId, PlatformTarget};
use model_store::{DownloadTuning, LocalModelStore, ModelCatalog};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

/// Removed when dropped.
pub struct TempDir(PathBuf);

impl TempDir {
    pub fn new(label: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_nanos());
        let path = std::env::temp_dir().join(format!(
            "models-test-{label}-{}-{nanos}-{}",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::SeqCst)
        ));
        fs::create_dir_all(&path).expect("create a temporary folder");
        Self(path)
    }

    pub fn path(&self) -> &Path {
        &self.0
    }

    pub fn join(&self, part: impl AsRef<Path>) -> PathBuf {
        self.0.join(part)
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Not all alike, and different for each `seed`.
pub fn content(len: usize, seed: u8) -> Vec<u8> {
    (0..len)
        .map(|i| (i.wrapping_mul(31) as u8) ^ seed.wrapping_mul(7).wrapping_add((i / 251) as u8))
        .collect()
}

pub fn file_entry(path: &str, bytes: &[u8], urls: &[String]) -> Value {
    json!({
        "path": path,
        "sizeBytes": bytes.len(),
        "sha256": sha256_hex(bytes),
        "downloadUrls": urls,
        "unpack": null,
    })
}

pub fn model_entry(id: &str, files: Vec<Value>) -> Value {
    json!({
        "id": id,
        "name": format!("Model {id}"),
        "kind": "asr",
        "engine": "test-engine",
        "group": null,
        "providedBySystem": false,
        "platforms": PlatformTarget::ALL.map(|platform| platform.as_str()),
        "languages": ["en", "fr"],
        "files": files,
    })
}

pub fn system_model_entry(id: &str) -> Value {
    let mut entry = model_entry(id, Vec::new());
    entry["providedBySystem"] = json!(true);
    entry
}

pub fn catalog_json(models: Vec<Value>) -> String {
    json!({ "models": models }).to_string()
}

pub fn catalog(models: Vec<Value>) -> ModelCatalog {
    ModelCatalog::from_json(&catalog_json(models)).expect("a valid test catalog")
}

fn bundled_download_group() -> Value {
    serde_json::from_str::<Value>(model_store::BUNDLED_CONFIG).expect("the bundled file is JSON")
        ["download"]
        .clone()
}

/// The shipped download settings, with `overrides` (keyed as in the file) replacing its own.
pub fn tuning_with(overrides: Value) -> DownloadTuning {
    let mut file: Value =
        serde_json::from_str(model_store::BUNDLED_CONFIG).expect("the bundled file is JSON");
    let group = file["download"].as_object_mut().expect("a download group");
    for (key, value) in overrides.as_object().expect("an object of overrides") {
        group.insert(key.clone(), value.clone());
    }
    model_store::ModelsConfig::from_json(&file.to_string())
        .expect("valid download settings")
        .download
}

/// Short enough to fail fast, long enough for slow answers, with no same-address retries or waits.
pub fn test_overrides(buffer_bytes: usize) -> Value {
    let group = bundled_download_group();
    let mut overrides = json!({
        "connectTimeoutMs": 2_000,
        "readTimeoutMs": 20_000,
        "bufferBytes": buffer_bytes,
    });
    if group.get("attemptsPerAddress").is_some() {
        overrides["attemptsPerAddress"] = json!(1);
    }
    if group.get("retryDelayMs").is_some() {
        overrides["retryDelayMs"] = json!(0);
    }
    overrides
}

pub fn tuning(buffer_bytes: usize) -> DownloadTuning {
    tuning_with(test_overrides(buffer_bytes))
}

pub fn store_tuned(catalog: ModelCatalog, root: &Path, tuning: &DownloadTuning) -> LocalModelStore {
    LocalModelStore::new(catalog, root.to_path_buf(), tuning, Vec::new()).expect("a store")
}

pub fn store(catalog: ModelCatalog, root: &Path) -> LocalModelStore {
    store_with(catalog, root, Vec::new())
}

pub const CACHE_PATTERN: &str = "*/blobs/{sha256}";

pub fn caches(dirs: &[PathBuf]) -> Vec<model_store::LocalCache> {
    dirs.iter()
        .map(|dir| model_store::LocalCache {
            dir: dir.clone(),
            file_pattern: CACHE_PATTERN.to_owned(),
        })
        .collect()
}

pub fn store_with(catalog: ModelCatalog, root: &Path, cache_dirs: Vec<PathBuf>) -> LocalModelStore {
    LocalModelStore::new(
        catalog,
        root.to_path_buf(),
        &tuning(1024),
        caches(&cache_dirs),
    )
    .expect("a store")
}

pub fn id(text: &str) -> ModelId {
    ModelId::new(text)
}

pub fn ignore(_: &DownloadProgress) {}

pub fn never() -> CancelToken {
    CancelToken::new()
}

/// Sorted by relative path; the empty lock files are left out, since they hold nothing.
pub fn files_under(dir: &Path) -> Vec<(String, Vec<u8>)> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<(String, Vec<u8>)>) {
        let Ok(entries) = fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(base, &path, out);
            } else {
                let relative = path
                    .strip_prefix(base)
                    .expect("inside the folder")
                    .to_string_lossy()
                    .replace('\\', "/");
                if !relative.starts_with("locks/") {
                    out.push((relative, fs::read(&path).expect("read a file")));
                }
            }
        }
    }
    let mut out = Vec::new();
    walk(dir, dir, &mut out);
    out.sort();
    out
}
