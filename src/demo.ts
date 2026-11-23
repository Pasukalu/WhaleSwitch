import { FIELD_ORDER, TOOLS } from "./domain.ts";
import type { Change, Fields, Profile, Snapshot, Target, Tool, View } from "./domain.ts";

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

function save(view: View, patch: Fields): View {
  if (!changes(view.fields, patch).length) return view;
  if (view.exists) add(view, "before_save");
  const saved = { ...view, exists: true, fields: { ...view.fields, ...patch }, revision: (version++).toString(16) };
  views[view.tool] = saved;
  return saved;
}

export async function demo<T>(command: string, args: Record<string, unknown>): Promise<T> {
  const target = args.target as Target | undefined;
  const tool = (target?.tool ?? args.tool ?? "codex") as Tool;
  const view = views[tool];
  if (target && (target.revision !== view.revision || target.path !== view.path)) throw "conflict";
  const patch = args.patch as Fields | undefined;
  const selected = history[tool].find((snapshot) => snapshot.id === args.id);
  const profile = profiles[tool].find((entry) => entry.id === args.id);
  let result: unknown;
  switch (command) {
    case "discover":
      result = TOOLS.map((id) => ({ tool: id, installed: id !== "opencode", executable: null, path: views[id].path, exists: views[id].exists, issue: null }));
      break;
    case "view": result = view; break;
    case "snapshots": result = history[tool]; break;
    case "preview": result = changes(view.fields, patch ?? {}); break;
    case "save": result = save(view, patch ?? {}); break;
    case "profiles": result = profiles[tool]; break;
    case "create_profile": {
      const name = typeof args.name === "string" ? args.name.trim() : "";
      if (!name || name.length > 64 || /[\p{Cc}]|sk[-_]|ghp_|github_pat_|xoxb-|sess-|eyjhb/iu.test(name)) throw "invalid_profile_name";
      if (profiles[tool].some((item) => item.name.toLowerCase() === name.toLowerCase())) throw "profile_name_exists";
      const created = { id: ++version, name, timestamp: Date.now(), fields: { ...view.fields, ...patch } };
      profiles[tool].unshift(created);
      result = created;
      break;
    }
    case "preview_profile":
      if (!profile) throw "profile_missing";
      result = { name: profile.name, changes: changes(view.fields, profile.fields) };
      break;
    case "apply_profile":
      if (!profile) throw "profile_missing";
      result = save(view, profile.fields);
      break;
    case "delete_profile":
      if (!profile) throw "profile_missing";
      profiles[tool] = profiles[tool].filter(({ id }) => id !== profile.id);
      break;
    case "snapshot":
      if (!view.exists) throw "snapshot_missing";
      result = add(view, "manual");
      break;
    case "compare_snapshot":
      if (!selected) throw "snapshot_missing";
      result = changes(view.fields, selected.view.fields);
      break;
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
