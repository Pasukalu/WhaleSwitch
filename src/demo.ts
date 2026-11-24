import { FIELD_ORDER, TOOLS } from "./domain.ts";
import type { Change, ConfigInspection, ConfigPlan, Fields, PlanFileResult, PlanPreview, PlanProfile, PlanRun, PlanSelection, Profile, Snapshot, Target, Tool, View } from "./domain.ts";

const views: Record<Tool, View> = {
  codex: { tool: "codex", path: "~/.codex/config.toml", format: "toml", exists: true, issue: null, revision: "4e71bb19", fields: {
    model: "gpt-5.2-codex", model_provider: "openai", openai_base_url: null, model_reasoning_effort: "medium",
    model_reasoning_summary: null, model_verbosity: null,
  } },
  claude: { tool: "claude", path: "~/.claude/settings.json", format: "json", exists: true, issue: null, revision: "d420361e", fields: { model: "sonnet", anthropic_base_url: null } },
  opencode: { tool: "opencode", path: "~/.config/opencode/opencode.json", format: "json", exists: false, issue: null, revision: "missing", fields: { model: null, default_agent: null } },
};
type DemoSnapshot = Snapshot & { view: View };
const history: Record<Tool, DemoSnapshot[]> = { codex: [], claude: [], opencode: [] };
const profiles: Record<Tool, Profile[]> = { codex: [], claude: [], opencode: [] };
let plans: ConfigPlan[] = [];
const planRuns: PlanRun[] = [];
let version = 1000;

function changes(current: Fields, next: Fields): Change[] {
  return FIELD_ORDER.filter((field) => field in next && next[field] !== current[field]).map((field) => ({
    field, before: current[field] ?? null, after: next[field] ?? null,
  }));
}

function add(view: View, reason: Snapshot["reason"]): Snapshot {
  const entry = { id: ++version, timestamp: Date.now(), reason, view: structuredClone(view) };
  history[view.tool].unshift(entry);
  return entry;
}

function save(view: View, patch: Fields, reason: Snapshot["reason"] = "before_save"): View {
  if (!changes(view.fields, patch).length) return view;
  if (view.exists) add(view, reason);
  const saved = { ...view, exists: true, fields: { ...view.fields, ...patch }, revision: (version++).toString(16) };
  views[view.tool] = saved;
  return saved;
}

function draftName(input: unknown, error: string) {
  const name = typeof input === "string" ? input.trim() : "";
  if (!name || [...name].length > 64 || /[\p{Cc}]|sk[-_]|ghp_|github_pat_|xoxb-|sess-|eyjhb/iu.test(name)) throw error;
  return name;
}

function asciiLower(value: string) {
  return value.replace(/[A-Z]/g, (character) => character.toLowerCase());
}

function profileName(tool: Tool, input: unknown, excluding?: number) {
  const name = draftName(input, "invalid_profile_name");
  if (profiles[tool].some((item) => item.id !== excluding && asciiLower(item.name) === asciiLower(name))) throw "profile_name_exists";
  return name;
}

export async function demo<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const target = args.target as Target | undefined;
  const tool = (target?.tool ?? args.tool ?? "codex") as Tool;
  const view = views[tool];
  if (target && (target.revision !== view.revision || target.path !== view.path)) throw "conflict";
  const patch = args.patch as Fields | undefined;
  const selected = history[tool].find((snapshot) => snapshot.id === args.id);
  const profile = profiles[tool].find((entry) => entry.id === args.id);
  const plan = plans.find((entry) => entry.id === args.id);
  let result: unknown;
  switch (command) {
    case "discover":
      result = TOOLS.map((id) => ({ tool: id, installed: id !== "opencode", executable: null, path: views[id].path, exists: views[id].exists, issue: null }));
      break;
    case "view": result = view; break;
    case "inspect_config":
      result = { config: { kind: "view", view }, active_env: [] } satisfies ConfigInspection;
      break;
    case "snapshots":
      if (args.path !== undefined && args.path !== view.path) throw "conflict";
      result = history[tool];
      break;
    case "preview": result = changes(view.fields, patch ?? {}); break;
    case "save": result = save(view, patch ?? {}); break;
    case "profiles": result = profiles[tool]; break;
    case "create_profile":
    case "duplicate_profile": {
      if (command === "duplicate_profile" && !profile) throw "profile_missing";
      const name = profileName(tool, args.name);
      const fields = command === "duplicate_profile" && profile ? { ...profile.fields } : { ...view.fields, ...patch };
      const created = { id: ++version, name, timestamp: Date.now(), fields };
      profiles[tool].unshift(created);
      result = created;
      break;
    }
    case "rename_profile":
      if (!profile) throw "profile_missing";
      profile.name = profileName(tool, args.name, profile.id);
      result = profile;
      break;
    case "preview_profile":
      if (!profile) throw "profile_missing";
      result = { name: profile.name, changes: changes(view.fields, profile.fields) };
      break;
    case "apply_profile": {
      if (!profile) throw "profile_missing";
      const selection = args.selection as Fields | undefined;
      if (selection) {
        const fields = Object.keys(selection) as (keyof Fields)[];
        if (!fields.length) throw "invalid_field";
        if (fields.some((field) => !(field in profile.fields) || profile.fields[field] !== selection[field])) throw "conflict";
      }
      result = save(view, selection ?? profile.fields);
      break;
    }
    case "delete_profile":
      if (!profile) throw "profile_missing";
      profiles[tool] = profiles[tool].filter(({ id }) => id !== profile.id);
      break;
    case "plans": result = plans; break;
    case "plan_runs": result = planRuns; break;
    case "create_plan": {
      const name = draftName(args.name, "invalid_plan");
      const sources = args.profiles as PlanProfile[] | undefined;
      if (!sources?.length || sources.length !== new Set(sources.map((source) => source.tool)).size) throw "invalid_plan";
      if (plans.some((item) => asciiLower(item.name) === asciiLower(name))) throw "plan_name_exists";
      const entries = sources.map((source) => {
        const chosen = profiles[source.tool]?.find((entry) => entry.id === source.profile_id);
        if (!chosen) throw "profile_missing";
        return { tool: source.tool, fields: { ...chosen.fields } };
      });
      const created = { id: ++version, name, timestamp: Date.now(), entries };
      plans.unshift(created);
      result = created;
      break;
    }
    case "delete_plan":
      if (!plan) throw "plan_missing";
      plans = plans.filter(({ id }) => id !== plan.id);
      break;
    case "preview_plan":
      if (!plan) throw "plan_missing";
      result = plan.entries.map((entry): PlanPreview => {
        const live = views[entry.tool];
        return { tool: live.tool, path: live.path, revision: live.revision, changes: changes(live.fields, entry.fields), issue: null };
      });
      break;
    case "apply_plan": {
      if (!plan) throw "plan_missing";
      const selections = args.selections as PlanSelection[] | undefined;
      if (!selections?.length || selections.length !== new Set(selections.map((selection) => selection.tool)).size) throw "invalid_plan";
      for (const selection of selections) {
        const live = views[selection.tool];
        const entry = plan.entries.find((candidate) => candidate.tool === selection.tool);
        if (!entry || !Object.keys(selection.fields).length) throw "invalid_plan";
        const pending = changes(live.fields, entry.fields);
        if (live.path !== selection.path || live.revision !== selection.revision ||
          Object.entries(selection.fields).some(([field, value]) => !pending.some((change) => change.field === field && change.after === value))) {
          throw "conflict";
        }
      }
      const files = selections.map((selection): PlanFileResult => {
        const live = views[selection.tool];
        save(live, selection.fields, "before_plan");
        return { tool: live.tool, status: "applied", snapshot_id: live.exists ? history[live.tool][0]?.id ?? null : null, issue: null };
      });
      const run: PlanRun = { id: ++version, name: plan.name, timestamp: Date.now(), finished: true, files };
      planRuns.unshift(run);
      result = run;
      break;
    }
    case "snapshot":
      if (!view.exists) throw "snapshot_missing";
      result = add(view, "manual");
      break;
    case "compare_snapshot":
    case "compare_snapshot_fields":
      if (!selected) throw "snapshot_missing";
      result = changes(view.fields, selected.view.fields);
      break;
    case "restore_snapshot_fields": {
      if (!selected) throw "snapshot_missing";
      const selection = args.selection as Fields | undefined;
      const pending = changes(view.fields, selected.view.fields);
      if (!selection || !Object.keys(selection).length) throw "invalid_field";
      if (Object.entries(selection).some(([field, value]) => !pending.some((change) => change.field === field && change.after === value))) {
        throw "conflict";
      }
      result = save(view, selection, "before_restore_fields");
      break;
    }
    case "restore":
      if (!selected) throw "snapshot_missing";
      if (view.exists) add(view, "before_restore");
      views[tool] = { ...selected.view, revision: (version++).toString(16) };
      result = views[tool];
      break;
    case "has_key": result = false; break;
    default: throw "preview_only";
  }
  return structuredClone(result) as T;
}
