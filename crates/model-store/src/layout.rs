//! On-disk layout: `installed/<id>` with a record of its files, `partial/<id>/downloads` and
//! `partial/<id>/assembled`, and `trash`, through which a model is replaced or deleted in one move.

use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::process;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use dictation_types::{AppError, AppResult, ModelDescriptor, ModelId};
use serde::{Deserialize, Serialize};
use serde_json::{from_str, to_string_pretty};

use crate::disk::{create_dir, list_files, remove_dir, storage, sync_dir, sync_tree};

const RECORD_FILE: &str = ".installed.json";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct RecordedFile {
    path: String,
    sha256: String,
    size_bytes: u64,
    unpacked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Record {
    files: Vec<RecordedFile>,
    /// Every file in the model folder apart from the record, sorted by path.
    contents: Vec<(String, u64)>,
}

fn catalog_files(descriptor: &ModelDescriptor) -> Vec<RecordedFile> {
    let mut files: Vec<RecordedFile> = descriptor
        .files
        .iter()
        .map(|file| RecordedFile {
            path: file.path.clone(),
            sha256: file.sha256.to_ascii_lowercase(),
            size_bytes: file.size_bytes,
            unpacked: file.unpack.is_some(),
        })
        .collect();
    files.sort_by(|a, b| a.path.cmp(&b.path));
    files
}

fn unique_name(id: &ModelId) -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| since.as_nanos());
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{id}-{stamp}-{}-{count}", process::id())
}

#[derive(Debug, Clone)]
pub(crate) struct Layout {
    root: PathBuf,
}

impl Layout {
    /// Leftover trash is removed, except folders of a model another program is installing or
    /// removing: it may still need to move its old folder back.
    pub(crate) fn new(root: PathBuf) -> Self {
        let layout = Self { root };
        let Ok(entries) = fs::read_dir(layout.trash_dir()) else {
            return layout;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            // Trash names are `<id>-<stamp>-<process>-<count>`.
            let id = name.rsplitn(4, '-').nth(3).unwrap_or(&name);
            let lock = layout.open_lock(format!("{id}.lock"));
            if lock.is_ok_and(|lock| lock.try_lock().is_ok()) {
                let _ = remove_dir(&entry.path());
            }
        }
        layout
    }

    pub(crate) fn installed_dir(&self, id: &ModelId) -> PathBuf {
        self.root.join("installed").join(id.as_str())
    }

    pub(crate) fn partial_dir(&self, id: &ModelId) -> PathBuf {
        self.root.join("partial").join(id.as_str())
    }

    /// Locked while installing or removing `id`; kept outside the model folders, which come and go.
    pub(crate) fn lock_file(&self, id: &ModelId) -> AppResult<File> {
        self.open_lock(format!("{id}.lock"))
    }

    /// Held with a shared lock while `id` is loaded; removal needs it unshared, so a model in use
    /// anywhere stays.
    pub(crate) fn in_use_file(&self, id: &ModelId) -> AppResult<File> {
        self.open_lock(format!("{id}.in-use"))
    }

    fn open_lock(&self, name: String) -> AppResult<File> {
        let dir = self.root.join("locks");
        create_dir(&dir)?;
        let path = dir.join(name);
        File::options()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .map_err(|err| storage(&path, "open", err))
    }

    pub(crate) fn downloads_dir(&self, id: &ModelId) -> PathBuf {
        self.partial_dir(id).join("downloads")
    }

    pub(crate) fn assembled_dir(&self, id: &ModelId) -> PathBuf {
        self.partial_dir(id).join("assembled")
    }

    fn trash_dir(&self) -> PathBuf {
        self.root.join("trash")
    }

    /// True only when the record names the same catalog files, every recorded file is there with
    /// its size, and every named path exists.
    pub(crate) fn is_installed(&self, descriptor: &ModelDescriptor) -> bool {
        let dir = self.installed_dir(&descriptor.id);
        let Ok(text) = fs::read_to_string(dir.join(RECORD_FILE)) else {
            return false;
        };
        let Ok(record) = from_str::<Record>(&text) else {
            return false;
        };
        record.files == catalog_files(descriptor)
            && record.contents.iter().all(|(path, size)| {
                fs::metadata(dir.join(path)).is_ok_and(|meta| meta.is_file() && meta.len() == *size)
            })
            && descriptor
                .named_paths
                .iter()
                .all(|(_, path)| dir.join(path).exists())
    }

    /// A model installed before is replaced only once the new one is ready, and stays when moving
    /// the new one in fails.
    pub(crate) fn finish(&self, descriptor: &ModelDescriptor) -> AppResult<PathBuf> {
        let id = &descriptor.id;
        let assembled = self.assembled_dir(id);
        for (_, path) in &descriptor.named_paths {
            if !assembled.join(path).exists() {
                return Err(AppError::model(format!(
                    "model {id} has no '{path}', which the catalog names"
                )));
            }
        }
        let record = Record {
            files: catalog_files(descriptor),
            contents: list_files(&assembled)?,
        };
        let text = to_string_pretty(&record)
            .map_err(|err| AppError::internal(format!("failed to write the record: {err}")))?;
        let record_path = assembled.join(RECORD_FILE);
        fs::write(&record_path, text).map_err(|err| storage(&record_path, "write", err))?;
        sync_tree(&assembled)?;

        let installed = self.installed_dir(id);
        let parent = installed
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| self.root.clone());
        create_dir(&parent)?;
        let set_aside = self.set_aside(id)?;
        if let Err(err) = fs::rename(&assembled, &installed) {
            let failed = storage(&installed, "move into", err);
            if let Some(old) = &set_aside
                && let Err(back) = fs::rename(old, &installed)
            {
                return Err(AppError::storage(format!(
                    "{}; moving the model installed before back also failed ({back}): it is \
                     now at '{}'",
                    failed.message,
                    old.display()
                )));
            }
            return Err(failed);
        }
        sync_dir(&parent)?;
        if let Some(old) = set_aside {
            let _ = remove_dir(&old);
        }
        let _ = remove_dir(&self.partial_dir(id));
        Ok(installed)
    }

    /// `None` when nothing was installed.
    fn set_aside(&self, id: &ModelId) -> AppResult<Option<PathBuf>> {
        let installed = self.installed_dir(id);
        if !installed.exists() {
            return Ok(None);
        }
        let trash = self.trash_dir();
        create_dir(&trash)?;
        let target = trash.join(unique_name(id));
        fs::rename(&installed, &target).map_err(|err| storage(&installed, "move away", err))?;
        Ok(Some(target))
    }

    /// Moved to the trash first, so the model stops counting as installed at once.
    pub(crate) fn remove(&self, id: &ModelId) -> AppResult<()> {
        if let Some(old) = self.set_aside(id)? {
            remove_dir(&old)?;
        }
        remove_dir(&self.partial_dir(id))
    }
}
