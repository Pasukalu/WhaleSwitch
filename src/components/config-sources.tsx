import { FolderOpen, FolderTree } from "lucide-react";
import { observedIssue, observedPath, observedView } from "../config-session";
import { CONFIG_ENV, fieldPath, type ConfigInspection, type ProjectConfigReport, type Tool } from "../domain";
import { errorText, type Copy, type Language } from "../i18n";
import { Button } from "./ui/button";

const FLAGS: Record<Tool, readonly string[]> = {
  codex: ["--config", "--profile"],
  claude: ["--settings", "--setting-sources"],
  opencode: [],
};

function FileStatus({ exists, issue, t }: { exists: boolean; issue: string | null; t: Copy }) {
  return <span className={issue ? "format-tag source-error" : "format-tag"}>
    {issue ? t.unavailable : exists ? t.sourceFound : t.sourceMissing}
  </span>;
}

export function ConfigSources({ t, language, tool, inspection, report, native, busy, onPick }: {
  t: Copy; language: Language; tool: Tool; inspection: ConfigInspection | null;
  report: ProjectConfigReport | null; native: boolean; busy: boolean; onPick: () => void;
}) {
  const config = inspection?.config;
  const issue = observedIssue(config);
  const files = report?.sources.filter((source) => source.tool === tool) ?? [];
  return <section className="panel">
    <div className="panel-head"><div className="section-icon"><FolderTree size={19}/></div>
      <div><h2>{t.sources}</h2><p>{t.sourceIntro}</p></div>
    </div>
    <div className="panel-body form-stack">
      <div className="source-heading">
        <h3>{t.sourceTarget}</h3>
        {inspection && <FileStatus t={t} exists={observedView(config)?.exists ?? false} issue={issue}/>}
      </div>
      <div className="path-row source-path"><code>{observedPath(config) ?? (inspection ? t.sourceNoTarget : t.loading)}</code></div>
      {issue && <p className="small source-error">{errorText(issue, language)}</p>}
      <div className="source-detail">
        <h3>{t.sourceEnvironment}</h3>
        <ul className="source-tags">{CONFIG_ENV[tool].map((name) => <li className="format-tag" key={name}>
          <code>{name}</code> <span>{inspection?.active_env == null ? "—" : inspection.active_env.includes(name) ? t.sourceSet : t.sourceUnset}</span>
        </li>)}</ul>
        <p className="small muted">{t.sourceEnvHint}</p>
        {tool === "opencode" && <p className="small muted">{t.sourceOpencodeHint}</p>}
      </div>
      {FLAGS[tool].length > 0 && <div className="source-detail">
        <h3>{t.sourceFlags}</h3>
        <ul className="source-tags">{FLAGS[tool].map((flag) => <li className="format-tag" key={flag}><code>{flag}</code></li>)}</ul>
        <p className="small muted">{t.sourceFlagsHint}</p>
      </div>}
    </div>
    <div className="panel-head spread sources-project-head">
      <div className="head-label"><div className="section-icon"><FolderOpen size={18}/></div>
        <div><h2>{t.sourceProject}</h2><p>{t.sourceReadOnly}</p></div>
      </div>
      {native && <Button size="sm" variant="outline" disabled={busy} onClick={onPick}><FolderOpen/>{t.sourcePick}</Button>}
    </div>
    <div className="panel-body form-stack">
      {!report ? <p className="small muted">{native ? t.sourceScanHint : t.skillsNative}</p> : <>
        <div className="path-row source-path"><span>{t.skillsFolder}</span><code>{report.root}</code></div>
        <ul className="source-files">
          {files.map((file) => <li className="source-file" key={file.path}>
            <div className="source-heading"><code>{file.path}</code><FileStatus t={t} exists={file.exists} issue={file.issue}/></div>
            {file.scope === "project_local" && <p className="small muted">{t.sourceLocalProject}</p>}
            {file.issue ? <p className="small source-error">{errorText(file.issue, language)}</p>
              : file.fields.length > 0 ? <div className="source-detail" aria-label={t.sourceFields}>
                <ul className="source-tags">{file.fields.map((field) => <li className="format-tag" key={field}><code>{fieldPath(field)}</code></li>)}</ul>
              </div> : file.exists && <p className="small muted">{t.sourceNoFields}</p>}
          </li>)}
        </ul>
      </>}
      <p className="small muted">{t.sourceScope}</p>
      {tool === "codex" && <p className="small muted">{t.sourceCodexHint}</p>}
    </div>
  </section>;
}
