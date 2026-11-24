import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { appendFile, mkdir, mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import test from "node:test";

const script = fileURLToPath(new URL("../scripts/stage-installers.mjs", import.meta.url));

test("release verification requires four installers and the portable ZIP with their checksums", async () => {
  const root = await mkdtemp(join(tmpdir(), "whaleswitch-release-"));
  const run = (...args) => spawnSync(process.execPath, [script, ...args], { cwd: root, encoding: "utf8" });
  try {
    await writeFile(join(root, "package.json"), '{"version":"1.4.0"}');
    for (const [target, bundle, extension] of [
      ["windows-x64", "nsis", "exe"],
      ["macos-arm64", "dmg", "dmg"],
      ["macos-x64", "dmg", "dmg"],
      ["linux-x64", "deb", "deb"],
    ]) {
      const directory = join(root, "src-tauri", "target", "release", "bundle", bundle);
      await mkdir(directory, { recursive: true });
      await writeFile(join(directory, `fixture.${extension}`), target);
      const staged = run(target);
      assert.equal(staged.status, 0, staged.stderr);
    }
    assert.match(run("--verify").stderr, /Unexpected release asset set/);
    await writeFile(join(root, "release-assets", "WhaleSwitch-1.4.0-windows-x64-portable.zip"), "portable fixture");
    const portableChecksum = run("--checksum-portable");
    assert.equal(portableChecksum.status, 0, portableChecksum.stderr);
    const verify = run("--verify");
    assert.equal(verify.status, 0, verify.stderr);
    assert.match(verify.stdout, /Verified 5 release assets/);

    const installer = join(root, "release-assets", "WhaleSwitch-1.4.0-windows-x64.exe");
    await appendFile(installer, "different bytes");
    assert.match(run("--verify").stderr, /SHA-256 mismatch/);
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
