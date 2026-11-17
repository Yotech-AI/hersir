import { useEffect, useState } from "react";
import { FolderOpen, Info } from "lucide-react";
import { dismissUnexpectedEnd, getLogsInfo, openLogsFolder } from "../api";
import type { LogsInfo } from "../types";
import { logName, unexpectedEndText } from "../lib/logs";

type End = NonNullable<LogsInfo["unexpectedEnd"]>;

/** GA-282: when the previous Hersir ended without quitting (GA-281 notices it), a notice above the page at start names
 *  its log, with Open log folder and Dismiss. Dismissed, it is gone, also after a restart (dismiss_unexpected_end).
 *  Without an unexpected end, or when Hersir can't say, it shows nothing. In the neutral note's colours, not gold:
 *  nothing needs doing. */
export function UnexpectedEnd() {
  const [end, setEnd] = useState<End | null>(null);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    let live = true;
    // in an async function, so any failure (no such command, an error) only means no notice
    (async () => {
      const info = await getLogsInfo();
      if (live) setEnd(info?.unexpectedEnd ?? null);
    })().catch(() => {});
    return () => { live = false; };
  }, []);
  if (!end) return null;
  const open = async () => {
    setErr(null);
    try { await openLogsFolder(); } catch (e) { setErr(String(e)); }
  };
  // it goes at once; when Hersir couldn't dismiss it, it comes back with why
  const dismiss = async () => {
    const was = end;
    setErr(null);
    setEnd(null);
    try { await dismissUnexpectedEnd(); } catch (e) { setEnd(was); setErr(String(e)); }
  };
  return (
    <div className="info-banner unexpected-end" role="status">
      <Info className="icon" />
      <span className="ib-text">{unexpectedEndText(end.lastWrittenAt)}<span className="mono" title={end.file}>{logName(end.file)}</span>
        {err && <span className="ib-error" role="alert">{err}</span>}</span>
      <span className="ib-actions">
        <button type="button" className="btn sm" onClick={open}><FolderOpen className="icon" />Open log folder</button>
        <button type="button" className="btn sm ghost" onClick={dismiss}>Dismiss</button>
      </span>
    </div>
  );
}
