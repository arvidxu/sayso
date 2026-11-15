use dictation_types::{AppResult, PermissionKind, PermissionReport};

pub trait PermissionProvider: Send + Sync {
    /// The permissions this platform requires and whether each is granted.
    fn permissions(&self) -> AppResult<PermissionReport>;

    /// Asks the system to show its own request for `kind`. It returns without waiting for the
    /// answer, so check [`Self::permissions`] again afterwards. Once the user has said no, most
    /// systems stop asking, and the permission can only be granted in the system settings.
    fn request(&self, kind: PermissionKind) -> AppResult<()>;

    fn open_settings(&self, kind: PermissionKind) -> AppResult<()>;
}
