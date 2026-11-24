import assert from "node:assert/strict";
import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync } from "node:fs";
import { join } from "node:path";

const [mode, tag, target, folder] = process.argv.slice(2);
assert(/^v\d+\.\d+\.\d+$/.test(tag), "Expected a release tag in vX.Y.Z format");
const repository = process.env.GITHUB_REPOSITORY;
assert(/^[\w.-]+\/[\w.-]+$/.test(repository), "Missing GitHub repository");

const windows = ["windows-x64.exe", "windows-x64-portable.zip"];
const suffixes = {
  "windows-x64": windows,
  "macos-arm64": ["macos-arm64.dmg"],
  "macos-x64": ["macos-x64.dmg"],
  "linux-x64": ["linux-x64.deb"],
};
const filenames = (suffix) => {
  const name = `WhaleSwitch-${tag.slice(1)}-${suffix}`;
  return [name, name + ".sha256"];
};
const gh = (args, stdio) => execFileSync("gh", args, { encoding: "utf8", stdio });
const checksum = (data) => createHash("sha256").update(data).digest("hex");

if (mode === "manifest") {
  const release = JSON.parse(gh(["api", `repos/${repository}/releases/tags/${tag}`]));
  assert(release.tag_name === tag && !release.draft, "Expected a published release");
  const manifest = Object.fromEntries(Object.values(suffixes).flat().flatMap(filenames).map((name) => {
    const asset = release.assets.find((entry) => entry.name === name && entry.state === "uploaded");
    assert(asset?.size > 0 && /^sha256:[a-f0-9]{64}$/.test(asset.digest), `Missing release asset digest for ${name}`);
    return [name, asset.digest.slice("sha256:".length)];
  }));
  console.log(JSON.stringify(manifest));
} else {
  assert.equal(mode, "download");
  assert(typeof folder === "string" && folder.length > 0, "Missing download directory");
  assert(target === "all" || Object.hasOwn(suffixes, target), "Unknown package target");
  const digests = JSON.parse(process.env.WHALESWITCH_RELEASE_DIGESTS);
  const selected = target === "all" ? Object.values(suffixes).flat() : suffixes[target];
  const patterns = selected.flatMap(filenames).flatMap((name) => ["--pattern", name]);
  mkdirSync(folder, { recursive: true });
  gh(["release", "download", tag, "--repo", repository, "--dir", folder, ...patterns], "inherit");
  for (const suffix of selected) {
    const [name, sumName] = filenames(suffix);
    const actual = checksum(readFileSync(join(folder, name)));
    const sum = readFileSync(join(folder, sumName));
    assert.equal(actual, digests[name], `Release package digest changed for ${name}`);
    assert.equal(checksum(sum), digests[sumName], `Release checksum asset changed for ${name}`);
    assert.equal(sum.toString("utf8"), `${actual}  ${name}\n`, `Invalid checksum file for ${name}`);
    console.log(`Verified published package ${name}`);
  }
}
