import assert from "node:assert/strict";
import { test } from "node:test";
import { compareVersions, latestReleasesByPlatform, releaseCommit } from "./publish.mjs";

test("compareVersions follows SemVer precedence", () => {
  const ascending = ["1.0.0-alpha", "1.0.0-alpha.1", "1.0.0-rc.1", "1.0.0", "1.2.1", "1.2.10"];
  for (let index = 1; index < ascending.length; index += 1) {
    assert.ok(compareVersions(ascending[index - 1], ascending[index]) < 0);
    assert.ok(compareVersions(ascending[index], ascending[index - 1]) > 0);
  }
  assert.equal(compareVersions("1.0.0+build.1", "1.0.0+build.2"), 0);
});

test("releaseCommit matches the whole subject", () => {
  const log = ["aaa Release v1.2.0-rc.1", "bbb Release v1.2.10", "ccc Release v1.2.0"].join("\n");
  assert.equal(releaseCommit(log, "v1.2.0"), "ccc");
  assert.equal(releaseCommit(log, "v1.2.1"), undefined);
  assert.equal(releaseCommit("", "v1.2.0"), undefined);
});

test("the latest earlier release of a platform is the highest version, not the last published", () => {
  const release = (tag) => ({ tag, draft: false, prerelease: false, assets: [`Sayso-${tag}-macos-arm64.dmg`] });
  // A fix to an older line published after a newer release.
  const latest = latestReleasesByPlatform([release("1.1.1"), release("1.2.0"), release("1.1.0")], "1.3.0");
  assert.equal(latest.macOS, "1.2.0");
});
