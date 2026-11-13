import i18n from "i18next";
import { initReactI18next } from "react-i18next";
import { setClock } from "@/lib/format";
import { getUses12HourClock, setNativeLanguage } from "@/services/app";
import {
  FALLBACK_LANGUAGE,
  getLanguage,
  getLanguagePreference,
  loadSystemLanguage,
  type Language,
} from "@/services/language";
import { loadSettings, subscribeSettings } from "@/services/settings";
import en from "./locales/en.json";
import zhCN from "./locales/zh-CN.json";
import zhTW from "./locales/zh-TW.json";

// The first language is the reference; one without translations or missing a key fails the build.
// The "native" part of each file is read by the native side, for the menu bar.
const resources: Record<Language, { translation: typeof en }> = {
  en: { translation: en },
  "zh-CN": { translation: zhCN satisfies typeof en },
  "zh-TW": { translation: zhTW satisfies typeof en },
};

function applyLanguage() {
  const language = getLanguage();
  document.documentElement.lang = language;
  setNativeLanguage(language, getLanguagePreference() === "system").catch(console.error);
  return i18n.changeLanguage(language);
}

// Before anything is translated, so the first render is already in the right language.
await Promise.all([
  loadSettings(),
  loadSystemLanguage(),
  // Times are written by the system's clock from the first render.
  getUses12HourClock().then(
    (uses12Hours) => uses12Hours !== null && setClock(uses12Hours),
    console.error,
  ),
]);

i18n
  .use(initReactI18next)
  .init({
    resources,
    lng: getLanguage(),
    fallbackLng: FALLBACK_LANGUAGE,
    // The renderer already escapes what it shows.
    interpolation: { escapeValue: false },
  })
  .then(applyLanguage)
  .catch(console.error);

subscribeSettings((key) => {
  if (key === "language") applyLanguage().catch(console.error);
});
// The system language changed; it matters when the user has not picked one.
addEventListener("languagechange", () => {
  loadSystemLanguage().then(applyLanguage).catch(console.error);
});
