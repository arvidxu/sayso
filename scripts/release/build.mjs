import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import {
  root,
  desktopDir,
  targets,
  bundles,
  setting,
  isTextList,
  publishedName,
  installerName,
} from "./settings.mjs";
import {
  env,
  releasesRepository,
  signingDir,
  outputDir,
  summary,
  hasAll,
  envWithout,
  run,
  capture,
  decodeBase64,
  writeSecretFile,
} from "./github.mjs";

const signingSecrets = [
  "APPLE_CERTIFICATE",
  "APPLE_CERTIFICATE_PASSWORD",
  "APPLE_API_KEY",
  "APPLE_API_KEY_ID",
  "APPLE_API_ISSUER",
  "WINDOWS_CERTIFICATE",
  "WINDOWS_CERTIFICATE_PASSWORD",
  "WINDOWS_TIMESTAMP_URL",
  "TAURI_SIGNING_PRIVATE_KEY",
  "TAURI_SIGNING_PRIVATE_KEY_PASSWORD",
];

// A group of secrets set only in part is a mistake, such as a misspelt name, and would ship an
// unsigned or unnotarized build without saying so, so it stops the build. None set skips it.
function isComplete(what, ...names) {
  const missing = names.filter((name) => !hasAll(name));
  if (missing.length === 0) return true;
  if (missing.length === names.length) return false;
  throw new Error(
    `${what}: ${missing.join(", ")} is not set while the rest is; set all of them or none`,
  );
}

// The bundler treats an empty variable as set, so only the secrets that are set are passed on.
function macSigning(childEnv) {
  if (!isComplete("macOS signing", "APPLE_CERTIFICATE", "APPLE_CERTIFICATE_PASSWORD")) {
    // An ad-hoc signature keeps the system from reporting a downloaded app as damaged.
    childEnv.APPLE_SIGNING_IDENTITY = "-";
    summary(
      "macOS signing skipped: certificate or password is missing; the app has an ad-hoc signature.",
    );
    return;
  }
  childEnv.APPLE_CERTIFICATE = env.APPLE_CERTIFICATE;
  childEnv.APPLE_CERTIFICATE_PASSWORD = env.APPLE_CERTIFICATE_PASSWORD;
  if (!isComplete("macOS notarization", "APPLE_API_KEY", "APPLE_API_KEY_ID", "APPLE_API_ISSUER")) {
    summary("macOS notarization skipped: Apple API credentials are missing.");
    return;
  }
  const keyPath = join(signingDir(), "AuthKey.p8");
  writeSecretFile(keyPath, decodeBase64("APPLE_API_KEY"));
  // The bundler reads the key ID from APPLE_API_KEY and the key file from APPLE_API_KEY_PATH.
  Object.assign(childEnv, {
    APPLE_API_KEY: env.APPLE_API_KEY_ID,
    APPLE_API_KEY_PATH: keyPath,
    APPLE_API_ISSUER: env.APPLE_API_ISSUER,
  });
}

// Returns the thumbprint of the imported certificate.
function importWindowsCertificate() {
  if (!isComplete("Windows signing", "WINDOWS_CERTIFICATE", "WINDOWS_CERTIFICATE_PASSWORD")) {
    summary("Windows signing skipped: certificate or password is missing.");
    return undefined;
  }
  const pfxPath = join(signingDir(), "windows.pfx");
  writeSecretFile(pfxPath, decodeBase64("WINDOWS_CERTIFICATE"));
  const thumbprint = capture("powershell", [
    "-NoProfile",
    "-Command",
    `(Import-PfxCertificate -FilePath '${pfxPath}' -CertStoreLocation Cert:\\CurrentUser\\My -Password (ConvertTo-SecureString -String $env:WINDOWS_CERTIFICATE_PASSWORD -Force -AsPlainText)).Thumbprint`,
  ]).trim();
  rmSync(pfxPath, { force: true });
  if (!/^[0-9A-F]{40}$/i.test(thumbprint)) {
    throw new Error("Importing WINDOWS_CERTIFICATE did not return a certificate thumbprint");
  }
  return thumbprint;
}

const removeWindowsCertificate = (thumbprint) =>
  capture("powershell", [
    "-NoProfile",
    "-Command",
    `Remove-Item -Path Cert:\\CurrentUser\\My\\${thumbprint}`,
  ]);

// The public key and the update addresses are added to the app here, so builds without the keys
// have no updater configuration.
function updateSigning(childEnv, args) {
  if (!hasAll("TAURI_SIGNING_PRIVATE_KEY")) {
    summary("Update files skipped: TAURI_SIGNING_PRIVATE_KEY is not set.");
    return false;
  }
  if (!hasAll("TAURI_SIGNING_PUBLIC_KEY")) {
    throw new Error(
      "TAURI_SIGNING_PRIVATE_KEY is set but the TAURI_SIGNING_PUBLIC_KEY variable is not; see .env.variables.example",
    );
  }
  childEnv.TAURI_SIGNING_PRIVATE_KEY = env.TAURI_SIGNING_PRIVATE_KEY;
  if (env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD) {
    childEnv.TAURI_SIGNING_PRIVATE_KEY_PASSWORD = env.TAURI_SIGNING_PRIVATE_KEY_PASSWORD;
  }
  const configPath = join(signingDir(), "tauri.updates.json");
  writeSecretFile(
    configPath,
    JSON.stringify({
      bundle: { createUpdaterArtifacts: true },
      plugins: {
        updater: {
          pubkey: env.TAURI_SIGNING_PUBLIC_KEY.trim(),
          // The updater tries them in order and moves on when one does not answer with success. It
          // only accepts HTTPS addresses in release builds.
          endpoints: setting(
            "updateEndpoints",
            (value) => isTextList(value) && value.every((url) => url.startsWith("https://")),
          ).map((url) => url.replaceAll("{releasesRepository}", releasesRepository)),
        },
      },
    }),
  );
  args.push("--config", configPath);
  return true;
}

// The key ID inside an update public key or signature, both stored as Base64 text.
const keyId = (base64) =>
  Buffer.from(Buffer.from(base64, "base64").toString("utf8").split("\n")[1], "base64")
    .subarray(2, 10)
    .toString("hex");

// The bundler only warns when the private key does not belong to the public key, and every
// installed app would then reject the update.
function readSignature(path) {
  const signature = readFileSync(`${path}.sig`, "utf8");
  if (keyId(signature) !== keyId(env.TAURI_SIGNING_PUBLIC_KEY.trim())) {
    throw new Error("TAURI_SIGNING_PRIVATE_KEY does not belong to TAURI_SIGNING_PUBLIC_KEY");
  }
  return signature;
}

// The platform name the updater looks up in latest.json, for example darwin-aarch64.
const updaterPlatform = (target) =>
  `${{ mac: "darwin", windows: "windows", linux: "linux" }[targets[target].platform]}-${target.split("-")[0]}`;

// The update an app installs when the updater cannot tell which format it was installed from.
const fallbackUpdates = ["app", "appimage", "msi", "nsis", "deb", "rpm"];

function onlyFile(dir, suffix) {
  const files = existsSync(dir) ? readdirSync(dir).filter((name) => name.endsWith(suffix)) : [];
  if (files.length !== 1) {
    throw new Error(`Expected one ${suffix} file in ${dir}, found ${files.length}`);
  }
  return join(dir, files[0]);
}

// Build output goes to the workspace root, not next to the app.
const bundleDir = (target) => join(root, "target", target, "release", "bundle");

export function desktopPackage(targetList) {
  const platformTargets = targetList.split(",");
  const childEnv = envWithout(signingSecrets);
  const args = [];
  const updates = updateSigning(childEnv, args);
  let thumbprint;
  if (env.RUNNER_OS === "macOS") {
    macSigning(childEnv);
    // With CI=true the bundler skips the step that applies the DMG background and icon
    // positions; this variable keeps that step.
    childEnv.TAURI_BUNDLER_DMG_IGNORE_CI = "true";
  }
  if (env.RUNNER_OS === "Windows") {
    thumbprint = importWindowsCertificate();
    if (thumbprint) {
      // Passed as a file: a JSON argument would not survive the shell that starts the package manager here.
      const configPath = join(signingDir(), "tauri.windows-signing.json");
      // A timestamp keeps the signature valid after the certificate expires.
      const timestampUrl = env.WINDOWS_TIMESTAMP_URL?.trim();
      if (!timestampUrl) {
        summary("Windows signature has no timestamp: WINDOWS_TIMESTAMP_URL is not set.");
      }
      writeSecretFile(
        configPath,
        JSON.stringify({
          bundle: {
            windows: {
              certificateThumbprint: thumbprint,
              digestAlgorithm: "sha256",
              ...(timestampUrl && { timestampUrl }),
            },
          },
        }),
      );
      args.push("--config", configPath);
    }
  }
  try {
    for (const target of platformTargets) {
      run("pnpm", ["tauri", "build", "--target", target, ...args], {
        cwd: desktopDir,
        env: childEnv,
      });
    }
  } finally {
    if (thumbprint) {
      removeWindowsCertificate(thumbprint);
    }
    rmSync(signingDir(), { recursive: true, force: true });
  }

  mkdirSync(outputDir(), { recursive: true });
  // The latest.json entries of these targets, keyed by updater platform and installer format.
  const updateEntries = {};
  for (const target of platformTargets) {
    const dir = bundleDir(target);
    const built = Object.keys(bundles).filter((bundle) => existsSync(join(dir, bundle)));
    if (built.length === 0) {
      throw new Error(`No installer was found in ${dir}`);
    }
    const files = {};
    for (const bundle of built) {
      const path = onlyFile(join(dir, bundle), bundles[bundle].suffix);
      const name = installerName(target, bundle);
      copyFileSync(path, join(outputDir(), name));
      files[bundle] = { path, name };
    }
    if (!updates) {
      continue;
    }
    // The updater installs the .app.tar.gz on "mac" and the installer itself elsewhere.
    delete files.dmg;
    if (targets[target].platform === "mac") {
      const name = publishedName(target, ".app.tar.gz");
      files.app = { path: onlyFile(join(dir, "macos"), ".app.tar.gz"), name };
      copyFileSync(files.app.path, join(outputDir(), name));
    }
    const platform = updaterPlatform(target);
    for (const [bundle, { path, name }] of Object.entries(files)) {
      updateEntries[`${platform}-${bundle}`] = {
        name,
        signature: readSignature(path),
      };
    }
    const fallback = fallbackUpdates.find((bundle) => files[bundle]);
    updateEntries[platform] = updateEntries[`${platform}-${fallback}`];
  }
  if (updates) {
    writeFileSync(
      join(outputDir(), `update-${platformTargets.join("_")}.json`),
      JSON.stringify(updateEntries),
    );
  }
  summary(`Built ${readdirSync(outputDir()).sort().join(", ")}.`);
}
