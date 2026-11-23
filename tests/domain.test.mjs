import assert from "node:assert/strict";
import test from "node:test";
import { fieldPatch, initialProbe, targetOf } from "../src/domain.ts";

const codex = {
  tool: "codex", path: "/user/.codex/config.toml", revision: "v1", format: "toml", exists: true,
  fields: { model: "gpt-5.2-codex", model_provider: "openai", openai_base_url: null }, issue: null,
};

test("sends only real field changes, including explicit removals", () => {
  assert.deepEqual(fieldPatch(codex.fields, { model: " gpt-5.2-codex ", openai_base_url: "  ", model_provider: "" }), { model_provider: null });
  assert.deepEqual(fieldPatch(codex.fields, { model: "new-model" }), { model: "new-model" });
});

test("save target only contains the binding fields", () => {
  assert.deepEqual(targetOf(codex), { tool: "codex", revision: "v1", path: "/user/.codex/config.toml" });
});

test("configured Codex OpenAI endpoint is suggested for manual probes", () => {
  const view = { ...codex, fields: { ...codex.fields, openai_base_url: "http://127.0.0.1:8888/v1" } };
  assert.deepEqual(initialProbe(view, "codex"), { tool: "codex", protocol: "openai", endpoint: "http://127.0.0.1:8888/v1" });
  assert.equal(initialProbe({ ...view, fields: { model_provider: "custom" } }, "codex").endpoint, "");
});

test("OpenCode and Claude use a protocol consistent with the selected tool", () => {
  const view = { ...codex, tool: "opencode", fields: { model: "anthropic/claude-sonnet" } };
  assert.equal(initialProbe(view, "opencode").protocol, "anthropic");
  assert.equal(initialProbe({ ...view, fields: { model: "other/provider-model" } }, "opencode").endpoint, "");
  assert.equal(initialProbe(null, "claude").endpoint, "https://api.anthropic.com");
});
