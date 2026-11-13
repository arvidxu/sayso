import { useEffect, useState, useSyncExternalStore } from "react";
import { getAppVersion, restoreNativeDefaults } from "@/services/app";
import {
  getLanguagePreference,
  getSystemLanguage,
  LANGUAGES,
  setLanguagePreference,
  type LanguagePreference,
} from "@/services/language";
import { getSettings, resetSettings, setSetting, subscribeSettings } from "@/services/settings";
import { getThemePreference, setThemePreference, type ThemePreference } from "@/services/theme";
import { requestUpdateCheck } from "@/services/updates";

export type SettingsSection = "general" | "appearance" | "models" | "advanced" | "about";

export function useSettings() {
  const saved = useSyncExternalStore(subscribeSettings, getSettings);
  const [section, setSection] = useState<SettingsSection>("general");
  const [version, setVersion] = useState("");

  useEffect(() => {
    getAppVersion().then(setVersion).catch(console.error);
  }, []);

  return {
    state: {
      section,
      language: getLanguagePreference(),
      languages: LANGUAGES,
      systemLanguage: getSystemLanguage(),
      theme: getThemePreference(),
      version,
      hasAutomaticUpdates: saved.automaticUpdates,
    },
    actions: {
      selectSection: setSection,
      selectLanguage(next: LanguagePreference) {
        setLanguagePreference(next);
      },
      selectTheme(next: ThemePreference) {
        setThemePreference(next);
      },
      setAutomaticUpdates(isEnabled: boolean) {
        setSetting("automaticUpdates", isEnabled);
      },
      restoreDefaults() {
        resetSettings();
        restoreNativeDefaults().catch(console.error);
      },
      checkForUpdates() {
        requestUpdateCheck();
      },
    },
  };
}

export type SettingsPanelModel = ReturnType<typeof useSettings>;
