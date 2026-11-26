import assert from "node:assert/strict";
import test from "node:test";
import { pageForSnapshot, snapshotPage } from "../src/snapshot-history.ts";

const history = Array.from({ length: 43 }, (_, offset) => {
  const id = 43 - offset;
  return { id, pinned: id === 42 || id === 2, reason: "manual", stored_bytes: 40, timestamp: id };
});

test("opens an older snapshot on its unfiltered page and clamps pages after cleanup", () => {
  const page = pageForSnapshot(history, 2);
  assert.equal(page, 2);
  assert.deepEqual(snapshotPage(history, "", false, page).items.map(({ id }) => id), [3, 2, 1]);
  assert.equal(pageForSnapshot(history, 44), null);

  const afterCleanup = snapshotPage(history.slice(0, 19), "", false, page);
  assert.equal(afterCleanup.index, 0);
  assert.equal(afterCleanup.pageCount, 1);
  assert.equal(afterCleanup.items.length, 19);
});

test("searches exact snapshot numbers and intersects the pinned filter", () => {
  assert.deepEqual(snapshotPage(history, "", true, 0).items.map(({ id }) => id), [42, 2]);
  assert.deepEqual(snapshotPage(history, " #2 ", true, 2).items.map(({ id }) => id), [2]);
  assert.deepEqual(snapshotPage(history, "43", true, 0).items, []);
  assert.equal(snapshotPage(history, "#", false, 0).count, 0);
  assert.equal(snapshotPage(history, " ", false, 0).count, 43);
});
