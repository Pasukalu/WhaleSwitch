import { useEffect, useState, type ReactNode } from "react";
import {
  Activity, ArrowDownToLine, ArrowUpFromLine, Bookmark, Boxes, Check, ChevronRight, CircleAlert, Cloud, CodeXml,
  FileCode2, FileQuestion, HardDrive, History, Languages, Layers, Moon, Plus, RefreshCw, Settings2,
  ShieldCheck, Sun, Terminal, Waves,
} from "lucide-react";
import { api, previewMode } from "./api";
import { Connection } from "./components/connection";
import { DiffDialog } from "./components/diff-dialog";
import { Profiles } from "./components/profiles";
import { Button } from "./components/ui/button";
import { Input } from "./components/ui/input";
import { FIELD_OPTIONS, FIELD_ORDER, TOOLS, TOOL_NAMES, fieldPatch, fieldPath, targetOf, type Change, type Field, type Fields, type Profile, type Snapshot, type Tool, type ToolInfo, type View } from "./domain";
import { copy, errorText, type Copy, type Language } from "./i18n";
import { version } from "../package.json";

type Tab = "config" | "profiles" | "snapshots" | "connectivity" | "settings";
type Theme = "dark" | "light" | "system";
type Review = { kind: "save"; changes: Change[]; patch: Fields }
  | { kind: "restore" | "compare"; changes: Change[]; id: number }
  | { kind: "profile"; changes: Change[]; id: number; name: string };
type Notice = { message: string; error: boolean };

const TOOL_ICON = { codex: Terminal, claude: Cloud, opencode: CodeXml };
const TABS = [{ id: "config", icon: FileCode2 }, { id: "profiles", icon: Bookmark },
  { id: "snapshots", icon: History }, { id: "connectivity", icon: Activity }] as const;

function setPreference(key: string, value: string) {
  try { localStorage.setItem(key, value); } catch { /* ephemeral preferences */ }
}

function preference(key: string) {
  try { return localStorage.getItem(key); } catch { return null; }
}

function ToolIcon({ tool }: { tool: Tool }) {
  const Icon = TOOL_ICON[tool];
  return <span className={`tool-icon ${tool}`}><Icon size={18} strokeWidth={1.9}/></span>;
}

function Empty({ icon, title, subtitle }: { icon: ReactNode; title: string; subtitle: string }) {
  return <div className="empty"><div className="empty-icon">{icon}</div><h3>{title}</h3><p>{subtitle}</p></div>;
}

function date(snapshot: Snapshot, lang: Language) {
  return new Intl.DateTimeFormat(lang === "zh" ? "zh-CN" : "en", { dateStyle: "medium", timeStyle: "short" }).format(snapshot.timestamp);
}

function fieldHint(field: Field, t: Copy, tool: Tool) {
  if (field === "model") return tool === "opencode" ? t.modelHintOpenCode : t.modelHint;
  return t[`${field}Hint`];
}

export function App() {
  const [language, setLanguage] = useState<Language>(() => {
    const saved = preference("whaleswitch-language");
    return saved === "en" || (saved === null && !navigator.language.startsWith("zh")) ? "en" : "zh";
  });
  const [theme, setTheme] = useState<Theme>(() => {
    const saved = preference("whaleswitch-theme");
    return saved === "light" || saved === "system" ? saved : "dark";
  });
  const [tool, setTool] = useState<Tool>("codex");
  const [tab, setTab] = useState<Tab>("config");
  const [tools, setTools] = useState<ToolInfo[]>([]);
  const [view, setView] = useState<View | null>(null);
  const [draft, setDraft] = useState<Fields>({});
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [review, setReview] = useState<Review | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [problem, setProblem] = useState<unknown>(null);
  const [historyProblem, setHistoryProblem] = useState<unknown>(null);
  const [profilesProblem, setProfilesProblem] = useState<unknown>(null);
  const [profilesLoading, setProfilesLoading] = useState(true);
  const [loading, setLoading] = useState(true);
  const [busy, setBusy] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const t = copy[language];
  const patch = view ? fieldPatch(view.fields, draft) : {};
  const dirty = Object.keys(patch).length > 0;
  const info = tools.find((entry) => entry.tool === tool);

  useEffect(() => {
    document.documentElement.lang = language === "zh" ? "zh-CN" : "en";
    setPreference("whaleswitch-language", language);
  }, [language]);

  useEffect(() => {
    const system = window.matchMedia("(prefers-color-scheme: dark)");
    const apply = () => { document.documentElement.dataset.theme = theme === "system" ? (system.matches ? "dark" : "light") : theme; };
    apply();
    system.addEventListener("change", apply);
    setPreference("whaleswitch-theme", theme);
    return () => system.removeEventListener("change", apply);
  }, [theme]);

  useEffect(() => {
    let active = true;
    void api.discover().then((value) => { if (active) setTools(value); }, (error) => {
      if (active) setProblem(error);
    });
    return () => { active = false; };
  }, [refresh]);

  useEffect(() => {
    let active = true;
    setLoading(true);
    setView(null);
    setDraft({});
    setSnapshots([]);
    setProfiles([]);
    setProfilesLoading(true);
    setProblem(null);
    setHistoryProblem(null);
    setProfilesProblem(null);
    setNotice(null);
    setReview(null);
    void api.view(tool).then((value) => {
      if (active) { setView(value); setDraft(value.fields); setLoading(false); }
    }, (error) => { if (active) { setProblem(error); setLoading(false); } });
    void api.snapshots(tool).then((value) => { if (active) setSnapshots(value); }, (error) => { if (active) setHistoryProblem(error); });
    void api.profiles(tool).then((value) => {
      if (active) { setProfiles(value); setProfilesLoading(false); }
    }, (error) => {
      if (active) { setProfilesProblem(error); setProfilesLoading(false); }
    });
    return () => { active = false; };
  }, [tool, refresh]);

  async function run(task: () => Promise<void>): Promise<boolean> {
    if (busy) return false;
    setBusy(true);
    setNotice(null);
    try { await task(); return true; }
    catch (err) {
      if (err !== "cancelled") setNotice({ message: errorText(err, language), error: true });
      return false;
    }
    finally { setBusy(false); }
  }

  async function refreshHistory() {
    try { setSnapshots(await api.snapshots(tool)); setHistoryProblem(null); }
    catch (error) { setHistoryProblem(error); }
  }

  function choose(next: Tool) {
    if (next !== tool && dirty && !window.confirm(t.discard)) return;
    if (next !== tool) { setView(null); setLoading(true); }
    setTool(next);
    setTab("config");
  }

  function reload() {
    if (dirty && !window.confirm(t.discard)) return;
    setRefresh((previous) => previous + 1);
  }

  function success(message: string) { setNotice({ message, error: false }); }

  function applyView(next: View) {
    setView(next);
    setDraft(next.fields);
    setTools((list) => list.map((entry) => entry.tool === next.tool ? { ...entry, exists: next.exists, issue: null } : entry));
    setReview(null);
  }

  async function preview() {
    if (!view) return;
    const changes = await api.preview(targetOf(view), patch);
    setReview({ kind: "save", changes, patch });
  }

  async function compare(id: number, kind: "compare" | "restore") {
    if (!view) return;
    if (kind === "restore" && dirty && !window.confirm(t.discard)) return;
    const changes = await api.compareSnapshot(targetOf(view), id);
    setReview({ kind, changes, id });
  }

  async function confirm() {
    if (!view || !review || review.kind === "compare") return;
    const target = targetOf(view);
    const next = review.kind === "save" ? await api.save(target, review.patch)
      : review.kind === "profile" ? await api.applyProfile(target, review.id) : await api.restore(target, review.id);
    applyView(next);
    success(review.kind === "save" ? t.saved : review.kind === "profile" ? t.profileApplied : t.restored);
    await refreshHistory();
  }

  async function capture() {
    if (!view) return;
    const snapshot = await api.snapshot(targetOf(view));
    setSnapshots((previous) => [snapshot, ...previous]);
    success(t.madeSnapshot);
  }

  async function createProfile(name: string) {
    if (!view || view.issue) return false;
    return run(async () => {
      const created = await api.createProfile(targetOf(view), name, patch);
      setProfiles((previous) => [created, ...previous]);
      success(t.profileCreated);
    });
  }

  async function previewProfile(profile: Profile) {
    if (!view || view.issue || (dirty && !window.confirm(t.discard))) return;
    const result = await api.previewProfile(targetOf(view), profile.id);
    setReview({ kind: "profile", id: profile.id, ...result });
  }

  async function removeProfile(profile: Profile) {
    if (!window.confirm(`${t.profileDeletePrompt}\n${profile.name}`)) return;
    await api.deleteProfile(tool, profile.id);
    setProfiles((previous) => previous.filter(({ id }) => id !== profile.id));
    success(t.profileRemoved);
  }

  async function importArchive() {
    const imported = await api.importArchive(tool);
    setDraft((previous) => ({ ...previous, ...imported }));
    success(t.imported);
  }

  return <div className="workbench">
    <aside className="sidebar">
      <button className="brand" onClick={() => setTab("config")} aria-label="WhaleSwitch" disabled={busy}>
          <span className="brand-mark"><img src="/favicon.png" alt="" width="40" height="40"/></span>
        <span><strong>WhaleSwitch</strong><small>{t.local}</small></span>
      </button>
      <div className="side-label"><span>{t.tools}</span><span>{TOOLS.length}</span></div>
      <nav className="tool-menu" aria-label={t.tools}>
        {TOOLS.map((entry) => {
          const state = tools.find((item) => item.tool === entry);
          return <button key={entry} className={`tool-link ${tool === entry && tab !== "settings" ? "active" : ""}`}
            aria-label={TOOL_NAMES[entry]} aria-current={tool === entry && tab !== "settings" ? "true" : undefined} disabled={busy} onClick={() => choose(entry)}>
            <ToolIcon tool={entry}/>
            <span className="tool-link-text"><strong>{TOOL_NAMES[entry]}</strong><small><span className={`dot ${state?.installed ? "live" : ""}`}/>{state ? state.installed ? t.found : t.notFound : "—"}</small></span>
            {tool === entry && tab !== "settings" && <ChevronRight className="tool-chevron" size={15}/>}
          </button>;
        })}
      </nav>
      <div className="sidebar-bottom">
        <Button variant="ghost" aria-label={t.settings} className={`settings-button ${tab === "settings" ? "active" : ""}`} disabled={busy} onClick={() => setTab("settings")}><Settings2/>{t.settings}</Button>
        <div className="local-status"><span className="dot live"/><span>{t.local}</span><span className="app-version">v{version}</span></div>
      </div>
    </aside>

    <main className="workspace">
      <header className="topbar">
        <div className="breadcrumbs"><Boxes size={15}/><span>{t.workspace}</span><ChevronRight size={14}/><strong>{tab === "settings" ? t.settings : TOOL_NAMES[tool]}</strong></div>
        <div className="top-actions">
          <Button variant="ghost" size="icon" aria-label={t.language} title={t.language} onClick={() => setLanguage(language === "zh" ? "en" : "zh")}><Languages/></Button>
          <Button variant="ghost" size="icon" aria-label={t.appearance} title={t.appearance} onClick={() => setTheme(document.documentElement.dataset.theme === "dark" ? "light" : "dark")}>{theme === "light" ? <Sun/> : <Moon/>}</Button>
          <Button variant="outline" size="sm" disabled={busy || loading} onClick={reload}><RefreshCw/>{t.scan}</Button>
        </div>
      </header>

      <div className="workspace-content">
        {previewMode && <div className="demo-label" role="status">{t.previewMode}</div>}
        {tab === "settings" ? <div className="settings-page">
          <div className="page-heading"><h1>{t.settings}</h1></div>
          <section className="panel">
            <div className="panel-head"><div className="section-icon"><Settings2 size={19}/></div><div><h2>{t.appearance}</h2></div></div>
            <div className="panel-body form-stack">
              <div className="field"><label htmlFor="appearance">{t.appearance}</label><select id="appearance" className="ui-input select" value={theme} onChange={(e) => setTheme(e.target.value as Theme)}>
                <option value="dark">{t.dark}</option><option value="light">{t.light}</option><option value="system">{t.system}</option>
              </select></div>
              <div className="field"><label htmlFor="language">{t.language}</label><select id="language" className="ui-input select" value={language} onChange={(e) => setLanguage(e.target.value as Language)}>
                <option value="zh">简体中文</option><option value="en">English</option>
              </select></div>
            </div>
          </section>
          <section className="panel"><div className="panel-head"><div className="section-icon"><Waves size={19}/></div><div><h2>{t.about}</h2><p>{t.aboutText}</p></div></div></section>
        </div> : <>
          <section className="hero" aria-labelledby="workspace-title">
            <div className="hero-grid"/>
            <div className="hero-text"><div className="hero-status"><span className={`dot ${view?.exists && !view.issue ? "live" : ""}`}/>{loading ? t.loading : problem || view?.issue ? t.unavailable : view?.exists ? t.ready : t.noFile}</div>
              <h1 id="workspace-title">{TOOL_NAMES[tool]}</h1><p>{t.intro}</p>
            </div>
            <div className="hero-art"><div className="orbit orbit-one"/><div className="orbit orbit-two"/><div className="art-glow"/>
              <img src="/whale.png" alt={t.mascot} draggable={false} onError={(event) => { event.currentTarget.hidden = true; }}/>
              <Waves className="hero-fallback" size={78} strokeWidth={0.8} aria-hidden="true"/>
            </div>
          </section>

          <nav className="section-nav" aria-label={t.workspace}>
            {TABS.map(({ id, icon: Icon }) => <button key={id} disabled={busy} aria-current={tab === id ? "page" : undefined}
              className={tab === id ? "selected" : ""} onClick={() => { setTab(id); setNotice(null); }}><Icon size={16}/>{t[id]}{id === "snapshots" && snapshots.length > 0 && <small>{snapshots.length}</small>}</button>)}
            {dirty && <span className="unsaved"><span className="dot"/>{t.modified}</span>}
          </nav>

          {notice && !review && <div role={notice.error ? "alert" : "status"} className={notice.error ? "inline-message error" : "inline-message success"}>
            {notice.error ? <CircleAlert size={17}/> : <Check size={17}/>}<span>{notice.message}</span>
          </div>}

          <div className="dashboard-grid">
            <div className="dashboard-main">
              {tab === "connectivity" ? <Connection key={`${tool}/${view?.revision ?? ""}`} t={t} language={language} view={view} tool={tool}/>
                : loading ? <section className="panel"><Empty icon={<RefreshCw size={22}/>} title={t.loading} subtitle={t.data}/></section>
                : tab === "profiles" ? <Profiles key={tool} t={t} language={language} profiles={profiles}
                  available={Boolean(view && !view.issue)} dirty={dirty} busy={busy} loading={profilesLoading}
                  error={profilesProblem ? errorText(profilesProblem, language) : null}
                  onCreate={createProfile} onApply={(profile) => void run(() => previewProfile(profile))}
                  onRemove={(profile) => void run(() => removeProfile(profile))}/>
                : tab === "snapshots" ? <section className="panel">
                  <div className="panel-head spread"><div className="head-label"><div className="section-icon"><History size={19}/></div><div><h2>{t.snapshots}</h2><p>{t.safeStorage}</p></div></div>
                    <Button variant="outline" size="sm" disabled={busy || !view?.exists || Boolean(view.issue)} onClick={() => void run(capture)}><Plus/>{t.createSnapshot}</Button>
                  </div>
                  {historyProblem ? <div className="panel-body"><div className="inline-message error" role="alert"><CircleAlert size={17}/>{errorText(historyProblem, language)}</div></div>
                    : snapshots.length === 0 ? <Empty icon={<Layers size={23}/>} title={t.noSnapshots} subtitle={t.snapHint}/>
                    : <div className="snapshot-list">{snapshots.map((snapshot) => <div className="snapshot-row" key={snapshot.id}>
                      <div className="snapshot-icon"><Layers size={18}/></div><div className="snapshot-text"><strong>{t[snapshot.reason]}</strong><small>{date(snapshot, language)} · #{snapshot.id}</small></div>
                      <Button variant="ghost" size="sm" disabled={busy || !view} onClick={() => void run(() => compare(snapshot.id, "compare"))}>{t.compare}</Button>
                      <Button variant="outline" size="sm" disabled={busy || !view} onClick={() => void run(() => compare(snapshot.id, "restore"))}>{t.restore}</Button>
                    </div>)}</div>}
                </section>
                : !view || view.issue ? <section className="panel">
                  <Empty icon={<CircleAlert size={23}/>} title={t.unavailable} subtitle={errorText(view?.issue ?? problem, language)}/>
                  <div className="panel-footer"><Button variant="outline" disabled={busy} onClick={reload}><RefreshCw/>{t.reload}</Button></div>
                </section>
                : <section className="panel">
                  <div className="panel-head spread"><div className="head-label"><div className="section-icon"><FileCode2 size={19}/></div><div><h2>{t.docs}</h2><p>{t.fieldsIntro}</p></div></div>
                    <span className="format-tag">{view.format.toUpperCase()}</span>
                  </div>
                  <div className="panel-body form-stack">
                    {!view.exists && <div className="inline-message"><FileQuestion size={17}/><span>{t.noFileHint}</span></div>}
                    {FIELD_ORDER.filter((field) => field in view.fields).map((field) => <div className="field" key={field}>
                      <label htmlFor={field}>{t[field]} <code>{fieldPath(field)}</code></label>
                      {FIELD_OPTIONS[field] ?
                        <select id={field} className="ui-input select mono" value={draft[field] ?? ""} disabled={busy} onChange={(e) => setDraft({ ...draft, [field]: e.target.value || null })}>
                          <option value="">{t.defaultValue}</option>{FIELD_OPTIONS[field]?.map((option) => <option key={option}>{option}</option>)}
                        </select>
                        : <Input id={field} className="mono" value={draft[field] ?? ""} placeholder={t.defaultValue} disabled={busy} spellCheck={false} maxLength={field.endsWith("_base_url") ? 512 : 180} onChange={(e) => setDraft({ ...draft, [field]: e.target.value })}/>}
                      <small>{fieldHint(field, t, tool)}</small>
                    </div>)}
                  </div>
                  <div className="panel-footer spread">
                    <div className="row-actions">
                      <Button variant="ghost" size="sm" disabled={busy || previewMode} onClick={() => void run(importArchive)}><ArrowDownToLine/>{t.import}</Button>
                      <Button variant="ghost" size="sm" disabled={busy || !view.exists || previewMode} onClick={() => void run(async () => { await api.exportArchive(targetOf(view)); success(t.exported); })}><ArrowUpFromLine/>{t.export}</Button>
                    </div>
                    <Button disabled={busy || !dirty} onClick={() => void run(preview)}>{t.review}<ChevronRight/></Button>
                  </div>
                </section>}
            </div>

            <aside className="dashboard-side">
              <section className="panel detail-panel">
                <div className="panel-head"><div className="section-icon"><HardDrive size={18}/></div><div><h2>{t.runtime}</h2><p>{t.data}</p></div></div>
                <div className="side-body">
                  <div className="metadata"><span>{t.selected}</span><strong>{TOOL_NAMES[tool]}</strong></div>
                  <div className="metadata"><span>{t.format}</span><code>{view?.format.toUpperCase() ?? "—"}</code></div>
                  <div className="metadata"><span>{t.revision}</span><code>{view?.exists ? view.revision.slice(0, 8) : "—"}</code></div>
                  <div className="path-row"><span>{t.file}</span><code title={view?.path ?? info?.path ?? ""}>{view?.path ?? info?.path ?? "—"}</code></div>
                </div>
              </section>
              <section className="panel detail-panel">
                <div className="panel-head"><div className="section-icon"><History size={18}/></div><div><h2>{t.recent}</h2><p>{snapshots.length} {t.snapshotCount}</p></div></div>
                {snapshots.length > 0 ? <div className="side-body recent-list">
                  {snapshots.slice(0, 3).map((snapshot) => <button key={snapshot.id} disabled={busy || !view} onClick={() => void run(() => compare(snapshot.id, "compare"))}>
                    <span><strong>{t[snapshot.reason]}</strong><small>{date(snapshot, language)}</small></span><ChevronRight size={15}/>
                  </button>)}
                </div> : <div className="side-body muted small">{t.snapHint}</div>}
                <div className="side-footer"><Button variant="ghost" size="sm" disabled={busy || !view?.exists || Boolean(view.issue)} onClick={() => void run(capture)}><Plus/>{t.createSnapshot}</Button></div>
              </section>
              <div className="security-note"><ShieldCheck size={17}/><span>{t.safeStorage}</span></div>
            </aside>
          </div>
        </>}
      </div>

      {review && <DiffDialog t={t} changes={review.changes} busy={busy} error={notice?.error ? notice.message : undefined} onClose={() => setReview(null)}
        title={review.kind === "save" ? t.review : review.kind === "profile" ? review.name : `${t.ref} #${review.id}`}
        hint={review.kind === "save" || review.kind === "profile" ? t.saveHint : review.kind === "restore" ? t.restoreHint : undefined}
        action={review.kind !== "compare" && (review.kind === "restore" || review.changes.length > 0)
          ? <Button disabled={busy} onClick={() => void run(confirm)}>{review.kind === "restore" ? t.confirmRestore : review.kind === "profile" ? t.profileApply : view?.exists ? t.saving : t.create}</Button>
          : undefined}/>}
    </main>
  </div>;
}
