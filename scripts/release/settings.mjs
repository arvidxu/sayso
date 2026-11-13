import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";

export const root = resolve(import.meta.dirname, "../..");

export const desktopDir = join(root, "apps/desktop");

export const tauriDir = join(desktopDir, "src-tauri");

const product = JSON.parse(readFileSync(join(root, "product.conf.json"), "utf8"));

export const settings = product.release ?? {};

export const updates = product.updates ?? {};

export const app = JSON.parse(readFileSync(join(tauriDir, "tauri.conf.json"), "utf8"));

export const tag = process.env.GITHUB_REF_NAME ?? "";

export const systems = ["mac", "windows", "linux"];

// os and arch are the names used in installer file names.
export const targets = {
  "aarch64-apple-darwin": { platform: "mac", os: "macos", arch: "arm64" },
  "x86_64-apple-darwin": { platform: "mac", os: "macos", arch: "x64" },
  "x86_64-pc-windows-msvc": { platform: "windows", os: "windows", arch: "x64" },
  "aarch64-pc-windows-msvc": { platform: "windows", os: "windows", arch: "arm64" },
  "x86_64-unknown-linux-gnu": { platform: "linux", os: "linux", arch: "x64" },
  "aarch64-unknown-linux-gnu": { platform: "linux", os: "linux", arch: "arm64" },
};

// Every format the bundler can build; which ones are built is set in the app configuration.
export const bundles = {
  dmg: { suffix: ".dmg", label: "DMG" },
  nsis: { suffix: "-setup.exe", label: "Installer" },
  msi: { suffix: ".msi", label: "MSI" },
  appimage: { suffix: ".AppImage", label: "AppImage" },
  deb: { suffix: ".deb", label: "DEB" },
  rpm: { suffix: ".rpm", label: "RPM" },
};

export function setting(path, isValid) {
  const value = path.split(".").reduce((object, key) => object?.[key], settings);
  if (!isValid(value)) {
    throw new TypeError(`product.conf.json release.${path} is missing or invalid`);
  }
  return value;
}

export const isText = (value) => typeof value === "string" && value.length > 0;

export const isPositive = (value) => Number.isInteger(value) && value > 0;

export const isTextList = (value) =>
  Array.isArray(value) && value.length > 0 && value.every(isText);

export function releaseVersion() {
  if (!/^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(tag)) {
    throw new Error(`Release tag "${tag}" must be a version such as 1.2.0`);
  }
  return tag;
}

export const isPrerelease = () => releaseVersion().split("+", 1)[0].includes("-");

// The published name of a built file, for example Sayso-1.2.0-macos-arm64.dmg.
export const publishedName = (target, suffix) =>
  `${app.productName}-${releaseVersion()}-${targets[target].os}-${targets[target].arch}${suffix}`;

export const installerName = (target, bundle) => publishedName(target, bundles[bundle].suffix);

const escape = (text) => text.replaceAll(/[.*+?^${}()|[\]\\]/g, "\\$&");

const installerPattern = () =>
  new RegExp(
    `-(?:macos|windows|linux)-(?:x64|arm64)(?:${Object.values(bundles)
      .map(({ suffix }) => escape(suffix))
      .join("|")})$`,
  );

export const isReleaseFile = (name) => installerPattern().test(name);
