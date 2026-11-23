import assert from "node:assert/strict";
import test from "node:test";
import { demo } from "../src/demo.ts";
import { fieldPatch, fieldPath, initialProbe, targetOf } from "../src/domain.ts";

test("profiles capture a draft, preview changes, and back up before applying", async () => {
  const view = await demo("view", { tool: "claude" });
  const target = targetOf(view);
  const gateway = "https://gateway.example/team";
  const created = await demo("create_profile", { target, name: " Team ", patch: { anthropic_base_url: gateway } });
  assert.equal(created.name, "Team");
  assert.equal((await demo("view", { tool: "claude" })).fields.anthropic_base_url, null);
  assert.deepEqual(await demo("profiles", { tool: "opencode" }), []);
  await assert.rejects(() => demo("create_profile", { target, name: "team" }), (error) => error === "profile_name_exists");
  const preview = await demo("preview_profile", { target, id: created.id });
  assert.deepEqual(preview.changes, [{ field: "anthropic_base_url", before: null, after: gateway }]);
  const applied = await demo("apply_profile", { target, id: created.id });
  assert.equal(applied.fields.anthropic_base_url, gateway);
  assert.equal((await demo("snapshots", { tool: "claude" }))[0].reason, "before_save");
  await assert.rejects(() => demo("apply_profile", { target, id: created.id }), (error) => error === "conflict");
  await demo("delete_profile", { tool: "claude", id: created.id });
  assert.deepEqual(await demo("profiles", { tool: "claude" }), []);
  assert.equal((await demo("view", { tool: "claude" })).fields.anthropic_base_url, gateway);
});

test("new CLI fields appear in field patches and Claude's endpoint prefills its probe", () => {
  const fields = { anthropic_base_url: "https://gateway.example/team" };
  assert.deepEqual(fieldPatch(fields, { anthropic_base_url: "" }), { anthropic_base_url: null });
  assert.equal(fieldPath("anthropic_base_url"), "env.ANTHROPIC_BASE_URL");
  assert.deepEqual(initialProbe({ fields }, "claude"), {
    endpoint: fields.anthropic_base_url, protocol: "anthropic", tool: "claude",
  });
  assert.deepEqual(fieldPatch({ default_agent: "build" }, { default_agent: "plan" }), { default_agent: "plan" });
});
