//! The permissions dictation needs: the microphone, and Accessibility for the shortcut tap and
//! sent keys. Input Monitoring is not needed: the tap also filters keys, which Accessibility
//! covers.

use std::ffi::c_void;
use std::process::Command;

use block2::RcBlock;
use dictation_api::PermissionProvider;
use dictation_types::{AppError, AppResult, PermissionKind, PermissionReport, PermissionStatus};
use objc2::runtime::Bool;
use objc2_av_foundation::{AVAuthorizationStatus, AVCaptureDevice, AVMediaTypeAudio};
use objc2_core_foundation::{CFBoolean, CFDictionary, CFString};

#[link(name = "ApplicationServices", kind = "framework")]
unsafe extern "C" {
    fn AXIsProcessTrusted() -> u8;
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    static kAXTrustedCheckOptionPrompt: &'static CFString;
}

/// Where each permission is granted in the system settings.
fn settings_address(kind: PermissionKind) -> Option<&'static str> {
    match kind {
        PermissionKind::Microphone => {
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Microphone")
        }
        PermissionKind::Accessibility => {
            Some("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        }
        PermissionKind::InputMonitoring => None,
    }
}

fn not_needed(kind: PermissionKind) -> AppError {
    AppError::validation(format!(
        "dictation on macOS does not need the {kind} permission"
    ))
}

fn microphone_status() -> PermissionStatus {
    // SAFETY: a constant string the system framework defines.
    let Some(audio) = (unsafe { AVMediaTypeAudio }) else {
        return PermissionStatus::NotDetermined;
    };
    // SAFETY: asks for the state of a media type the system framework defines.
    match unsafe { AVCaptureDevice::authorizationStatusForMediaType(audio) } {
        AVAuthorizationStatus::Authorized => PermissionStatus::Granted,
        AVAuthorizationStatus::Denied => PermissionStatus::Denied,
        AVAuthorizationStatus::Restricted => PermissionStatus::Restricted,
        _ => PermissionStatus::NotDetermined,
    }
}

/// The system says only whether Accessibility is granted, so not granted is reported as denied,
/// even before this application ever asked.
fn accessibility_status() -> PermissionStatus {
    // SAFETY: takes no arguments and only reads system state.
    if unsafe { AXIsProcessTrusted() } != 0 {
        PermissionStatus::Granted
    } else {
        PermissionStatus::Denied
    }
}

/// Accessibility shows as denied until granted, even before
/// asking, so offer both [`PermissionProvider::request`] and [`PermissionProvider::open_settings`].
#[derive(Debug, Default, Clone, Copy)]
pub struct MacosPermissions;

impl PermissionProvider for MacosPermissions {
    fn permissions(&self) -> AppResult<PermissionReport> {
        Ok(PermissionReport::new(vec![
            (PermissionKind::Microphone, microphone_status()),
            (PermissionKind::Accessibility, accessibility_status()),
        ]))
    }

    fn request(&self, kind: PermissionKind) -> AppResult<()> {
        match kind {
            PermissionKind::Microphone => {
                // SAFETY: a constant string the system framework defines.
                let audio = unsafe { AVMediaTypeAudio }
                    .ok_or_else(|| AppError::internal("AVFoundation has no audio media type"))?;
                // The answer is read again through `permissions`.
                let done = RcBlock::new(|_granted: Bool| {});
                // SAFETY: the block matches the callback signature and is kept alive until called.
                unsafe {
                    AVCaptureDevice::requestAccessForMediaType_completionHandler(audio, &done);
                }
                Ok(())
            }
            PermissionKind::Accessibility => {
                // SAFETY: a constant string the Accessibility API defines.
                let prompt = unsafe { kAXTrustedCheckOptionPrompt };
                let options = CFDictionary::from_slices(&[prompt], &[CFBoolean::new(true)]);
                // SAFETY: the dictionary is valid for the call; the system copies what it needs.
                unsafe {
                    AXIsProcessTrustedWithOptions(std::ptr::from_ref(&*options).cast());
                }
                Ok(())
            }
            PermissionKind::InputMonitoring => Err(not_needed(kind)),
        }
    }

    fn open_settings(&self, kind: PermissionKind) -> AppResult<()> {
        let address = settings_address(kind).ok_or_else(|| not_needed(kind))?;
        // By its full path, so a program of the same name elsewhere on the search path is not
        // run instead.
        let status = Command::new("/usr/bin/open")
            .arg(address)
            .status()
            .map_err(|err| AppError::internal(format!("could not open System Settings: {err}")))?;
        if status.success() {
            Ok(())
        } else {
            Err(AppError::internal(format!(
                "could not open System Settings: open exited with {status}"
            )))
        }
    }
}
