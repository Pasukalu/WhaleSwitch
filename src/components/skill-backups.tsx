import { useEffect, useState } from "react";
import { CircleAlert, History, RefreshCw, X } from "lucide-react";
import { api } from "../api";
import type { SkillBackup, SkillEditPreview } from "../domain";
import { errorText, type Copy, type Language } from "../i18n";
import { SkillEditReview } from "./skill-edit-review";
import { Button } from "./ui/button";

export function SkillBackups({ t, language, root, refreshVersion, busy, onTask, onRestored, onBackupChanged }: {
  t: Copy;
  language: Language;
  root: string;
  refreshVersion: number;
  busy: boolean;
  onTask: (action: () => Promise<void>) => Promise<void>;
  onRestored: () => Promise<void>;
  onBackupChanged: () => void;
}) {
  const [records, setRecords] = useState<SkillBackup[] | null>(null);
  const [error, setError] = useState<unknown>(null);
  const [review, setReview] = useState<{ id: number; preview: SkillEditPreview } | null>(null);
  const [saved, setSaved] = useState<number | null>(null);
  const format = new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en", { dateStyle: "medium", timeStyle: "short" });

  useEffect(() => {
    let active = true;
    setError(null);
    void api.skillBackups(root).then((data) => {
      if (active) setRecords(data);
    }, (failure: unknown) => {
      if (active) setError(failure);
    });
    return () => { active = false; };
  }, [root, refreshVersion]);

  function previewRestore(id: number) {
    void onTask(async () => {
      const preview = await api.previewSkillRestore(root, id);
      setReview({ id, preview });
      setSaved(null);
    });
  }

  function restore(selected: string[]) {
    if (!review) return;
    void onTask(async () => {
      try {
        const backup = await api.applySkillRestore(root, review.id, review.preview.revision, selected);
        setSaved(backup.id);
        setReview(null);
        await onRestored();
      } finally {
        onBackupChanged();
      }
    });
  }

  return <section className="panel" aria-busy={busy}>
    <div className="panel-head"><div className="section-icon"><History size={19}/></div>
      <div><h2>{t.skillBackups}</h2><p>{t.skillBackupsHelp}</p></div>
    </div>
    {error !== null && <div className="panel-body">
      <div className="inline-message error" role="alert"><CircleAlert size={17}/>{errorText(error, language)}</div>
    </div>}
    {saved !== null && <div className="panel-body"><p className="inline-message success" role="status">
      {t.skillRestoreSaved} · {t.skillBackupNumber} #{saved}
    </p></div>}
    {review && <div className="panel-body form-stack skills-summary">
      <div className="path-row"><span>{t.skillImportTarget}</span><code dir="ltr">{review.preview.destination}</code></div>
      <div className="row-actions">
        <span className="small muted">{t.skillBackupNumber} #{review.id}</span>
        <Button variant="ghost" size="icon" disabled={busy} aria-label={t.planRecheck} title={t.planRecheck}
          onClick={() => previewRestore(review.id)}><RefreshCw/></Button>
        <Button variant="ghost" size="icon" disabled={busy} aria-label={t.close} onClick={() => setReview(null)}><X/></Button>
      </div>
      <SkillEditReview key={review.preview.revision} t={t} preview={review.preview} action="restore" busy={busy} onApply={restore}/>
    </div>}
    {records && records.length > 0 ? <ul className="skills-list">
      {records.map((record) => <li className="skill-row" key={record.id}>
        <div className="plan-line">
          <strong dir="ltr">{record.destination}</strong>
          <Button variant="outline" size="sm" disabled={busy} onClick={() => previewRestore(record.id)}>{t.skillRestorePreview}</Button>
        </div>
        <div className="skill-labels small muted">
          <span className="format-tag">{t.skillBackupAction[record.action]}</span>
          <span>#{record.id} · {format.format(record.timestamp)} · {record.file_count} {t.skillBackupFiles}</span>
        </div>
      </li>)}
    </ul> : error === null && <p className="panel-body small muted" role="status">
      {records === null ? t.loading : t.skillBackupsEmpty}
    </p>}
  </section>;
}
