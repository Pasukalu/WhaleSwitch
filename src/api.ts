import { invoke, isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { demo } from "./demo";
import type { Change, ConfigInspection, Fields, ProbeInput, ProbeResult, Profile, ProfileArchive, ProfilePreview, ProjectConfigReport, SkillInventory, Snapshot, Target, Tool, ToolInfo, View } from "./domain";

export const previewMode = !isTauri();
export async function onNativeWindowFocus(onFocus: () => void): Promise<() => void> {
  return getCurrentWindow().onFocusChanged(({ payload }) => { if (payload) onFocus(); });
}

function request<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  return previewMode ? demo<T>(command, args ?? {}) : invoke<T>(command, args);
}

export const api = {
  discover: () => request<ToolInfo[]>("discover"),
  pickProjectSkills: () => request<SkillInventory>("pick_project_skills"),
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
  compareSnapshot: (target: Target, id: number) => request<Change[]>("compare_snapshot", { target, id }),
  restore: (target: Target, id: number) => request<View>("restore", { target, id }),
  importArchive: (tool: Tool) => request<Fields>("import_archive", { tool }),
  exportArchive: (target: Target) => request<void>("export_archive", { target }),
  hasKey: (input: ProbeInput) => request<boolean>("has_key", { input }),
  setKey: (input: ProbeInput, secret: string) => request<void>("set_key", { input, secret }),
  removeKey: (input: ProbeInput) => request<void>("remove_key", { input }),
  probe: (input: ProbeInput) => request<ProbeResult>("probe", { input }),
};
