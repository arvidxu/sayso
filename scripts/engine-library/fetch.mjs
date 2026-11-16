// Fetches and verifies the engines' prebuilt libraries for one target into target/engine-libraries.
//   node scripts/engine-library/fetch.mjs [target]
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { createReadStream, existsSync, mkdirSync, readFileSync, renameSync, rmSync } from "node:fs";
import { writeFile } from "node:fs/promises";
import { setTimeout as sleep } from "node:timers/promises";
import { join, resolve } from "node:path";

export const root = resolve(import.meta.dirname, "../..");

const folder = join(root, "target/engine-libraries");

function sha256Of(path) {
  return new Promise((done, failed) => {
    const hash = createHash("sha256");
    createReadStream(path)
      .on("data", (chunk) => hash.update(chunk))
      .on("end", () => done(hash.digest("hex")))
      .on("error", failed);
  });
}

export function lockedVersion(lock, name) {
  const entry = lock.split("[[package]]").find((text) => text.includes(`name = "${name}"`));
  return entry?.match(/version = "([^"]+)"/)?.[1];
}

function hostTarget() {
  const info = execFileSync("rustc", ["-vV"], { encoding: "utf8" });
  return info.match(/^host: (.+)$/m)?.[1];
}

// A broken connection or a server error is tried again, waiting longer each time; an answer the
// server would give again, such as a missing file, and a wrong checksum are not.
async function download(name, engine, url, partial) {
  for (const key of ["downloadTries", "retryPauseMs"]) {
    if (!Number.isInteger(engine[key]) || engine[key] < 1) {
      throw new Error(`${name}: engine-libraries.json needs ${key} as a whole number above zero`);
    }
  }
  for (let attempt = 1; ; attempt += 1) {
    let response;
    try {
      response = await fetch(url);
      if (response.ok) {
        await writeFile(partial, response.body);
        return;
      }
    } catch (error) {
      if (attempt >= engine.downloadTries) {
        throw new Error(`${name}: downloading ${url} failed: ${error.message}`);
      }
    }
    if (response && (response.status < 500 || attempt >= engine.downloadTries)) {
      throw new Error(`${name}: ${url} answered ${response.status}`);
    }
    await sleep(engine.retryPauseMs * attempt);
  }
}

export async function fetchLibrary(name, engine, target, into, lock) {
  const locked = lockedVersion(lock, engine.lockedPackage);
  if (locked !== engine.version) {
    throw new Error(
      `${name}: Cargo.lock has ${engine.lockedPackage} ${locked}, but engine-libraries.json ` +
        `lists the libraries of ${engine.version}; update the file with the new checksums`,
    );
  }
  const archive = engine.archives[target];
  if (!archive) {
    throw new Error(`${name}: engine-libraries.json has no library for ${target}`);
  }
  const path = join(into, archive.file);
  if (existsSync(path) && (await sha256Of(path)) === archive.sha256) {
    return path;
  }
  mkdirSync(into, { recursive: true });
  const url = `${engine.downloadBase}/${archive.file}`;
  const partial = `${path}.partial`;
  // A download that breaks off or does not match leaves nothing behind.
  try {
    await download(name, engine, url, partial);
    const actual = await sha256Of(partial);
    if (actual !== archive.sha256) {
      throw new Error(
        `${name}: ${archive.file} has checksum ${actual}, not ${archive.sha256}; it was not kept`,
      );
    }
    renameSync(partial, path);
  } finally {
    rmSync(partial, { force: true });
  }
  return path;
}

if (import.meta.main) {
  const engines = JSON.parse(readFileSync(join(root, "engine-libraries.json"), "utf8"));
  const lock = readFileSync(join(root, "Cargo.lock"), "utf8");
  const target = process.argv[2] ?? hostTarget();
  for (const [name, engine] of Object.entries(engines)) {
    console.log(await fetchLibrary(name, engine, target, folder, lock));
  }
}
