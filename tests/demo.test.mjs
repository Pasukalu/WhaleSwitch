import assert from "node:assert/strict";
import test from "node:test";
import { demo } from "../src/demo.ts";
import { targetOf } from "../src/domain.ts";

test("browser workbench reviews, saves, compares and restores a simulated configuration", async () => {
  const original = await demo("view", { tool: "codex" });
  const target = targetOf(original);
  const patch = { model: "demo-replacement" };
  const diff = await demo("preview", { target, patch });
  assert.deepEqual(diff, [{ field: "model", before: original.fields.model, after: patch.model }]);
  assert.equal((await demo("view", { tool: "codex" })).fields.model, original.fields.model);

  const saved = await demo("save", { target, patch });
  assert.equal(saved.fields.model, patch.model);
  await assert.rejects(() => demo("save", { target, patch }), (error) => error === "conflict");
  const [backup] = await demo("snapshots", { tool: "codex" });
  assert.equal(backup.reason, "before_save");
  const [undo] = await demo("compare_snapshot", { target: targetOf(saved), id: backup.id });
  assert.equal(undo.after, original.fields.model);
  const restored = await demo("restore", { target: targetOf(saved), id: backup.id });
  assert.equal(restored.fields.model, original.fields.model);
});

test("an absent config is created without recording a prior file", async () => {
  const missing = await demo("view", { tool: "opencode" });
  assert.equal(missing.exists, false);
  const view = await demo("save", { target: targetOf(missing), patch: { model: "openai/demo" } });
  assert.equal(view.exists, true);
  assert.equal((await demo("snapshots", { tool: "opencode" })).length, 0);
});

test("browser preview cannot send an API key or make a real request", async () => {
  const input = { tool: "codex", protocol: "openai", endpoint: "https://api.openai.com/v1" };
  assert.equal(await demo("has_key", { input }), false);
  await assert.rejects(() => demo("set_key", { input, secret: "fake-key-for-test" }), (error) => error === "preview_only");
  await assert.rejects(() => demo("probe", { input }), (error) => error === "preview_only");
});

test("browser preview inspects only fixture settings and keeps snapshot lists bound to the displayed path", async () => {
  const current = await demo("inspect_config", { tool: "claude" });
  assert.equal(current.config.kind, "view");
  assert.equal(current.config.view.path, "~/.claude/settings.json");
  assert.deepEqual(current.active_env, []);
  await assert.rejects(() => demo("pick_project_config", {}), (error) => error === "preview_only");
  await assert.rejects(() => demo("snapshots", { tool: "claude", path: "/wrong/settings.json" }), (error) => error === "conflict");
});
