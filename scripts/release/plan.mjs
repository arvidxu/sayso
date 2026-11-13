import { spawnSync } from "node:child_process";
import { mkdirSync, readFileSync, rmSync } from "node:fs";
import { join } from "node:path";
import {
  root,
  app,
  tag,
  systems,
  targets,
  setting,
  updates,
  isText,
  isPositive,
  isTextList,
  releaseVersion,
} from "./settings.mjs";
import {
  env,
  separateReleases,
  releasesRepository,
  setOutput,
  summary,
  run,
  capture,
  viewRelease,
} from "./github.mjs";
import { releaseNotes } from "./publish.mjs";

// The release jobs receive signing and publishing secrets, so a tag releases only a commit that is
// already on the default branch.
export function checkTag() {
  const event = JSON.parse(readFileSync(env.GITHUB_EVENT_PATH, "utf8"));
  const branch = event.repository?.default_branch;
  if (!isText(branch)) {
    throw new Error("The push event does not name the default branch");
  }
  const result = spawnSync("git", [
    "-C",
    root,
    "merge-base",
    "--is-ancestor",
    env.GITHUB_SHA,
    `origin/${branch}`,
  ]);
  if (result.status !== 0) {
    throw new Error(
      `Tag ${tag} points to ${env.GITHUB_SHA}, which is not on ${branch}. Nothing was built; merge the change into ${branch}, then tag a commit on ${branch}.`,
    );
  }
}

// Each platform lists only targets that it can build.
const isPlatformList = (value, isValid) =>
  Boolean(value) &&
  systems.every(
    (name) => isTextList(value[name]) && value[name].every((item) => isValid(item, name)),
  );

// A target in targetRunners builds on its own machine, the others on the machine of their platform.
function groupBuilds(switches, runners, desktopTargets, targetRunners) {
  return systems
    .filter((name) => switches[name])
    .flatMap((name) => {
      const groups = new Map();
      for (const target of desktopTargets[name]) {
        const os = targetRunners[target] ?? runners[name];
        const key = JSON.stringify(os);
        groups.set(key, { os, targets: [...(groups.get(key)?.targets ?? []), target] });
      }
      return [...groups.values()].map(({ os, targets: list }) => ({
        name:
          groups.size === 1
            ? name
            : `${name}-${list.map((target) => targets[target].arch).join("-")}`,
        os,
        targets: list.join(","),
      }));
    });
}

// For the check and the release alike; MAC_SELF_HOSTED_RUNNER, when set, replaces the configured
// runner of its platform.
function platformRunners() {
  const configured = setting(
    "runners",
    (value) => value && systems.every((name) => isText(value[name])),
  );
  const macSelfHosted = env.MAC_SELF_HOSTED_RUNNER?.trim();
  return macSelfHosted ? { ...configured, mac: ["self-hosted", macSelfHosted] } : configured;
}

export function runners() {
  setOutput("runners", JSON.stringify(platformRunners()));
}

// Checks the settings it reads, the version and the release notes before any machine starts
// building. The other release settings are checked where they are used.
export function plan() {
  const version = releaseVersion();
  const switches = setting(
    "platforms",
    (value) =>
      value &&
      Object.keys(value).length === systems.length &&
      systems.every((name) => typeof value[name] === "boolean"),
  );
  const runners = platformRunners();
  const macSelfHosted = env.MAC_SELF_HOSTED_RUNNER?.trim();
  const timeoutMinutes = setting(
    "timeoutMinutes",
    (value) => value && ["build", "release", "checkWait"].every((name) => isPositive(value[name])),
  );
  const artifactRetentionDays = setting("artifactRetentionDays", isPositive);
  // The app stops at startup on a missing or invalid "updates" section.
  if (
    !["firstCheckSeconds", "checkIntervalHours", "laterHours", "checkTimeoutSeconds"].every((name) =>
      isPositive(updates[name]),
    )
  ) {
    throw new TypeError("product.conf.json updates is missing or invalid");
  }
  const desktopTargets = setting("desktop.targets", (value) =>
    isPlatformList(value, (target, name) => targets[target]?.platform === name),
  );
  // A self-hosted runner builds every target of its platform, so it overrides targetRunners there.
  const targetRunners = Object.fromEntries(
    Object.entries(
      setting(
        "desktop.targetRunners",
        (value) =>
          value &&
          Object.entries(value).every(([target, runner]) => targets[target] && isText(runner)),
      ),
    ).filter(([target]) => !(macSelfHosted && targets[target].platform === "mac")),
  );

  if (app.version !== version) {
    throw new Error(
      `Tag ${version} does not match the version (${app.version}) in apps/desktop/src-tauri/tauri.conf.json. Nothing was built.`,
    );
  }
  releaseNotes();
  const builds = groupBuilds(switches, runners, desktopTargets, targetRunners);
  if (builds.length === 0) {
    throw new Error("Every platform is turned off in release.platforms.");
  }
  setOutput("builds", JSON.stringify(builds));
  setOutput("settings", JSON.stringify({ runners, timeoutMinutes, artifactRetentionDays }));
  summary(
    `Release ${version}: ${builds.map(({ name, targets: list }) => `${name} (${list})`).join("; ")}.`,
  );
}

// Runs before any build, so a wrong release setting fails without building anything.
export function check() {
  if (separateReleases) {
    if (env.RELEASES_TOKEN_SET !== "true") {
      throw new Error(
        "RELEASES_REPOSITORY is set but RELEASES_TOKEN is not. Add RELEASES_TOKEN, or clear RELEASES_REPOSITORY to release in this repository.",
      );
    }
    if (!/^[A-Za-z0-9-]+\/[A-Za-z0-9._-]+$/.test(releasesRepository)) {
      throw new Error("RELEASES_REPOSITORY must be in the form owner/name");
    }
  }
  if (viewRelease()?.isDraft === false) {
    throw new Error(`Release ${tag} is already published in ${releasesRepository}`);
  }
  if (separateReleases) {
    checkReleasesWriteAccess();
  }
}

// Reading releases works with a read-only token, so a dry-run push checks that RELEASES_TOKEN can
// also write. A dry run sends nothing, but is only allowed with write access.
function checkReleasesWriteAccess() {
  const dir = join(env.RUNNER_TEMP, "releases-write-check");
  const git = (...args) => capture("git", ["-C", dir, ...args]);
  mkdirSync(dir, { recursive: true });
  run("gh", ["auth", "setup-git"]);
  git("init", "--quiet");
  git(
    "-c",
    "user.name=github-actions[bot]",
    "-c",
    "user.email=41898282+github-actions[bot]@users.noreply.github.com",
    "commit",
    "--allow-empty",
    "--quiet",
    "-m",
    "Write access check",
  );
  try {
    git(
      "push",
      "--dry-run",
      `https://github.com/${releasesRepository}.git`,
      "HEAD:refs/heads/release-write-check",
    );
  } catch (error) {
    throw new Error(
      `RELEASES_TOKEN cannot write to ${releasesRepository}. Give it Contents: Read and write on that repository, then push the tag again: ${error.stderr}`,
    );
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
