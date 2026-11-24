import { useState } from "react";
import { BookOpen, CircleAlert, FolderOpen } from "lucide-react";
import { api, previewMode } from "../api";
import { TOOL_NAMES, type SkillInventory } from "../domain";
import { errorText, type Copy, type Language } from "../i18n";
import { Button } from "./ui/button";

export function ProjectSkills({ t, language }: { t: Copy; language: Language }) {
  const [inventory, setInventory] = useState<SkillInventory | null>(null);
  const [scanning, setScanning] = useState(false);
  const [error, setError] = useState<unknown>(null);

  async function scan() {
    setScanning(true);
    setError(null);
    try {
      setInventory(await api.pickProjectSkills());
    } catch (nextError) {
      if (nextError !== "cancelled") setError(nextError);
    } finally {
      setScanning(false);
    }
  }

  const emptyMessage = previewMode ? t.skillsNative
    : scanning ? t.skillsScanning
    : inventory ? t.skillsEmpty : t.skillsStart;

  return <section className="panel" aria-busy={scanning}>
    <div className="panel-head spread skills-head">
      <div className="head-label">
        <div className="section-icon"><BookOpen size={19}/></div>
        <div><h2>{t.skills}</h2><p>{t.skillsIntro}</p></div>
      </div>
      {!previewMode && <Button size="sm" variant="outline" disabled={scanning} onClick={() => void scan()}>
        <FolderOpen/>{scanning ? t.skillsScanning : t.skillsSelect}
      </Button>}
    </div>
    {error !== null && <div className="panel-body"><div className="inline-message error" role="alert">
      <CircleAlert size={17}/>{errorText(error, language)}
    </div></div>}
    {inventory && <div className="panel-body skills-summary">
      <div className="path-row"><span>{t.skillsFolder}</span><code>{inventory.root}</code></div>
      <p className="small muted" role="status">{inventory.entries.length} {t.skillsCount}</p>
      {(inventory.skipped > 0 || inventory.limited) && <div className="inline-message" role="status"><CircleAlert size={17}/>
        <div>{inventory.skipped > 0 && <p>{inventory.skipped} {t.skillsSkipped}</p>}{inventory.limited && <p>{t.skillsLimited}</p>}</div>
      </div>}
      <p className="small muted">{t.skillsScope}</p>
    </div>}
    {inventory && inventory.entries.length > 0 ? <ul className="skills-list">
      {inventory.entries.map((entry) => <li className="skill-row" key={entry.path}>
        <strong dir="auto">{entry.name}</strong><code dir="ltr">{entry.path}</code>
        <div className="skill-labels">
          {entry.tools.map((tool) => <span className="format-tag" key={tool}>{TOOL_NAMES[tool]}</span>)}
          {entry.same_name && <span className="format-tag skill-match">{t.skillsSameName}</span>}
          <code className="skill-fingerprint" title={`${t.skillsFingerprint} ${entry.fingerprint}`}>SHA-256 {entry.fingerprint.slice(0, 12)}</code>
        </div>
      </li>)}
    </ul> : <div className="empty" role="status">
      <div className="empty-icon"><BookOpen size={22}/></div><h3>{emptyMessage}</h3><p>{t.skillsLocationHint}</p>
    </div>}
  </section>;
}
