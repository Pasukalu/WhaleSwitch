import { useEffect, useRef, useState } from "react";
import { Activity, CircleCheck, KeyRound, ShieldCheck, TriangleAlert } from "lucide-react";
import { api, previewMode } from "../api";
import { DEFAULT_ENDPOINT, initialProbe, type ProbeResult, type Protocol, type Tool, type View } from "../domain";
import { errorText, type Copy, type Language } from "../i18n";
import { Button } from "./ui/button";
import { Input } from "./ui/input";

export function Connection({ t, language, view, tool }: { t: Copy; language: Language; view: View | null; tool: Tool }) {
  const [input, setInput] = useState(() => initialProbe(view, tool));
  const [keyPresent, setKeyPresent] = useState<boolean | null>(null);
  const [status, setStatus] = useState<{ message: string; error: boolean } | null>(null);
  const [result, setResult] = useState<ProbeResult | null>(null);
  const [busy, setBusy] = useState(false);
  const [eligible, setEligible] = useState(false);
  const key = useRef<HTMLInputElement>(null);

  useEffect(() => {
    let active = true;
    if (!input.endpoint || previewMode || input.anonymous) return;
    const timer = window.setTimeout(() => {
      void api.hasKey(input).then(
        (present) => { if (active) setKeyPresent(present); },
        () => { if (active) setKeyPresent(null); },
      );
    }, 180);
    return () => { active = false; window.clearTimeout(timer); };
  }, [input]);

  function update(protocol: Protocol, endpoint: string, anonymous = Boolean(input.anonymous)) {
    setInput({ ...input, protocol, endpoint, anonymous });
    setResult(null);
    setStatus(null);
    setKeyPresent(null);
    setEligible(false);
    if (key.current) key.current.value = "";
  }

  async function run(task: () => Promise<void>) {
    setBusy(true); setStatus(null); setResult(null);
    try { await task(); }
    catch (error) { setStatus({ message: errorText(error, language), error: true }); }
    finally { setBusy(false); }
  }

  return <section className="panel">
    <div className="panel-head"><div className="section-icon"><Activity size={19}/></div><div><h2>{t.diagnostic}</h2><p>{t.keyHelp}</p></div></div>
    <div className="panel-body form-stack">
      {previewMode && <div className="inline-message"><ShieldCheck size={17}/>{t.demoProbe}</div>}
      <div className="field">
        <label htmlFor="protocol">{t.protocol}</label>
        <select id="protocol" className="ui-input select" value={input.protocol} disabled={busy || input.anonymous} onChange={(e) => {
          const protocol = e.target.value as Protocol;
          update(protocol, DEFAULT_ENDPOINT[protocol]);
        }}>
          <option value="openai">OpenAI / OpenAI-compatible</option>
          <option value="anthropic">Anthropic</option>
        </select>
      </div>
      <div className="field">
        <label htmlFor="endpoint">{t.endpoint}</label>
        <Input id="endpoint" className="mono" spellCheck={false} value={input.endpoint} disabled={busy} maxLength={512} placeholder="https://api.example.com/v1" onChange={(e) => update(input.protocol, e.target.value)}/>
      </div>
      <label className="check-row">
        <input type="checkbox" checked={Boolean(input.anonymous)} disabled={busy} onChange={(event) => {
          const enabled = event.target.checked;
          update(enabled ? "openai" : input.protocol, enabled ? "http://127.0.0.1:11434/v1" : input.endpoint, enabled);
        }}/>
        {t.localProbe}
      </label>
      {!input.anonymous && <><div className="field">
        <label htmlFor="api-key">{t.key}</label>
        <Input ref={key} id="api-key" type="password" autoComplete="off" maxLength={1024} disabled={busy || previewMode} placeholder={t.keyPlaceholder} onChange={(e) => setEligible(e.target.value.length >= 8)}/>
        <small>{keyPresent === null ? "" : keyPresent ? t.keySaved : t.keyEmpty}</small>
      </div>
      <div className="row-actions">
        <Button variant="outline" disabled={busy || previewMode || !eligible || !input.endpoint} onClick={() => void run(async () => {
          const secret = key.current?.value ?? "";
          if (key.current) key.current.value = "";
          setEligible(false);
          await api.setKey(input, secret);
          setKeyPresent(true);
          setStatus({ message: t.stored, error: false });
        })}><KeyRound/>{t.storeKey}</Button>
        <Button variant="ghost" disabled={busy || !keyPresent} onClick={() => void run(async () => {
          await api.removeKey(input);
          setKeyPresent(false);
          setStatus({ message: t.removed, error: false });
        })}>{t.removeKey}</Button>
      </div></>}
      {status && <div role={status.error ? "alert" : "status"} className={status.error ? "inline-message error" : "inline-message success"}>{status.error ? <TriangleAlert size={17}/> : <CircleCheck size={17}/>} {status.message}</div>}
      {result && <div role="status" className={result.outcome === "connected" ? "inline-message success" : "inline-message error"}>
        {result.outcome === "connected" ? <CircleCheck size={17}/> : <TriangleAlert size={17}/>}
        <span>{t[result.outcome]} · {result.status ? `HTTP ${result.status} · ` : ""}{result.elapsed_ms} ms{result.models === null ? "" : ` · ${result.models} ${t.models}`}</span>
      </div>}
    </div>
    <div className="panel-footer"><Button disabled={busy || previewMode || !input.endpoint} onClick={() => void run(async () => setResult(await api.probe(input)))}><Activity/>{busy ? t.testing : t.test}</Button></div>
  </section>;
}
