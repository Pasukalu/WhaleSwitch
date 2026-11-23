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
const target = process.argv[2];
assert(Object.hasOwn(targets, target), "Unknown installer target");
const [bundle, extension] = targets[target];
const { version } = JSON.parse(await readFile("package.json", "utf8"));
const folder = join("src-tauri", "target", "release", "bundle", bundle);
const files = (await readdir(folder, { withFileTypes: true }))
  .filter((file) => file.isFile() && file.name.endsWith(extension));
assert.equal(files.length, 1, `Expected one ${extension} bundle in ${folder}`);
const filename = `WhaleSwitch-${version}-${target}${extension}`;
const out = join("release-assets", filename);
await mkdir("release-assets", { recursive: true });
await copyFile(join(folder, files[0].name), out);
const digest = createHash("sha256").update(await readFile(out)).digest("hex");
await writeFile(`${out}.sha256`, `${digest}  ${filename}\n`);
console.log(filename);
