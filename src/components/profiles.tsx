import { useEffect, useRef, useState } from "react";
import { Bookmark, ChevronRight, Copy as CopyIcon, Pencil, Plus, Trash2, X } from "lucide-react";
import type { Profile } from "../domain";
import type { Copy, Language } from "../i18n";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

export type ProfileEdit = "rename" | "duplicate";

export function Profiles({ t, language, profiles, available, dirty, busy, loading, error, onCreate, onApply, onRemove, onEdit }: {
  t: Copy; language: Language; profiles: Profile[]; available: boolean; dirty: boolean; busy: boolean; loading: boolean; error: string | null;
  onCreate: (name: string) => Promise<boolean>; onApply: (profile: Profile) => void; onRemove: (profile: Profile) => void;
  onEdit: (kind: ProfileEdit, profile: Profile, name: string) => Promise<boolean>;
}) {
  const [name, setName] = useState("");
  const [editing, setEditing] = useState<{ kind: ProfileEdit; profile: Profile } | null>(null);
  const nameInput = useRef<HTMLInputElement>(null);
  const formatter = new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en", { dateStyle: "medium", timeStyle: "short" });
  const enabled = (available || editing !== null) && !busy && !loading;
  const action = editing?.kind === "rename" ? t.profileRename : editing?.kind === "duplicate" ? t.profileDuplicate
    : dirty ? t.profileSaveDraft : t.profileSaveCurrent;

  useEffect(() => {
    if (editing) nameInput.current?.focus();
  }, [editing]);

  function start(kind: ProfileEdit, profile: Profile) {
    setEditing({ kind, profile });
    setName(kind === "rename" ? profile.name : "");
  }

  function reset() {
    setEditing(null);
    setName("");
  }
  return <section className="panel">
    <div className="panel-head">
      <div className="section-icon"><Bookmark size={19}/></div>
      <div><h2>{t.profiles}</h2><p>{t.profilesIntro}</p></div>
    </div>
    {error && <div className="panel-body"><p className="inline-message error" role="alert">{error}</p></div>}
    <form className="panel-body profile-form" onSubmit={(event) => {
      event.preventDefault();
      if (!enabled || !name.trim()) return;
      const saving = editing ? onEdit(editing.kind, editing.profile, name) : onCreate(name);
      void saving.then((saved) => { if (saved) reset(); });
    }}>
      <div className="field">
        <label htmlFor="profile-name">{editing ? `${action} · ${editing.profile.name}` : t.profileName}</label>
        <Input ref={nameInput} id="profile-name" maxLength={64} value={name} disabled={!enabled} placeholder={t.profileExample} onChange={(e) => setName(e.target.value)}/>
      </div>
      <div className="row-actions">
        {editing && <Button variant="ghost" size="icon" disabled={busy} aria-label={t.cancel} onClick={reset}><X/></Button>}
        <Button type="submit" variant="outline" disabled={!enabled || !name.trim()}>
          {editing?.kind === "rename" ? <Pencil/> : editing ? <CopyIcon/> : <Plus/>}{action}
        </Button>
      </div>
    </form>
    {!profiles.length && !error && <p className="profile-empty muted">{loading ? t.loading : t.noProfiles}</p>}
    {profiles.length > 0 && <div className="snapshot-list profiles-list">
      {profiles.map((profile) => <div className="snapshot-row" key={profile.id}>
        <div className="snapshot-icon"><Bookmark size={16}/></div>
        <div className="snapshot-text"><strong title={profile.name}>{profile.name}</strong><small>{formatter.format(profile.timestamp)}</small></div>
        <Button variant="outline" size="sm" disabled={busy || !available} onClick={() => onApply(profile)}>{t.review}<ChevronRight/></Button>
        <Button variant="ghost" size="icon" disabled={busy} title={t.profileRename} aria-label={t.profileRename} onClick={() => start("rename", profile)}><Pencil/></Button>
        <Button variant="ghost" size="icon" disabled={busy} title={t.profileDuplicate} aria-label={t.profileDuplicate} onClick={() => start("duplicate", profile)}><CopyIcon/></Button>
        <Button variant="ghost" size="icon" disabled={busy} title={t.profileDelete} aria-label={t.profileDelete} onClick={() => onRemove(profile)}><Trash2/></Button>
      </div>)}
    </div>}
  </section>;
}
