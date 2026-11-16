import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { existsSync, mkdtempSync, readFileSync, readdirSync, writeFileSync } from "node:fs";
import { createServer } from "node:http";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { fetchLibrary, lockedVersion, root } from "./fetch.mjs";

const lock = '[[package]]\nname = "other"\nversion = "9.9.9"\n\n[[package]]\nname = "engine-sys"\nversion = "1.2.3"\n';

const body = Buffer.from("the library");
const sha256 = createHash("sha256").update(body).digest("hex");

// `failFirst` answers that many requests with a server error before serving `content`.
async function serve(content, status = 200, failFirst = 0) {
  const seen = [];
  const server = createServer((request, response) => {
    seen.push(request.url);
    response.writeHead(seen.length <= failFirst ? 503 : status).end(content);
  });
  await new Promise((ready) => server.listen(0, "127.0.0.1", ready));
  return { seen, server, base: `http://127.0.0.1:${server.address().port}/release` };
}

function engine(base, overrides = {}) {
  return {
    lockedPackage: "engine-sys",
    version: "1.2.3",
    downloadBase: base,
    downloadTries: 3,
    retryPauseMs: 1,
    archives: { "some-target": { file: "lib.tar.bz2", sha256, ...overrides } },
  };
}

test("lockedVersion reads the version of the named package only", () => {
  assert.equal(lockedVersion(lock, "engine-sys"), "1.2.3");
  assert.equal(lockedVersion(lock, "other"), "9.9.9");
  assert.equal(lockedVersion(lock, "missing"), undefined);
});

test("a library is downloaded, verified and then kept without asking again", async () => {
  const { seen, server, base } = await serve(body);
  const into = mkdtempSync(join(tmpdir(), "engine-library-"));
  try {
    const path = await fetchLibrary("x", engine(base), "some-target", into, lock);
    assert.deepEqual(readFileSync(path), body);
    assert.deepEqual(seen, ["/release/lib.tar.bz2"]);
    await fetchLibrary("x", engine(base), "some-target", into, lock);
    assert.equal(seen.length, 1);
  } finally {
    server.close();
  }
});

test("a download with the wrong checksum is refused and not kept", async () => {
  const { server, base } = await serve(Buffer.from("something else"));
  const into = mkdtempSync(join(tmpdir(), "engine-library-"));
  try {
    await assert.rejects(fetchLibrary("x", engine(base), "some-target", into, lock), /checksum/);
    assert.deepEqual(readdirSync(into), []);
  } finally {
    server.close();
  }
});

test("a file that is there but does not match is fetched again", async () => {
  const { seen, server, base } = await serve(body);
  const into = mkdtempSync(join(tmpdir(), "engine-library-"));
  writeFileSync(join(into, "lib.tar.bz2"), "tampered");
  try {
    const path = await fetchLibrary("x", engine(base), "some-target", into, lock);
    assert.deepEqual(readFileSync(path), body);
    assert.equal(seen.length, 1);
  } finally {
    server.close();
  }
});

test("a server error, an unknown target and a version that moved on are all refused", async () => {
  const { seen, server, base } = await serve("gone", 404);
  const into = mkdtempSync(join(tmpdir(), "engine-library-"));
  try {
    await assert.rejects(fetchLibrary("x", engine(base), "some-target", into, lock), /404/);
    await assert.rejects(fetchLibrary("x", engine(base), "other-target", into, lock), /no library/);
    const moved = { ...engine(base), version: "1.2.4" };
    await assert.rejects(fetchLibrary("x", moved, "some-target", into, lock), /Cargo\.lock/);
    assert.equal(seen.length, 1);
    assert.ok(!existsSync(join(into, "lib.tar.bz2")));
  } finally {
    server.close();
  }
});

test("the shipped list matches Cargo.lock and names a library for every desktop target", () => {
  const engines = JSON.parse(readFileSync(join(root, "engine-libraries.json"), "utf8"));
  const shippedLock = readFileSync(join(root, "Cargo.lock"), "utf8");
  const product = JSON.parse(readFileSync(join(root, "product.conf.json"), "utf8"));
  const targets = Object.values(product.release.desktop.targets).flat();
  for (const [name, entry] of Object.entries(engines)) {
    assert.equal(lockedVersion(shippedLock, entry.lockedPackage), entry.version, name);
    for (const target of targets) {
      assert.match(entry.archives[target]?.sha256 ?? "", /^[0-9a-f]{64}$/, `${name} ${target}`);
    }
  }
});

test("server errors are tried again until the tries run out", async () => {
  const into = mkdtempSync(join(tmpdir(), "engine-library-"));
  const recovers = await serve(body, 200, 2);
  try {
    const path = await fetchLibrary("x", engine(recovers.base), "some-target", into, lock);
    assert.deepEqual(readFileSync(path), body);
    assert.equal(recovers.seen.length, 3);
  } finally {
    recovers.server.close();
  }
  const fresh = mkdtempSync(join(tmpdir(), "engine-library-"));
  const never = await serve(body, 200, 3);
  try {
    await assert.rejects(fetchLibrary("x", engine(never.base), "some-target", fresh, lock), /503/);
    assert.equal(never.seen.length, 3);
    assert.deepEqual(readdirSync(fresh), []);
  } finally {
    never.server.close();
  }
});

test("a list entry without its download tries is refused", async () => {
  const into = mkdtempSync(join(tmpdir(), "engine-library-"));
  const { downloadTries, ...missing } = engine("http://127.0.0.1:9/release");
  assert.equal(downloadTries, 3);
  await assert.rejects(fetchLibrary("x", missing, "some-target", into, lock), /downloadTries/);
});
