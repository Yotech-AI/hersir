// Settings → Hersir Cloud (GA-360, GA-317's design: design/hersir-cloud.md, sheet A and frame 1), on GA-359's commands
// (research 5.4). One status, `cloud_status` and then `cloud-changed`, picks what shows:
// - signed out: the plan and Sign in, then Restore from Hersir Cloud (CloudRestore); signing in: the waiting line and Cancel;
// - signed in on the account's first computer (key "missing"): Save your recovery code, which can't be skipped: Cloud Backup
//   stays off and the rest of the tab waits until the code's last group is typed back;
// - signed in on another computer (key "needed"): its recovery code, then the backups;
// - signed in: the account and its plan, storage, the recovery code (Make a new recovery code), the computers (Remove…),
//   and Cloud Backup (Back up, Last backup with Back up now and its step, the backups with Restore), with a failure, a full
//   quota and a lapsed plan.
// The controls save at once, as Settings → General → Backups does; a refusal shows its sentence under its field, as it is.
// Settings keeps every tab mounted: the backups and computers are read, and the recovery code made, once the tab is shown.
// A new recovery code waits for its last group while the tab is shown (GA-370): leaving the tab drops it, and until Done
// the old code keeps working.
import { useEffect, useRef, useState } from "react";
import { CircleX, DatabaseBackup, ExternalLink, KeyRound, LoaderCircle, LogIn, LogOut, Monitor, Trash2, TriangleAlert } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { cloudBackUpNow, cloudCancelSignIn, cloudComputers, cloudConfirmRecovery, cloudEnterRecovery, cloudListBackups, cloudMakeKey, cloudNewRecovery,
  cloudOpenAccount, cloudRemoveComputer, cloudSaveSettings, cloudSignIn, cloudSignOut, cloudStartRestore } from "../api";
import { BACKUP_TEXT, BACKUP_TEXT_SHORT, CLOUD_EVERY, CLOUD_TEXT, CLOUD_TEXT_SIGNED_IN, CODE_PLACEHOLDER, cloudConfirmText, cloudStepText, computerLine,
  COMPUTERS_HINT, day, failedLine, isDownloadStep, isLastGroup, LAPSED_HINT, lapsed, lapsedLine, LAST_GROUP_LABEL, lastLine, LIST_HINT,
  NEW_CODE_ASK, NEW_CODE_ASK_TEXT, NEW_CODE_HINT, NEW_CODE_SAVE, ON_QUIT, PLAN_PRICE, planLine, QUOTA_HINT, quotaFull, quotaLine, RECOVERY_HINT,
  REGISTER_URL, REMOVE_TEXT, removeAsk, RESTORE_TEXT, RESTORE_TITLE, SAVE_CODE_TEXT, SAVE_CODE_TITLE, SIGN_IN_HINT, storageShare, storageText,
  WAITING, WAITING_HINT, WRONG_GROUP } from "../lib/cloud";
import { useCloud } from "../lib/useCloud";
import { useRestore } from "../lib/useRestore";
import type { CloudBackup, CloudComputer, CloudSettingsChange } from "../types";
import { CloudAsk, CloudBackupRows, CloudRestore, CodeBox, RestoreProgress, showsRestore } from "./CloudRestore";
import { Field, FormSection } from "./Form";

/** The fields a refusal shows under. */
type Errs = Partial<Record<"account" | "key" | "code" | "newCode" | "every" | "now" | "list" | "computers" | "restore", string | null>>;
/** What the ask asks: a new recovery code, removing a computer, or restoring a backup. */
type Asking = { kind: "new-code" } | { kind: "remove"; computer: CloudComputer } | { kind: "restore"; backup: CloudBackup };

/** `shown`: the tab is open (Settings keeps the others mounted); what talks to Hersir Cloud waits for it. */
export function CloudSettings({ shown = true }: { shown?: boolean } = {}) {
  const { status: s, error: loadErr, answer } = useCloud();
  // the tab has been open: from then on its lists stay read
  const [seen, setSeen] = useState(shown);
  useEffect(() => { if (shown) setSeen(true); }, [shown]);
  const restore = useRestore();
  const [errs, setErrs] = useState<Errs>({});
  const [busy, setBusy] = useState<string | null>(null);
  // Save your recovery code (key "missing"): the code cloud_make_key made, and its last group as typed
  const [code, setCode] = useState<string | null>(null);
  const [typed, setTyped] = useState("");
  // A computer whose key is "needed": its recovery code as typed
  const [entered, setEntered] = useState("");
  // Make a new recovery code: the new code until Done, and its last group as typed
  const [newCode, setNewCode] = useState<string | null>(null);
  const [newTyped, setNewTyped] = useState("");
  const [asking, setAsking] = useState<Asking | null>(null);
  const [list, setList] = useState<CloudBackup[] | null>(null);
  const [computers, setComputers] = useState<CloudComputer[] | null>(null);
  // Restore from Hersir Cloud's own Sign in was pressed: its steps stay open after the sign-in, until the tab is left
  const [restoring, setRestoring] = useState(false);
  const making = useRef(false);
  // Try again after the recovery code couldn't be made
  const [makeTry, setMakeTry] = useState(0);
  const cancelled = useRef(false);
  const alive = useRef(true);
  useEffect(() => { alive.current = true; return () => { alive.current = false; }; }, []);

  const setErr = (k: keyof Errs, v: string | null) => setErrs((e) => (e[k] === v ? e : { ...e, [k]: v }));
  const signedIn = !!s?.signedIn;
  const key = s?.key;

  // The account's first computer, signed in: its key and recovery code, made once and shown until the last group is typed.
  useEffect(() => {
    if (!seen || !signedIn || key !== "missing" || code !== null || making.current) return;
    making.current = true;
    setErr("key", null);
    cloudMakeKey().then((r) => { if (alive.current) setCode(r.recoveryCode); })
      .catch((e) => { if (alive.current) setErr("key", String(e)); })
      .finally(() => { making.current = false; });
  }, [seen, signedIn, key, code, makeTry]);
  // Signed out, or the code was typed back: the code isn't kept
  useEffect(() => { if (!signedIn || key !== "missing") { setCode(null); setTyped(""); } }, [signedIn, key]);
  useEffect(() => { if (!signedIn) setEntered(""); }, [signedIn]);
  // A new recovery code changes nothing until its last group is typed back (cloud_confirm_recovery): leaving the tab,
  // signing out or losing the key drops it here, and the old code keeps working
  useEffect(() => {
    if (newCode === null || (shown && signedIn && key === "here")) return;
    setNewCode(null);
    setNewTyped("");
    setErr("newCode", null);
  }, [newCode, shown, signedIn, key]);

  // The backups and the computers, once this computer has the key: read again when a backup ends or fails
  const keyHere = signedIn && key === "here";
  const listKey = seen && keyHere && s ? JSON.stringify([s.backup.last?.name ?? null, s.backup.lastError?.at ?? null, !!s.backup.running]) : null;
  useEffect(() => {
    if (listKey === null) { setList(null); return; }
    let live = true;
    cloudListBackups().then((l) => { if (live) { setList(l ?? []); setErr("list", null); } }).catch((e) => { if (live) setErr("list", String(e)); });
    return () => { live = false; };
  }, [listKey]);
  useEffect(() => {
    if (!signedIn) { setComputers(null); return; }
    if (!seen) return;
    let live = true;
    cloudComputers().then((l) => { if (live) { setComputers(l ?? []); setErr("computers", null); } }).catch((e) => { if (live) setErr("computers", String(e)); });
    return () => { live = false; };
  }, [seen, signedIn, key]);

  if (!s) {
    return loadErr ? (
      <FormSection title="Hersir Cloud" text={CLOUD_TEXT}><Field label="Hersir Cloud" wide error={loadErr}><span className="faint">Hersir Cloud can't be shown now.</span></Field></FormSection>
    ) : null;
  }

  const now = Date.now();
  /** Runs a command for `what`: the buttons wait, and a refusal shows under `field`. */
  const run = (what: string, field: keyof Errs, f: () => Promise<unknown>, then?: () => void) => {
    setErr(field, null);
    setBusy(what);
    f().then(() => { if (alive.current) then?.(); })
      .catch((e) => { if (alive.current) setErr(field, String(e)); })
      .finally(() => { if (alive.current) setBusy((b) => (b === what ? null : b)); });
  };
  const signIn = () => {
    cancelled.current = false;
    setErr("account", null);
    setBusy("signIn");
    answer(cloudSignIn).catch((e) => { if (alive.current && !cancelled.current) setErr("account", String(e)); })
      .finally(() => { if (alive.current) setBusy((b) => (b === "signIn" ? null : b)); });
  };
  const cancelSignIn = () => { cancelled.current = true; cloudCancelSignIn().catch((e) => { if (alive.current) setErr("account", String(e)); }); };
  const signOut = () => run("signOut", "account", () => answer(cloudSignOut), () => setRestoring(false));
  const openAccount = () => { setErr("account", null); cloudOpenAccount().catch((e) => { if (alive.current) setErr("account", String(e)); }); };
  const save = (change: CloudSettingsChange) => run("save", "every", () => answer(() => cloudSaveSettings(change)));
  const backUp = () => run("now", "now", () => answer(cloudBackUpNow));
  const turnOn = () => {
    if (!code) return;
    if (!isLastGroup(code, typed)) { setErr("code", WRONG_GROUP); return; }
    run("confirm", "code", () => answer(() => cloudConfirmRecovery(typed.trim())), () => { setCode(null); setTyped(""); });
  };
  const enterCode = () => {
    if (!entered.trim()) return;
    run("enter", "code", () => answer(() => cloudEnterRecovery(entered.trim())), () => setEntered(""));
  };
  const makeNewCode = () => {
    setAsking(null);
    run("newCode", "newCode", async () => { const r = await cloudNewRecovery(); if (alive.current) { setNewCode(r.recoveryCode); setNewTyped(""); } });
  };
  // Only now does the new code replace the old one
  const doneNewCode = () => {
    if (!newCode || busy === "confirmNew") return;
    if (!isLastGroup(newCode, newTyped)) { setErr("newCode", WRONG_GROUP); return; }
    run("confirmNew", "newCode", () => answer(() => cloudConfirmRecovery(newTyped.trim())), () => { setNewCode(null); setNewTyped(""); });
  };
  const remove = (c: CloudComputer) => {
    setAsking(null);
    run(`remove-${c.id}`, "computers", async () => { const l = await cloudRemoveComputer(c.id, false); if (alive.current) setComputers(l ?? []); });
  };
  const startRestore = (b: CloudBackup) => {
    setAsking(null);
    setErr("restore", null);
    cloudStartRestore(b.name).catch((e) => { if (alive.current) setErr("restore", String(e)); });
  };

  const signing = s.signingIn || busy === "signIn";
  const plan = s.plan;
  const ended = lapsed(plan);
  const full = quotaFull(plan);
  // Restore from Hersir Cloud follows the account while signed out (not while the account's own Sign in waits), and stays
  // open after its own Sign in; on an account with no key yet there is nothing to restore
  const restoreSteps = !signedIn ? !signing || restoring : restoring && key !== "missing";

  /** The account's line: the email and the plan, Account page (or Renew on the account page) and Sign out. */
  const account = (withAccountPage: boolean) => (
    <Field label="Account" wide error={errs.account} hint={ended ? LAPSED_HINT : undefined}>
      <div className="bk-actions cloud-account">
        <span><span>{s.email}</span><span className="muted">{planLine(plan)}</span></span>
        {withAccountPage && (ended
          ? <button type="button" className="btn sm" onClick={openAccount}><ExternalLink className="icon" />Renew on the account page</button>
          : <button type="button" className="btn sm" onClick={openAccount}><ExternalLink className="icon" />Account page</button>)}
        <button type="button" className="btn sm ghost" disabled={busy === "signOut"} onClick={signOut}><LogOut className="icon" />Sign out</button>
      </div>
      {ended && plan && <div className="gh-line warning" role="status"><TriangleAlert className="icon" /><span>{lapsedLine(plan)}</span></div>}
    </Field>
  );

  /** Signed out: the plan and Sign in; signing in: the waiting line and Cancel. */
  const signedOut = (
    <FormSection title="Hersir Cloud" text={CLOUD_TEXT}>
      {signing && restoring ? (
        // the restore's steps below show the waiting line
        <Field label="Personal plan" wide><span className="muted">{PLAN_PRICE}</span></Field>
      ) : signing ? (
        <Field label="Account" wide error={errs.account} hint={WAITING_HINT}>
          <div className="bk-actions">
            <span className="gh-line bk-step" role="status"><LoaderCircle className="icon spin" /><span>{WAITING}</span></span>
            <button type="button" className="btn sm ghost" onClick={cancelSignIn}>Cancel</button>
          </div>
        </Field>
      ) : (
        <>
          <Field label="Personal plan" wide><span className="muted">{PLAN_PRICE}</span></Field>
          <Field label="Account" wide error={errs.account} hint={SIGN_IN_HINT}>
            <div className="bk-actions">
              <button type="button" className="btn primary" onClick={signIn}><LogIn className="icon" />Sign in</button>
              <button type="button" className="btn" onClick={() => { openUrl(REGISTER_URL).catch((e) => setErr("account", String(e))); }}>
                <ExternalLink className="icon" />Create an account</button>
            </div>
          </Field>
        </>
      )}
    </FormSection>
  );

  const restoreSection = (
    <FormSection title={RESTORE_TITLE} text={RESTORE_TEXT}>
      <div className="field wide"><CloudRestore status={s} answer={answer} onSignIn={() => setRestoring(true)} idPrefix="cloud-settings-restore" /></div>
    </FormSection>
  );

  if (!signedIn) return <>{signedOut}{restoreSteps && restoreSection}</>;

  // The account's first computer: Save your recovery code, before anything else
  if (key === "missing") {
    return (
      <>
        <FormSection title="Hersir Cloud" text={CLOUD_TEXT}>{account(false)}</FormSection>
        <FormSection title="Cloud Backup" text={BACKUP_TEXT_SHORT}>
          <div className="field wide cloud-save-code">
            <div className="cloud-needs"><KeyRound className="icon" /><h4>{SAVE_CODE_TITLE}</h4><span className="badge needs">Needs you</span></div>
            <p className="cloud-warning">{SAVE_CODE_TEXT}</p>
            {code ? <CodeBox code={code} email={s.email} />
              : errs.key ? (
                <div className="bk-actions">
                  <span className="error" role="alert">{errs.key}</span>
                  <button type="button" className="btn sm" onClick={() => setMakeTry((n) => n + 1)}>Try again</button>
                </div>
              )
              : <span className="gh-line bk-step" role="status"><LoaderCircle className="icon spin" /><span>Making your recovery code…</span></span>}
          </div>
          {code && (
            <Field label={LAST_GROUP_LABEL} htmlFor="cloud-last-group" wide error={errs.code}>
              <div className="cloud-code-row">
                <input id="cloud-last-group" className="input mono cloud-group-input" value={typed} spellCheck={false} autoComplete="off"
                  aria-invalid={errs.code ? true : undefined} onChange={(e) => { setTyped(e.target.value); if (errs.code) setErr("code", null); }}
                  onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); turnOn(); } }} />
                <button type="button" className="btn primary" disabled={busy === "confirm" || !typed.trim()} onClick={turnOn}>Turn on Cloud Backup</button>
              </div>
            </Field>
          )}
        </FormSection>
      </>
    );
  }

  const running = s.backup.running && !isDownloadStep(s.backup.running.step) ? s.backup.running : null;
  const restoreShown = showsRestore(s, restore) || !!errs.restore;
  const canBackUp = s.backup.on && !running && !full && !ended && busy !== "now" && !showsRestore(s, restore);

  return (
    <>
      <FormSection title="Hersir Cloud" text={CLOUD_TEXT_SIGNED_IN}>
        {account(true)}
        {plan && (
          <Field label="Storage" wide hint={full ? QUOTA_HINT : undefined}>
            <div className="cloud-storage">
              <span className={`cloud-meter${full ? " full" : ""}`} role="meter" aria-label="Storage used" aria-valuemin={0} aria-valuemax={plan.quotaBytes}
                aria-valuenow={Math.min(plan.usedBytes, plan.quotaBytes)} aria-valuetext={storageText(plan)}>
                <span style={{ width: `${Math.round(storageShare(plan) * 100)}%` }} /></span>
              <span className="mono">{storageText(plan)}</span>
            </div>
            {full && <div className="gh-line failed" role="alert"><CircleX className="icon" /><span>{quotaLine(plan)}</span></div>}
          </Field>
        )}
        {key === "here" && (newCode ? (
          <Field label="Recovery code" wide error={errs.newCode} hint={NEW_CODE_HINT}>
            <span>{NEW_CODE_SAVE}</span>
            <CodeBox code={newCode} email={s.email} />
            <div className="cloud-code-row">
              <input id="cloud-new-group" className="input mono cloud-group-input" value={newTyped} spellCheck={false} autoComplete="off" aria-label={LAST_GROUP_LABEL}
                aria-invalid={errs.newCode ? true : undefined} onChange={(e) => { setNewTyped(e.target.value); if (errs.newCode) setErr("newCode", null); }}
                onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); doneNewCode(); } }} />
              <button type="button" className="btn sm" disabled={busy === "confirmNew" || !newTyped.trim()} onClick={doneNewCode}>Done</button>
            </div>
          </Field>
        ) : (
          <Field label="Recovery code" wide error={errs.newCode} hint={RECOVERY_HINT}>
            <div className="bk-actions">
              <span>{s.recoveryMadeAt != null ? `Made on ${day(s.recoveryMadeAt)}` : "Made"}</span>
              <button type="button" className="btn sm" disabled={busy === "newCode"} onClick={() => setAsking({ kind: "new-code" })}>Make a new recovery code</button>
            </div>
          </Field>
        ))}
        <Field label="Your computers" wide error={errs.computers} hint={computers && computers.length > 0 ? COMPUTERS_HINT : undefined}>
          {!computers ? <span className="faint">Looking for your computers…</span>
            : computers.length === 0 ? <span className="faint">No computers yet.</span>
            : (
              <ul className="backup-list cloud-computers" aria-label="Your computers">
                {computers.map((c) => (
                  <li key={c.id}>
                    <div className="bk-row">
                      <span className="bk-when"><Monitor className="icon" />{c.name}{c.this && <span className="muted"> · this computer</span>}</span>
                      <span className="bk-what muted">{computerLine(c, now)}</span>
                      <span className="bk-do">
                        {!c.this && <button type="button" className="btn sm ghost" aria-label={`Remove ${c.name}…`} disabled={busy === `remove-${c.id}`}
                          onClick={() => setAsking({ kind: "remove", computer: c })}><Trash2 className="icon" />Remove…</button>}
                      </span>
                    </div>
                  </li>
                ))}
              </ul>
            )}
        </Field>
      </FormSection>

      {restoreSteps ? restoreSection : (
        <FormSection title="Cloud Backup" text={BACKUP_TEXT}>
          {key === "needed" ? (
            <Field label="Your recovery code" htmlFor="cloud-enter-code" wide error={errs.code}
              hint="This account has a recovery code already. Type it to back up this computer and to see your backups.">
              <div className="cloud-code-row">
                <input id="cloud-enter-code" className="input mono cloud-code-input" value={entered} placeholder={CODE_PLACEHOLDER} spellCheck={false} autoComplete="off"
                  aria-invalid={errs.code ? true : undefined} onChange={(e) => { setEntered(e.target.value); if (errs.code) setErr("code", null); }}
                  onKeyDown={(e) => { if (e.key === "Enter") { e.preventDefault(); enterCode(); } }} />
                <button type="button" className="btn" disabled={busy === "enter" || !entered.trim()} onClick={enterCode}>{busy === "enter" ? "Checking…" : "Check"}</button>
              </div>
            </Field>
          ) : (
            <>
              {!s.backup.on ? (
                <Field label="Cloud Backup" wide error={errs.every}>
                  <div className="bk-actions">
                    <span className="muted">Off on this computer.</span>
                    <button type="button" className="btn primary" disabled={busy === "save"} onClick={() => save({ on: true })}>Turn on Cloud Backup</button>
                  </div>
                </Field>
              ) : (
                <Field label="Back up" wide error={errs.every}>
                  <div className="radios" role="radiogroup" aria-label="Back up">
                    {CLOUD_EVERY.map(([value, label]) => (
                      <label key={value}><input type="radio" name="cloud-every" checked={s.backup.every === value} onChange={() => save({ every: value })} />{label}</label>
                    ))}
                  </div>
                  <label className="check"><input type="checkbox" checked={s.backup.onQuit} onChange={(e) => save({ onQuit: e.target.checked })} />{ON_QUIT}</label>
                </Field>
              )}
              <Field label="Last backup" wide error={errs.now}>
                <span>{lastLine(s.backup, now)}</span>
                {s.backup.lastError && <div className="gh-line failed" role="alert"><CircleX className="icon" /><span className="bk-error">{failedLine(s.backup.lastError, now)}</span></div>}
                <div className="bk-actions">
                  <button type="button" className="btn sm" onClick={backUp} disabled={!canBackUp}><DatabaseBackup className="icon" />Back up now</button>
                  {full && <button type="button" className="btn sm" onClick={openAccount}><ExternalLink className="icon" />Account page</button>}
                  {running && <span className="gh-line bk-step" role="status"><LoaderCircle className="icon spin" /><span>{cloudStepText(running)}</span></span>}
                </div>
              </Field>
              {restoreShown && (
                <Field label="Restore" wide error={errs.restore}>
                  <div className="bk-restore"><RestoreProgress status={s} restore={restore} now={now} onError={(e) => setErr("restore", e)} /></div>
                </Field>
              )}
              <Field label="Backups" wide hint={list && list.length > 0 ? LIST_HINT : undefined}>
                {errs.list ? <span className="warn">{errs.list}</span>
                  : !list ? <span className="faint">Looking for your backups…</span>
                  : list.length === 0 ? <span className="faint">No backups in Hersir Cloud yet.</span>
                  : <CloudBackupRows list={list} now={now} disabled={showsRestore(s, restore) || !!running} label="Backups in Hersir Cloud"
                      onRestore={(b) => setAsking({ kind: "restore", backup: b })} />}
              </Field>
            </>
          )}
        </FormSection>
      )}

      {asking?.kind === "new-code" && (
        <CloudAsk id="cloud-new-code" title={NEW_CODE_ASK} text={NEW_CODE_ASK_TEXT} action="Make a new code" onAction={makeNewCode} onCancel={() => setAsking(null)} />
      )}
      {asking?.kind === "remove" && (
        <CloudAsk id="cloud-remove" title={removeAsk(asking.computer.name)} text={REMOVE_TEXT} action="Remove" danger
          onAction={() => remove(asking.computer)} onCancel={() => setAsking(null)} />
      )}
      {asking?.kind === "restore" && (
        <CloudAsk id="cloud-restore-ask" title="Restore this backup?" text={cloudConfirmText(asking.backup, now)} action="Restore"
          onAction={() => startRestore(asking.backup)} onCancel={() => setAsking(null)} />
      )}
    </>
  );
}

