// Settings → MCP servers → Claude Desktop (GA-340, on GA-338's contract): connect the Claude Desktop app on this computer to
// Hersir, through the hersir-mcp program, so Claude gets the Chief's tools and works by its rules (GA-341); and end that
// again. Its texts are in lib/claudeDesktop.ts. It saves at once, as McpSettings does: Save settings has nothing to do with it.
//
// Connect (and Connect again: a new key, the old one ends) adds the entry to Claude Desktop's settings when the box is on,
// or shows it to paste. What it says stays until you leave Settings (the panel is only hidden between tabs); Disconnect
// asks first and clears it, as its key no longer works.
import { useEffect, useRef, useState, type KeyboardEvent } from "react";
import { Check, CircleCheck, Copy } from "lucide-react";
import { claudeDesktopConnect, claudeDesktopDisconnect, claudeDesktopStatus } from "../api";
import { addedText, addToConfig, backupText, CD_ADD, CD_ASK_TEXT, CD_ASK_TITLE, CD_KEY, CD_NO_CONFIG, CD_NO_ENTRY, CD_NO_SHIM, CD_PASTE, CD_REOPEN,
  connectedText, missingEntry } from "../lib/claudeDesktop";
import { fullWhen } from "../lib/limits";
import type { ClaudeDesktopConnection, ClaudeDesktopStatus } from "../types";

/** Disconnect's question, the app's confirm dialog. Escape and a click beside it cancel; focus starts on Cancel and stays in
 *  the dialog. */
function DisconnectConfirm({ onDisconnect, onCancel }: { onDisconnect: () => void; onCancel: () => void }) {
  const box = useRef<HTMLDivElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  useEffect(() => { cancel.current?.focus(); }, []);
  const onKey = (e: KeyboardEvent) => {
    if (e.key === "Escape") { e.preventDefault(); e.stopPropagation(); onCancel(); return; }
    if (e.key !== "Tab" || !box.current) return;
    const all = [...box.current.querySelectorAll<HTMLElement>("button")];
    const first = all[0], last = all[all.length - 1];
    if (e.shiftKey && document.activeElement === first) { e.preventDefault(); last?.focus(); }
    else if (!e.shiftKey && document.activeElement === last) { e.preventDefault(); first?.focus(); }
  };
  return (
    <div className="ask-scrim" onMouseDown={(e) => { if (e.target === e.currentTarget) onCancel(); }}>
      <div ref={box} className="ask" role="alertdialog" aria-modal="true" aria-labelledby="cd-ask-q" aria-describedby="cd-ask-why" onKeyDown={onKey}>
        <h2 id="cd-ask-q">{CD_ASK_TITLE}</h2>
        <p id="cd-ask-why">{CD_ASK_TEXT}</p>
        <div className="ask-actions">
          <button ref={cancel} type="button" className="btn ghost" onClick={onCancel}>Cancel</button>
          <button type="button" className="btn danger" onClick={onDisconnect}>Disconnect</button>
        </div>
      </div>
    </div>
  );
}

/** What Connect answered: added to the settings file, or the entry to paste with Copy. */
function ConnectResult({ c, onCopyError }: { c: ClaudeDesktopConnection; onCopyError: (e: string | null) => void }) {
  const [copied, setCopied] = useState(false);
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  useEffect(() => () => { if (timer.current) clearTimeout(timer.current); }, []);
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(c.entry);
      onCopyError(null);
      setCopied(true);
      if (timer.current) clearTimeout(timer.current);
      timer.current = setTimeout(() => setCopied(false), 2000);
    } catch { onCopyError("Couldn't copy to the clipboard: select the entry and copy it yourself."); }
  };
  if (c.wroteConfig) {
    const backup = backupText(c.backupPath);
    return (
      <div className="cd-result" role="status">
        <span className="cd-line ok"><CircleCheck className="icon" /><span>{addedText(c.configPath ?? c.status.configPath)}</span></span>
        {backup && <span>{backup}</span>}
        <span>{CD_REOPEN}</span>
      </div>
    );
  }
  return (
    <div className="cd-result" role="status">
      {c.configError && <span className="warn">{c.configError}</span>}
      <div className="cd-entry">
        <pre className="mono" tabIndex={0} aria-label="The entry for Claude Desktop's settings">{c.entry}</pre>
        <button type="button" className="btn sm" aria-label="Copy the entry" onClick={copy}>
          {copied ? <><Check className="icon" />Copied</> : <><Copy className="icon" />Copy</>}</button>
      </div>
      <span>{CD_PASTE}</span>
      <span className="hint">{CD_KEY}</span>
    </div>
  );
}

export function ClaudeDesktopSettings() {
  const [st, setSt] = useState<ClaudeDesktopStatus | null>(null);
  // the box as you set it; null: untouched, so on when the settings file was found
  const [ticked, setTicked] = useState<boolean | null>(null);
  const [result, setResult] = useState<ClaudeDesktopConnection | null>(null);
  const [busy, setBusy] = useState<"" | "load" | "connect" | "disconnect">("load");
  const [err, setErr] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  // counts the commands: a status asked for before one of them answered is older than its answer, so it is dropped
  const seq = useRef(0);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);

  /** Asks for the status again. Connect's answer stays only while the connection it made is still the one. */
  const refresh = async () => {
    const mine = seq.current;
    try {
      const s = await claudeDesktopStatus();
      if (!alive.current || mine !== seq.current) return;
      setSt(s);
      setResult((r) => (r && s.connected && s.connectedAt === r.status.connectedAt ? r : null));
    } catch (e) { if (alive.current && mine === seq.current) setErr(String(e)); }
  };
  const load = async () => {
    setBusy("load");
    setErr(null);
    await refresh();
    if (alive.current) setBusy("");
  };
  useEffect(() => { void load(); }, []);
  // back from Claude Desktop (an entry pasted there): whether its settings have the entry now
  useEffect(() => {
    const onFocus = () => { if (!busy) void refresh(); };
    window.addEventListener("focus", onFocus);
    return () => window.removeEventListener("focus", onFocus);
  }, [busy]);

  if (!st) {
    return (
      <div className="field wide cd" role="group" aria-label="Claude Desktop">
        {err ? <>
          <span className="warn" role="alert">{err}</span>
          <div><button type="button" className="btn sm" disabled={busy === "load"} onClick={() => void load()}>{busy === "load" ? "Trying…" : "Try again"}</button></div>
        </> : <span className="faint">Checking Claude Desktop…</span>}
      </div>
    );
  }

  const add = addToConfig(st, ticked);
  const connect = async () => {
    const mine = ++seq.current;
    setBusy("connect");
    setErr(null);
    try {
      const c = await claudeDesktopConnect(add);
      if (!alive.current) return;
      setResult(c);
      setSt(c.status);
    } catch (e) {
      if (!alive.current) return;
      setErr(String(e));
      // what is connected now: Connect's answer only stays when its connection still is
      if (mine === seq.current) await refresh();
    }
    if (alive.current) setBusy("");
  };
  const disconnect = async () => {
    setAsking(false);
    ++seq.current;
    setBusy("disconnect");
    setErr(null);
    try {
      const s = await claudeDesktopDisconnect();
      if (!alive.current) return;
      setSt(s);
      setResult(null);
    } catch (e) {
      if (!alive.current) return;
      setErr(String(e));
      await refresh();
    }
    if (alive.current) setBusy("");
  };

  const noShim = !st.shimPath;
  return (
    <div className="field wide cd" role="group" aria-label="Claude Desktop">
      {st.connected && (
        <span className="cd-line ok" title={st.connectedAt != null ? fullWhen(st.connectedAt) : undefined}>
          <CircleCheck className="icon" /><span>{connectedText(st)}</span></span>
      )}
      {st.connected && missingEntry(st) && <span className="warn">{CD_NO_ENTRY}</span>}
      <div className="cd-check">
        <label className="check"><input type="checkbox" checked={add} disabled={!st.configPath || !!busy}
          onChange={(e) => setTicked(e.target.checked)} />{CD_ADD}</label>
        {!st.configPath && <span className="hint">{CD_NO_CONFIG}</span>}
      </div>
      <div className="cd-actions">
        {st.connected ? <>
          <button type="button" className="btn" disabled={noShim || !!busy} title={noShim ? CD_NO_SHIM : undefined} onClick={() => void connect()}>
            {busy === "connect" ? "Connecting…" : "Connect again"}</button>
          <button type="button" className="btn ghost" disabled={!!busy} onClick={() => setAsking(true)}>
            {busy === "disconnect" ? "Disconnecting…" : "Disconnect"}</button>
        </> : (
          <button type="button" className="btn primary" disabled={noShim || !!busy} title={noShim ? CD_NO_SHIM : undefined} onClick={() => void connect()}>
            {busy === "connect" ? "Connecting…" : "Connect"}</button>
        )}
      </div>
      {noShim && <span className="warn">{CD_NO_SHIM}</span>}
      {result && <ConnectResult key={result.entry} c={result} onCopyError={setErr} />}
      {err && <span className="warn" role="alert">{err}</span>}
      {asking && <DisconnectConfirm onDisconnect={() => void disconnect()} onCancel={() => setAsking(false)} />}
    </div>
  );
}
