import React from "react";
import ReactDOM from "react-dom/client";
// Also loads the settings: an import, waiting included, finishes before this module runs.
import "@/i18n";
import App from "./App";
import { BACKDROP_CHANGE, BACKDROP_FADE_MS, BACKDROP_OPACITY, backdropDepth } from "@/lib/backdrop";
import { subscribeSettings } from "@/services/settings";
import { getThemePreference } from "@/services/theme";
import { setWindowBackground, setWindowTheme, showWindow } from "@/services/window";

const media = matchMedia("(prefers-color-scheme: dark)");
const reducedMotion = matchMedia("(prefers-reduced-motion: reduce)");

const THEME_FADE_MS = 200;
document.documentElement.style.setProperty("--theme-fade", `${THEME_FADE_MS}ms`);

// Requests to the window are asynchronous and can finish out of order, so only one runs at a time
// and the latest color always wins.
let wantedBackground = "";
let nativeBackground = "";
let isSettingBackground = false;
async function setNativeBackground(color: string) {
  wantedBackground = color;
  if (isSettingBackground) return;
  isSettingBackground = true;
  try {
    while (nativeBackground !== wantedBackground) {
      const next = wantedBackground;
      await setWindowBackground(next);
      nativeBackground = next;
    }
  } finally {
    isSettingBackground = false;
  }
}

function pageBackground(): number[] {
  const channels = getComputedStyle(document.documentElement)
    .getPropertyValue("--shell")
    .trim()
    .match(/^#(..)(..)(..)$/);
  // Each backdrop over another lets the same share of what is under it through.
  const dim = (1 - BACKDROP_OPACITY) ** backdropDepth();
  return (channels?.slice(1) ?? ["ff", "ff", "ff"]).map((channel) => parseInt(channel, 16) * dim);
}

// The native background shows in the title bar and wherever the page has not painted yet while
// resizing, so it fades in step with the page.
let shownBackground: number[] | undefined;
let backgroundFrame = 0;
function syncBackground(fadeMs = 0) {
  const from = shownBackground;
  const start = performance.now();
  cancelAnimationFrame(backgroundFrame);
  // Read once, not on every frame: reading a style makes the page work out its styles again.
  const to = pageBackground();
  const step = (now: number) => {
    // A frame can carry a time from before the fade began.
    const progress = from && fadeMs > 0 ? Math.min(1, Math.max(0, (now - start) / fadeMs)) : 1;
    shownBackground = to.map((channel, index) =>
      from ? from[index] + (channel - from[index]) * progress : channel,
    );
    const color = shownBackground
      .map((channel) => Math.floor(channel).toString(16).padStart(2, "0"))
      .join("");
    setNativeBackground(`#${color}`).catch(console.error);
    if (progress < 1) backgroundFrame = requestAnimationFrame(step);
  };
  step(start);
}

function isDarkWanted() {
  const preference = getThemePreference();
  return preference === "system" ? media.matches : preference === "dark";
}

async function syncTheme(fadeMs = 0) {
  const isDark = isDarkWanted();
  const root = document.documentElement;
  if (root.classList.contains("dark") !== isDark) {
    if (fadeMs > 0 && !reducedMotion.matches && "startViewTransition" in document) {
      // One cross-fade of the whole page: per-element color transitions lag on inherited colors and
      // would still be running after it finishes.
      root.classList.add("theme-switching");
      const transition = document.startViewTransition(() => {
        root.classList.toggle("dark", isDark);
      });
      transition.finished
        .finally(() => root.classList.remove("theme-switching"))
        .catch(console.error);
      // The fade starts when the page is ready to; a skipped transition still changes the theme.
      await transition.ready.catch(() => transition.updateCallbackDone);
      // The page starts its fade on the next frame.
      requestAnimationFrame(() => syncBackground(fadeMs));
    } else {
      root.classList.toggle("dark", isDark);
      syncBackground();
    }
  } else if (!shownBackground) {
    syncBackground();
  }
  const preference = getThemePreference();
  await setWindowTheme(preference === "system" ? null : preference);
}

// Also fires when the window stops forcing a theme because the user picked "System".
media.addEventListener("change", () => {
  syncTheme(THEME_FADE_MS).catch(console.error);
});
addEventListener(BACKDROP_CHANGE, () => {
  syncBackground(reducedMotion.matches ? 0 : BACKDROP_FADE_MS);
});
subscribeSettings((key) => {
  if (key === "theme") syncTheme(THEME_FADE_MS).catch(console.error);
});
// Start hidden in tauri.conf.json; show after theme initialization to reduce startup flashing.
syncTheme().catch(console.error).finally(showWindow).catch(console.error);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
