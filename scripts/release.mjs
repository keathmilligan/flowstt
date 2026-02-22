#!/usr/bin/env node

import { execFileSync } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";

const usage = `Usage: node scripts/release.mjs vX.Y.Z [--dry-run]

Validates version increment, updates versioned files and README links,
commits changes, pushes, creates the tag, and pushes the tag.

Options:
  --dry-run   Apply local file updates only (no git commands).`;

const args = process.argv.slice(2);
if (args.includes("-h") || args.includes("--help")) {
  console.log(usage);
  process.exit(0);
}

const dryRun = args.includes("--dry-run");
const filteredArgs = args.filter((arg) => arg !== "--dry-run");
if (filteredArgs.length !== 1) {
  console.error(usage);
  process.exit(1);
}

const tag = filteredArgs[0];
if (!/^v\d+\.\d+\.\d+$/.test(tag)) {
  console.error(`Invalid version tag: ${tag}. Expected format vX.Y.Z`);
  process.exit(1);
}

const version = tag.slice(1);

const run = (cmd, cmdArgs, options = {}) =>
  execFileSync(cmd, cmdArgs, { encoding: "utf8", ...options }).trim();

const scriptDir = path.dirname(new URL(import.meta.url).pathname);
const repoRoot = dryRun ? run("git", ["rev-parse", "--show-toplevel"]) : run("git", ["rev-parse", "--show-toplevel"]);
process.chdir(repoRoot);

const dirty = run("git", ["status", "--porcelain"]);
if (dirty && !dryRun) {
  console.error("Working tree is not clean. Commit or stash changes first.");
  process.exit(1);
}

const parseSemver = (input) => {
  const match = /^v?(\d+)\.(\d+)\.(\d+)$/.exec(input);
  if (!match) return null;
  return {
    major: Number(match[1]),
    minor: Number(match[2]),
    patch: Number(match[3]),
  };
};

const compareSemver = (a, b) => {
  if (a.major !== b.major) return a.major - b.major;
  if (a.minor !== b.minor) return a.minor - b.minor;
  return a.patch - b.patch;
};

const isValidIncrement = (next, prev) => {
  if (next.major === prev.major && next.minor === prev.minor) {
    return next.patch > prev.patch;
  }
  if (next.major === prev.major && next.minor > prev.minor) {
    return next.patch === 0;
  }
  if (next.major > prev.major) {
    return next.minor === 0 && next.patch === 0;
  }
  return false;
};

const latestTag = run("git", ["tag", "--list", "v*", "--merged", "HEAD", "--sort=-v:refname"])
  .split("\n")
  .filter(Boolean)[0];

const tagExists = run("git", ["tag", "--list", tag]);
if (tagExists) {
  console.error(`Tag ${tag} already exists.`);
  process.exit(1);
}

if (latestTag) {
  const prev = parseSemver(latestTag);
  const next = parseSemver(version);
  if (!prev || !next) {
    console.error("Unable to parse versions for comparison.");
    process.exit(1);
  }
  if (compareSemver(next, prev) <= 0 || !isValidIncrement(next, prev)) {
    console.error(`Version ${version} is not a valid increment of ${latestTag}.`);
    process.exit(1);
  }
}

const updateJsonVersion = async (filePath) => {
  const content = await readFile(filePath, "utf8");
  const data = JSON.parse(content);
  data.version = version;
  await writeFile(filePath, `${JSON.stringify(data, null, 2)}\n`);
};

const updateCargoVersion = async (filePath) => {
  const content = await readFile(filePath, "utf8");
  const updated = content.replace(
    /^(version\s*=\s*")\d+\.\d+\.\d+("\s*)$/m,
    `$1${version}$2`
  );
  if (updated === content) {
    throw new Error(`No version field updated in ${filePath}`);
  }
  await writeFile(filePath, updated);
};

const getRepoSlugFromRemote = () => {
  const remote = run("git", ["config", "--get", "remote.origin.url"]);
  const match =
    /github\.com[:/](.+?)(?:\.git)?$/.exec(remote) ||
    /git@github\.com:(.+?)(?:\.git)?$/.exec(remote);
  if (!match) {
    throw new Error("Unable to determine GitHub repo from origin remote.");
  }
  return match[1];
};

const getRepoSlugFromReadme = (content) => {
  const match = /github\.com\/([\w.-]+\/[\w.-]+)/.exec(content);
  if (!match) {
    throw new Error("Unable to determine GitHub repo from README.");
  }
  return match[1];
};

const updateReadme = async (filePath) => {
  const content = await readFile(filePath, "utf8");
  const repo = getRepoSlugFromRemote();
  const baseUrl = `https://github.com/${repo}/releases/download/${tag}`;
  const linkBlock = [
    "<!-- release-links:start -->",
    `**Current release:** ${tag}`,
    "",
    "**Download packages**",
    `- Windows: [FlowSTT_${version}_x64.msi](${baseUrl}/FlowSTT_${version}_x64.msi)`,
    `- macOS: [FlowSTT_aarch64.dmg](${baseUrl}/FlowSTT_aarch64.dmg), [FlowSTT_x64.dmg](${baseUrl}/FlowSTT_x64.dmg)`,
    "<!-- release-links:end -->",
  ].join("\n");

  let updated = content;
  if (updated.includes("<!-- release-links:start -->")) {
    updated = updated.replace(
      /<!-- release-links:start -->[\s\S]*?<!-- release-links:end -->/,
      linkBlock
    );
  } else {
    updated = updated.replace(
      /## Installation\n\n([\s\S]*?\n)\n/,
      (match) => `${match}${linkBlock}\n\n`
    );
  }

  updated = updated
    .replace(/X\.X\.X/g, version)
    .replace(/v\d+\.\d+\.\d+/g, tag)
    .replace(/FlowSTT_\d+\.\d+\.\d+_x64\.msi/g, `FlowSTT_${version}_x64.msi`)
    .replace(/FlowSTT_\d+\.\d+\.\d+_aarch64\.dmg/g, "FlowSTT_aarch64.dmg")
    .replace(/FlowSTT_\d+\.\d+\.\d+_x64\.dmg/g, "FlowSTT_x64.dmg")
    ;

  await writeFile(filePath, updated);
};

const updateTargets = async () => {
  await updateJsonVersion(path.join(repoRoot, "package.json"));
  await updateJsonVersion(path.join(repoRoot, "src-tauri", "tauri.conf.json"));
  await updateCargoVersion(path.join(repoRoot, "src-tauri", "Cargo.toml"));
  await updateCargoVersion(path.join(repoRoot, "src-engine", "Cargo.toml"));
  await updateCargoVersion(path.join(repoRoot, "src-common", "Cargo.toml"));
  await updateCargoVersion(path.join(repoRoot, "src-cli", "Cargo.toml"));
  await updateReadme(path.join(repoRoot, "README.md"));
};

const commitMessage = `chore: release ${tag}`;

const validatePackageJsonVersion = async () => {
  const content = await readFile(path.join(repoRoot, "package.json"), "utf8");
  const data = JSON.parse(content);
  const current = parseSemver(data.version);
  const next = parseSemver(version);
  if (!current || !next) {
    console.error("Unable to parse package.json version for comparison.");
    process.exit(1);
  }
  if (compareSemver(next, current) <= 0 || !isValidIncrement(next, current)) {
    console.error(`Version ${version} is not a valid increment of package.json version ${data.version}.`);
    process.exit(1);
  }
};

const main = async () => {
  await validatePackageJsonVersion();
  await updateTargets();
  if (dryRun) {
    console.log("Dry run complete. Local files updated; no git commands executed.");
    return;
  }
  run("git", ["add", "package.json", "src-tauri/tauri.conf.json", "src-tauri/Cargo.toml", "src-engine/Cargo.toml", "src-common/Cargo.toml", "src-cli/Cargo.toml", "README.md"]);
  const staged = run("git", ["diff", "--cached", "--name-only"]);
  if (!staged) {
    console.error("No changes staged for commit. Aborting release.");
    process.exit(1);
  }
  run("git", ["commit", "-m", commitMessage]);
  run("git", ["push"]);
  run("git", ["tag", tag]);
  run("git", ["push", "origin", tag]);
  console.log(`Release ${tag} prepared and pushed.`);
};

main().catch((err) => {
  console.error(err.message || err);
  process.exit(1);
});
