import { load, type Store } from "@tauri-apps/plugin-store";
import { app } from "../../../../product.conf.json";

// The native side reads the same file.
export type SavedSettings = {
  theme: "system" | "light" | "dark";
  // "system", or a language code.
  language: string;
  automaticUpdates: boolean;
};

const DEFAULTS: SavedSettings = {
  theme: "system",
  language: "system",
  // The native side reads the same initial value.
  automaticUpdates: app.initialSettings.automaticUpdates,
};

let store: Store | undefined;
// Replaced on every change, never changed in place, so that a reader can tell by comparing.
let values = DEFAULTS;
let loading: Promise<void> | undefined;
const listeners = new Set<(key: keyof SavedSettings) => void>();

// Reads the file once, before the first render, so that settings can be read without waiting.
export function loadSettings() {
  loading ??= (async () => {
    try {
      store = await load("settings.json");
      for (const key of Object.keys(DEFAULTS) as (keyof SavedSettings)[]) {
        const saved = await store.get(key);
        // A value of another type, as from a later version or an edit by hand, is left unused.
        if (typeof saved === typeof DEFAULTS[key]) values = { ...values, [key]: saved };
      }
    } catch (error) {
      console.error(error);
    }
  })();
  return loading;
}

export function getSettings(): SavedSettings {
  return values;
}

export function getSetting<Key extends keyof SavedSettings>(key: Key): SavedSettings[Key] {
  return values[key];
}

export function setSetting<Key extends keyof SavedSettings>(key: Key, value: SavedSettings[Key]) {
  if (values[key] === value) return;
  values = { ...values, [key]: value };
  store?.set(key, value).catch(console.error);
  for (const listener of listeners) listener(key);
}

export function resetSettings() {
  for (const key of Object.keys(DEFAULTS) as (keyof SavedSettings)[]) {
    setSetting(key, DEFAULTS[key]);
  }
}

export function subscribeSettings(listener: (key: keyof SavedSettings) => void) {
  listeners.add(listener);
  return () => {
    listeners.delete(listener);
  };
}
