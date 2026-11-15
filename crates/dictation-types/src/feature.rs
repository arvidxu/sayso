use crate::{ExtensionId, FeatureId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum FeatureMountPoint {
    NavigationRoute,
    SettingsSection,
    FloatingPanel,
    TrayMenuItem,
}

/// The app shell looks up what to render by `id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FeatureDescriptor {
    pub id: FeatureId,
    pub title_key: String,
    pub mount_point: FeatureMountPoint,
    pub route_path: Option<String>,
    pub order: i32,
    pub enabled_by_default: bool,
    /// `None` for built-in features.
    pub extension_id: Option<ExtensionId>,
}

impl FeatureDescriptor {
    pub fn new(
        id: FeatureId,
        title_key: impl Into<String>,
        mount_point: FeatureMountPoint,
        enabled_by_default: bool,
    ) -> Self {
        Self {
            id,
            title_key: title_key.into(),
            mount_point,
            route_path: None,
            order: 0,
            enabled_by_default,
            extension_id: None,
        }
    }

    pub fn with_route(mut self, route_path: impl Into<String>) -> Self {
        self.route_path = Some(route_path.into());
        self
    }

    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }
}
