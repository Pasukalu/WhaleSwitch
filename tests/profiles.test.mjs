import assert from "node:assert/strict";
import test from "node:test";
import { demo } from "../src/demo.ts";
import { defaultChangeSelection, fieldPatch, fieldPath, initialProbe, selectedChangePatch, targetOf } from "../src/domain.ts";

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

test("renaming and duplicating profiles preserves captured fields and leaves live files untouched", async () => {
  const tool = "codex";
  const view = await demo("view", { tool });
  const target = targetOf(view);
  const original = await demo("create_profile", { target, name: "Daily", patch: { model: "draft-model", model_verbosity: null } });
  const copied = await demo("duplicate_profile", { tool, id: original.id, name: "Nightly" });
  assert.notEqual(copied.id, original.id);
  assert.deepEqual(copied.fields, original.fields);
  await assert.rejects(() => demo("rename_profile", { tool, id: original.id, name: "nightly" }), (error) => error === "profile_name_exists");
  await assert.rejects(() => demo("duplicate_profile", { tool: "claude", id: original.id, name: "Other tool" }), (error) => error === "profile_missing");
  const renamed = await demo("rename_profile", { tool, id: original.id, name: " Daily work " });
  assert.deepEqual(renamed, { ...original, name: "Daily work" });
  assert.deepEqual(await demo("view", { tool }), view);
  assert.deepEqual(await demo("snapshots", { tool }), []);
});

test("profile review defaults to keeping fields marked for removal and writes only the selected values", async () => {
  const tool = "claude";
  const initial = await demo("view", { tool });
  const current = await demo("save", { target: targetOf(initial), patch: { anthropic_base_url: "https://current.example" } });
  const target = targetOf(current);
  const profile = await demo("create_profile", { target, name: "Foreign device", patch: { model: "remote-model", anthropic_base_url: null } });
  const { changes } = await demo("preview_profile", { target, id: profile.id });
  const selection = selectedChangePatch(changes, defaultChangeSelection(changes));
  assert.deepEqual(selection, { model: "remote-model" });
  assert.deepEqual(selectedChangePatch(changes, ["anthropic_base_url"]), { anthropic_base_url: null });
  assert.deepEqual(selectedChangePatch(changes, ["default_agent"]), {});
  await assert.rejects(() => demo("apply_profile", { target, id: profile.id, selection: { model: "other" } }), (error) => error === "conflict");
  const applied = await demo("apply_profile", { target, id: profile.id, selection });
  assert.equal(applied.fields.model, "remote-model");
  assert.equal(applied.fields.anthropic_base_url, "https://current.example");
});
