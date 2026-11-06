// The restore's notice above Company in the sidebar, where the update's goes (GA-219). While a restore runs it says its
// step, and a restore that stopped says so: both link to Settings → General, which has Backups. Ready: "Restore ready: <time
// of the backup>" with Restart now and Cancel restore. After the start: how it went, until Dismiss.
import { useState } from "react";
import { CircleCheck, CircleX, DatabaseBackup, LoaderCircle, TriangleAlert } from "lucide-react";
import { cancelRestore, dismissRestoreResult, restartToRestore } from "../api";
import { href } from "../router";
import { isRestoring, readyText, restoreStep, resultText } from "../lib/restore";
import { useRestore } from "../lib/useRestore";
import { RestartButton } from "./UpdateNotice";

export function RestoreNotice() {
  const s = useRestore();
  const [err, setErr] = useState<string | null>(null);
  if (!s) return null;
  const settings = href({ page: "settings" }); // opens Settings → General, which has Backups
  const now = Date.now();
  const fail = (e: unknown) => setErr(String(e));
  const r = s.result;
  return (
    <>
      {r && (
        <div className="update-box" aria-label="Restore result">
          <div className={`restore-note ${r.ok ? "ok" : "failed"}`} role={r.ok ? "status" : "alert"}>
            {r.ok ? <CircleCheck className="icon" /> : <CircleX className="icon" />}<span>{resultText(r, now)}</span>
          </div>
          <button type="button" className="btn sm ghost" onClick={() => { setErr(null); dismissRestoreResult().catch(fail); }}>Dismiss</button>
        </div>
      )}
      {isRestoring(s) && (
        <div className="update-box" aria-label="Restore">
          <a className="update-notice working" href={settings} title="Hersir stays usable meanwhile. Settings shows more."><LoaderCircle className="icon spin" /><span>{restoreStep(s)}</span></a>
        </div>
      )}
      {s.state === "failed" && (
        <div className="update-box" aria-label="Restore">
          <a className="update-notice failed" href={settings} title={s.error ?? undefined}><TriangleAlert className="icon" /><span>The restore failed</span></a>
        </div>
      )}
      {s.state === "ready" && (
        <div className="update-box" aria-label="Restore">
          <div className="restore-note" role="status"><DatabaseBackup className="icon" /><span>{readyText(s, now)}</span></div>
          <RestartButton className="update-notice ready" label="Restart now" restart={restartToRestore} />
          <button type="button" className="btn sm ghost" onClick={() => { setErr(null); cancelRestore().catch(fail); }}>Cancel restore</button>
        </div>
      )}
      {err && <div className="update-error" role="alert">{err}</div>}
    </>
  );
}
