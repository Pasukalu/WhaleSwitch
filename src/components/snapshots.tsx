import { useCallback, useEffect, useRef, useState } from "react";
import { ArrowUpFromLine, CircleAlert, History, Layers, Pin, PinOff, Plus, RefreshCw, Trash2 } from "lucide-react";
import { api, previewMode } from "../api";
import type { ConfigActivity, Snapshot, SnapshotCleanupPreview, Tool, View } from "../domain";
import { errorText, type Copy, type Language } from "../i18n";
import { pageForSnapshot, snapshotPage } from "../snapshot-history";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

type CompareMode = "compare" | "restore" | "restore_fields";
type Task = (action: () => Promise<void>) => Promise<boolean>;

export function SnapshotsPanel({ t, language, tool, view, snapshots, focus, loading, problem, busy,
  onLocated, onTask, onCapture, onCompare, onRefresh, onNotice }: {
  t: Copy;
  language: Language;
  tool: Tool;
  view: View | null;
  snapshots: Snapshot[];
  focus: number | null;
  loading: boolean;
  problem: unknown;
  busy: boolean;
  onLocated: () => void;
  onTask: Task;
  onCapture: () => Promise<void>;
  onCompare: (id: number, kind: CompareMode) => Promise<void>;
  onRefresh: (view: View) => Promise<void>;
  onNotice: (message: string) => void;
}) {
  const [review, setReview] = useState<SnapshotCleanupPreview | null>(null);
  const [selected, setSelected] = useState<number[]>([]);
  const [activityVersion, setActivityVersion] = useState(0);
  const [query, setQuery] = useState("");
  const [pinnedOnly, setPinnedOnly] = useState(false);
  const [pageIndex, setPageIndex] = useState(0);
  const [location, setLocation] = useState<{ id: number } | null>(null);
  const locatedRow = useRef<HTMLDivElement>(null);
  const locale = language === "zh" ? "zh-CN" : "en";
  const format = new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" });
  const selectedBytes = review?.candidates.reduce((sum, item) => sum + (selected.includes(item.id) ? item.stored_bytes : 0), 0) ?? 0;
  const totalBytes = snapshots.reduce((sum, item) => sum + item.stored_bytes, 0);
  const disabled = busy || loading;
  const page = snapshotPage(snapshots, query, pinnedOnly, pageIndex);

  const locate = useCallback((id: number) => {
    const index = pageForSnapshot(snapshots, id);
    if (index === null) return false;
    setQuery("");
    setPinnedOnly(false);
    setPageIndex(index);
    setLocation({ id });
    return true;
  }, [snapshots]);

  useEffect(() => {
    if (focus !== null && !loading && !problem && locate(focus)) onLocated();
  }, [focus, loading, problem, locate, onLocated]);

  useEffect(() => {
    if (!location) return;
    locatedRow.current?.focus({ preventScroll: true });
    locatedRow.current?.scrollIntoView({ block: "nearest" });
  }, [location]);

  function clearReview() {
    setReview(null);
    setSelected([]);
  }

  function togglePin(snapshot: Snapshot) {
    if (!view) return;
    void onTask(async () => {
      await api.pinSnapshot(tool, view.path, snapshot.id, !snapshot.pinned);
      clearReview();
      await onRefresh(view);
    });
  }

  function previewCleanup() {
    if (!view) return;
    void onTask(async () => {
      clearReview();
      setReview(await api.previewSnapshotCleanup(tool, view.path));
    });
  }

  function cleanUp() {
    if (!view || !review || !selected.length) return;
    void onTask(async () => {
      clearReview();
      try {
        const result = await api.applySnapshotCleanup(tool, view.path, review.revision, selected);
        onNotice(t.snapshotCleanupDone.replace("{count}", String(result.removed)));
      } finally {
        setActivityVersion((version) => version + 1);
        await onRefresh(view);
      }
    });
  }

  return <div className="form-stack">
    <section className="panel" aria-busy={loading}>
      <div className="panel-head spread history-head">
        <div className="head-label"><div className="section-icon"><History size={19}/></div>
          <div><h2>{t.snapshots}</h2><p>{t.safeStorage}</p></div>
        </div>
        <div className="row-actions">
          <Button variant="ghost" size="sm" disabled={disabled || !view || !snapshots.length || Boolean(problem)}
            onClick={previewCleanup}><Trash2/>{t.previewSnapshotCleanup}</Button>
          <Button variant="outline" size="sm" disabled={disabled || !view?.exists || Boolean(view.issue)} onClick={() => void onTask(async () => {
            clearReview();
            try { await onCapture(); } finally { setActivityVersion((version) => version + 1); }
          })}><Plus/>{t.createSnapshot}</Button>
        </div>
      </div>
      {review && <div className="panel-body form-stack cleanup-review">
        <div className="plan-line"><h3>{t.snapshotCleanupTitle}</h3>
          <span className="format-tag">{t.snapshotCleanupProtected.replace("{count}", String(review.protected))}</span>
        </div>
        <p className="small muted">{t.snapshotCleanupRule.replace("{count}", String(review.keep_recent))}</p>
        {review.eligible > review.candidates.length && <p className="small muted">
          {t.snapshotCleanupBatch.replace("{shown}", String(review.candidates.length)).replace("{total}", String(review.eligible))}
        </p>}
        {review.candidates.length ? <div className="cleanup-list">
          {review.candidates.map((item) => <label className="check-row" key={item.id}>
            <input type="checkbox" checked={selected.includes(item.id)} disabled={disabled}
              onChange={(event) => {
                const checked = event.currentTarget.checked;
                setSelected((ids) => checked ? [...ids, item.id] : ids.filter((id) => id !== item.id));
              }}/>
            <span><strong>#{item.id} · {t[item.reason]}</strong><small>{format.format(item.timestamp)}</small></span>
            <code>{byteSize(item.stored_bytes, locale)}</code>
          </label>)}
        </div> : <p className="small muted" role="status">{t.snapshotCleanupEmpty}</p>}
        <div className="plan-line">
          {review.candidates.length > 0 && <p className="small muted" role="status">
            {t.snapshotCleanupSelected.replace("{count}", String(selected.length)).replace("{size}", byteSize(selectedBytes, locale))}
          </p>}
          <div className="row-actions">
            <Button variant="ghost" size="sm" disabled={busy} onClick={clearReview}>{t.close}</Button>
            {review.candidates.length > 0 && <Button variant="danger" size="sm" disabled={disabled || !selected.length}
              onClick={cleanUp}>{t.snapshotCleanupConfirm}</Button>}
          </div>
        </div>
      </div>}
      {focus !== null && !loading && !problem && !snapshots.some(({ id }) => id === focus) &&
        <div className="panel-body"><p className="inline-message" role="status">{t.planSnapshotUnavailable} #{focus}</p></div>}
      {loading ? <p className="panel-body small muted" role="status">{t.loading}</p>
        : problem ? <div className="panel-body"><div className="inline-message error" role="alert">
          <CircleAlert size={17}/>{errorText(problem, language)}
        </div></div>
        : snapshots.length === 0 ? <div className="empty"><div className="empty-icon"><Layers size={23}/></div>
          <h3>{t.noSnapshots}</h3><p>{t.snapHint}</p></div>
        : <>
          <div className="panel-body snapshot-filters">
            <div className="field"><label htmlFor="snapshot-search">{t.snapshotSearch}</label>
              <Input id="snapshot-search" type="search" placeholder="#123" value={query} onChange={(event) => {
                setQuery(event.currentTarget.value);
                setPageIndex(0);
              }}/></div>
            <label className="check-row"><input type="checkbox" checked={pinnedOnly} onChange={(event) => {
              setPinnedOnly(event.currentTarget.checked);
              setPageIndex(0);
            }}/>{t.snapshotPinnedOnly}</label>
          </div>
          <div className="snapshot-list">
            <div className="history-size plan-line small muted">
              <span role="status">{page.count ? t.snapshotMatches.replace("{count}", String(page.count)) : t.snapshotNoMatches}</span>
              <span>{t.snapshotPayload} · {byteSize(totalBytes, locale)}</span>
            </div>
            {page.items.map((item) => <div className="snapshot-row managed-snapshot" key={item.id}
              role="group" aria-label={`${t.ref} #${item.id}`} ref={location?.id === item.id ? locatedRow : undefined} tabIndex={-1}>
              <div className="snapshot-icon"><Layers size={18}/></div>
              <div className="snapshot-text">
                <div className="skill-labels"><strong>{t[item.reason]}</strong>
                  {item.pinned && <span className="format-tag">{t.pinnedSnapshot}</span>}
                </div>
                <small>{format.format(item.timestamp)} · #{item.id} · {byteSize(item.stored_bytes, locale)}</small>
              </div>
              <div className="row-actions snapshot-actions">
                <Button variant="ghost" size="icon" disabled={disabled || !view}
                  aria-label={`${item.pinned ? t.unpinSnapshot : t.pinSnapshot} #${item.id}`}
                  title={item.pinned ? t.unpinSnapshot : t.pinSnapshot} onClick={() => togglePin(item)}>
                  {item.pinned ? <PinOff/> : <Pin/>}
                </Button>
                <Button variant="ghost" size="sm" disabled={disabled || !view}
                  onClick={() => void onTask(() => onCompare(item.id, "compare"))}>{t.compare}</Button>
                <Button variant="ghost" size="sm" disabled={disabled || !view}
                  onClick={() => void onTask(() => onCompare(item.id, "restore"))}>{t.restore}</Button>
                <Button variant="outline" size="sm" disabled={disabled || !view || Boolean(view.issue)}
                  onClick={() => void onTask(() => onCompare(item.id, "restore_fields"))}>{t.restoreFields}</Button>
              </div>
            </div>)}
          </div>
          {page.pageCount > 1 && <div className="panel-footer spread snapshot-pagination">
            <span className="small muted" role="status">
              {t.snapshotPage.replace("{page}", String(page.index + 1)).replace("{total}", String(page.pageCount))}
            </span>
            <div className="row-actions">
              <Button variant="ghost" size="sm" aria-disabled={page.index === 0}
                onClick={() => { if (page.index > 0) setPageIndex(page.index - 1); }}>{t.snapshotPreviousPage}</Button>
              <Button variant="ghost" size="sm" aria-disabled={page.index + 1 === page.pageCount}
                onClick={() => { if (page.index + 1 < page.pageCount) setPageIndex(page.index + 1); }}>{t.snapshotNextPage}</Button>
            </div>
          </div>}
        </>}
    </section>
    <ActivityPanel key={tool} t={t} language={language} tool={tool} snapshots={snapshots} refresh={activityVersion}
      busy={busy} onLocate={locate} onTask={onTask} onNotice={onNotice}/>
  </div>;
}

function ActivityPanel({ t, language, tool, snapshots, refresh, busy, onLocate, onTask, onNotice }: {
  t: Copy;
  language: Language;
  tool: Tool;
  snapshots: Snapshot[];
  refresh: number;
  busy: boolean;
  onLocate: (id: number) => void;
  onTask: Task;
  onNotice: (message: string) => void;
}) {
  const [events, setEvents] = useState<ConfigActivity[] | null>(null);
  const [problem, setProblem] = useState<unknown>(null);
  const [retry, setRetry] = useState(0);
  const format = new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en", { dateStyle: "medium", timeStyle: "medium" });
  const snapshotIds = new Set(snapshots.map(({ id }) => id));

  useEffect(() => {
    let active = true;
    setProblem(null);
    void api.configActivity(tool).then((data) => {
      if (active) setEvents(data);
    }, (error: unknown) => {
      if (active) setProblem(error);
    });
    return () => { active = false; };
  }, [tool, snapshots, refresh, retry]);

  return <section className="panel">
    <div className="panel-head spread history-head">
      <div className="head-label"><div className="section-icon"><History size={19}/></div>
        <div><h2>{t.configActivity}</h2><p>{t.configActivityHelp}</p></div>
      </div>
      <div className="row-actions">
        <Button variant="ghost" size="icon" disabled={busy} aria-label={t.reload} title={t.reload}
          onClick={() => setRetry((value) => value + 1)}><RefreshCw/></Button>
        {!previewMode && <Button variant="outline" size="sm" disabled={busy || !events?.length || Boolean(problem)}
          onClick={() => void onTask(async () => { await api.exportConfigActivity(tool); onNotice(t.activityExported); })}>
          <ArrowUpFromLine/>{t.activityExport}
        </Button>}
      </div>
    </div>
    {problem ? <div className="panel-body"><p className="inline-message error" role="alert">
      <CircleAlert size={17}/>{errorText(problem, language)}
    </p></div>
      : events?.length ? <ul className="snapshot-list">
        {events.map((event, index) => {
          const id = event.snapshot_id;
          return <li className="snapshot-row" key={`${event.timestamp}-${event.action}-${index}`}>
            <div className="snapshot-text"><strong>{t.activityActions[event.action]}</strong>
              <small>{format.format(event.timestamp)}{id !== null ? ` · ${t.ref} #${id}` : ""}</small>
            </div>
            {id !== null && snapshotIds.has(id) && <Button variant="ghost" size="sm"
              aria-label={`${t.snapshotLocate} #${id}`} disabled={busy} onClick={() => onLocate(id)}>{t.snapshotLocate}</Button>}
            <span className={`format-tag activity-status ${event.status}`}>{t.activityStatuses[event.status]}</span>
          </li>;
        })}
      </ul> : <p className="panel-body small muted" role="status">{events === null ? t.loading : t.configActivityEmpty}</p>}
  </section>;
}

function byteSize(value: number, locale: string) {
  const format = new Intl.NumberFormat(locale, { maximumFractionDigits: 1 });
  if (value >= 1024 ** 2) return `${format.format(value / 1024 ** 2)} MiB`;
  if (value >= 1024) return `${format.format(value / 1024)} KiB`;
  return `${format.format(value)} B`;
}
