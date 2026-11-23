import { useState } from "react";
import { Bookmark, ChevronRight, Plus, Trash2 } from "lucide-react";
import type { Profile } from "../domain";
import type { Copy, Language } from "../i18n";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

export function Profiles({ t, language, profiles, available, dirty, busy, loading, error, onCreate, onApply, onRemove }: {
  t: Copy; language: Language; profiles: Profile[]; available: boolean; dirty: boolean; busy: boolean; loading: boolean; error: string | null;
  onCreate: (name: string) => Promise<boolean>; onApply: (profile: Profile) => void; onRemove: (profile: Profile) => void;
}) {
  const [name, setName] = useState("");
  const formatter = new Intl.DateTimeFormat(language === "zh" ? "zh-CN" : "en", { dateStyle: "medium", timeStyle: "short" });
  return <section className="panel">
    <div className="panel-head">
      <div className="section-icon"><Bookmark size={19}/></div>
      <div><h2>{t.profiles}</h2><p>{t.profilesIntro}</p></div>
    </div>
    {error && <div className="panel-body"><p className="inline-message error" role="alert">{error}</p></div>}
    <form className="panel-body profile-form" onSubmit={(event) => {
      event.preventDefault();
      if (busy || loading || !available || !name.trim()) return;
      void onCreate(name).then((created) => { if (created) setName(""); });
    }}>
      <div className="field">
        <label htmlFor="profile-name">{t.profileName}</label>
        <Input id="profile-name" maxLength={64} value={name} disabled={busy || loading || !available} placeholder={t.profileExample} onChange={(e) => setName(e.target.value)}/>
      </div>
      <Button type="submit" variant="outline" disabled={!available || busy || loading || !name.trim()}>
        <Plus/>{dirty ? t.profileSaveDraft : t.profileSaveCurrent}
      </Button>
    </form>
    {!profiles.length && !error && <p className="profile-empty muted">{loading ? t.loading : t.noProfiles}</p>}
    {profiles.length > 0 && <div className="snapshot-list profiles-list">
      {profiles.map((profile) => <div className="snapshot-row" key={profile.id}>
        <div className="snapshot-icon"><Bookmark size={16}/></div>
        <div className="snapshot-text"><strong title={profile.name}>{profile.name}</strong><small>{formatter.format(profile.timestamp)}</small></div>
        <Button variant="outline" size="sm" disabled={busy || !available} onClick={() => onApply(profile)}>{t.review}<ChevronRight/></Button>
        <Button variant="ghost" size="icon" disabled={busy} title={t.profileDelete} aria-label={t.profileDelete} onClick={() => onRemove(profile)}><Trash2/></Button>
      </div>)}
    </div>}
  </section>;
}
