import { useSyncExternalStore } from "react";
import {
  getLanguagePreference,
  LANGUAGES,
  setLanguagePreference,
  type LanguagePreference,
} from "@/services/language";
import { getSettings, subscribeSettings } from "@/services/settings";

export function useLanguage() {
  useSyncExternalStore(subscribeSettings, getSettings);

  return {
    state: { language: getLanguagePreference(), languages: LANGUAGES },
    actions: {
      select(language: LanguagePreference) {
        setLanguagePreference(language);
      },
    },
  };
}
