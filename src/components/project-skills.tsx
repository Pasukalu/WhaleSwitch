import { useState } from "react";
import { BookOpen, CircleAlert, FolderOpen, RefreshCw } from "lucide-react";
import { api, previewMode } from "../api";
import { TOOL_NAMES, type ProjectSkillsReport, type SkillEntry } from "../domain";
import { errorText, type Copy, type Language } from "../i18n";
import { SkillImportPanel } from "./project-skill-import";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

export function ProjectSkills({ t, language }: { t: Copy; language: Language }) {
  const [report, setReport] = useState<ProjectSkillsReport | null>(null);
  const [section, setSection] = useState<"inventory" | "changes">("inventory");
  const [projectSelection, setProjectSelection] = useState(0);
  const [query, setQuery] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);

  async function task(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true);
    setError(null);
    try {
      await action();
    } catch (nextError) {
      if (nextError !== "cancelled") setError(nextError);
    } finally {
      setBusy(false);
    }
  }

  const inventory = report?.inventory;
  const complete = Boolean(inventory && !inventory.skipped && !inventory.limited);
  const baselineAt = report?.baseline_at ?? null;
  const root = inventory?.root;
  const search = query.trim().toLowerCase();
  const entries = inventory?.entries.filter(({ path }) => path.toLowerCase().includes(search)) ?? [];
  const changes = report?.changes.filter(({ path }) => path.toLowerCase().includes(search)) ?? [];
  const format = new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en", { dateStyle: "medium", timeStyle: "short" });

  async function choose() {
    const data = await api.pickProjectSkills();
    setReport(data);
    setSection(data.baseline_at === null ? "inventory" : "changes");
    setQuery("");
    setProjectSelection((previous) => previous + 1);
  }

  async function refresh() {
    if (root) setReport(await api.refreshProjectSkills(root));
  }

  async function capture() {
    if (!report) return;
    const data = await api.captureSkillBaseline(report.inventory.root, report.revision);
    setReport(data);
    setSection("changes");
    setQuery("");
  }

  return <div className="form-stack">
    <section className="panel" aria-busy={busy}>
      <div className="panel-head spread skills-head">
        <div className="head-label">
          <div className="section-icon"><BookOpen size={19}/></div>
          <div><h2>{t.skills}</h2><p>{t.skillsIntro}</p></div>
        </div>
        {!previewMode && <div className="row-actions">
          <Button size="sm" variant="outline" disabled={busy} onClick={() => void task(choose)}><FolderOpen/>{t.skillsSelect}</Button>
          {root && <Button size="icon" variant="ghost" disabled={busy} onClick={() => void task(refresh)}
            aria-label={t.reload} title={t.reload}><RefreshCw/></Button>}
        </div>}
      </div>
      {error !== null && <div className="panel-body"><div className="inline-message error" role="alert">
        <CircleAlert size={17}/>{error === "conflict" ? t.skillsConflict : errorText(error, language)}
      </div></div>}
      {inventory ? <>
        <div className="panel-body skills-summary">
          <div className="path-row"><span>{t.skillsFolder}</span><code>{inventory.root}</code></div>
          <p className="small muted" role="status">{inventory.entries.length} {t.skillsCount}</p>
          {!complete && <div className="inline-message" role="status"><CircleAlert size={17}/>
            <div>{inventory.skipped > 0 && <p>{inventory.skipped} {t.skillsSkipped}</p>}{inventory.limited && <p>{t.skillsLimited}</p>}</div>
          </div>}
          <div className="plan-line">
            <span className="small muted">{baselineAt === null ? t.skillsBaselineStart : `${t.skillsBaselineTime} · ${format.format(baselineAt)}`}</span>
            <Button variant="outline" size="sm" disabled={busy || !complete} onClick={() => void task(capture)}>
              {baselineAt === null ? t.skillsBaselineSave : t.skillsBaselineUpdate}
            </Button>
          </div>
          <div className="skills-toolbar">
            <div className="row-actions">
              <Button variant={section === "inventory" ? "default" : "ghost"} size="sm" aria-pressed={section === "inventory"}
                onClick={() => setSection("inventory")}>{t.skillsInventory}</Button>
              {baselineAt !== null && <Button variant={section === "changes" ? "default" : "ghost"} size="sm"
                aria-pressed={section === "changes"} onClick={() => setSection("changes")}>{t.skillsChanges} · {report?.changes.length}</Button>}
            </div>
            <Input type="search" placeholder={t.skillsSearch} aria-label={t.skillsSearch} value={query} onChange={(event) => setQuery(event.currentTarget.value)}/>
          </div>
        </div>
        {section === "changes" && baselineAt !== null ? <>
          {!complete && <p className="panel-body small muted" role="status">{t.skillsChangesPending}</p>}
          {changes.length ? <ul className="skills-list">
            {changes.map((change) => <li className="skill-row" key={change.path}>
              <div className="skill-labels"><span className={`format-tag skill-change-${change.kind}`}>{t.skillsChangeKind[change.kind]}</span>
                <code dir="ltr">{change.path}</code></div>
              <div className="skill-labels">
                {change.before && <code title={change.before}>SHA-256 {change.before.slice(0, 12)}</code>}
                {change.before && change.after && <span aria-hidden="true">→</span>}
                {change.after && <code title={change.after}>SHA-256 {change.after.slice(0, 12)}</code>}
              </div>
            </li>)}
          </ul> : (search || complete) && <p className="panel-body small muted" role="status">
            {search ? t.skillsNoMatches : t.skillsNoChanges}
          </p>}
        </> : <>
          <p className="panel-body small muted">{t.skillsScope}</p>
          {entries.length ? <SkillInventoryList entries={entries} t={t}/>
            : <p className="panel-body small muted" role="status">{search ? t.skillsNoMatches : t.skillsEmpty}</p>}
        </>}
      </> : <div className="empty" role="status">
        <div className="empty-icon"><BookOpen size={22}/></div>
        <h3>{previewMode ? t.skillsNative : busy ? t.skillsScanning : t.skillsStart}</h3><p>{t.skillsLocationHint}</p>
      </div>}
    </section>
    {root && inventory && <SkillImportPanel key={projectSelection} t={t} root={root} entries={inventory.entries}
      busy={busy} onTask={task} onInstalled={refresh}/>}
  </div>;
}

function SkillInventoryList({ entries, t }: { entries: SkillEntry[]; t: Copy }) {
  return <ul className="skills-list">
      {entries.map((entry) => <li className="skill-row" key={entry.path}>
        <strong dir="auto">{entry.name}</strong><code dir="ltr">{entry.path}</code>
        <div className="skill-labels">
          {entry.tools.map((tool) => <span className="format-tag" key={tool}>{TOOL_NAMES[tool]}</span>)}
          {entry.same_name && <span className="format-tag skill-match">{t.skillsSameName}</span>}
          <code className="skill-fingerprint" title={`${t.skillsFingerprint} ${entry.fingerprint}`}>SHA-256 {entry.fingerprint.slice(0, 12)}</code>
        </div>
      </li>)}
    </ul>;
}
