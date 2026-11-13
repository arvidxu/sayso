import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

// The native side holds the update itself.
export type UpdateOutcome =
  | { status: "unavailable" }
  // A check or a download is already running; what is on screen stays.
  | { status: "busy" }
  | { status: "latest"; version: string }
  | { status: "available"; version: string; notes: string | null }
  | { status: "ready"; version: string; notes: string | null }
  | { status: "failed"; message: string };

export const UPDATE_CHECK_REQUEST = "update-check-request";

// Asks for a check from anywhere in the page; the update dialog runs it and shows the result.
export function requestUpdateCheck() {
  dispatchEvent(new Event(UPDATE_CHECK_REQUEST));
}

export function checkForUpdates(): Promise<UpdateOutcome> {
  return invoke("check_for_updates");
}

export function downloadUpdate(): Promise<UpdateOutcome> {
  return invoke("download_update");
}

// Restarts the app on success, so it returns only when the installation failed.
export function installUpdate(): Promise<UpdateOutcome> {
  return invoke("install_update");
}

export function cancelUpdateDownload(): Promise<void> {
  return invoke("cancel_update_download");
}

// What a timed check found before the page was listening, once.
export function takePendingUpdate(): Promise<UpdateOutcome | null> {
  return invoke("take_pending_update");
}

export function skipUpdate(): Promise<void> {
  return invoke("skip_update");
}

export function postponeUpdate(): Promise<void> {
  return invoke("postpone_update");
}

// "Check for Updates…" in the menu bar.
export function onMenuUpdateCheck(listener: () => void) {
  const unlisten = listen("check-for-updates", listener);
  return () => {
    unlisten.then((stop) => stop()).catch(console.error);
  };
}

// A timed check found a version worth offering.
export function onUpdateFound(listener: (outcome: UpdateOutcome) => void) {
  const unlisten = listen<UpdateOutcome>("update-found", (event) => listener(event.payload));
  return () => {
    unlisten.then((stop) => stop()).catch(console.error);
  };
}
