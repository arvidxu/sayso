use std::path::{Path, PathBuf};

use dictation_types::{
    AppResult, CancelToken, DownloadProgress, ModelDescriptor, ModelId, ModelStatus,
};

/// The model catalog and the models on this device.
pub trait ModelStore: Send + Sync {
    fn list_catalog(&self) -> AppResult<Vec<ModelDescriptor>>;

    fn get_status(&self, id: &ModelId) -> AppResult<Option<ModelStatus>>;

    fn list_statuses(&self) -> AppResult<Vec<ModelStatus>>;

    /// Downloads a catalog model and verifies every file against its checksum. A cancelled or
    /// interrupted download keeps what it has and resumes on the next call; an installed model is
    /// not downloaded again.
    fn install(
        &self,
        id: &ModelId,
        progress: &dyn Fn(&DownloadProgress),
        cancel: &CancelToken,
    ) -> AppResult<ModelStatus>;

    /// Adopts model files already on disk under the folder `path` after verifying them
    /// against the catalog's checksums. The files are copied; `path` is left as it was. A
    /// mismatch fails and marks the model corrupted.
    fn import_local(&self, id: &ModelId, path: &Path) -> AppResult<ModelStatus>;

    /// The folder holding an installed model's files, for the engine that runs it. `None`
    /// when the model is not installed.
    fn local_path(&self, id: &ModelId) -> AppResult<Option<PathBuf>>;

    /// Deletes an installed model's files, and anything kept from an unfinished download,
    /// returning whether it was installed.
    fn delete(&self, id: &ModelId) -> AppResult<bool>;

    /// Records whether an installed model is loaded in memory. The store only remembers it, for
    /// display and so that a model in use is not deleted.
    fn mark_loaded(&self, id: &ModelId, loaded: bool) -> AppResult<()>;
}
