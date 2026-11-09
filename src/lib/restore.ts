// Restore (GA-219) in words: the confirm, the steps, the notices and how the restore went at the start, from contract BR
// (GA-218's `restore_status` and `restore-changed`). No React here.
import type { BackupEntry, BackupStatus, RestoreStatus } from "../types";
import { backupWhen, fromText, reasonText } from "./backups";

/** The title of Restore from another folder…'s folder picker. */
export const PICK_TITLE = "Choose a folder with backups";

/** A restore runs now: its checks, saving today's data or copying. */
export const isRestoring = (s: Pick<RestoreStatus, "state"> | null): boolean =>
  s?.state === "checking" || s?.state === "saving" || s?.state === "staging";

/** The step it is at, as the backend says it: "Checking the database…". */
export const restoreStep = (s: Pick<RestoreStatus, "step">) => `${s.step ?? "Starting the restore"}…`;

/** "Restore ready: today 09:02": the time of the backup, not of the restore. */
export const readyText = (s: Pick<RestoreStatus, "snapshot">, now: number) =>
  s.snapshot ? `Restore ready: ${backupWhen(s.snapshot.madeAt, now)}` : "Restore ready";

/** A restore that stopped before the restart: today's data is as it was. */
export const failedText = (why: string | null) => `The restore failed: ${why ?? "Hersir didn't say why."}`;

/** The folder today's data is saved in first, as the backend picks it: the backups folder in Settings, or else (none, or it
 *  can't be used now) the folder restored from. */
export const saveFolder = (s: Pick<BackupStatus, "folder" | "folderProblem">, from: string) => (s.folder && !s.folderProblem ? s.folder : from);

/** The confirm's text. `saveTo`: the folder today's data goes to. */
export function confirmText(e: Pick<BackupEntry, "reason" | "madeAt" | "thisComputer" | "deviceName">, saveTo: string, now: number): string {
  return `${reasonText(e.reason)}, ${backupWhen(e.madeAt, now)}, ${fromText(e) ?? "from this computer"}. Hersir first saves today's data as a backup in ${saveTo}. `
    + "After a restart, Hersir shows your data as it was then: what changed since, also until the restart, is not kept. "
    + "Keys and passwords stay as they are on this computer. Agents are paused afterwards.";
}

/** When a snapshot was made, from its name (`gizai-<reason>-<stamp>-<device>`, docs/backup-format.md): the result names the
 *  snapshot, and its `at` is when the restart was. The stamp is the local time of the computer that made it. null: a name
 *  that isn't a snapshot's. */
export function snapshotMadeAt(name: string): number | null {
  const m = /^gizai-[a-z-]+-(\d{4})(\d{2})(\d{2})-(\d{2})(\d{2})(\d{2})-(\d{3})-/.exec(name);
  if (!m) return null;
  const [y, mo, d, h, mi, s, ms] = m.slice(1).map(Number) as [number, number, number, number, number, number, number];
  return new Date(y, mo - 1, d, h, mi, s, ms).getTime();
}

/** How the restore went at the start, until it is dismissed. */
export function resultText(r: NonNullable<RestoreStatus["result"]>, now: number): string {
  if (!r.ok) return `The restore failed: ${r.message.replace(/[.\s]+$/, "")}. Hersir started with today's data.`;
  const made = snapshotMadeAt(r.snapshot);
  return `Restored the backup of ${made === null ? r.snapshot : backupWhen(made, now)} from ${r.fromComputer.trim() || "another computer"}. `
    + "Agents are paused: resume them in Settings → Agents and runs. Today's data was saved as a backup first.";
}
