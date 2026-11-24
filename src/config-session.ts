import { FIELD_ORDER, fieldPatch, sameTarget, type ConfigInspection, type ConfigObservation, type Field, type Fields, type View } from "./domain.ts";

export type ConfigChange = "modified" | "created" | "deleted" | "target_changed" | "unavailable" | "recovered";

export type ConfigSession = {
  baseline: ConfigObservation | null;
  latest: ConfigInspection | null;
  draft: Fields;
  autoChange: ConfigChange | null;
};

export const emptyConfigSession: ConfigSession = { baseline: null, latest: null, draft: {}, autoChange: null };

export function observedView(config: ConfigObservation | null | undefined): View | null {
  return config?.kind === "view" ? config.view : null;
}

export function observedPath(config: ConfigObservation | null | undefined): string | null {
  if (!config) return null;
  return config.kind === "view" ? config.view.path : config.path;
}

export function observedIssue(config: ConfigObservation | null | undefined): string | null {
  if (!config) return null;
  return config.kind === "view" ? config.view.issue : config.issue;
}

function changeBetween(before: ConfigObservation, after: ConfigObservation): ConfigChange | null {
  const previousPath = observedPath(before);
  const nextPath = observedPath(after);
  if (previousPath !== null && nextPath !== null && previousPath !== nextPath) return "target_changed";

  if (after.kind === "unavailable") {
    return before.kind === "unavailable" && before.issue === after.issue && previousPath === nextPath
      ? null : "unavailable";
  }
  if (before.kind === "view" && sameTarget(before.view, after.view)
    && before.view.exists === after.view.exists && before.view.issue === after.view.issue) return null;
  if (after.view.issue) return "unavailable";
  if (before.kind === "unavailable" || before.view.issue) return "recovered";
  if (before.view.exists !== after.view.exists) return after.view.exists ? "created" : "deleted";
  return "modified";
}

export function acceptConfig(latest: ConfigInspection, autoChange: ConfigChange | null = null): ConfigSession {
  return { baseline: latest.config, latest, draft: observedView(latest.config)?.fields ?? {}, autoChange };
}

export function observeConfig(session: ConfigSession, latest: ConfigInspection): ConfigSession {
  if (!session.baseline) return acceptConfig(latest);
  const change = changeBetween(session.baseline, latest.config);
  if (!change) return { ...session, latest };
  const baseView = observedView(session.baseline);
  const dirty = baseView && Object.keys(fieldPatch(baseView.fields, session.draft)).length > 0;
  return dirty ? { ...session, latest, autoChange: null } : acceptConfig(latest, change);
}

type DriftRow = {
  field: Field;
  before: string | null;
  current: string | null;
  draft: string | null;
  edited: boolean;
  conflict: boolean;
};

export type ConfigDrift = {
  kind: ConfigChange;
  before: View;
  latest: ConfigInspection;
  rows: DriftRow[];
  readable: boolean;
  canMerge: boolean;
};

export function describeDrift(session: ConfigSession): ConfigDrift | null {
  const { baseline, latest, draft } = session;
  if (!baseline || baseline.kind !== "view" || !latest) return null;
  const kind = changeBetween(baseline, latest.config);
  if (!kind) return null;

  const before = baseline.view;
  const latestView = observedView(latest.config);
  const current = latestView && !latestView.issue ? latestView : null;
  const canMerge = current !== null && current.path === before.path && current.tool === before.tool;
  const patch = fieldPatch(before.fields, draft);
  const rows = FIELD_ORDER.flatMap((field): DriftRow[] => {
    const previousValue = before.fields[field] ?? null;
    const fileValue = current?.fields[field] ?? null;
    const draftValue = patch[field];
    const edited = draftValue !== undefined;
    const changedOnDisk = current !== null && previousValue !== fileValue;
    if (!edited && !changedOnDisk) return [];
    return [{
      field, before: previousValue, current: fileValue, draft: draftValue ?? null, edited,
      conflict: canMerge && edited && changedOnDisk && draftValue !== fileValue,
    }];
  });
  return { kind, before, latest, rows, readable: current !== null, canMerge };
}

export function mergeDraft(session: ConfigSession, keepConflicts: readonly Field[]): ConfigSession {
  const drift = describeDrift(session);
  if (!drift?.canMerge) return session;
  const next = acceptConfig(drift.latest);
  const draft = { ...next.draft };
  for (const row of drift.rows) {
    if (row.edited && (!row.conflict || keepConflicts.includes(row.field))) draft[row.field] = row.draft;
  }
  return { ...next, draft };
}
