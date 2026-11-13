import { getVersion } from "@tauri-apps/api/app";
import { invoke } from "@tauri-apps/api/core";

export function getAppVersion(): Promise<string> {
  return getVersion();
}

// The menu bar is shown in this language too.
// `followsSystem` tells whether the language is the system's, not one the user picked.
export function setNativeLanguage(language: string, followsSystem: boolean): Promise<void> {
  return invoke("set_language", { language, followsSystem });
}

// null where the native side cannot tell.
export function getUses12HourClock(): Promise<boolean | null> {
  return invoke("uses_12_hour_clock");
}

// The language of the system, as one of the app's own.
export function getNativeSystemLanguage(): Promise<string> {
  return invoke("get_system_language");
}

// The part of "Restore default settings" the page cannot do: the window goes back to its original
// size and position, and a skipped update is offered again.
export function restoreNativeDefaults(): Promise<void> {
  return invoke("restore_defaults");
}
