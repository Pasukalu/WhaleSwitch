import assert from "node:assert/strict";
import test from "node:test";
import { demo } from "../src/demo.ts";
import { defaultChangeSelection, selectedChangePatch, targetOf } from "../src/domain.ts";

test("reviews a multi-tool plan, rejects a stale target, and keeps unchecked removals", async () => {
  const codex = await demo("view", { tool: "codex" });
  const claude = await demo("view", { tool: "claude" });
  const codexProfile = await demo("create_profile", {
    target: targetOf(codex), name: "CLI profile", patch: { model: "local-model", model_provider: null },
  });
  const claudeProfile = await demo("create_profile", {
    target: targetOf(claude), name: "CLI profile", patch: { model: "opus" },
  });
  const plan = await demo("create_plan", {
    name: "Local inference", profiles: [
      { tool: "codex", profile_id: codexProfile.id },
      { tool: "claude", profile_id: claudeProfile.id },
    ],
  });
  await demo("delete_profile", { tool: "codex", id: codexProfile.id });

  async function choices() {
    const previews = await demo("preview_plan", { id: plan.id });
    return previews.map(({ tool, path, revision, changes }) => ({
      tool, path, revision, fields: selectedChangePatch(changes, defaultChangeSelection(changes)),
    }));
  }

  const stale = await choices();
  await demo("save", { target: targetOf(claude), patch: { model: "external" } });
  await assert.rejects(() => demo("apply_plan", { id: plan.id, selections: stale }), (error) => error === "conflict");
  assert.equal((await demo("view", { tool: "codex" })).fields.model, codex.fields.model);

  const run = await demo("apply_plan", { id: plan.id, selections: await choices() });
  assert.equal(run.finished, true);
  assert.deepEqual(run.files.map((file) => file.status), ["applied", "applied"]);
  assert.ok(run.files.every((file) => file.snapshot_id !== null));
  const updatedCodex = await demo("view", { tool: "codex" });
  assert.equal(updatedCodex.fields.model, "local-model");
  assert.equal(updatedCodex.fields.model_provider, codex.fields.model_provider);
  assert.equal((await demo("view", { tool: "claude" })).fields.model, "opus");
  assert.equal((await demo("plan_runs", {}))[0].id, run.id);
});
