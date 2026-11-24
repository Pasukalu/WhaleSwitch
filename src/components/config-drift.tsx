import { useState } from "react";
import { CircleAlert, RefreshCw } from "lucide-react";
import { observedIssue, observedPath, type ConfigDrift } from "../config-session";
import { fieldPath, type Field } from "../domain";
import { errorText, type Copy, type Language } from "../i18n";
import { Button } from "./ui/button";

export function ConfigDriftPanel({ change, t, language, busy, onCheck, onDiscard, onMerge }: {
  change: ConfigDrift; t: Copy; language: Language; busy: boolean;
  onCheck: () => void; onDiscard: () => void; onMerge: (keep: Field[]) => void;
}) {
  const [keep, setKeep] = useState<Field[]>([]);
  const currentPath = observedPath(change.latest.config);
  const issue = observedIssue(change.latest.config);
  return <section className="panel drift-panel" aria-labelledby="drift-heading">
    <div className="panel-head"><div className="section-icon"><CircleAlert size={19}/></div>
      <div><h2 id="drift-heading">{t.driftTitle}</h2><p role="status">{t.driftReasons[change.kind]}</p></div>
    </div>
    <div className="panel-body form-stack">
      <p className="small muted">{change.canMerge ? t.driftMergeHint : t.driftBlockedHint}</p>
      {change.kind === "deleted" && change.canMerge && <p className="small muted">{t.driftRecreateHint}</p>}
      {(currentPath !== change.before.path) && <div className="drift-paths">
        <div className="path-row source-path"><span>{t.driftOldPath}</span><code>{change.before.path}</code></div>
        <div className="path-row source-path"><span>{t.driftNewPath}</span><code>{currentPath ?? t.sourceNoTarget}</code></div>
      </div>}
      {issue && <p className="small source-error">{errorText(issue, language)}</p>}
      <table className="drift-table">
        <thead><tr><th scope="col">{t.change}</th><th scope="col">{t.driftPrevious}</th>
          <th scope="col">{t.driftCurrent}</th><th scope="col">{t.driftDraft}</th></tr></thead>
        <tbody>{change.rows.map((row) => <tr key={row.field}>
          <th scope="row"><code>{fieldPath(row.field)}</code></th>
          <td><code>{row.before ?? t.none}</code></td>
          <td><code>{change.readable ? row.current ?? t.none : t.driftUnreadable}</code></td>
          <td>{row.edited ? <code>{row.draft ?? t.none}</code> : <span className="muted">{t.driftUntouched}</span>}
            {row.conflict && <label className="check-row drift-choice">
              <input type="checkbox" checked={keep.includes(row.field)} disabled={busy}
                aria-label={fieldPath(row.field) + " · " + t.driftKeepField}
                onChange={(event) => {
                  const checked = event.currentTarget.checked;
                  setKeep((previous) => checked ? [...previous, row.field] : previous.filter((field) => field !== row.field));
                }}/>
              <span>{t.driftKeepField}</span>
            </label>}
          </td>
        </tr>)}</tbody>
      </table>
    </div>
    <div className="panel-footer drift-actions">
      <Button variant="ghost" size="sm" disabled={busy} onClick={onCheck}><RefreshCw/>{t.driftCheck}</Button>
      <Button variant="outline" size="sm" disabled={busy} onClick={onDiscard}>{t.driftDiscard}</Button>
      {change.canMerge && <Button size="sm" disabled={busy} onClick={() => onMerge(keep)}>{t.driftMerge}</Button>}
    </div>
  </section>;
}
