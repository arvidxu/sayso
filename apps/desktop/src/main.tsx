import React from "react";
import ReactDOM from "react-dom/client";
import { getCurrentWebviewWindow } from "@tauri-apps/api/webviewWindow";
import App from "./App";

const media = matchMedia("(prefers-color-scheme: dark)");

// Match the native background to the page to reduce color differences during resizing.
async function syncTheme() {
  document.documentElement.classList.toggle("dark", media.matches);
  const background = getComputedStyle(document.documentElement)
    .getPropertyValue("--background")
    .trim();
  await getCurrentWebviewWindow().setBackgroundColor(background);
}

media.addEventListener("change", () => {
  syncTheme().catch(console.error);
});
// Start hidden in tauri.conf.json; show after theme initialization to reduce startup flashing.
syncTheme()
  .catch(console.error)
  .finally(() => getCurrentWebviewWindow().show())
  .catch(console.error);

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
