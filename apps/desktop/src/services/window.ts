import { listen } from "@tauri-apps/api/event";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";

export function showWindow(): Promise<void> {
  return getCurrentWebviewWindow().show();
}

export function setWindowBackground(color: string): Promise<void> {
  return getCurrentWebviewWindow().setBackgroundColor(color);
}

export function setWindowTheme(theme: "light" | "dark" | null): Promise<void> {
  return getCurrentWebviewWindow().setTheme(theme);
}

// "Settings…" in the menu bar.
export function onSettingsToggle(listener: () => void) {
  const unlisten = listen("toggle-settings", listener);
  return () => {
    unlisten.then((stop) => stop()).catch(console.error);
  };
}
