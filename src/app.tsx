import { useCallback, useEffect, useRef, useState, type ReactNode } from "react";
import {
  Activity, ArrowDownToLine, ArrowUpFromLine, BookOpen, Bookmark, Boxes, Check, ChevronRight, CircleAlert, Cloud, CodeXml,
  FileCode2, FileQuestion, FolderTree, HardDrive, History, Languages, Layers, Moon, Plus, RefreshCw, Settings2,
  ShieldCheck, Sun, Terminal, Waves,
} from "lucide-react";
import { api, onNativeWindowFocus, previewMode } from "./api";
import { ConfigDriftPanel } from "./components/config-drift";
import { ConfigSources } from "./components/config-sources";
import { Connection } from "./components/connection";
import { DiffDialog } from "./components/diff-dialog";
import { Profiles, type ProfileEdit } from "./components/profiles";
import { ProjectSkills } from "./components/project-skills";
import { Button } from "./components/ui/button";
import { Input } from "./components/ui/input";
import { acceptConfig, describeDrift, emptyConfigSession, mergeDraft, observeConfig, observedIssue, observedPath, observedView } from "./config-session";
import { FIELD_OPTIONS, FIELD_ORDER, TOOLS, TOOL_NAMES, defaultChangeSelection, fieldPatch, fieldPath, selectedChangePatch, sameTarget, targetOf, type Change, type ConfigInspection, type Field, type Fields, type Profile, type ProfileArchive, type ProjectConfigReport, type RuntimeInfo, type Snapshot, type Target, type Tool, type ToolInfo, type View } from "./domain";
import { copy, errorText, type Copy, type Language } from "./i18n";
import { version } from "../package.json";

type Tab = "config" | "sources" | "profiles" | "snapshots" | "skills" | "connectivity" | "settings";
type Theme = "dark" | "light" | "system";
type Review = { target: Target } & ({ kind: "save"; changes: Change[]; patch: Fields }
  | { kind: "restore" | "compare"; changes: Change[]; id: number }
  | { kind: "restore_fields"; changes: Change[]; id: number; selected: Field[] }
  | { kind: "profile"; changes: Change[]; id: number; name: string; selected: Field[] });
type Notice = { message: string; error: boolean };

const TOOL_ICON = { codex: Terminal, claude: Cloud, opencode: CodeXml };
const TABS = [{ id: "config", icon: FileCode2 }, { id: "sources", icon: FolderTree }, { id: "profiles", icon: Bookmark },
  { id: "snapshots", icon: History }, { id: "skills", icon: BookOpen }, { id: "connectivity", icon: Activity }] as const;
const RECHECK_ERRORS = new Set([
  "conflict", "path_unavailable", "ambiguous_config", "unsafe_path", "file_too_large",
  "invalid_config", "io", "recovery_required",
]);

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
  const [runtime, setRuntime] = useState<RuntimeInfo | null>(null);
  const [session, setSession] = useState(emptyConfigSession);
  const [projectConfig, setProjectConfig] = useState<ProjectConfigReport | null>(null);
  const [snapshots, setSnapshots] = useState<Snapshot[]>([]);
  const [profiles, setProfiles] = useState<Profile[]>([]);
  const [review, setReview] = useState<Review | null>(null);
  const [notice, setNotice] = useState<Notice | null>(null);
  const [historyProblem, setHistoryProblem] = useState<unknown>(null);
  const [profilesProblem, setProfilesProblem] = useState<unknown>(null);
  const [profilesLoading, setProfilesLoading] = useState(true);
  const [checking, setChecking] = useState(false);
  const [busy, setBusy] = useState(false);
  const [refresh, setRefresh] = useState(0);
  const inspectionInFlight = useRef<symbol | null>(null);
  const taskInFlight = useRef(false);
  const focusAfterTask = useRef(false);
  const t = copy[language];
  const view = observedView(session.baseline);
  const baselineIssue = observedIssue(session.baseline);
  const viewPath = view?.path;
  const loading = session.baseline === null;
  const { draft } = session;
  const patch = view ? fieldPatch(view.fields, draft) : {};
  const dirty = Object.keys(patch).length > 0;
  const drift = describeDrift(session);
  const controlsBusy = busy || checking;
  const configLocked = controlsBusy || Boolean(drift);
  const activeReview = review && view && !drift && sameTarget(review.target, view) ? review : null;
  const selectableReview = activeReview && "selected" in activeReview ? activeReview : null;
  const info = tools.find((entry) => entry.tool === tool);
  const filePath = session.baseline ? observedPath(session.baseline) : info?.path;

  const checkConfig = useCallback(() => {
    if (inspectionInFlight.current || taskInFlight.current) return;
    const ticket = Symbol();
    inspectionInFlight.current = ticket;
    setChecking(true);
    const receive = (inspection: ConfigInspection) => {
      if (inspectionInFlight.current !== ticket) return;
      inspectionInFlight.current = null;
      setSession((current) => observeConfig(current, inspection));
      setChecking(false);
    };
    void api.inspectConfig(tool).then(receive, (error: unknown) => {
      receive({ config: { kind: "unavailable", path: null, issue: typeof error === "string" ? error : "io" }, active_env: null });
    });
  }, [tool]);

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
    if (previewMode) return;
    void api.runtimeInfo().then((value) => { if (active) setRuntime(value); }, () => {});
    return () => { active = false; };
  }, []);

  useEffect(() => {
    let active = true;
    void api.discover().then((value) => { if (active) setTools(value); }, () => {
      if (active) setTools([]);
    });
    return () => { active = false; };
  }, [refresh]);

  useEffect(() => {
    checkConfig();
    return () => { inspectionInFlight.current = null; };
  }, [checkConfig]);

  useEffect(() => {
    if (previewMode) return;
    let active = true;
    let unlisten: (() => void) | undefined;
    const onFocus = () => {
      if (!active) return;
      if (taskInFlight.current) focusAfterTask.current = true;
      else checkConfig();
    };
    void onNativeWindowFocus(onFocus).then((dispose) => {
      if (active) unlisten = dispose;
      else dispose();
    }, () => {
      if (!active) return;
      window.addEventListener("focus", onFocus);
      unlisten = () => window.removeEventListener("focus", onFocus);
    });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [checkConfig]);

  useEffect(() => {
    if (review && !activeReview) setReview(null);
  }, [review, activeReview]);

  useEffect(() => {
    let active = true;
    setSnapshots([]);
    setHistoryProblem(null);
    if (viewPath !== undefined) {
      void api.snapshots(tool, viewPath).then((value) => { if (active) setSnapshots(value); },
        (error) => { if (active) setHistoryProblem(error); });
    }
    return () => { active = false; };
  }, [tool, viewPath, refresh]);

  useEffect(() => {
    let active = true;
    setProfiles([]);
    setProfilesLoading(true);
    setProfilesProblem(null);
    void api.profiles(tool).then((value) => {
      if (active) { setProfiles(value); setProfilesLoading(false); }
    }, (error) => {
      if (active) { setProfilesProblem(error); setProfilesLoading(false); }
    });
    return () => { active = false; };
  }, [tool, refresh]);

  async function run(task: () => Promise<void>): Promise<boolean> {
    if (taskInFlight.current || inspectionInFlight.current) return false;
    taskInFlight.current = true;
    let recheck = false;
    setBusy(true);
    setNotice(null);
    try { await task(); return true; }
    catch (err) {
      if (err !== "cancelled") setNotice({ message: errorText(err, language), error: true });
      recheck = typeof err === "string" && RECHECK_ERRORS.has(err);
      if (recheck) setReview(null);
      return false;
    }
    finally {
      taskInFlight.current = false;
      setBusy(false);
      const shouldCheck = recheck || focusAfterTask.current;
      focusAfterTask.current = false;
      if (shouldCheck) checkConfig();
    }
  }

  async function refreshHistory(next: View) {
    try { setSnapshots(await api.snapshots(next.tool, next.path)); setHistoryProblem(null); }
    catch (error) { setHistoryProblem(error); }
  }

  function choose(next: Tool) {
    if (next !== tool && dirty && !window.confirm(t.discard)) return;
    if (next !== tool) {
      inspectionInFlight.current = null;
      setChecking(false);
      setSession(emptyConfigSession);
      setSnapshots([]);
      setProfiles([]);
      setReview(null);
      setNotice(null);
    }
    setTool(next);
    if (tab !== "sources") setTab("config");
  }

  function reload() {
    setNotice(null);
    checkConfig();
    setRefresh((previous) => previous + 1);
  }

  function updateDraft(update: (previous: Fields) => Fields) {
    setSession((current) => ({ ...current, draft: update(current.draft), autoChange: null }));
  }

  function resolveDrift(keep?: Field[]) {
    if (!drift || taskInFlight.current || inspectionInFlight.current || (keep && !drift.canMerge)) return;
    setSession((current) => {
      if (current.latest !== drift.latest) return current;
      return keep ? mergeDraft(current, keep) : acceptConfig(drift.latest);
    });
    setReview(null);
    setTab("config");
    success(keep ? t.driftMerged : t.driftLoaded);
  }

  function success(message: string) { setNotice({ message, error: false }); }

  function applyView(next: View) {
    setSession((current) => acceptConfig({ config: { kind: "view", view: next }, active_env: current.latest?.active_env ?? null }));
    setReview(null);
  }

  async function preview() {
    if (!view || drift) return;
    const target = targetOf(view);
    const changes = await api.preview(target, patch);
    setReview({ kind: "save", target, changes, patch });
  }

  async function compare(id: number, kind: "compare" | "restore" | "restore_fields") {
    if (!view || drift) return;
    if (kind !== "compare" && dirty && !window.confirm(t.discard)) return;
    const target = targetOf(view);
    if (kind === "restore_fields") {
      const changes = await api.compareSnapshotFields(target, id);
      setReview({ kind, target, changes, id, selected: defaultChangeSelection(changes) });
    } else {
      const changes = await api.compareSnapshot(target, id);
      setReview({ kind, target, changes, id });
    }
  }

  async function confirm() {
    const review = activeReview;
    if (!review || review.kind === "compare") return;
    if ("selected" in review && !review.selected.length) return;
    const { target } = review;
    const next = review.kind === "save" ? await api.save(target, review.patch)
      : review.kind === "profile" ? await api.applyProfile(target, review.id, selectedChangePatch(review.changes, review.selected))
      : review.kind === "restore_fields" ? await api.restoreSnapshotFields(target, review.id, selectedChangePatch(review.changes, review.selected))
      : await api.restore(target, review.id);
    applyView(next);
    success(review.kind === "save" ? t.saved : review.kind === "profile" ? t.profileApplied
      : review.kind === "restore_fields" ? t.restoredFields : t.restored);
    await refreshHistory(next);
  }

  async function capture() {
    if (!view || drift) return;
    const snapshot = await api.snapshot(targetOf(view));
    setSnapshots((previous) => [snapshot, ...previous]);
    success(t.madeSnapshot);
  }

  async function createProfile(name: string) {
    if (!view || view.issue || drift) return false;
    return run(async () => {
      const created = await api.createProfile(targetOf(view), name, patch);
      setProfiles((previous) => [created, ...previous]);
      success(t.profileCreated);
    });
  }

  async function previewProfile(profile: Profile) {
    if (!view || view.issue || drift || (dirty && !window.confirm(t.discard))) return;
    const target = targetOf(view);
    const result = await api.previewProfile(target, profile.id);
    setReview({ kind: "profile", id: profile.id, ...result, target, selected: defaultChangeSelection(result.changes) });
  }

  function editProfile(kind: ProfileEdit, profile: Profile, name: string) {
    return run(async () => {
      const edited = kind === "rename" ? await api.renameProfile(tool, profile.id, name)
        : await api.duplicateProfile(tool, profile.id, name);
      setProfiles((previous) => kind === "rename"
        ? previous.map((item) => item.id === edited.id ? edited : item)
        : [edited, ...previous]);
      success(kind === "rename" ? t.profileRenamed : t.profileDuplicated);
    });
  }

  async function pickProfileArchive(): Promise<ProfileArchive | null> {
    let archive: ProfileArchive | null = null;
    await run(async () => { archive = await api.readProfileArchive(tool); });
    return archive;
  }

  function importProfile(archive: ProfileArchive, name: string) {
    return run(async () => {
      const saved = await api.importProfile(tool, archive, name);
      setProfiles((previous) => [saved, ...previous]);
      success(t.profileImported);
    });
  }

  async function exportProfile(profile: Profile) {
    await api.exportProfileArchive(tool, profile.id);
    success(t.profileExported);
  }

  async function removeProfile(profile: Profile) {
    if (!window.confirm(`${t.profileDeletePrompt}\n${profile.name}`)) return;
    await api.deleteProfile(tool, profile.id);
    setProfiles((previous) => previous.filter(({ id }) => id !== profile.id));
    success(t.profileRemoved);
  }

  async function importArchive() {
    const imported = await api.importArchive(tool);
    updateDraft((previous) => ({ ...previous, ...imported }));
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
          <Button variant="outline" size="sm" disabled={controlsBusy || loading} aria-busy={checking} onClick={reload}><RefreshCw/>{t.scan}</Button>
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
          {runtime && <section className="panel">
            <div className="panel-head"><div className="section-icon"><HardDrive size={19}/></div><div><h2>{t.appStorage}</h2>
              <p>{runtime.portable ? t.portableMode : t.standardStorage}</p></div></div>
            <div className="panel-body form-stack">
              <div className="path-row source-path"><code>{runtime.data_dir}</code></div>
              {runtime.portable && <p className="small muted">{t.portableStorageHint}</p>}
            </div>
          </section>}
          <section className="panel"><div className="panel-head"><div className="section-icon"><Waves size={19}/></div><div><h2>{t.about}</h2><p>{t.aboutText}</p></div></div></section>
        </div> : <>
          <section className="hero" aria-labelledby="workspace-title">
            <div className="hero-grid"/>
            <div className="hero-text"><div className="hero-status"><span className={`dot ${view?.exists && !view.issue && !drift ? "live" : ""}`}/>{loading ? t.loading : drift ? t.driftTitle : baselineIssue ? t.unavailable : view?.exists ? t.ready : t.noFile}</div>
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

          {notice && !activeReview && <div role={notice.error ? "alert" : "status"} className={notice.error ? "inline-message error" : "inline-message success"}>
            {notice.error ? <CircleAlert size={17}/> : <Check size={17}/>}<span>{notice.message}</span>
          </div>}
          {session.autoChange && <div className="inline-message" role="status">
            <RefreshCw size={17}/><span>{t.driftReasons[session.autoChange]} {t.driftAuto}</span>
          </div>}
          {drift && <ConfigDriftPanel t={t} language={language} change={drift} busy={controlsBusy}
            key={JSON.stringify([observedPath(drift.latest.config), observedView(drift.latest.config)?.revision, observedIssue(drift.latest.config)])}
            onCheck={checkConfig} onMerge={resolveDrift} onDiscard={() => resolveDrift()}/>}

          <div className="dashboard-grid">
            <div className="dashboard-main">
              {tab === "connectivity" ? <Connection key={`${tool}/${view?.revision ?? ""}`} t={t} language={language} view={view} tool={tool}/>
                : tab === "skills" ? <ProjectSkills t={t} language={language}/>
                : tab === "sources" ? <ConfigSources t={t} language={language} tool={tool} inspection={session.latest}
                  report={projectConfig} busy={controlsBusy} native={!previewMode}
                  onPick={() => void run(async () => { setProjectConfig(await api.pickProjectConfig()); })}/>
                : loading ? <section className="panel"><Empty icon={<RefreshCw size={22}/>} title={t.loading} subtitle={t.data}/></section>
                : tab === "profiles" ? <Profiles key={tool} t={t} language={language} profiles={profiles}
                  available={Boolean(view && !view.issue && !drift)} dirty={dirty} busy={controlsBusy} loading={profilesLoading}
                  error={profilesProblem ? errorText(profilesProblem, language) : null}
                  onCreate={createProfile} onApply={(profile) => void run(() => previewProfile(profile))}
                  onRemove={(profile) => void run(() => removeProfile(profile))} onEdit={editProfile}
                  canTransfer={!previewMode} onPickArchive={pickProfileArchive} onImport={importProfile}
                  onExport={(profile) => void run(() => exportProfile(profile))}/>
                : tab === "snapshots" ? <section className="panel">
                  <div className="panel-head spread"><div className="head-label"><div className="section-icon"><History size={19}/></div><div><h2>{t.snapshots}</h2><p>{t.safeStorage}</p></div></div>
                    <Button variant="outline" size="sm" disabled={configLocked || !view?.exists || Boolean(view.issue)} onClick={() => void run(capture)}><Plus/>{t.createSnapshot}</Button>
                  </div>
                  {historyProblem ? <div className="panel-body"><div className="inline-message error" role="alert"><CircleAlert size={17}/>{errorText(historyProblem, language)}</div></div>
                    : snapshots.length === 0 ? <Empty icon={<Layers size={23}/>} title={t.noSnapshots} subtitle={t.snapHint}/>
                    : <div className="snapshot-list">{snapshots.map((snapshot) => <div className="snapshot-row" key={snapshot.id}>
                      <div className="snapshot-icon"><Layers size={18}/></div><div className="snapshot-text"><strong>{t[snapshot.reason]}</strong><small>{date(snapshot, language)} · #{snapshot.id}</small></div>
                      <div className="row-actions snapshot-actions">
                        <Button variant="ghost" size="sm" disabled={configLocked || !view} onClick={() => void run(() => compare(snapshot.id, "compare"))}>{t.compare}</Button>
                        <Button variant="ghost" size="sm" disabled={configLocked || !view} onClick={() => void run(() => compare(snapshot.id, "restore"))}>{t.restore}</Button>
                        <Button variant="outline" size="sm" disabled={configLocked || !view || Boolean(view.issue)}
                          onClick={() => void run(() => compare(snapshot.id, "restore_fields"))}>{t.restoreFields}</Button>
                      </div>
                    </div>)}</div>}
                </section>
                : !view || view.issue ? <section className="panel">
                  <Empty icon={<CircleAlert size={23}/>} title={t.unavailable} subtitle={errorText(baselineIssue, language)}/>
                  <div className="panel-footer"><Button variant="outline" disabled={controlsBusy} onClick={reload}><RefreshCw/>{t.reload}</Button></div>
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
                        <select id={field} className="ui-input select mono" value={draft[field] ?? ""} disabled={configLocked} onChange={(event) => {
                          const value = event.currentTarget.value || null;
                          updateDraft((previous) => ({ ...previous, [field]: value }));
                        }}>
                          <option value="">{t.defaultValue}</option>{FIELD_OPTIONS[field]?.map((option) => <option key={option}>{option}</option>)}
                        </select>
                        : <Input id={field} className="mono" value={draft[field] ?? ""} placeholder={t.defaultValue} disabled={configLocked}
                          spellCheck={false} maxLength={field.endsWith("_base_url") ? 512 : 180} onChange={(event) => {
                            const value = event.currentTarget.value;
                            updateDraft((previous) => ({ ...previous, [field]: value }));
                          }}/>}
                      <small>{fieldHint(field, t, tool)}</small>
                    </div>)}
                  </div>
                  <div className="panel-footer spread">
                    <div className="row-actions">
                      <Button variant="ghost" size="sm" disabled={configLocked || previewMode} onClick={() => void run(importArchive)}><ArrowDownToLine/>{t.import}</Button>
                      <Button variant="ghost" size="sm" disabled={configLocked || !view.exists || previewMode} onClick={() => void run(async () => { await api.exportArchive(targetOf(view)); success(t.exported); })}><ArrowUpFromLine/>{t.export}</Button>
                    </div>
                    <Button disabled={configLocked || !dirty} onClick={() => void run(preview)}>{t.review}<ChevronRight/></Button>
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
                  <div className="path-row"><span>{t.file}</span><code title={filePath ?? ""}>{filePath ?? "—"}</code></div>
                </div>
              </section>
              <section className="panel detail-panel">
                <div className="panel-head"><div className="section-icon"><History size={18}/></div><div><h2>{t.recent}</h2><p>{snapshots.length} {t.snapshotCount}</p></div></div>
                {snapshots.length > 0 ? <div className="side-body recent-list">
                  {snapshots.slice(0, 3).map((snapshot) => <button key={snapshot.id} disabled={configLocked || !view} onClick={() => void run(() => compare(snapshot.id, "compare"))}>
                    <span><strong>{t[snapshot.reason]}</strong><small>{date(snapshot, language)}</small></span><ChevronRight size={15}/>
                  </button>)}
                </div> : <div className="side-body muted small">{t.snapHint}</div>}
                <div className="side-footer"><Button variant="ghost" size="sm" disabled={configLocked || !view?.exists || Boolean(view.issue)} onClick={() => void run(capture)}><Plus/>{t.createSnapshot}</Button></div>
              </section>
              <div className="security-note"><ShieldCheck size={17}/><span>{t.safeStorage}</span></div>
            </aside>
          </div>
        </>}
      </div>

      {activeReview && <DiffDialog t={t} changes={activeReview.changes} busy={busy} error={notice?.error ? notice.message : undefined} onClose={() => setReview(null)}
        title={activeReview.kind === "save" ? t.review : activeReview.kind === "profile" ? activeReview.name : `${t.ref} #${activeReview.id}`}
        hint={activeReview.kind === "profile" ? t.profileSelectionHint : activeReview.kind === "save" ? t.saveHint
          : activeReview.kind === "restore" ? t.restoreHint : activeReview.kind === "restore_fields" ? t.restoreFieldsHint : undefined}
        selection={selectableReview ? {
          fields: selectableReview.selected,
          onChange: (selected) => setReview((current) => current && "selected" in current ? { ...current, selected } : current),
        } : undefined}
        action={activeReview.kind !== "compare" && (activeReview.kind === "restore" || activeReview.changes.length > 0)
          ? <Button disabled={configLocked || (selectableReview !== null && !selectableReview.selected.length)} onClick={() => void run(confirm)}>
            {activeReview.kind === "restore" ? t.confirmRestore : activeReview.kind === "profile" ? t.profileApplySelected
              : activeReview.kind === "restore_fields" ? t.restoreFields : view?.exists ? t.saving : t.create}
          </Button>
          : undefined}/>}
    </main>
  </div>;
}
