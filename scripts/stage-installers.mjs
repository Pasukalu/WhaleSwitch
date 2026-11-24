import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { copyFile, mkdir, readFile, readdir, writeFile } from "node:fs/promises";
import { join } from "node:path";

const targets = {
  "windows-x64": ["nsis", ".exe"],
  "macos-arm64": ["dmg", ".dmg"],
  "macos-x64": ["dmg", ".dmg"],
  "linux-x64": ["deb", ".deb"],
};
const { version } = JSON.parse(await readFile("package.json", "utf8"));
const output = "release-assets";

function installerName(target) {
  return `WhaleSwitch-${version}-${target}${targets[target][1]}`;
}

async function checksumLine(filename) {
  const bytes = await readFile(join(output, filename));
  assert(bytes.length > 0, `Empty installer ${filename}`);
  const digest = createHash("sha256").update(bytes).digest("hex");
  return `${digest}  ${filename}\n`;
}

async function stage(target) {
  assert(Object.hasOwn(targets, target), "Unknown installer target");
  const [bundle, extension] = targets[target];
  const folder = join("src-tauri", "target", "release", "bundle", bundle);
  const files = (await readdir(folder, { withFileTypes: true }))
    .filter((file) => file.isFile() && file.name.endsWith(extension));
  assert.equal(files.length, 1, `Expected one ${extension} bundle in ${folder}`);
  const filename = installerName(target);
  await mkdir(output, { recursive: true });
  await copyFile(join(folder, files[0].name), join(output, filename));
  await writeFile(join(output, filename + ".sha256"), await checksumLine(filename));
  console.log(filename);
}

async function verify() {
  const filenames = Object.keys(targets).map(installerName);
  const expected = filenames.flatMap((name) => [name, name + ".sha256"]);
  const received = await readdir(output, { withFileTypes: true });
  assert(received.every((entry) => entry.isFile()), "Release assets must be regular files");
  assert.deepEqual(received.map((entry) => entry.name).sort(), expected.sort(), "Unexpected release asset set");
  for (const filename of filenames) {
    const checksum = await readFile(join(output, filename + ".sha256"), "utf8");
    assert.equal(checksum, await checksumLine(filename), `SHA-256 mismatch for ${filename}`);
  }
  console.log(`Verified ${filenames.length} installers and SHA-256 files for ${version}`);
}

if (process.argv[2] === "--verify") {
  await verify();
} else {
  await stage(process.argv[2]);
}
