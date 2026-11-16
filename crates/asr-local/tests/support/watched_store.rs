use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dictation_api::ModelStore;
use dictation_api::fakes::FakeModelStore;
use dictation_types::{
    AppError, AppResult, CancelToken, DownloadProgress, ModelDescriptor, ModelId, ModelStatus,
};

use super::locked;

/// Wraps an in-memory store, records loaded/unloaded marks, and can refuse a mark.
#[derive(Clone)]
pub struct WatchedStore {
    inner: FakeModelStore,
    marks: Arc<Mutex<Vec<bool>>>,
    mark_errors: Arc<Mutex<VecDeque<AppError>>>,
}

impl WatchedStore {
    pub fn new(inner: FakeModelStore) -> Self {
        Self {
            inner,
            marks: Arc::new(Mutex::new(Vec::new())),
            mark_errors: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// `true` for loaded, `false` for unloaded, in order.
    pub fn marks(&self) -> Vec<bool> {
        locked(&self.marks).clone()
    }

    pub fn refuse_next_mark_loaded(&self, error: AppError) {
        locked(&self.mark_errors).push_back(error);
    }
}

impl ModelStore for WatchedStore {
    fn list_catalog(&self) -> AppResult<Vec<ModelDescriptor>> {
        self.inner.list_catalog()
    }

    fn get_status(&self, id: &ModelId) -> AppResult<Option<ModelStatus>> {
        self.inner.get_status(id)
    }

    fn list_statuses(&self) -> AppResult<Vec<ModelStatus>> {
        self.inner.list_statuses()
    }

    fn install(
        &self,
        id: &ModelId,
        progress: &dyn Fn(&DownloadProgress),
        cancel: &CancelToken,
    ) -> AppResult<ModelStatus> {
        self.inner.install(id, progress, cancel)
    }

    fn import_local(&self, id: &ModelId, path: &Path) -> AppResult<ModelStatus> {
        self.inner.import_local(id, path)
    }

    fn local_path(&self, id: &ModelId) -> AppResult<Option<PathBuf>> {
        self.inner.local_path(id)
    }

    fn delete(&self, id: &ModelId) -> AppResult<bool> {
        self.inner.delete(id)
    }

    fn mark_loaded(&self, id: &ModelId, loaded: bool) -> AppResult<()> {
        locked(&self.marks).push(loaded);
        if loaded && let Some(error) = locked(&self.mark_errors).pop_front() {
            return Err(error);
        }
        self.inner.mark_loaded(id, loaded)
    }
}
