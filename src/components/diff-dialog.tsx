import { useEffect, useRef, type ReactNode } from "react";
import { ArrowRight, X } from "lucide-react";
import { fieldPath, type Change } from "../domain";
import type { Copy } from "../i18n";
import { Button } from "./ui/button";

export function DiffDialog({ t, changes, title, hint, action, onClose, busy, error }: {
  t: Copy; changes: Change[]; title: string; hint?: string; action?: ReactNode;
  onClose: () => void; busy: boolean; error?: string;
}) {
  const dialog = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const element = dialog.current;
    element?.showModal();
    return () => { if (element?.open) element.close(); };
  }, []);
  return <dialog ref={dialog} aria-labelledby="diff-title" className="diff-dialog" onCancel={(event) => { event.preventDefault(); if (!busy) onClose(); }}>
    <div className="dialog-heading">
      <div><h2 id="diff-title">{title}</h2>{hint && <p>{hint}</p>}</div>
      <Button variant="ghost" size="icon" aria-label={t.close} onClick={onClose} disabled={busy}><X/></Button>
    </div>
    {changes.length ? <div className="diff-list">
      {changes.map(({ field, before, after }) => <div className="diff-row" key={field}>
        <code>{fieldPath(field)}</code>
        <div className="diff-values"><code aria-label={t.current} className="before">{before ?? t.none}</code><ArrowRight size={15}/><code aria-label={t.next} className="after">{after ?? t.none}</code></div>
      </div>)}
    </div> : <p className="dialog-empty">{t.noChanges}</p>}
    {error && <p className="dialog-error inline-message error" role="alert">{error}</p>}
    <div className="dialog-actions"><Button autoFocus variant="outline" onClick={onClose} disabled={busy}>{t.cancel}</Button>{action}</div>
  </dialog>;
}
