// Restore from Hersir Cloud (GA-360, GA-317's sheet A, A9), in Settings → Hersir Cloud and on the first screen: three
// numbered steps, Sign in (cloudSignIn), Your recovery code (cloudEnterRecovery, for a computer whose key is "needed") and
// Pick a backup (cloudListBackups, newest first, each with the computer that made it). Restore asks first, as a folder's
// backup does; cloudStartRestore downloads it, then GA-218's restore takes over with its steps, Restart now and Cancel
// restore. The parts the Hersir Cloud tab shares are here too: the ask, the recovery code's box, the backups' rows and the
// restore's progress.
import { useEffect, useRef, useState, type KeyboardEvent, type ReactNode } from "react";
import { ArchiveRestore, Check, CircleX, Copy, Download, LoaderCircle } from "lucide-react";
import { cancelRestore, cloudCancelSignIn, cloudEnterRecovery, cloudListBackups, cloudSignIn, cloudStartRestore, restartToRestore } from "../api";
import { backupProblem, backupWhat, CODE_FILE_NAME, CODE_PLACEHOLDER, cloudConfirmText, cloudStepText, codeFileText, isDownloadStep, newestFirst,
  PICK_BEFORE, PICK_HINT, RESTORE_SIGN_IN_HINT, WAITING, when } from "../lib/cloud";
import { formatBytes } from "../lib/format";
import { fullWhen } from "../lib/limits";
import { failedText, isRestoring, readyText, restoreStep } from "../lib/restore";
import { useRestore } from "../lib/useRestore";
import type { CloudBackup, CloudStatus, RestoreStatus } from "../types";
import { RestartButton } from "./UpdateNotice";

/** A question in BackupSettings' ask: what will happen, then `action` or Cancel. Escape and a click beside it cancel; focus
 *  starts on Cancel and stays in the dialog. */
export function CloudAsk({ id, title, text, action, danger, onAction, onCancel }: {
  id: string; title: string; text: string; action: string; danger?: boolean; onAction: () => void; onCancel: () => void;
}) {
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
      <div ref={box} className="ask bk-ask" role="alertdialog" aria-modal="true" aria-labelledby={`${id}-q`} aria-describedby={`${id}-why`} onKeyDown={onKey}>
        <h2 id={`${id}-q`}>{title}</h2>
        <p id={`${id}-why`}>{text}</p>
        <div className="ask-actions">
          <button ref={cancel} type="button" className="btn ghost" onClick={onCancel}>Cancel</button>
          <button type="button" className={danger ? "btn danger" : "btn primary"} onClick={onAction}>{action}</button>
        </div>
      </div>
    </div>
  );
}

/** The recovery code in mono on `raised`, with Copy (then "Copied") and Save as a file… (a text file with the code and the
 *  warning, through the browser's download). */
export function CodeBox({ code, email }: { code: string; email: string | null }) {
  const [copied, setCopied] = useState(false);
  const [err, setErr] = useState<string | null>(null);
  useEffect(() => {
    if (!copied) return;
    const t = setTimeout(() => setCopied(false), 2000);
    return () => clearTimeout(t);
  }, [copied]);
  const copy = () => {
    setErr(null);
    Promise.resolve().then(() => navigator.clipboard.writeText(code)).then(() => setCopied(true)).catch((e) => setErr(String(e)));
  };
  const saveFile = () => {
    setErr(null);
    try {
      const url = URL.createObjectURL(new Blob([codeFileText(code, email)], { type: "text/plain;charset=utf-8" }));
      const a = document.createElement("a");
      a.href = url;
      a.download = CODE_FILE_NAME;
      document.body.appendChild(a);
      a.click();
      a.remove();
      setTimeout(() => URL.revokeObjectURL(url), 1000);
    } catch (e) { setErr(String(e)); }
  };
  return (
    <div className="cloud-code">
      <span className="cloud-code-text mono" aria-label="Recovery code">{code}</span>
      <div className="bk-actions">
        <button type="button" className="btn sm" onClick={copy}><Copy className="icon" />{copied ? "Copied" : "Copy"}</button>
        <button type="button" className="btn sm" onClick={saveFile}><Download className="icon" />Save as a file…</button>
      </div>
      {err && <span className="error">{err}</span>}
    </div>
  );
}

/** The backups in Hersir Cloud, as BackupSettings' rows: when, why and which computer, size, Restore or why it can't. */
export function CloudBackupRows({ list, now, disabled, label, onRestore }: {
  list: CloudBackup[]; now: number; disabled: boolean; label: string; onRestore: (b: CloudBackup) => void;
}) {
  return (
    <ul className="backup-list" aria-label={label}>
      {newestFirst(list).map((b) => {
        const problem = backupProblem(b);
        const at = when(b.madeAt, now);
        return (
          <li key={b.name}>
            <div className="bk-row">
              <span className="bk-when" title={fullWhen(b.madeAt)}>{at}</span>
              <span className="bk-what">{backupWhat(b)}</span>
              <span className="bk-size mono">{formatBytes(b.bytes)}</span>
              <span className="bk-do">
                {!problem && <button type="button" className="btn sm" aria-label={`Restore the backup of ${at}`} disabled={disabled}
                  onClick={() => onRestore(b)}><ArchiveRestore className="icon" />Restore</button>}
              </span>
            </div>
            {problem && <span className="warn">{problem}</span>}
          </li>
        );
      })}
    </ul>
  );
}

/** Whether a restore shows its progress: a cloud backup downloading or being checked, GA-218's steps, ready or failed. */
export const showsRestore = (s: CloudStatus | null, r: RestoreStatus | null) =>
  isDownloadStep(s?.backup.running?.step) || isRestoring(r) || r?.state === "ready" || r?.state === "failed";

/** The restore once it started: the download's step, then GA-218's steps, its failure with Dismiss, or the ready line with
 *  Restart now and Cancel restore. */
export function RestoreProgress({ status, restore, now, onError }: {
  status: CloudStatus | null; restore: RestoreStatus | null; now: number; onError: (e: string | null) => void;
}) {
  const cancel = () => { onError(null); cancelRestore().catch((e) => onError(String(e))); };
  const running = status?.backup.running;
  if (running && isDownloadStep(running.step)) {
    return (
      <div className="bk-actions">
        <span className="gh-line bk-step" role="status"><LoaderCircle className="icon spin" /><span>{cloudStepText(running)}</span></span>
        <button type="button" className="btn sm ghost" onClick={cancel}>Cancel restore</button>
      </div>
    );
  }
  if (restore && isRestoring(restore)) {
    return (
      <div className="bk-actions">
        <span className="gh-line bk-step" role="status"><LoaderCircle className="icon spin" /><span>{restoreStep(restore)}</span></span>
        <button type="button" className="btn sm ghost" onClick={cancel}>Cancel restore</button>
      </div>
    );
  }
  if (restore?.state === "failed") {
    return (
      <div className="bk-actions">
        <div className="gh-line failed" role="alert"><CircleX className="icon" /><span className="bk-error">{failedText(restore.error)}</span></div>
        <button type="button" className="btn sm ghost" onClick={cancel}>Dismiss</button>
      </div>
    );
  }
  if (restore?.state === "ready") {
    return (
      <div className="bk-restore">
        <span className="bk-ready" role="status">{readyText(restore, now)}</span>
        <div className="bk-actions">
          <RestartButton className="btn primary" label="Restart now" restart={restartToRestore} />
          <button type="button" className="btn ghost" onClick={cancel}>Cancel restore</button>
        </div>
      </div>
    );
  }
  return null;
}

type StepState = "done" | "on" | "later";

/** One numbered step: its number on `accent` while it is the current one, a `success` check once done, `line-2` later. */
function Step({ n, state, title, htmlFor, children }: { n: number; state: StepState; title: string; htmlFor?: string; children?: ReactNode }) {
  return (
    <li className={`cloud-step ${state}`} aria-current={state === "on" ? "step" : undefined}>
      <span className="num" aria-hidden="true">{state === "done" ? <Check className="icon" /> : n}</span>
      <div className="body">
        {htmlFor ? <label className="title" htmlFor={htmlFor}>{title}</label> : <span className="title">{title}</span>}
        {state === "done" && <span className="sr-only">Done.</span>}
        {children}
      </div>
    </li>
  );
}

/** The three steps. `status` and `answer` come from useCloud, shared with the tab around it. `onSignIn`: the step's Sign in
 *  was pressed (the tab keeps these steps open after the sign-in). `idPrefix` keeps ids apart when two are on a page. */
export function CloudRestore({ status, answer, onSignIn, idPrefix = "cloud-restore" }: {
  status: CloudStatus; answer: (f: () => Promise<CloudStatus>) => Promise<CloudStatus>; onSignIn?: () => void; idPrefix?: string;
}) {
  const restore = useRestore();
  const [code, setCode] = useState("");
  const [errs, setErrs] = useState<{ signIn?: string | null; code?: string | null; pick?: string | null }>({});
  const [busy, setBusy] = useState<"signIn" | "code" | null>(null);
  const [list, setList] = useState<CloudBackup[] | null>(null);
  const [listErr, setListErr] = useState<string | null>(null);
  const [asking, setAsking] = useState<CloudBackup | null>(null);
  const cancelled = useRef(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);

  const signedIn = status.signedIn;
  const keyHere = signedIn && status.key === "here";
  // the backups, read once the code is right (and again after a sign-out and in)
  useEffect(() => {
    if (!keyHere) { setList(null); setListErr(null); return; }
    let live = true;
    cloudListBackups().then((l) => { if (live) { setList(l ?? []); setListErr(null); } }).catch((e) => { if (live) setListErr(String(e)); });
    return () => { live = false; };
  }, [keyHere]);

  const now = Date.now();
  const setErr = (k: keyof typeof errs, v: string | null) => setErrs((e) => ({ ...e, [k]: v }));
  const signIn = () => {
    setErr("signIn", null);
    cancelled.current = false;
    setBusy("signIn");
    onSignIn?.();
    answer(cloudSignIn).catch((e) => { if (alive.current && !cancelled.current) setErr("signIn", String(e)); })
      .finally(() => { if (alive.current) setBusy(null); });
  };
  const cancelSignIn = () => {
    cancelled.current = true;
    cloudCancelSignIn().catch((e) => { if (alive.current) setErr("signIn", String(e)); });
  };
  const check = () => {
    if (busy || !code.trim()) return;
    setErr("code", null);
    setBusy("code");
    answer(() => cloudEnterRecovery(code.trim())).then(() => { if (alive.current) setCode(""); })
      .catch((e) => { if (alive.current) setErr("code", String(e)); })
      .finally(() => { if (alive.current) setBusy(null); });
  };
  const startIt = (b: CloudBackup) => {
    setAsking(null);
    setErr("pick", null);
    cloudStartRestore(b.name).catch((e) => { if (alive.current) setErr("pick", String(e)); });
  };

  if (showsRestore(status, restore)) {
    return (
      <div className="cloud-restore">
        <RestoreProgress status={status} restore={restore} now={now} onError={(e) => setErr("pick", e)} />
        {errs.pick && <span className="error">{errs.pick}</span>}
      </div>
    );
  }

  const signing = status.signingIn || busy === "signIn";
  const step1: StepState = signedIn ? "done" : "on";
  const step2: StepState = keyHere ? "done" : signedIn ? "on" : "later";
  const step3: StepState = keyHere ? "on" : "later";
  const codeId = `${idPrefix}-code`;
  return (
    <div className="cloud-restore">
      <ol className="cloud-steps">
        <Step n={1} state={step1} title="Sign in">
          {signedIn ? <span className="muted">Signed in as {status.email ?? "your account"}</span>
            : signing ? (
              <div className="bk-actions">
                <span className="gh-line bk-step" role="status"><LoaderCircle className="icon spin" /><span>{WAITING}</span></span>
                <button type="button" className="btn sm ghost" onClick={cancelSignIn}>Cancel</button>
              </div>
            ) : (
              <>
                <div className="bk-actions"><button type="button" className="btn primary" onClick={signIn}>Sign in</button></div>
                <span className="hint">{RESTORE_SIGN_IN_HINT}</span>
              </>
            )}
          {errs.signIn && <span className="error">{errs.signIn}</span>}
        </Step>
        <Step n={2} state={step2} title="Your recovery code" htmlFor={step2 === "done" || status.key === "missing" ? undefined : codeId}>
          {step2 === "done" ? <span className="muted">Accepted.</span>
            : signedIn && status.key === "missing" ? <span className="faint">This account has no backups in Hersir Cloud yet.</span>
            : (
              <>
                <div className="cloud-code-row">
                  <input id={codeId} className="input mono cloud-code-input" value={code} placeholder={CODE_PLACEHOLDER} spellCheck={false} autoComplete="off"
                    disabled={!signedIn || busy === "code"} aria-invalid={errs.code ? true : undefined}
                    onChange={(e) => { setCode(e.target.value); if (errs.code) setErr("code", null); }}
                    onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); check(); } }} />
                  {signedIn && <button type="button" className="btn" disabled={busy === "code" || !code.trim()} onClick={check}>{busy === "code" ? "Checking…" : "Check"}</button>}
                </div>
                {errs.code && <span className="error" role="alert">{errs.code}</span>}
              </>
            )}
        </Step>
        <Step n={3} state={step3} title="Pick a backup">
          {step3 === "on" ? (
            listErr ? <span className="warn">{listErr}</span>
              : !list ? <span className="faint">Looking for your backups…</span>
              : list.length === 0 ? <span className="faint">No backups in Hersir Cloud yet.</span>
              : (
                <>
                  <CloudBackupRows list={list} now={now} disabled={isRestoring(restore)} label="Backups in Hersir Cloud" onRestore={setAsking} />
                  <span className="hint">{PICK_HINT}</span>
                </>
              )
          ) : step2 === "on" && status.key !== "missing" ? <span className="faint">{PICK_BEFORE}</span> : null}
          {errs.pick && <span className="error">{errs.pick}</span>}
        </Step>
      </ol>
      {asking && (
        <CloudAsk id={`${idPrefix}-ask`} title="Restore this backup?" text={cloudConfirmText(asking, now)} action="Restore"
          onAction={() => startIt(asking)} onCancel={() => setAsking(null)} />
      )}
    </div>
  );
}
