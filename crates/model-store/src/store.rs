use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::fs::{File, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard, PoisonError};

use dictation_api::ModelStore;
use dictation_types::{
    AppError, AppResult, CancelToken, DownloadProgress, ModelDescriptor, ModelId,
    ModelInstallState, ModelStatus,
};

use crate::layout::Layout;
use crate::transfer::Downloader;
use crate::{DownloadTuning, LocalCache, ModelCatalog};

#[derive(Debug, Default)]
pub(crate) struct StoreState {
    busy: BTreeSet<ModelId>,
    /// (bytes on disk, total bytes).
    pub(crate) downloading: BTreeMap<ModelId, (u64, u64)>,
    /// Not installed and failed verification, with why.
    pub(crate) corrupted: BTreeMap<ModelId, String>,
    /// Each keeps its in-use file locked, so other programs sharing the folder leave the model.
    loaded: BTreeMap<ModelId, File>,
}

pub(crate) struct Claim<'a> {
    store: &'a LocalModelStore,
    id: ModelId,
    /// Locked so no other program using the same folder touches the model meanwhile.
    _lock: File,
}

impl<'a> Claim<'a> {
    pub(crate) fn take(store: &'a LocalModelStore, id: &ModelId) -> AppResult<Self> {
        let mut state = store.state();
        if state.busy.contains(id) {
            return Err(AppError::already_exists(format!(
                "model {id} is already being installed or removed"
            )));
        }
        let lock = store.layout.lock_file(id)?;
        match lock.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                return Err(AppError::already_exists(format!(
                    "model {id} is being installed or removed by another program"
                )));
            }
            Err(TryLockError::Error(err)) => {
                return Err(AppError::storage(format!(
                    "model {id} could not be reserved for this program: {err}"
                )));
            }
        }
        state.busy.insert(id.clone());
        Ok(Self {
            store,
            id: id.clone(),
            _lock: lock,
        })
    }

    pub(crate) fn report_downloading(&self, total_bytes: u64) {
        self.store
            .state()
            .downloading
            .insert(self.id.clone(), (0, total_bytes));
    }
}

impl Drop for Claim<'_> {
    fn drop(&mut self) {
        let mut state = self.store.state();
        state.busy.remove(&self.id);
        state.downloading.remove(&self.id);
    }
}

pub struct LocalModelStore {
    pub(crate) catalog: ModelCatalog,
    pub(crate) layout: Layout,
    pub(crate) downloader: Downloader,
    pub(crate) caches: Vec<LocalCache>,
    state: Mutex<StoreState>,
}

impl fmt::Debug for LocalModelStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("LocalModelStore")
            .field("models", &self.catalog.entries().len())
            .finish_non_exhaustive()
    }
}

impl LocalModelStore {
    /// Files are first looked for in `caches`; pass none to always download. Only one store should
    /// use a folder at a time.
    pub fn new(
        catalog: ModelCatalog,
        root: PathBuf,
        tuning: &DownloadTuning,
        caches: Vec<LocalCache>,
    ) -> AppResult<Self> {
        Ok(Self {
            catalog,
            downloader: Downloader::new(tuning)?,
            layout: Layout::new(root),
            caches,
            state: Mutex::new(StoreState::default()),
        })
    }

    pub(crate) fn state(&self) -> MutexGuard<'_, StoreState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub(crate) fn entry(&self, id: &ModelId) -> AppResult<&ModelDescriptor> {
        self.catalog
            .get(id)
            .ok_or_else(|| AppError::not_found(format!("model {id} is not in the catalog")))
    }

    /// Refuses when any program has `id` loaded; the returned lock keeps it from being loaded.
    pub(crate) fn lock_unused(&self, id: &ModelId) -> AppResult<File> {
        if self.state().loaded.contains_key(id) {
            return Err(AppError::already_exists(format!(
                "model {id} is loaded; unload it first"
            )));
        }
        let in_use = self.layout.in_use_file(id)?;
        if in_use.try_lock().is_err() {
            return Err(AppError::already_exists(format!(
                "model {id} is in use by another program"
            )));
        }
        Ok(in_use)
    }

    fn is_usable(&self, descriptor: &ModelDescriptor) -> bool {
        descriptor.provided_by_system || self.layout.is_installed(descriptor)
    }

    pub(crate) fn status_of(&self, descriptor: &ModelDescriptor) -> ModelStatus {
        let id = &descriptor.id;
        let usable = self.is_usable(descriptor);
        let state = self.state();
        let install_state = if let Some((done, total)) = state.downloading.get(id) {
            ModelInstallState::Downloading {
                downloaded_bytes: *done,
                total_bytes: *total,
            }
        } else if usable {
            let local_path = if descriptor.provided_by_system {
                String::new()
            } else {
                self.layout.installed_dir(id).to_string_lossy().into_owned()
            };
            ModelInstallState::Installed {
                local_path,
                loaded_in_memory: state.loaded.contains_key(id),
            }
        } else if let Some(reason) = state.corrupted.get(id) {
            ModelInstallState::Corrupted {
                reason: reason.clone(),
            }
        } else {
            ModelInstallState::NotInstalled
        };
        ModelStatus {
            descriptor: descriptor.clone(),
            state: install_state,
        }
    }
}

impl ModelStore for LocalModelStore {
    fn list_catalog(&self) -> AppResult<Vec<ModelDescriptor>> {
        Ok(self.catalog.entries().to_vec())
    }

    fn get_status(&self, id: &ModelId) -> AppResult<Option<ModelStatus>> {
        Ok(self.catalog.get(id).map(|entry| self.status_of(entry)))
    }

    fn list_statuses(&self) -> AppResult<Vec<ModelStatus>> {
        Ok(self
            .catalog
            .entries()
            .iter()
            .map(|entry| self.status_of(entry))
            .collect())
    }

    fn install(
        &self,
        id: &ModelId,
        progress: &dyn Fn(&DownloadProgress),
        cancel: &CancelToken,
    ) -> AppResult<ModelStatus> {
        self.install_model(id, progress, cancel)
    }

    fn import_local(&self, id: &ModelId, path: &Path) -> AppResult<ModelStatus> {
        self.import_model(id, path)
    }

    fn local_path(&self, id: &ModelId) -> AppResult<Option<PathBuf>> {
        let entry = self.entry(id)?;
        let installed = !entry.provided_by_system && self.layout.is_installed(entry);
        Ok(installed.then(|| self.layout.installed_dir(id)))
    }

    fn delete(&self, id: &ModelId) -> AppResult<bool> {
        let entry = self.entry(id)?;
        let _claim = Claim::take(self, id)?;
        let _unused = self.lock_unused(id)?;
        let was_installed = self.layout.is_installed(entry);
        self.layout.remove(id)?;
        self.state().corrupted.remove(id);
        Ok(was_installed)
    }

    fn mark_loaded(&self, id: &ModelId, loaded: bool) -> AppResult<()> {
        let entry = self.entry(id)?;
        if !loaded {
            self.state().loaded.remove(id);
            return Ok(());
        }
        if self.state().loaded.contains_key(id) {
            return Ok(());
        }
        // Locked before checking, so nothing removes the model between the check and the lock.
        let in_use = self.layout.in_use_file(id)?;
        if in_use.try_lock_shared().is_err() {
            return Err(AppError::model(format!(
                "model {id} is being replaced or removed"
            )));
        }
        if !self.is_usable(entry) {
            return Err(AppError::model(format!(
                "model {id} is not installed, so it cannot be loaded"
            )));
        }
        self.state().loaded.entry(id.clone()).or_insert(in_use);
        Ok(())
    }
}
