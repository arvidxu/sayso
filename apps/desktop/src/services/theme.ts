import { getSetting, setSetting, type SavedSettings } from "@/services/settings";

export type ThemePreference = SavedSettings["theme"];

export function getThemePreference(): ThemePreference {
  const saved = getSetting("theme");
  return saved === "light" || saved === "dark" ? saved : "system";
}

export function setThemePreference(preference: ThemePreference) {
  setSetting("theme", preference);
}
