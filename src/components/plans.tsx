import { useEffect, useState } from "react";
import { ArrowDownToLine, ArrowUpFromLine, ChevronRight, History, Layers, Plus, RefreshCw, Trash2, X } from "lucide-react";
import { api } from "../api";
import { defaultChangeSelection, fieldPath, selectedChangePatch, TOOL_NAMES, TOOLS, type ConfigPlan, type Field,
  type PlanArchive, type PlanPreview, type PlanProfile, type PlanRun, type PlanSelection, type Profile, type Tool } from "../domain";
import { errorText, type Copy, type Language } from "../i18n";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

type ProfileGroup = { tool: Tool; profiles: Profile[] };
type Review = { plan: ConfigPlan; previews: PlanPreview[]; selected: Partial<Record<Tool, Field[]>> };

export function Plans({ t, language, busy, native, onTask, onApply, onOpenBackup }: {
  t: Copy;
  language: Language;
  busy: boolean;
  native: boolean;
  onTask: (task: () => Promise<void>) => Promise<boolean>;
  onApply: (id: number, selections: PlanSelection[]) => Promise<PlanRun | null>;
  onOpenBackup: (tool: Tool, id: number) => void;
}) {
  const [plans, setPlans] = useState<ConfigPlan[]>([]);
  const [groups, setGroups] = useState<ProfileGroup[]>([]);
  const [runs, setRuns] = useState<PlanRun[]>([]);
  const [name, setName] = useState("");
  const [sources, setSources] = useState<Partial<Record<Tool, string>>>({});
  const [archive, setArchive] = useState<PlanArchive | null>(null);
  const [review, setReview] = useState<Review | null>(null);
  const [loadError, setLoadError] = useState<unknown>(null);
  const [loading, setLoading] = useState(true);
  const [reloads, setReloads] = useState(0);
  const [message, setMessage] = useState("");
  const disabled = busy || loading;
  const date = new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en", { dateStyle: "medium", timeStyle: "short" });

  useEffect(() => {
    let active = true;
    setLoading(true);
    setLoadError(null);
    void Promise.all([api.plans(), api.planRuns(), Promise.all(TOOLS.map(async (tool) => ({ tool, profiles: await api.profiles(tool) })))])
      .then(([plans, runs, groups]) => {
        if (!active) return;
        setPlans(plans);
        setRuns(runs);
        setGroups(groups);
        setLoading(false);
      }, (error: unknown) => {
        if (!active) return;
        setLoadError(error);
        setLoading(false);
      });
    return () => { active = false; };
  }, [reloads]);

  const profiles: PlanProfile[] = groups.flatMap(({ tool, profiles }) => {
    const id = Number(sources[tool]);
    return profiles.some((profile) => profile.id === id) ? [{ tool, profile_id: id }] : [];
  });
  const selections: PlanSelection[] = review?.previews.flatMap((preview) => {
    const fields = selectedChangePatch(preview.changes, review.selected[preview.tool] ?? []);
    return preview.path && preview.revision && !preview.issue && Object.keys(fields).length
      ? [{ tool: preview.tool, path: preview.path, revision: preview.revision, fields }]
      : [];
  }) ?? [];
  const changeCount = selections.reduce((total, selection) => total + Object.keys(selection.fields).length, 0);

  function savePlan() {
    void onTask(async () => {
      const plan = archive ? await api.importPlan(archive, name) : await api.createPlan(name, profiles);
      setPlans((previous) => [plan, ...previous]);
      setName("");
      setSources({});
      setArchive(null);
      setMessage(t.planSaved);
    });
  }

  function openPreview(plan: ConfigPlan) {
    setMessage("");
    void onTask(async () => {
      const previews = await api.previewPlan(plan.id);
      const selected = Object.fromEntries(previews.map((preview) => [preview.tool, defaultChangeSelection(preview.changes)]));
      setReview({ plan, previews, selected });
    });
  }

  async function apply() {
    if (!review || !selections.length) return;
    const run = await onApply(review.plan.id, selections);
    if (!run) return;
    setReview(null);
    setRuns((previous) => [run, ...previous.filter(({ id }) => id !== run.id)].slice(0, 20));
  }

  return <div className="form-stack">
    <section className="panel">
      <div className="panel-head spread"><div className="head-label"><div className="section-icon"><Layers size={19}/></div>
        <div><h2>{t.plans}</h2><p>{t.plansIntro}</p></div></div>
        <div className="row-actions">
          <Button variant="ghost" size="icon" aria-label={t.reload} title={t.reload} disabled={disabled}
            onClick={() => { setReview(null); setReloads((value) => value + 1); }}><RefreshCw/></Button>
          {native && <Button variant="outline" size="sm" disabled={disabled} onClick={() => void onTask(async () => {
            const imported = await api.readPlanArchive();
            setArchive(imported);
            setName(imported.name);
            setMessage("");
          })}><ArrowDownToLine/>{t.planImport}</Button>}
        </div>
      </div>
      <form className="panel-body form-stack" onSubmit={(event) => { event.preventDefault(); savePlan(); }}>
        <div className="field"><label htmlFor="plan-name">{t.planName}</label>
          <Input id="plan-name" value={name} maxLength={64} placeholder={t.planNamePlaceholder} disabled={disabled}
            onChange={(event) => setName(event.currentTarget.value)}/></div>
        {archive
          ? <div className="plan-line">
            <p className="small">{t.planImported} · {archive.entries.map((entry) => TOOL_NAMES[entry.tool]).join(" / ")}</p>
            <Button variant="ghost" size="icon" aria-label={t.cancel} disabled={disabled}
              onClick={() => { setArchive(null); setName(""); }}><X/></Button>
          </div>
          : <div className="plan-tool-grid">{groups.map(({ tool, profiles }) => <div className="field" key={tool}>
            <label htmlFor={`plan-source-${tool}`}>{TOOL_NAMES[tool]}</label>
            <select id={`plan-source-${tool}`} className="ui-input select" disabled={disabled || !profiles.length}
              value={sources[tool] ?? ""} onChange={(event) => setSources({ ...sources, [tool]: event.currentTarget.value })}>
              <option value="">{t.planOmitTool}</option>
              {profiles.map((profile) => <option key={profile.id} value={profile.id}>{profile.name}</option>)}
            </select>
          </div>)}</div>}
        {!archive && !loading && !groups.some(({ profiles }) => profiles.length) && <p className="small muted">{t.planNoProfiles}</p>}
        {loading && <p className="small muted" role="status">{t.loading}</p>}
        <div className="row-actions"><Button type="submit" size="sm" disabled={disabled || !name.trim() || (!archive && !profiles.length)}>
          <Plus/>{t.planSave}
        </Button></div>
        {loadError !== null && <p className="inline-message error" role="alert">{errorText(loadError, language)}</p>}
        {message && <p className="small" role="status">{message}</p>}
      </form>
      {!loading && !plans.length && <p className="panel-body small muted">{t.planEmpty}</p>}
      <div className="snapshot-list">{plans.map((plan) => <div key={plan.id} className="snapshot-row">
        <div className="snapshot-text"><strong>{plan.name}</strong><small>{plan.entries.map((entry) => TOOL_NAMES[entry.tool]).join(" / ")}</small></div>
        <div className="row-actions snapshot-actions">
          <Button variant="outline" size="sm" disabled={disabled} onClick={() => openPreview(plan)}>{t.planPreview}<ChevronRight/></Button>
          {native && <Button variant="ghost" size="icon" aria-label={t.planExport} title={t.planExport} disabled={disabled}
            onClick={() => void onTask(async () => { await api.exportPlan(plan.id); setMessage(t.planExported); })}><ArrowUpFromLine/></Button>}
          <Button variant="ghost" size="icon" aria-label={t.planDelete} title={t.planDelete} disabled={disabled}
            onClick={() => {
              if (!window.confirm(t.planDeleteConfirm)) return;
              void onTask(async () => {
                await api.deletePlan(plan.id);
                setPlans((previous) => previous.filter(({ id }) => id !== plan.id));
                if (review?.plan.id === plan.id) setReview(null);
              });
            }}><Trash2/></Button>
        </div>
      </div>)}</div>
    </section>

    {review && <section className="panel" aria-label={t.planPreview}>
      <div className="panel-head spread"><div><h2>{review.plan.name}</h2><p>{t.planReviewHint}</p></div>
        <Button variant="ghost" size="icon" aria-label={t.close} disabled={busy} onClick={() => setReview(null)}><X/></Button></div>
      <div className="panel-body form-stack">{review.previews.map((preview) => <section key={preview.tool} className="plan-target">
        <div className="plan-line"><strong>{TOOL_NAMES[preview.tool]}</strong></div>
        {preview.path && <div className="path-row source-path"><code>{preview.path}</code></div>}
        {preview.issue
          ? <p className="inline-message error" role="alert">{errorText(preview.issue, language)}</p>
          : !preview.changes.length ? <p className="small muted">{t.noChanges}</p>
          : preview.changes.map((change) => <div className="diff-row" key={change.field}>
            <label className="diff-field-choice">
              <input type="checkbox" checked={review.selected[preview.tool]?.includes(change.field) ?? false} disabled={busy}
                onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  setReview((current) => {
                    if (!current) return current;
                    const previous = current.selected[preview.tool] ?? [];
                    const selected = checked ? [...previous, change.field] : previous.filter((field) => field !== change.field);
                    return { ...current, selected: { ...current.selected, [preview.tool]: selected } };
                  });
                }}/>
              <code>{fieldPath(change.field)}</code>
              {change.after === null && <span className="before">{t.profileClearsField}</span>}
            </label>
            <div className="diff-values"><code aria-label={t.current} className="before">{change.before ?? t.none}</code>
              <ChevronRight size={15}/><code aria-label={t.next} className="after">{change.after ?? t.none}</code></div>
          </div>)}
      </section>)}</div>
      <div className="panel-footer spread snapshot-actions">
        <span className="small muted" aria-live="polite">{changeCount} {t.planChangesCount}</span>
        <div className="row-actions"><Button variant="outline" size="sm" disabled={busy}
          onClick={() => openPreview(review.plan)}><RefreshCw/>{t.planRecheck}</Button>
          <Button size="sm" disabled={busy || !changeCount} onClick={() => void apply()}>{t.planApply}</Button>
        </div>
      </div>
    </section>}

    <section className="panel">
      <div className="panel-head"><div className="section-icon"><History size={19}/></div><div><h2>{t.planRuns}</h2></div></div>
      <div className="panel-body form-stack">
        {!loading && !runs.length && <p className="small muted">{t.planNoRuns}</p>}
        {runs.map((run, index) => <details className="plan-run" key={run.id} open={index === 0 || !run.finished}>
          <summary>{run.name} <span className="small muted">· {date.format(run.timestamp)}</span></summary>
          {!run.finished && <p className="small muted">{t.planRunPending}</p>}
          {run.files.map((file) => <div className="plan-line" key={file.tool}>
            <div><strong>{TOOL_NAMES[file.tool]}</strong><div className="small">{t.planStatuses[file.status]}</div>
              {file.issue && <div className="small muted">{errorText(file.issue, language)}</div>}</div>
            {file.snapshot_id !== null && <Button variant="ghost" size="sm" disabled={busy}
              onClick={() => { if (file.snapshot_id !== null) onOpenBackup(file.tool, file.snapshot_id); }}>
              {t.planOpenBackup} #{file.snapshot_id}
            </Button>}
          </div>)}
        </details>)}
      </div>
    </section>
  </div>;
}
