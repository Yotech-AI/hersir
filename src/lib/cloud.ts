// Settings → Hersir Cloud and Restore from Hersir Cloud (GA-360) in words, from GA-317's design (design/hersir-cloud.md,
// "Every text, word for word") and the commands of research 5.4 (GA-359). No React here.
import type { CloudBackup, CloudPlan, CloudStatus, CloudStep } from "../types";
import { backupWhen, reasonText } from "./backups";
import { formatBytes } from "./format";
import { RESTORE_AFTER } from "./restore";

/** The address of the page that makes an account. */
export const REGISTER_URL = "https://cloud.hersir.dev/register";

export const CLOUD_TEXT = "Encrypted backups of your data, kept off your computer. Restore them on a new one.";
/** The section's text once signed in. */
export const CLOUD_TEXT_SIGNED_IN = `${CLOUD_TEXT} Hersir can't read what it stores.`;
export const PLAN_PRICE = "€5 a month or €48 a year, VAT included. Hersir can't read what it stores.";
export const SIGN_IN_HINT = "Signing in opens your browser.";
export const WAITING = "Waiting for you to sign in in your browser…";
export const WAITING_HINT = "Sign in with your password and your authenticator's code. Didn't a tab open? Cancel and sign in again.";
export const RECOVERY_HINT = "Hersir never keeps it on a computer. A new code replaces the old one, which stops working.";
export const QUOTA_HINT = "Backups stop until there is room. Older backups go after 90 days, or delete some on the account page.";
export const LAPSED_HINT = "Hersir makes no new cloud backups until you renew. Your data on this computer is untouched.";

export const BACKUP_TEXT_SHORT = "A copy of this computer's data in Hersir Cloud, encrypted before it leaves.";
export const BACKUP_TEXT = `${BACKUP_TEXT_SHORT} Each backup is kept 7 days, then one a day up to 90 days.`;
/** Back up's choices. */
export const CLOUD_EVERY: [CloudStatus["backup"]["every"], string][] = [["daily", "Daily"], ["manual", "Only when I ask"]];
export const ON_QUIT = "Also when I quit Hersir completely";
export const LIST_HINT = "Restoring saves today's data first and finishes when Hersir restarts.";

/** Decision 2's warning, word for word: the recovery code's, and Make a new recovery code's. */
export const KEY_WARNING = "Hersir encrypts your data on this computer before it leaves. Only your computers and this recovery code can open it. "
  + "If you lose this code and every computer signed in to Hersir Cloud, your cloud backups can't be opened by anyone, Hersir included.";
export const SAVE_CODE_TITLE = "Save your recovery code";
export const SAVE_CODE_TEXT = `${KEY_WARNING} Hersir won't show it again.`;
export const LAST_GROUP_LABEL = "Type its last group to show you have it";
export const WRONG_GROUP = "That isn't the code's last group. Look at the code above again.";
export const NEW_CODE_ASK = "Make a new recovery code?";
// GA-370: a new code replaces the old one only once its last group is typed back
export const NEW_CODE_ASK_TEXT = `${KEY_WARNING} Your current code stops working once you type the new one's last group.`;
export const NEW_CODE_SAVE = "Save your new recovery code. It replaces your old one once you type its last group.";
export const NEW_CODE_HINT = "Type its last group to show you have it.";

export const RESTORE_TITLE = "Restore from Hersir Cloud";
export const RESTORE_TEXT = "Your data from a backup in Hersir Cloud. It replaces what is on this computer, which Hersir saves first.";
export const RESTORE_SIGN_IN_HINT = "Opens your browser: your password, then your authenticator's code.";
export const CODE_PLACEHOLDER = "HC-____-____-____-____-____-____";
export const PICK_BEFORE = "Your backups show once the code is right.";
export const PICK_HINT = "Hersir restarts to finish, as a restore from a folder does.";
export const FIRST_SCREEN_LINE = "Moving from another computer? Your data comes back from your backup in Hersir Cloud.";

/** Your computers (research 5.1: the account page's words): each signs in on its own. */
export const COMPUTERS_HINT = "Each has its own sign-in. Remove one you no longer use, or lost: it is signed out at once.";

const upper = (s: string) => s.charAt(0).toUpperCase() + s.slice(1);

/** A day: "9 Nov 2027". */
export const day = (ms: number) => new Date(ms).toLocaleDateString("en-GB", { day: "numeric", month: "short", year: "numeric" });

/** A moment, as a line starts with it: "Today 09:02", "Yesterday 18:40", "Tue 18 Nov, 09:01". */
export const when = (ms: number, now: number) => upper(backupWhen(ms, now));

/** The plan has ended: no new backups, and the backups can still be restored for a while. `past_due` changes nothing. */
export const lapsed = (p: CloudPlan | null | undefined) => p?.status === "lapsed";

/** The storage is used up. */
export const quotaFull = (p: CloudPlan | null | undefined) => !!p && p.quotaBytes > 0 && p.usedBytes >= p.quotaBytes;

/** The account's line after the email: " · Personal, renews 9 Nov 2027", " · Personal ended on 9 Dec 2026". */
export function planLine(p: CloudPlan | null): string {
  if (!p || p.name === "none" || p.status === "none") return " · No plan";
  if (p.status === "lapsed") return p.validUntil != null ? ` · Personal ended on ${day(p.validUntil)}` : " · Personal ended";
  return p.validUntil != null ? ` · Personal, renews ${day(p.validUntil)}` : " · Personal";
}

/** "Your plan ended on 9 Dec 2026: backups can be restored until 7 Feb 2027." */
export function lapsedLine(p: CloudPlan): string {
  const ended = p.validUntil != null ? `Your plan ended on ${day(p.validUntil)}` : "Your plan ended";
  return p.readOnlyUntil != null ? `${ended}: backups can be restored until ${day(p.readOnlyUntil)}.` : `${ended}.`;
}

/** Sizes of the storage in the quota's own unit: a quota of 10 GB is 10 × 1000³ or 10 × 1024³ bytes. */
const gbOf = (quota: number) => (quota > 0 && quota % 1e9 === 0 ? 1e9 : 1024 ** 3);

/** "1.4 of 10 GB". */
export function storageText(p: Pick<CloudPlan, "usedBytes" | "quotaBytes">): string {
  const gb = gbOf(p.quotaBytes);
  const quota = p.quotaBytes / gb;
  return `${(p.usedBytes / gb).toFixed(1)} of ${Number.isInteger(quota) ? quota : quota.toFixed(1)} GB`;
}

/** How full the meter is, 0 to 1. */
export const storageShare = (p: Pick<CloudPlan, "usedBytes" | "quotaBytes">) => (p.quotaBytes > 0 ? Math.min(1, p.usedBytes / p.quotaBytes) : 0);

/** "Hersir Cloud is full: 10.0 of 10 GB." */
export const quotaLine = (p: Pick<CloudPlan, "usedBytes" | "quotaBytes">) => `Hersir Cloud is full: ${storageText(p)}.`;

/** Megabytes, whole, with thousands: "1,200". */
const mb = (bytes: number) => Math.round(bytes / 1024 ** 2).toLocaleString("en-GB");

/** The step a backup or a download is at: "Uploading: 112 of 240 MB…". */
export function cloudStepText(r: { step: CloudStep | string; done: number; total: number }): string {
  switch (r.step) {
    case "copying": return "Copying your data…";
    case "chunking": return "Encrypting…";
    case "uploading": return `Uploading: ${mb(r.done)} of ${mb(r.total)} MB…`;
    case "registering": return "Finishing…";
    case "downloading": return `Downloading: ${mb(r.done)} of ${mb(r.total)} MB…`;
    case "checking": return "Checking the backup…";
  }
  return `${r.step}…`;
}

/** The steps of a restore's download, before GA-218's restore takes over. */
export const isDownloadStep = (step: CloudStep | string | undefined) => step === "downloading" || step === "checking";

/** "Today 09:02 · 38 MB sent · next tomorrow 09:02"; without a backup "No backup yet". Only when I ask has no next. */
export function lastLine(b: CloudStatus["backup"], now: number): string {
  const last = b.last ? `${when(b.last.at, now)} · ${formatBytes(b.last.sentBytes)} sent` : "No backup yet";
  if (b.every === "manual" || b.next == null) return last;
  return `${last} · next ${b.next <= now ? "within the hour" : backupWhen(b.next, now)}`;
}

/** "Today 09:02: the backup failed. Hersir Cloud couldn't be reached. Hersir tries again at the next backup, or back up now." */
export function failedLine(e: { at: number; message: string }, now: number): string {
  const why = e.message.trim();
  return `${when(e.at, now)}: the backup failed. ${why ? `${/[.!?]$/.test(why) ? why : `${why}.`} ` : ""}Hersir tries again at the next backup, or back up now.`;
}

/** The computer that made a backup: "this computer", or its name. */
export const madeBy = (b: Pick<CloudBackup, "device">) => (b.device.this ? "this computer" : b.device.name.trim() || "another computer");

/** A backup's row: "Daily · this computer". */
export const backupWhat = (b: Pick<CloudBackup, "reason" | "device">) => `${reasonText(b.reason)} · ${madeBy(b)}`;

/** Why a backup can't be restored, or null when it can. */
export const backupProblem = (b: Pick<CloudBackup, "restorable" | "problem">) => (b.restorable ? null : b.problem || "It can't be restored.");

/** Newest first, whatever order they came in. */
export const newestFirst = (l: CloudBackup[]) => [...l].sort((a, b) => b.madeAt - a.madeAt);

/** Restore's question for a backup in Hersir Cloud: what will happen, as for a folder (BackupSettings' ask). */
export function cloudConfirmText(b: Pick<CloudBackup, "reason" | "madeAt" | "device">, now: number): string {
  const from = b.device.this ? "from this computer" : `from ${madeBy(b)}`;
  return `${reasonText(b.reason)}, ${backupWhen(b.madeAt, now)}, ${from}. Hersir first saves today's data as a backup. ${RESTORE_AFTER}`;
}

/** The recovery code's last group: "Q8YC" of "HC-7K2Q-…-Q8YC". */
export const lastGroup = (code: string) => code.trim().split("-").pop() ?? "";

/** Whether `typed` is the code's last group (case and spaces don't count). */
export const isLastGroup = (code: string, typed: string) => {
  const want = lastGroup(code).toUpperCase();
  return want !== "" && typed.replace(/\s+/g, "").toUpperCase() === want;
};

/** A computer's line: "Laptop · Linux · Hersir 0.21.0 · online", "… · seen yesterday 18:40". */
export function computerLine(c: { name: string; platform: string; appVersion: string; lastSeenAt: number | null; online: boolean }, now: number): string {
  const seen = c.online ? "online" : c.lastSeenAt != null ? `seen ${backupWhen(c.lastSeenAt, now)}` : null;
  return [c.platform.trim(), c.appVersion.trim() && `Hersir ${c.appVersion.trim()}`, seen].filter(Boolean).join(" · ");
}

/** Remove's question for a computer. */
export const removeAsk = (name: string) => `Remove ${name.trim() || "this computer"}?`;
export const REMOVE_TEXT = "It is signed out of Hersir Cloud at once and can no longer back up to it or restore from it. Its own data stays on it.";

/** The recovery code as a file's text. */
export const codeFileText = (code: string, email: string | null) =>
  `Hersir Cloud recovery code${email ? ` for ${email}` : ""}\n\n${code}\n\n${KEY_WARNING}\n`;
export const CODE_FILE_NAME = "Hersir Cloud recovery code.txt";
