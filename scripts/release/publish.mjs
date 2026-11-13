import { execFileSync } from "node:child_process";
import { existsSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import {
  root,
  app,
  tag,
  bundles,
  releaseVersion,
  isPrerelease,
  isReleaseFile,
} from "./settings.mjs";
import {
  env,
  separateReleases,
  releasesRepository,
  summary,
  run,
  capture,
  viewRelease,
} from "./github.mjs";

// The platforms of the download table, one row per architecture with its formats side by side.
const platforms = [
  {
    label: "macOS",
    marker: "-macos-",
    architectures: [
      ["Apple silicon (arm64)", "-macos-arm64"],
      ["Intel (x64)", "-macos-x64"],
    ],
  },
  {
    label: "Windows",
    marker: "-windows-",
    architectures: [
      ["x64", "-windows-x64"],
      ["arm64", "-windows-arm64"],
    ],
  },
  {
    label: "Linux",
    marker: "-linux-",
    architectures: [
      ["x64", "-linux-x64"],
      ["arm64", "-linux-arm64"],
    ],
  },
];

const server = () => env.GITHUB_SERVER_URL ?? "https://github.com";

const downloadUrl = (name) =>
  `${server()}/${releasesRepository}/releases/download/${encodeURIComponent(tag)}/${encodeURIComponent(name)}`;

// latestReleases maps a platform label to the latest earlier release that has its files.
export function releaseNotes(files = [], latestReleases = {}) {
  const version = releaseVersion();
  const changelog = readFileSync(join(root, "CHANGELOG.md"), "utf8");
  const sections = changelog.split(/^## /m);
  const matches = sections
    .slice(1)
    .filter((section) => section.split(/\r?\n/, 1)[0].trim() === version);
  if (matches.length !== 1) {
    throw new Error(`CHANGELOG.md must contain exactly one "## ${version}" section`);
  }
  const notes = matches[0].split(/\r?\n/).slice(1).join("\n").trim();
  if (!notes.split(/\r?\n/).some((line) => line.trim() && !line.trim().startsWith("#"))) {
    throw new Error(`CHANGELOG.md has no release notes for ${version}`);
  }
  if (files.length === 0) {
    return notes;
  }

  const listed = new Set();
  const rows = platforms.flatMap(({ label, architectures }) =>
    architectures.flatMap(([architectureLabel, architectureMarker]) => {
      const links = Object.values(bundles).flatMap(({ suffix, label: formatLabel }) =>
        files
          .filter((name) => name.endsWith(`${architectureMarker}${suffix}`))
          .map((name) => {
            listed.add(name);
            return `[${formatLabel}](${downloadUrl(name)})`;
          }),
      );
      return links.length > 0 ? [`| ${label} | ${architectureLabel} | ${links.join(" · ")} |`] : [];
    }),
  );
  const unlisted = files.filter((name) => !listed.has(name));
  if (unlisted.length > 0) {
    throw new Error(`No download table row for: ${unlisted.join(", ")}`);
  }
  // A platform this release skips links to the latest earlier release that has it, so its users
  // still find a download; a platform never released has no row.
  const notIncluded = platforms
    .filter(
      ({ label, marker }) => latestReleases[label] && !files.some((name) => name.includes(marker)),
    )
    .map(
      ({ label }) =>
        `| ${label} | — | Not in this release; latest is [${latestReleases[label]}](${server()}/${releasesRepository}/releases/tag/${encodeURIComponent(latestReleases[label])}) |`,
    );
  return [
    `## Downloads (${tag})`,
    "",
    "| Platform | Architecture | Download |",
    "| --- | --- | --- |",
    ...rows,
    ...notIncluded,
    "",
    "<details>",
    "<summary>Release Notes</summary>",
    "",
    notes,
    "",
    "</details>",
  ].join("\n");
}

// Semantic-version precedence, so releases.json never replaces a newer version with an older one.
export function compareVersions(left, right) {
  const parse = (version) => {
    const [core, prerelease] = version.split("+", 1)[0].split(/-(.*)/s);
    return { core: core.split(".").map(Number), prerelease: prerelease?.split(".") ?? [] };
  };
  const a = parse(left);
  const b = parse(right);
  for (let index = 0; index < 3; index += 1) {
    if (a.core[index] !== b.core[index]) {
      return a.core[index] - b.core[index];
    }
  }
  if (a.prerelease.length === 0 || b.prerelease.length === 0) {
    return b.prerelease.length - a.prerelease.length;
  }
  for (let index = 0; index < Math.max(a.prerelease.length, b.prerelease.length); index += 1) {
    const [x, y] = [a.prerelease[index], b.prerelease[index]];
    if (x === y) {
      continue;
    }
    if (x === undefined || y === undefined) {
      return x === undefined ? -1 : 1;
    }
    const [isXNumber, isYNumber] = [/^\d+$/.test(x), /^\d+$/.test(y)];
    if (isXNumber && isYNumber) {
      return Number(x) - Number(y);
    }
    if (isXNumber !== isYNumber) {
      return isXNumber ? -1 : 1;
    }
    return x < y ? -1 : 1;
  }
  return 0;
}

// The commit that recorded this release, in `git log --format="%H %s"` output. The subject has to
// match in full: "Release 1.2.0" is not the commit of "Release 1.2.0-rc.1".
export function releaseCommit(log, tag) {
  const line = log.split("\n").find((line) => line.slice(line.indexOf(" ") + 1) === `Release ${tag}`);
  return line?.slice(0, line.indexOf(" "));
}

// Returns the commit the tag points at. Prereleases are recorded under "prerelease".
function recordRelease() {
  const dir = join(env.RUNNER_TEMP, "releases-repository");
  const git = (...args) => capture("git", ["-C", dir, ...args]).trim();
  run("gh", ["auth", "setup-git"]);
  run("gh", ["repo", "clone", releasesRepository, dir]);
  const branch = capture("gh", [
    "api",
    `repos/${releasesRepository}`,
    "--jq",
    ".default_branch",
  ]).trim();
  git("config", "user.name", "github-actions[bot]");
  git("config", "user.email", "41898282+github-actions[bot]@users.noreply.github.com");
  const manifestPath = join(dir, "releases.json");
  const readManifest = () =>
    existsSync(manifestPath) ? JSON.parse(readFileSync(manifestPath, "utf8")) : {};
  // An empty repository has no commits yet; this release makes its first one.
  const isEmpty = git("rev-list", "--all", "--max-count=1") === "";
  if (isEmpty) {
    git("checkout", "-B", branch);
  }
  // A rerun finds the commit that an earlier attempt of this release pushed.
  if (!isEmpty) {
    const pushed = releaseCommit(git("log", "--format=%H %s"), tag);
    if (pushed) {
      return pushed;
    }
  }

  const manifest = readManifest();
  const channel = isPrerelease() ? (manifest.prerelease ??= {}) : manifest;
  const recorded = channel.desktop?.version;
  if (!recorded || compareVersions(app.version, recorded) > 0) {
    channel.desktop = { version: app.version, tag };
  }
  writeFileSync(manifestPath, `${JSON.stringify(manifest, null, 2)}\n`);
  git("add", "releases.json");
  git("commit", "--allow-empty", "-m", `Release ${tag}`);
  // A push rejected because someone pushed meanwhile is replayed on top of their commits.
  for (let attempt = 1; ; attempt += 1) {
    try {
      git("push", "origin", `HEAD:${branch}`);
      break;
    } catch (error) {
      if (attempt === 3) {
        throw new Error(`Pushing releases.json failed: ${error.stderr}`);
      }
      git("pull", "--rebase", "origin", branch);
    }
  }
  return git("rev-parse", "HEAD");
}

// Only the fields read here: the full list overflows the output limit of capture.
function listReleases() {
  return capture("gh", [
    "api",
    "--paginate",
    `repos/${releasesRepository}/releases?per_page=100`,
    "--jq",
    ".[] | {tag: .tag_name, draft, prerelease, assets: [.assets[].name]}",
  ])
    .split("\n")
    .filter(Boolean)
    .map((line) => JSON.parse(line));
}

// For each platform, the latest earlier stable release in the releases repository that has its files.
export function latestReleasesByPlatform(releases, currentTag = tag) {
  const latest = {};
  for (const { tag: releaseTag, draft, prerelease, assets } of releases) {
    if (draft || prerelease || releaseTag === currentTag) {
      continue;
    }
    for (const { label, marker } of platforms) {
      // The highest version, which is not always the one published last.
      const newer = !latest[label] || compareVersions(releaseTag, latest[label]) > 0;
      if (newer && assets.some((name) => name.includes(marker))) {
        latest[label] = releaseTag;
      }
    }
  }
  return latest;
}

// The "latest" download link reads the latest release, so a fix to an older line must not take it over.
const isNewestDesktopRelease = (version, releases) =>
  releases
    .filter(
      ({ draft, prerelease, assets }) =>
        !draft && !prerelease && assets.some((name) => isReleaseFile(name)),
    )
    .every(({ tag: releaseTag }) => compareVersions(version, releaseTag) >= 0);

// Installed apps read latest.json from the latest release to find an update. Returns the files it
// needs besides the installers.
function writeUpdateManifest(filesDir, names) {
  const entries = Object.assign(
    {},
    ...names
      .filter((name) => /^update-.+\.json$/.test(name))
      .map((name) => JSON.parse(readFileSync(join(filesDir, name), "utf8"))),
  );
  if (Object.keys(entries).length === 0) {
    summary("No latest.json: the builds made no update files.");
    return [];
  }
  const platforms = Object.fromEntries(
    Object.entries(entries).map(([platform, { name, signature }]) => [
      platform,
      { url: downloadUrl(name), signature },
    ]),
  );
  writeFileSync(
    join(filesDir, "latest.json"),
    `${JSON.stringify(
      {
        version: releaseVersion(),
        notes: releaseNotes(),
        pub_date: new Date().toISOString(),
        platforms,
      },
      null,
      2,
    )}\n`,
  );
  const updateFiles = Object.values(entries)
    .map(({ name }) => name)
    .filter((name) => !isReleaseFile(name));
  return ["latest.json", ...new Set(updateFiles)];
}

export function publish(filesDir) {
  const names = readdirSync(filesDir);
  const files = names.filter(isReleaseFile).sort();
  // Checked again here: a tag pushed again while the first run built is let through by its own
  // prepare job, and replacing published files would hand users different builds of one version.
  const release = viewRelease();
  if (release && !release.isDraft) {
    throw new Error(`${tag} is already published; its files were left as they are`);
  }
  const releases = listReleases();
  const notes = releaseNotes(files, latestReleasesByPlatform(releases));
  const updateFiles = writeUpdateManifest(filesDir, names);
  const recordedCommit = separateReleases ? recordRelease() : undefined;
  if (!release) {
    // The source repository already has the pushed tag; the releases repository gets it on the
    // commit that records this release.
    const target = recordedCommit ? ["--target", recordedCommit] : ["--verify-tag"];
    execFileSync(
      "gh",
      [
        "release",
        "create",
        tag,
        "--repo",
        releasesRepository,
        ...target,
        "--title",
        tag,
        "--draft",
        "--notes-file",
        "-",
      ],
      { input: notes, stdio: ["pipe", "inherit", "inherit"] },
    );
  }
  run("gh", [
    "release",
    "upload",
    tag,
    ...[...files, ...updateFiles].map((name) => join(filesDir, name)),
    "--repo",
    releasesRepository,
    "--clobber",
  ]);
  // Only the newest stable release becomes the latest release.
  let flags = ["--latest=false"];
  if (isPrerelease()) {
    flags = ["--prerelease", "--latest=false"];
  } else if (isNewestDesktopRelease(releaseVersion(), releases)) {
    flags = ["--latest"];
  }
  execFileSync(
    "gh",
    [
      "release",
      "edit",
      tag,
      "--repo",
      releasesRepository,
      "--notes-file",
      "-",
      "--draft=false",
      ...flags,
    ],
    { input: notes, stdio: ["pipe", "inherit", "inherit"] },
  );
  summary(`Published ${tag} in ${releasesRepository} with ${files.length} files.`);
}
