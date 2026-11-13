import { execFileSync, execSync } from "node:child_process";
import { appendFileSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { tag } from "./settings.mjs";

export const { env } = process;

export const separateReleases = Boolean(env.RELEASES_REPOSITORY);

export const releasesRepository = env.RELEASES_REPOSITORY || env.GITHUB_REPOSITORY;

export const signingDir = () => join(env.RUNNER_TEMP, "release-signing");

export const outputDir = () => join(env.RUNNER_TEMP, "release-output", "desktop");

const append = (file, line) => appendFileSync(file, `${line}\n`);

export const setOutput = (name, value) => append(env.GITHUB_OUTPUT, `${name}=${value}`);

export const summary = (message) => append(env.GITHUB_STEP_SUMMARY, message);

export const hasAll = (...names) => names.every((name) => env[name]);

export const envWithout = (names) =>
  Object.fromEntries(Object.entries(env).filter(([name]) => !names.includes(name)));

const quoteForShell = (argument) =>
  /[\s"]/.test(argument) ? `"${String(argument).replaceAll('"', '\\"')}"` : argument;

export function run(command, args, options = {}) {
  const runOptions = { stdio: "inherit", ...options };
  // The package manager is a .cmd shim here, which only starts through a shell. The shell splits the line at
  // spaces, so an argument with one, such as a path, is quoted.
  if (process.platform === "win32") {
    execSync([command, ...args].map(quoteForShell).join(" "), runOptions);
  } else {
    execFileSync(command, args, runOptions);
  }
}

export const capture = (command, args, input) =>
  execFileSync(command, args, { encoding: "utf8", input, stdio: ["pipe", "pipe", "pipe"] });

export function decodeBase64(name) {
  const value = env[name] ?? "";
  if (!value || !/^(?:[A-Za-z0-9+/]{4})*(?:[A-Za-z0-9+/]{2}==|[A-Za-z0-9+/]{3}=)?$/.test(value)) {
    throw new Error(`${name} must contain Base64 file contents; see .env.example`);
  }
  return Buffer.from(value, "base64");
}

export function writeSecretFile(path, data) {
  mkdirSync(dirname(path), { recursive: true, mode: 0o700 });
  writeFileSync(path, data, { mode: 0o600 });
}

// Returns undefined only when the release does not exist; any other failure stops the release.
export function viewRelease() {
  try {
    return JSON.parse(
      capture("gh", ["release", "view", tag, "--repo", releasesRepository, "--json", "isDraft"]),
    );
  } catch (error) {
    if (String(error.stderr).includes("release not found")) {
      return undefined;
    }
    throw new Error(`Could not read release ${tag} from ${releasesRepository}: ${error.stderr}`);
  }
}
