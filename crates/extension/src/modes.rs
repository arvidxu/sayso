use std::collections::BTreeMap;

use dictation_types::{AppError, AppResult, Mode, ModeId};

#[derive(Debug, Default, Clone)]
pub struct ModeRegistry {
    modes: BTreeMap<ModeId, Mode>,
}

impl ModeRegistry {
    pub(crate) fn insert(&mut self, mode: Mode, replace: bool) -> AppResult<()> {
        mode.validate()?;
        if !replace && self.modes.contains_key(&mode.id) {
            return Err(AppError::already_exists(format!(
                "mode '{}' is already registered",
                mode.id
            )));
        }
        self.modes.insert(mode.id.clone(), mode);
        Ok(())
    }

    pub(crate) fn remove(&mut self, id: &ModeId) -> Option<Mode> {
        self.modes.remove(id)
    }

    pub fn get(&self, id: &ModeId) -> Option<&Mode> {
        self.modes.get(id)
    }

    pub fn list(&self) -> Vec<&Mode> {
        self.modes.values().collect()
    }
}
