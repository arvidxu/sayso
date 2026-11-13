import languages from "@/i18n/languages.json";
import { getNativeSystemLanguage } from "@/services/app";
import { getSetting, setSetting } from "@/services/settings";

// A new language is added to languages.json; the build and tests then point at what else it needs.
export type Language = keyof typeof languages;
export type LanguagePreference = "system" | Language;

const CODES = Object.keys(languages) as Language[];

// For a system language the app lacks, and for a text another language lacks; the native side
// falls back to the same one.
export const FALLBACK_LANGUAGE = CODES[0];

// Each language is listed under its own name, so it can be found whatever the current one is.
export const LANGUAGES = CODES.map((value) => ({ value, label: languages[value].name }));

export function getLanguagePreference(): LanguagePreference {
  const saved = getSetting("language");
  return LANGUAGES.find(({ value }) => value === saved)?.value ?? "system";
}

export function setLanguagePreference(preference: LanguagePreference) {
  setSetting("language", preference);
}

export function getLanguage(): Language {
  const preference = getLanguagePreference();
  return preference === "system" ? getSystemLanguage() : preference;
}

// The language of the system, as the native side matches it to one of the app's. The first of
// languages.json until it has answered, and when it has none of the system's languages.
let systemLanguage: Language = FALLBACK_LANGUAGE;

// Called before the first render, and again when the system language changes.
export async function loadSystemLanguage() {
  try {
    systemLanguage = (await getNativeSystemLanguage()) as Language;
  } catch (error) {
    console.error(error);
  }
}

export function getSystemLanguage(): Language {
  return systemLanguage;
}
