import assert from "node:assert/strict";
import test from "node:test";
import { demo } from "../src/demo.ts";
import { targetOf } from "../src/domain.ts";

test("preview pins a snapshot, rejects stale cleanup, and records successful cleanup without configuration values", async () => {
  const view = await demo("view", { tool: "codex" });
  const scope = { tool: view.tool, path: view.path };
  const target = targetOf(view);
  const snapshots = [];
  for (let index = 0; index < 7; index += 1) snapshots.push(await demo("snapshot", { target }));
  const [oldest, older] = snapshots;
  const prior = await demo("preview_snapshot_cleanup", scope);
  assert.deepEqual(prior.candidates.map(({ id }) => id), [oldest.id, older.id]);
  await demo("pin_snapshot", { ...scope, id: oldest.id, pinned: true });
  await assert.rejects(
    () => demo("apply_snapshot_cleanup", { ...scope, revision: prior.revision, selected: [older.id] }),
    (error) => error === "conflict",
  );
  const review = await demo("preview_snapshot_cleanup", scope);
  assert.deepEqual(review.candidates.map(({ id }) => id), [older.id]);
  assert.equal(review.protected, 6);
  await assert.rejects(
    () => demo("apply_snapshot_cleanup", { ...scope, revision: review.revision, selected: [oldest.id] }),
    (error) => error === "snapshot_cleanup_selection",
  );
  const outcome = await demo("apply_snapshot_cleanup", { ...scope, revision: review.revision, selected: [older.id] });
  assert.equal(outcome.removed, 1);
  assert.equal((await demo("snapshots", scope)).find(({ id }) => id === oldest.id).pinned, true);
  assert.deepEqual(await demo("view", { tool: view.tool }), view);
  const [cleanup, capture] = await demo("config_activity", { tool: view.tool });
  assert.deepEqual(Object.keys(cleanup).sort(), ["action", "snapshot_id", "status", "timestamp", "tool"]);
  assert.equal(cleanup.action, "cleanup");
  assert.equal(capture.action, "snapshot");
});
