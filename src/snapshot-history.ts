import type { Snapshot } from "./domain.ts";

const PAGE_SIZE = 20;

export function snapshotPage(snapshots: readonly Snapshot[], query: string, pinnedOnly: boolean, requestedPage: number) {
  const input = query.trim();
  const id = input.replace(/^#/, "");
  const matches = snapshots.filter((snapshot) =>
    (!pinnedOnly || snapshot.pinned) && (!input || String(snapshot.id) === id));
  const pageCount = Math.max(1, Math.ceil(matches.length / PAGE_SIZE));
  const index = Math.min(Math.max(0, requestedPage), pageCount - 1);
  const offset = index * PAGE_SIZE;
  return { items: matches.slice(offset, offset + PAGE_SIZE), index, pageCount, count: matches.length };
}

export function pageForSnapshot(snapshots: readonly Snapshot[], id: number): number | null {
  const index = snapshots.findIndex((snapshot) => snapshot.id === id);
  return index < 0 ? null : Math.floor(index / PAGE_SIZE);
}
