//! Which key pastes on the active keyboard layout. The layout functions must be called on the
//! main thread.

use std::ffi::c_void;
use std::ptr::NonNull;

use objc2_core_foundation::{CFData, CFRetained, CFString, CFType};

use crate::keys::{VirtualKey, paste_key_among};
use crate::keys_data::layout;

#[link(name = "Carbon", kind = "framework")]
unsafe extern "C" {
    fn TISCopyCurrentKeyboardLayoutInputSource() -> *mut c_void;
    fn TISGetInputSourceProperty(source: *const c_void, key: &CFString) -> *const c_void;
    static kTISPropertyUnicodeKeyLayoutData: &'static CFString;
    fn LMGetKbdType() -> u8;
    fn UCKeyTranslate(
        layout: *const c_void,
        key: u16,
        action: u16,
        modifiers: u32,
        keyboard_type: u32,
        options: u32,
        dead_key_state: *mut u32,
        max_length: usize,
        actual_length: *mut usize,
        units: *mut u16,
    ) -> i32;
}

/// The key that yields `v` with Command held. Command matters: many layouts that type no Latin
/// letters switch to US positions while it is held, and some Latin layouts move V regardless.
pub(crate) fn paste_key() -> VirtualKey {
    paste_key_among(yields_with_command().unwrap_or_default())
}

/// What each key yields with Command held, or `None` when the layout has no data to read, as
/// with some input methods.
fn yields_with_command() -> Option<Vec<(VirtualKey, String)>> {
    // SAFETY: takes nothing; the result follows the create rule and `CFRetained` releases it.
    let source = NonNull::new(unsafe { TISCopyCurrentKeyboardLayoutInputSource() })?;
    // SAFETY: a non-null object the call above handed over.
    let source: CFRetained<CFType> = unsafe { CFRetained::from_raw(source.cast()) };
    let source_ptr: *const CFType = &*source;
    // SAFETY: a valid input source and a constant key the framework defines. The result follows
    // the get rule: the source owns it, and the source outlives every use below.
    let data =
        unsafe { TISGetInputSourceProperty(source_ptr.cast(), kTISPropertyUnicodeKeyLayoutData) };
    // SAFETY: the layout data property is a data object when present.
    let data: &CFData = unsafe { data.cast::<CFData>().as_ref()? };
    let layout_ptr: *const c_void = data.byte_ptr().cast();
    // SAFETY: takes nothing.
    let keyboard_type = u32::from(unsafe { LMGetKbdType() });
    let yields = (0..=layout::LAST_KEY)
        .filter_map(|key| {
            let mut dead_keys = 0_u32;
            let mut length = 0_usize;
            let mut units = [0_u16; layout::MAX_UNITS];
            // SAFETY: the layout data stays alive with `source`; the out pointers and the
            // buffer of the stated length outlive the call.
            let status = unsafe {
                UCKeyTranslate(
                    layout_ptr,
                    key,
                    layout::KEY_DOWN,
                    layout::COMMAND_STATE,
                    keyboard_type,
                    layout::NO_DEAD_KEYS,
                    &mut dead_keys,
                    units.len(),
                    &mut length,
                    units.as_mut_ptr(),
                )
            };
            let units = units.get(..length)?;
            (status == 0).then(|| (key, String::from_utf16_lossy(units)))
        })
        .collect();
    drop(source);
    Some(yields)
}
