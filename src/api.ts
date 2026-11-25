import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { demo } from "./demo";
import type { Change, ConfigInspection, ConfigPlan, Fields, PlanArchive, PlanPreview, PlanProfile, PlanRun, PlanSelection, ProbeInput, ProbeResult, Profile, ProfileArchive, ProfilePreview, ProjectConfigReport, ProjectSkillsReport, RuntimeInfo, SkillBackup, SkillEditPreview, SkillImportPreview, SkillLocation, Snapshot, Target, Tool, ToolInfo, View } from "./domain";

export const previewMode = !isTauri();
export async function onNativeWindowFocus(onFocus: () => void): Promise<() => void> {
  return getCurrentWindow().onFocusChanged(({ payload }) => { if (payload) onFocus(); });
}

function request<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return previewMode ? demo<T>(command, args ?? {}) : invoke<T>(command, args);
}

export const api = {
  discover: () => request<ToolInfo[]>("discover"),
  runtimeInfo: () => request<RuntimeInfo>("runtime_info"),
  pickProjectSkills: () => request<ProjectSkillsReport>("pick_project_skills"),
  refreshProjectSkills: (root: string) => request<ProjectSkillsReport>("refresh_project_skills", { root }),
  captureSkillBaseline: (root: string, revision: string) => request<ProjectSkillsReport>("capture_skill_baseline", { root, revision }),
  pickSkillImport: (root: string, location: SkillLocation) => request<SkillImportPreview>("pick_skill_import", { root, location }),
  previewSkillImport: (root: string, location: SkillLocation) => request<SkillImportPreview>("preview_skill_import", { root, location }),
  applySkillImport: (root: string, location: SkillLocation, revision: string) => request<void>("apply_skill_import", { root, location, revision }),
  previewSkillUpgrade: (root: string, location: SkillLocation) => request<SkillEditPreview>("preview_skill_upgrade", { root, location }),
  applySkillUpgrade: (root: string, location: SkillLocation, revision: string, selected: string[]) =>
    request<SkillBackup>("apply_skill_upgrade", { root, location, revision, selected }),
  skillBackups: (root: string) => request<SkillBackup[]>("skill_backups", { root }),
  previewSkillRestore: (root: string, id: number) => request<SkillEditPreview>("preview_skill_restore", { root, id }),
  applySkillRestore: (root: string, id: number, revision: string, selected: string[]) =>
    request<SkillBackup>("apply_skill_restore", { root, id, revision, selected }),
  pickProjectConfig: () => request<ProjectConfigReport>("pick_project_config"),
  inspectConfig: (tool: Tool) => request<ConfigInspection>("inspect_config", { tool }),
  view: (tool: Tool) => request<View>("view", { tool }),
  preview: (target: Target, patch: Fields) => request<Change[]>("preview", { target, patch }),
  save: (target: Target, patch: Fields) => request<View>("save", { target, patch }),
  snapshots: (tool: Tool, path: string) => request<Snapshot[]>("snapshots", { tool, path }),
  snapshot: (target: Target) => request<Snapshot>("snapshot", { target }),
  profiles: (tool: Tool) => request<Profile[]>("profiles", { tool }),
  createProfile: (target: Target, name: string, patch: Fields) => request<Profile>("create_profile", { target, name, patch }),
  previewProfile: (target: Target, id: number) => request<ProfilePreview>("preview_profile", { target, id }),
  applyProfile: (target: Target, id: number, selection: Fields) => request<View>("apply_profile", { target, id, selection }),
  deleteProfile: (tool: Tool, id: number) => request<void>("delete_profile", { tool, id }),
  renameProfile: (tool: Tool, id: number, name: string) => request<Profile>("rename_profile", { tool, id, name }),
  duplicateProfile: (tool: Tool, id: number, name: string) => request<Profile>("duplicate_profile", { tool, id, name }),
  readProfileArchive: (tool: Tool) => request<ProfileArchive>("read_profile_archive", { tool }),
  importProfile: (tool: Tool, archive: ProfileArchive, name: string) => request<Profile>("import_profile", { tool, archive, name }),
  exportProfileArchive: (tool: Tool, id: number) => request<void>("export_profile_archive", { tool, id }),
  plans: () => request<ConfigPlan[]>("plans"),
  createPlan: (name: string, profiles: PlanProfile[]) => request<ConfigPlan>("create_plan", { name, profiles }),
  deletePlan: (id: number) => request<void>("delete_plan", { id }),
  readPlanArchive: () => request<PlanArchive>("read_plan_archive"),
  importPlan: (archive: PlanArchive, name: string) => request<ConfigPlan>("import_plan", { archive, name }),
  exportPlan: (id: number) => request<void>("export_plan", { id }),
  previewPlan: (id: number) => request<PlanPreview[]>("preview_plan", { id }),
  applyPlan: (id: number, selections: PlanSelection[]) => request<PlanRun>("apply_plan", { id, selections }),
  planRuns: () => request<PlanRun[]>("plan_runs"),
  compareSnapshot: (target: Target, id: number) => request<Change[]>("compare_snapshot", { target, id }),
  compareSnapshotFields: (target: Target, id: number) => request<Change[]>("compare_snapshot_fields", { target, id }),
  restoreSnapshotFields: (target: Target, id: number, selection: Fields) => request<View>("restore_snapshot_fields", { target, id, selection }),
  restore: (target: Target, id: number) => request<View>("restore", { target, id }),
  importArchive: (tool: Tool) => request<Fields>("import_archive", { tool }),
  exportArchive: (target: Target) => request<void>("export_archive", { target }),
  hasKey: (input: ProbeInput) => request<boolean>("has_key", { input }),
  setKey: (input: ProbeInput, secret: string) => request<void>("set_key", { input, secret }),
  removeKey: (input: ProbeInput) => request<void>("remove_key", { input }),
  probe: (input: ProbeInput) => request<ProbeResult>("probe", { input }),
};
