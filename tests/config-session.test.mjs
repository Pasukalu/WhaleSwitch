import assert from "node:assert/strict";
import test from "node:test";
import { acceptConfig, describeDrift, emptyConfigSession, mergeDraft, observeConfig } from "../src/config-session.ts";
import { fieldPatch, sameTarget } from "../src/domain.ts";

const original = {
  tool: "codex", path: "/user/.codex/config.toml", format: "toml", exists: true, issue: null, revision: "old",
  fields: { model: "original-model", model_provider: "openai", model_verbosity: "medium" },
};

function inspection(view) {
  return { config: { kind: "view", view }, active_env: [] };
}

function edit(fields) {
  const session = observeConfig(emptyConfigSession, inspection(original));
  return { ...session, draft: { ...session.draft, ...fields } };
}

test("clean sessions adopt new, deleted, and moved files and flag byte-only updates", () => {
  const starting = edit({});
  const annotated = { ...original, revision: "comment-changed" };
  const refreshed = observeConfig(starting, inspection(annotated));
  assert.equal(refreshed.autoChange, "modified");
  assert.equal(refreshed.baseline.view.revision, "comment-changed");
  assert.equal(describeDrift(refreshed), null);
  assert.equal(sameTarget(refreshed.baseline.view, original), false);

  const missing = { ...annotated, exists: false, revision: "missing", fields: { model: null } };
  const deleted = observeConfig(refreshed, inspection(missing));
  assert.equal(deleted.autoChange, "deleted");
  assert.deepEqual(deleted.draft, { model: null });
  const recreated = observeConfig(deleted, inspection({ ...original, revision: "recreated" }));
  assert.equal(recreated.autoChange, "created");
  const moved = observeConfig(recreated, inspection({ ...recreated.baseline.view, path: "/other/config.toml" }));
  assert.equal(moved.autoChange, "target_changed");
  assert.equal(moved.baseline.view.path, "/other/config.toml");
});

test("merges independent edits and requires an explicit choice to override another edit to the same field", () => {
  const session = edit({ model: "my-draft", model_provider: null });
  const after = { ...original, revision: "external", fields: { ...original.fields, model: "external-model", model_verbosity: "high" } };
  const pending = observeConfig(session, inspection(after));
  assert.equal(pending.baseline.view.revision, "old");
  assert.deepEqual(pending.draft, session.draft);
  assert.deepEqual(describeDrift(pending).rows, [
    { field: "model", before: "original-model", current: "external-model", draft: "my-draft", edited: true, conflict: true },
    { field: "model_provider", before: "openai", current: "openai", draft: null, edited: true, conflict: false },
    { field: "model_verbosity", before: "medium", current: "high", draft: null, edited: false, conflict: false },
  ]);
  const keepFile = mergeDraft(pending, []);
  assert.deepEqual(keepFile.draft, { model: "external-model", model_provider: null, model_verbosity: "high" });
  assert.deepEqual(fieldPatch(after.fields, keepFile.draft), { model_provider: null });
  const keepMine = mergeDraft(pending, ["model", "model_verbosity"]);
  assert.deepEqual(fieldPatch(after.fields, keepMine.draft), { model: "my-draft", model_provider: null });
  assert.equal(keepMine.baseline.view.revision, "external");
  assert.equal(describeDrift(keepMine), null);
  assert.equal(sameTarget(keepMine.baseline.view, original), false);
});

test("convergent edits do not conflict and a reverted external revision keeps the draft", () => {
  const session = edit({ model: "my-draft" });
  const after = { ...original, revision: "external", fields: { ...original.fields, model: "my-draft" } };
  const pending = observeConfig(session, inspection(after));
  assert.equal(describeDrift(pending).rows[0].conflict, false);
  assert.deepEqual(fieldPatch(after.fields, mergeDraft(pending, []).draft), {});
  const reverted = observeConfig(pending, inspection(original));
  assert.equal(describeDrift(reverted), null);
  assert.deepEqual(reverted.draft, session.draft);
});

test("requires a new review after a deletion and never transports a draft to another path", () => {
  const session = edit({ model: "my-draft" });
  const missing = { ...original, exists: false, revision: "missing", fields: { model: null, model_provider: null, model_verbosity: null } };
  const pending = observeConfig(session, inspection(missing));
  assert.equal(describeDrift(pending).kind, "deleted");
  const rebased = mergeDraft(pending, ["model"]);
  assert.equal(rebased.baseline.view.exists, false);
  assert.deepEqual(fieldPatch(missing.fields, rebased.draft), { model: "my-draft" });
  const moved = observeConfig(pending, inspection({ ...original, path: "/other/config.toml" }));
  assert.equal(describeDrift(moved).kind, "target_changed");
  assert.equal(describeDrift(moved).canMerge, false);
  assert.equal(mergeDraft(moved, ["model"]), moved);
  assert.equal(acceptConfig(moved.latest).draft.model, "original-model");
});

test("preserves a draft through unresolved paths and malformed files", () => {
  const session = edit({ model: "my-draft" });
  const unresolved = observeConfig(session, { config: { kind: "unavailable", path: null, issue: "ambiguous_config" }, active_env: [] });
  for (const unavailable of [unresolved, observeConfig(unresolved, inspection({ ...original, fields: {}, issue: "invalid_config", revision: "broken" }))]) {
    const drift = describeDrift(unavailable);
    assert.equal(drift.kind, "unavailable");
    assert.equal(drift.readable, false);
    assert.equal(drift.canMerge, false);
    assert.equal(unavailable.draft.model, "my-draft");
    assert.equal(mergeDraft(unavailable, ["model"]), unavailable);
  }
  const repaired = { ...original, revision: "repaired", fields: { ...original.fields, model_verbosity: "low" } };
  const pending = observeConfig(unresolved, inspection(repaired));
  assert.equal(describeDrift(pending).canMerge, true);
  assert.deepEqual(mergeDraft(pending, []).draft, { ...repaired.fields, model: "my-draft" });
});
