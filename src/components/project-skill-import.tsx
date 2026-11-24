import { useState } from "react";
import { ArrowDownToLine, CircleAlert, FolderOpen, RefreshCw, X } from "lucide-react";
import { api } from "../api";
import type { SkillEntry, SkillImportPreview, SkillLocation } from "../domain";
import type { Copy } from "../i18n";
import { Button } from "./ui/button";

const LOCATIONS = [
  { id: "agents", text: ".agents/skills · Codex CLI / OpenCode" },
  { id: "claude", text: ".claude/skills · Claude Code / OpenCode" },
  { id: "opencode", text: ".opencode/skills · OpenCode" },
] as const;

export function SkillImportPanel({ t, root, entries, busy, onTask, onInstalled }: {
  t: Copy;
  root: string;
  entries: SkillEntry[];
  busy: boolean;
  onTask: (action: () => Promise<void>) => Promise<void>;
  onInstalled: () => Promise<void>;
}) {
  const [location, setLocation] = useState<SkillLocation>("agents");
  const [preview, setPreview] = useState<SkillImportPreview | null>(null);
  const [saved, setSaved] = useState(false);
  const sameName = preview && entries.some((entry) => entry.name.toLowerCase() === preview.name.toLowerCase()
    && entry.path !== `${preview.destination}/SKILL.md`);

  function retarget(next: SkillLocation) {
    if (!preview) {
      setLocation(next);
      return;
    }
    void onTask(async () => {
      const data = await api.previewSkillImport(root, next);
      setLocation(next);
      setPreview(data);
    });
  }

  return <section className="panel" aria-busy={busy}>
    <div className="panel-head spread skills-head">
      <div className="head-label">
        <div className="section-icon"><ArrowDownToLine size={19}/></div>
        <div><h2>{t.skillImport}</h2><p>{t.skillImportIntro}</p></div>
      </div>
      <Button variant="outline" size="sm" disabled={busy} onClick={() => void onTask(async () => {
        setSaved(false);
        setPreview(await api.pickSkillImport(root, location));
      })}><FolderOpen/>{t.skillImportPick}</Button>
    </div>
    <div className="panel-body form-stack skills-summary">
      <div className="field">
        <label htmlFor="skill-location">{t.skillImportLocation}</label>
        <select id="skill-location" className="ui-input select" value={location} disabled={busy}
          onChange={(event) => retarget(event.currentTarget.value as SkillLocation)}>
          {LOCATIONS.map(({ id, text }) => <option key={id} value={id}>{text}</option>)}
        </select>
      </div>
      {saved && <p className="inline-message success" role="status">{t.skillImportSaved}</p>}
      {preview && <>
        <div className="path-row"><span>{t.skillImportSource}</span><code>{preview.source}</code></div>
        <div className="path-row"><span>{t.skillImportTarget}</span><code>{preview.destination}</code></div>
        {(preview.existing || sameName || preview.skipped > 0) && <div className="inline-message" role="status"><CircleAlert size={17}/>
          <div>
            {preview.existing && <p>{t.skillImportExisting}</p>}
            {sameName && <p>{t.skillImportShadow}</p>}
            {preview.skipped > 0 && <p>{preview.skipped} {t.skillImportSkipped}</p>}
            {preview.excluded.length > 0 && <details>
              <summary>{t.skillImportExcluded}</summary>
              <ul>{preview.excluded.map((path) => <li key={path}><code dir="ltr">{path}</code></li>)}</ul>
            </details>}
          </div>
        </div>}
        <p className="small muted">{t.skillImportReview}</p>
        <div className="form-stack">
          {preview.files.map((file) => <details className="skill-document" key={file.path}>
            <summary title={t.skillImportDocument}><code>{file.path}</code> <small>{file.bytes} B · SHA-256 {file.fingerprint.slice(0, 12)}</small></summary>
            <pre>{file.text}</pre>
          </details>)}
        </div>
        <div className="row-actions">
          <Button size="sm" disabled={busy || preview.existing} onClick={() => void onTask(async () => {
            await api.applySkillImport(root, location, preview.revision);
            setPreview(null);
            setSaved(true);
            await onInstalled();
          })}>{t.skillImportConfirm}</Button>
          <Button variant="outline" size="sm" disabled={busy} onClick={() => void onTask(async () => {
            setPreview(await api.previewSkillImport(root, location));
          })}><RefreshCw/>{t.planRecheck}</Button>
          <Button variant="ghost" size="icon" aria-label={t.close} disabled={busy} onClick={() => setPreview(null)}><X/></Button>
        </div>
      </>}
    </div>
  </section>;
}
