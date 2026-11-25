import { useState } from "react";
import type { SkillEditPreview, SkillTextChange } from "../domain";
import type { Copy } from "../i18n";
import { Button } from "./ui/button";

export function SkillEditReview({ t, preview, action, busy, onApply }: {
  t: Copy;
  preview: SkillEditPreview;
  action: "upgrade" | "restore";
  busy: boolean;
  onApply: (selected: string[]) => void;
}) {
  const [selected, setSelected] = useState<string[]>([]);
  if (preview.changes.length === 0) {
    return <p className="small muted" role="status">{t.skillEditNoChanges}</p>;
  }
  return <div className="form-stack">
    <p className="small muted">{t.skillEditSelect}</p>
    {preview.changes.map((change) => <SkillChangeRow key={change.path} t={t} change={change}
      checked={selected.includes(change.path)} busy={busy}
      onCheck={(checked) => setSelected((paths) => checked
        ? [...paths, change.path] : paths.filter((path) => path !== change.path))}/>)}
    <div className="row-actions">
      <Button size="sm" disabled={busy || selected.length === 0} onClick={() => onApply(selected)}>
        {action === "upgrade" ? t.skillUpgradeApply : t.skillRestoreApply} · {selected.length}
      </Button>
    </div>
  </div>;
}

function SkillChangeRow({ t, change, checked, busy, onCheck }: {
  t: Copy;
  change: SkillTextChange;
  checked: boolean;
  busy: boolean;
  onCheck: (checked: boolean) => void;
}) {
  const kind = change.after === null ? "removed" : change.before === null ? "added" : "modified";
  return <div className="skill-document skill-edit">
    <label className="check-row">
      <input type="checkbox" checked={checked} disabled={busy || change.blocked}
        onChange={(event) => onCheck(event.currentTarget.checked)}/>
      <code dir="ltr">{change.path}</code>
      <span className="format-tag">{t.skillsChangeKind[kind]}</span>
    </label>
    {change.blocked && <p className="small muted">{t.skillRestoreBlocked}</p>}
    <details>
      <summary>{t.review}</summary>
      <div className="skill-comparison">
        <div><small>{t.current}</small><pre>{change.before ?? t.skillFileMissing}</pre></div>
        <div><small>{t.next}</small><pre>{change.after ?? t.skillFileMissing}</pre></div>
      </div>
    </details>
  </div>;
}
