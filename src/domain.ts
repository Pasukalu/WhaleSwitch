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
export type ToolInfo = { tool: Tool; installed: boolean; executable: string | null; path: string | null; exists: boolean; issue: string | null };
export type Change = { field: Field; before: string | null; after: string | null };
export type Snapshot = { id: number; timestamp: number; reason: "manual" | "before_save" | "before_restore" };
export type Profile = { id: number; name: string; timestamp: number; fields: Fields };
export type ProfilePreview = { name: string; changes: Change[] };
export type ProbeInput = { tool: Tool; protocol: Protocol; endpoint: string; anonymous?: boolean };
export type ProbeResult = {
  elapsed_ms: number; status: number | null; models: number | null;
  outcome: "connected" | "timeout" | "network_error" | "redirect" | "rejected" | "unsupported" | "rate_limited" | "http_error" | "unexpected_response";
};
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

export function fieldPatch(current: Fields, draft: Fields): Fields {
  const patch: Fields = {};
  for (const key of FIELD_ORDER) {
    if (!(key in draft)) continue;
    const next = draft[key]?.trim() || null;
    if (next !== (current[key] ?? null)) patch[key] = next;
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
