use crate::insertion::SecureInput;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn IsSecureEventInputEnabled() -> u8;
}

/// Whether secure input is on, as a password field or secure keyboard entry turns it on. While
/// it is, global shortcuts are not seen.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemSecureInput;

impl SecureInput for SystemSecureInput {
    fn is_active(&self) -> bool {
        // SAFETY: takes no arguments and only reads system state.
        unsafe { IsSecureEventInputEnabled() != 0 }
    }
}
