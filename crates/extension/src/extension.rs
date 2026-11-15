use crate::Registry;

use dictation_types::AppResult;

pub trait Extension: Send + Sync {
    /// Unique among loaded extensions, ignoring ASCII letter case.
    fn id(&self) -> &str;

    /// Returning an error undoes everything registered so far.
    fn register(&self, registry: &mut Registry) -> AppResult<()>;
}
