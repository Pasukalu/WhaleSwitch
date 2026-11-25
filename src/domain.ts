export const TOOLS = ["codex", "claude", "opencode"] as const;
export type Tool = typeof TOOLS[number];
export const TOOL_NAMES: Record<Tool, string> = {
  codex: "Codex CLI", claude: "Claude Code", opencode: "OpenCode",
};
export type Field = "model" | "model_provider" | "openai_base_url" | "model_reasoning_effort"
  | "model_reasoning_summary" | "model_verbosity" | "anthropic_base_url" | "default_agent";
export type Fields = Partial<Record<Field, string | null>>;
export type Protocol = "openai" | "anthropic";
export type Format = "toml" | "json" | "jsonc";
export type Target = { tool: Tool; path: string; revision: string };
export type View = Target & { format: Format; exists: boolean; fields: Fields; issue: string | null };
export const CONFIG_ENV = {
  codex: ["CODEX_HOME"],
  claude: ["CLAUDE_CONFIG_DIR"],
  opencode: ["XDG_CONFIG_HOME", "OPENCODE_CONFIG", "OPENCODE_CONFIG_DIR", "OPENCODE_CONFIG_CONTENT"],
} as const;
type ConfigEnv = (typeof CONFIG_ENV)[Tool][number];
export type ConfigObservation = { kind: "view"; view: View }
  | { kind: "unavailable"; path: string | null; issue: string };
export type ConfigInspection = { config: ConfigObservation; active_env: ConfigEnv[] | null };
export type ProjectConfigSource = {
  tool: Tool; scope: "project" | "project_local"; path: string;
  exists: boolean; issue: string | null; fields: Field[];
};
export type ProjectConfigReport = { root: string; sources: ProjectConfigSource[] };
export type ToolInfo = { tool: Tool; installed: boolean; executable: string | null; path: string | null; exists: boolean; issue: string | null };
export type Change = { field: Field; before: string | null; after: string | null };
export type Snapshot = {
  id: number; timestamp: number; reason: "manual" | "before_save" | "before_restore" | "before_restore_fields" | "before_plan";
  pinned: boolean; stored_bytes: number;
};
export type SnapshotCleanupPreview = {
  revision: string; keep_recent: number; protected: number; eligible: number; candidates: Snapshot[];
};
export type SnapshotCleanupResult = { removed: number; stored_bytes: number };
export type ActivityAction = "save" | "snapshot" | "restore_file" | "restore_fields" | "cleanup" | "plan";
export type ActivityStatus = "unconfirmed" | "succeeded" | "failed" | "rolled_back" | "recovery_required" | "unchanged" | "skipped";
export type ConfigActivity = { timestamp: number; tool: Tool; action: ActivityAction; status: ActivityStatus; snapshot_id: number | null };
export type RuntimeInfo = { portable: boolean; data_dir: string };
export type Profile = { id: number; name: string; timestamp: number; fields: Fields };
export type ProfileArchive = { version: 1; tool: Tool; name: string; fields: Fields };
export type ProfilePreview = { name: string; changes: Change[] };
export type PlanEntry = { tool: Tool; fields: Fields };
export type ConfigPlan = { id: number; name: string; timestamp: number; entries: PlanEntry[] };
export type PlanArchive = { version: 1; name: string; entries: PlanEntry[] };
export type PlanProfile = { tool: Tool; profile_id: number };
export type PlanPreview = { tool: Tool; path: string | null; revision: string | null; changes: Change[]; issue: string | null };
export type PlanSelection = Target & { fields: Fields };
export type PlanFileStatus = "pending" | "applied" | "restored" | "unchanged" | "failed" | "skipped" | "recovery_required";
export type PlanFileResult = { tool: Tool; snapshot_id: number | null; status: PlanFileStatus; issue: string | null };
export type PlanRun = { id: number; name: string; timestamp: number; finished: boolean; files: PlanFileResult[] };
export type SkillEntry = { name: string; path: string; tools: Tool[]; fingerprint: string; same_name: boolean };
export type SkillInventory = { root: string; entries: SkillEntry[]; skipped: number; limited: boolean };
export type SkillLocation = "agents" | "claude" | "opencode";
export type SkillChangeKind = "added" | "modified" | "removed";
export type SkillChange = { path: string; kind: SkillChangeKind; before: string | null; after: string | null };
export type ProjectSkillsReport = { inventory: SkillInventory; revision: string; baseline_at: number | null; changes: SkillChange[] };
export type SkillImportFile = { path: string; fingerprint: string; bytes: number; text: string };
export type SkillImportPreview = {
  source: string; destination: string; name: string; revision: string; existing: boolean; skipped: number;
  excluded: string[]; files: SkillImportFile[];
};
export type SkillTextChange = { path: string; before: string | null; after: string | null; blocked: boolean };
export type SkillEditPreview = { destination: string; revision: string; changes: SkillTextChange[] };
export type SkillBackup = {
  id: number; destination: string; timestamp: number; action: "upgrade" | "restore"; file_count: number;
};
export type ProbeInput = { tool: Tool; protocol: Protocol; endpoint: string; anonymous?: boolean };
export type ProbeStage = "dns" | "connect" | "tls" | "request" | "http" | "body" | "models";
export type ProbeOutcome = "connected" | "timeout" | "dns_error" | "tls_error" | "connection_refused"
  | "network_error" | "redirect" | "rejected" | "forbidden" | "unsupported" | "rate_limited"
  | "server_error" | "http_error" | "body_error" | "response_too_large" | "invalid_json" | "unexpected_response";
export type ProbeResult = {
  elapsed_ms: number; status: number | null; models: number | null; retry_after_seconds: number | null;
  stage: ProbeStage; outcome: ProbeOutcome;
};

export function probeReport(input: Pick<ProbeInput, "tool" | "protocol" | "anonymous">, result: ProbeResult) {
  return JSON.stringify({
    tool: input.tool,
    protocol: input.protocol,
    auth_mode: input.anonymous ? "anonymous" : "api_key",
    stage: result.stage,
    outcome: result.outcome,
    http_status: result.status,
    elapsed_ms: result.elapsed_ms,
    models_returned: result.models,
    retry_after_seconds: result.retry_after_seconds,
  }, null, 2);
}
export const FIELD_ORDER: Field[] = ["model", "model_provider", "openai_base_url", "anthropic_base_url",
  "model_reasoning_effort", "model_reasoning_summary", "model_verbosity", "default_agent"];
export const FIELD_OPTIONS: Partial<Record<Field, string[]>> = {
  model_reasoning_effort: ["none", "minimal", "low", "medium", "high", "xhigh", "max", "ultra"],
  model_reasoning_summary: ["auto", "concise", "detailed", "none"],
  model_verbosity: ["low", "medium", "high"],
};

export function fieldPath(field: Field): string {
  return field === "anthropic_base_url" ? "env.ANTHROPIC_BASE_URL" : field;
}
export const DEFAULT_ENDPOINT: Record<Protocol, string> = {
  openai: "https://api.openai.com/v1", anthropic: "https://api.anthropic.com",
};

export function targetOf(view: View): Target {
  return { tool: view.tool, path: view.path, revision: view.revision };
}

export function sameTarget(left: Target, right: Target): boolean {
  return left.tool === right.tool && left.path === right.path && left.revision === right.revision;
}

export function fieldPatch(current: Fields, draft: Fields): Fields {
  const patch: Fields = {};
  for (const key of FIELD_ORDER) {
    if (!(key in draft)) continue;
    const next = draft[key]?.trim() || null;
    if (next !== (current[key] ?? null)) patch[key] = next;
  }
  return patch;
}

export function defaultChangeSelection(changes: readonly Change[]): Field[] {
  return changes.filter(({ after }) => after !== null).map(({ field }) => field);
}

export function selectedChangePatch(changes: readonly Change[], selected: readonly Field[]): Fields {
  const patch: Fields = {};
  const fields = new Set(selected);
  for (const { field, after } of changes) {
    if (fields.has(field)) patch[field] = after;
  }
  return patch;
}

export function initialProbe(view: View | null, tool: Tool): ProbeInput {
  const model = view?.fields.model;
  const protocol = tool === "claude" || (tool === "opencode" && model?.startsWith("anthropic/"))
    ? "anthropic" : "openai";
  const custom = tool === "claude" ? view?.fields.anthropic_base_url
    : tool === "codex" && (!view?.fields.model_provider || view.fields.model_provider === "openai")
      ? view?.fields.openai_base_url : null;
  const customProvider = tool === "codex" && view?.fields.model_provider && view.fields.model_provider !== "openai";
  const unknownOpenCode = tool === "opencode" && model && !/^(openai|anthropic)\//.test(model);
  return { tool, protocol, endpoint: custom || (customProvider || unknownOpenCode ? "" : DEFAULT_ENDPOINT[protocol]) };
}
