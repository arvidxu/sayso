use std::collections::BTreeMap;

use dictation_types::{AppError, AppResult, FeatureDescriptor, FeatureId, FeatureMountPoint};

/// Feature entries: what the app shell shows and where.
#[derive(Debug, Default, Clone)]
pub struct FeatureRegistry {
    features: BTreeMap<FeatureId, FeatureDescriptor>,
}

impl FeatureRegistry {
    pub(crate) fn insert(&mut self, feature: FeatureDescriptor) -> AppResult<()> {
        if feature.id.is_empty() {
            return Err(AppError::validation("feature ID must not be empty"));
        }
        if feature.title_key.trim().is_empty() {
            return Err(AppError::validation("feature title_key must not be empty"));
        }
        if self.features.contains_key(&feature.id) {
            return Err(AppError::already_exists(format!(
                "feature '{}' is already registered",
                feature.id
            )));
        }
        self.features.insert(feature.id.clone(), feature);
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: &FeatureId) -> Option<FeatureDescriptor> {
        self.features.remove(id)
    }

    pub fn get(&self, id: &FeatureId) -> Option<&FeatureDescriptor> {
        self.features.get(id)
    }

    /// All features, sorted by `(order, id)`.
    pub fn list(&self) -> Vec<&FeatureDescriptor> {
        let mut items: Vec<&FeatureDescriptor> = self.features.values().collect();
        items.sort_by(|a, b| a.order.cmp(&b.order).then_with(|| a.id.cmp(&b.id)));
        items
    }

    /// The features at `mount_point`, sorted by `(order, id)`.
    pub fn list_by_mount_point(&self, mount_point: FeatureMountPoint) -> Vec<&FeatureDescriptor> {
        self.list()
            .into_iter()
            .filter(|f| f.mount_point == mount_point)
            .collect()
    }
}
