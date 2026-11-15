use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use dictation_types::{
    AppError, AppResult, CancelToken, DownloadProgress, InstallPhase, ModelDescriptor, ModelId,
    ModelInstallState, ModelStatus,
};

use super::lock::lock_unpoisoned;
use crate::ModelStore;

#[derive(Debug)]
struct FakeModelEntry {
    descriptor: ModelDescriptor,
    state: ModelInstallState,
}

#[derive(Debug, Default)]
struct FakeModelStoreState {
    models: BTreeMap<ModelId, FakeModelEntry>,
}

/// Nothing is actually downloaded, copied or hashed.
#[derive(Debug, Clone, Default)]
pub struct FakeModelStore {
    state: Arc<Mutex<FakeModelStoreState>>,
}

impl FakeModelStore {
    pub fn new(catalog: Vec<ModelDescriptor>) -> Self {
        let mut models = BTreeMap::new();
        for descriptor in catalog {
            models.insert(
                descriptor.id.clone(),
                FakeModelEntry {
                    descriptor,
                    state: ModelInstallState::NotInstalled,
                },
            );
        }
        Self {
            state: Arc::new(Mutex::new(FakeModelStoreState { models })),
        }
    }
}

impl ModelStore for FakeModelStore {
    fn list_catalog(&self) -> AppResult<Vec<ModelDescriptor>> {
        let guard = lock_unpoisoned(&self.state);
        Ok(guard
            .models
            .values()
            .map(|e| e.descriptor.clone())
            .collect())
    }

    fn get_status(&self, id: &ModelId) -> AppResult<Option<ModelStatus>> {
        let guard = lock_unpoisoned(&self.state);
        Ok(guard.models.get(id).map(|e| ModelStatus {
            descriptor: e.descriptor.clone(),
            state: e.state.clone(),
        }))
    }

    fn list_statuses(&self) -> AppResult<Vec<ModelStatus>> {
        let guard = lock_unpoisoned(&self.state);
        Ok(guard
            .models
            .values()
            .map(|e| ModelStatus {
                descriptor: e.descriptor.clone(),
                state: e.state.clone(),
            })
            .collect())
    }

    fn install(
        &self,
        id: &ModelId,
        progress: &dyn Fn(&DownloadProgress),
        cancel: &CancelToken,
    ) -> AppResult<ModelStatus> {
        let descriptor = {
            let mut guard = lock_unpoisoned(&self.state);
            let entry = guard
                .models
                .get_mut(id)
                .ok_or_else(|| AppError::not_found(format!("model {id} not found in catalog")))?;
            entry.descriptor.validate()?;
            if entry.state.is_installed() {
                return Ok(ModelStatus {
                    descriptor: entry.descriptor.clone(),
                    state: entry.state.clone(),
                });
            }
            entry.descriptor.clone()
        };
        // Progress is reported without holding the lock, so the callback may use the store.
        let total_bytes = descriptor.size_bytes();
        let mut downloaded_bytes = 0;
        for file in &descriptor.files {
            if cancel.is_cancelled() {
                return Err(AppError::cancelled(format!(
                    "installing model {id} was cancelled"
                )));
            }
            downloaded_bytes += file.size_bytes;
            progress(&DownloadProgress {
                model_id: id.clone(),
                phase: InstallPhase::Downloading,
                file_path: file.path.clone(),
                downloaded_bytes,
                total_bytes,
            });
        }
        if cancel.is_cancelled() {
            return Err(AppError::cancelled(format!(
                "installing model {id} was cancelled"
            )));
        }
        let mut guard = lock_unpoisoned(&self.state);
        let entry = guard
            .models
            .get_mut(id)
            .ok_or_else(|| AppError::not_found(format!("model {id} not found in catalog")))?;
        entry.state = ModelInstallState::Installed {
            local_path: format!("memory://models/{id}"),
            loaded_in_memory: false,
        };
        Ok(ModelStatus {
            descriptor: entry.descriptor.clone(),
            state: entry.state.clone(),
        })
    }

    fn import_local(&self, id: &ModelId, path: &Path) -> AppResult<ModelStatus> {
        let mut guard = lock_unpoisoned(&self.state);
        let entry = guard
            .models
            .get_mut(id)
            .ok_or_else(|| AppError::not_found(format!("model {id} not found in catalog")))?;
        entry.descriptor.validate()?;
        if entry.state.is_loaded() {
            return Err(AppError::already_exists(format!(
                "model {id} is loaded; unload it before replacing it"
            )));
        }
        entry.state = ModelInstallState::Installed {
            local_path: path.to_string_lossy().into_owned(),
            loaded_in_memory: false,
        };
        Ok(ModelStatus {
            descriptor: entry.descriptor.clone(),
            state: entry.state.clone(),
        })
    }

    fn local_path(&self, id: &ModelId) -> AppResult<Option<PathBuf>> {
        let guard = lock_unpoisoned(&self.state);
        Ok(guard.models.get(id).and_then(|entry| match &entry.state {
            ModelInstallState::Installed { local_path, .. } => Some(PathBuf::from(local_path)),
            _ => None,
        }))
    }

    fn delete(&self, id: &ModelId) -> AppResult<bool> {
        let mut guard = lock_unpoisoned(&self.state);
        let Some(entry) = guard.models.get_mut(id) else {
            return Ok(false);
        };
        if entry.state.is_loaded() {
            return Err(AppError::already_exists(format!(
                "model {id} is loaded; unload it before deleting it"
            )));
        }
        let was_installed = entry.state.is_installed();
        entry.state = ModelInstallState::NotInstalled;
        Ok(was_installed)
    }

    fn mark_loaded(&self, id: &ModelId, loaded: bool) -> AppResult<()> {
        let mut guard = lock_unpoisoned(&self.state);
        let entry = guard
            .models
            .get_mut(id)
            .ok_or_else(|| AppError::not_found(format!("model {id} not found in catalog")))?;
        match &mut entry.state {
            ModelInstallState::Installed {
                loaded_in_memory, ..
            } => {
                *loaded_in_memory = loaded;
                Ok(())
            }
            // Like the real store, a model that is not installed is not loaded, so marking it
            // unloaded succeeds and changes nothing.
            _ if !loaded => Ok(()),
            _ => Err(AppError::model(format!(
                "cannot mark uninstalled model {id} as loaded"
            ))),
        }
    }
}
