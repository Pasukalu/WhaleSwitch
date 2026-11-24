import assert from "node:assert/strict";
import { test } from "node:test";
import { probeReport } from "../src/domain.ts";

test("copies only the approved fields from a diagnostics result", () => {
  const input = {
    tool: "claude",
    protocol: "anthropic",
    endpoint: "https://private.example/internal",
    apiKey: "secret-fixture",
  };
  const report = {
    stage: "http",
    outcome: "rate_limited",
    status: 429,
    elapsed_ms: 741,
    models: null,
    retry_after_seconds: 30,
    raw_response: "private server response",
    headers: { authorization: "Bearer secret-fixture" },
    file_path: "C:/private/settings.json",
  };
  assert.deepEqual(JSON.parse(probeReport(input, report)), {
    tool: "claude", protocol: "anthropic", auth_mode: "api_key", stage: "http",
    outcome: "rate_limited", http_status: 429, elapsed_ms: 741, models_returned: null, retry_after_seconds: 30,
  });
  assert.equal(JSON.parse(probeReport({ ...input, anonymous: true }, report)).auth_mode, "anonymous");
});
