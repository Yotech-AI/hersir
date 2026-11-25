// The browser mode's Hersir Cloud (GA-360): GA-359's commands (research 5.4) with a made-up account, so Settings → Hersir
// Cloud and Restore from Hersir Cloud can be seen and clicked through in a browser. Each change goes out as `cloud-changed`.
// `?cloud=<state>` in the address starts in a state of GA-317's sheet A: signed-out (the default: an account with no
// backups yet), new-computer (signed out, on an account with backups: Restore from Hersir Cloud's way), missing (signed in
// on the account's first computer), needed (signed in on another computer), signed-in, off, running, failed, full or lapsed.
// Sign in waits a moment and can be cancelled; the recovery code's last group is Q8YC, and a computer whose key is needed
// takes HC-7K2Q-M4XD-9TRP-B6WN-3HFJ-Q8YC. Signing out forgets this computer's key, so the next sign-in needs the code.
import { emit } from "@tauri-apps/api/event";
import type { CloudBackup, CloudComputer, CloudStatus } from "../types";

type Row = Record<string, unknown>;
/** The restore's fixture and how to say it changed: mock.ts's, so GA-218's restore goes on from here. */
export type RestoreHooks = { status: () => Row | undefined; changed: () => unknown };

const GB = 1024 ** 3, MB = 1024 ** 2, DAY = 86_400_000;
const STEP_MS = 900;
const CODE = "HC-7K2Q-M4XD-9TRP-B6WN-3HFJ-Q8YC";
const NEW_CODE = "HC-3MWT-9QXE-7RKD-B2NA-6PLF-X7KD";
const EMAIL = "you@example.com";

const at = (daysAgo: number, h: number, m: number) => {
  const d = new Date(Date.now() - daysAgo * DAY);
  d.setHours(h, m, 0, 0);
  return d.getTime();
};

const noBackup = (): CloudStatus["backup"] => ({ on: false, every: "daily", onQuit: false, running: null, last: null, next: null, lastError: null });
const signedOut = (): CloudStatus => ({ signedIn: false, email: null, signingIn: false, plan: null, key: "missing", recoveryMadeAt: null, backup: noBackup() });
const activePlan = (): NonNullable<CloudStatus["plan"]> =>
  ({ name: "personal", status: "active", validUntil: Date.now() + 349 * DAY, readOnlyUntil: null, quotaBytes: 10 * GB, usedBytes: 1.4 * GB });

/** Signed in on a computer with the key, Cloud Backup on, daily and on quit. */
function backingUp(): CloudStatus {
  return {
    signedIn: true, email: EMAIL, signingIn: false, plan: activePlan(), key: "here", recoveryMadeAt: Date.now() - 16 * DAY,
    backup: { on: true, every: "daily", onQuit: true, running: null, last: { at: at(0, 9, 2), name: "today", sentBytes: 38 * MB }, next: at(-1, 9, 2), lastError: null },
  };
}

function initial(state: string): { st: CloudStatus; accountHasKey: boolean } {
  const st = backingUp();
  switch (state) {
    case "new-computer": return { st: signedOut(), accountHasKey: true };
    case "missing": return { st: { ...st, key: "missing", recoveryMadeAt: null, backup: noBackup() }, accountHasKey: false };
    case "needed": return { st: { ...st, key: "needed", backup: noBackup() }, accountHasKey: true };
    case "signed-in": return { st, accountHasKey: true };
    case "off": return { st: { ...st, backup: { ...st.backup, on: false, next: null } }, accountHasKey: true };
    case "running": return { st: { ...st, backup: { ...st.backup, running: { step: "uploading", done: 112 * MB, total: 240 * MB } } }, accountHasKey: true };
    case "failed": return { st: { ...st, backup: { ...st.backup, last: { at: at(1, 9, 1), name: "yesterday", sentBytes: 41 * MB },
      lastError: { at: at(0, 9, 2), message: "Hersir Cloud couldn't be reached." } } }, accountHasKey: true };
    case "full": return { st: { ...st, plan: { ...activePlan(), usedBytes: 10 * GB }, backup: { ...st.backup, last: { at: at(3, 9, 2), name: "3 days ago", sentBytes: 52 * MB } } }, accountHasKey: true };
    case "lapsed": return { st: { ...st, plan: { ...activePlan(), status: "lapsed", validUntil: Date.now() - 2 * DAY, readOnlyUntil: Date.now() + 58 * DAY } }, accountHasKey: true };
  }
  return { st: signedOut(), accountHasKey: false };
}

const asked = new URLSearchParams(globalThis.location?.search ?? "").get("cloud") ?? "";
let { st, accountHasKey } = initial(asked);
/** The recovery code made for this computer and not typed back yet. */
let shown: string | null = null;
let signIn: { timer: ReturnType<typeof setTimeout>; reject: (why: string) => void } | null = null;
let run = 0;

let backups: CloudBackup[] = [
  { name: "today", madeAt: at(0, 9, 2), device: { id: "d1", name: "Desktop", this: true }, reason: "daily", bytes: 1.2 * GB, restorable: true, problem: null },
  { name: "laptop-quit", madeAt: at(1, 18, 40), device: { id: "d2", name: "Laptop", this: false }, reason: "quit", bytes: 1.2 * GB, restorable: true, problem: null },
  { name: "older", madeAt: at(7, 9, 1), device: { id: "d1", name: "Desktop", this: true }, reason: "daily", bytes: 1.1 * GB, restorable: true, problem: null },
  { name: "newer", madeAt: at(2, 12, 15), device: { id: "d3", name: "Work laptop", this: false }, reason: "manual", bytes: 1.3 * GB, restorable: false,
    problem: "A newer Hersir made it: update Hersir to restore it." },
];
let computers: CloudComputer[] = [
  { id: "d1", name: "Desktop", platform: "Linux", appVersion: "0.21.0", lastSeenAt: Date.now(), online: true, this: true, chief: true },
  { id: "d2", name: "Laptop", platform: "macOS", appVersion: "0.21.0", lastSeenAt: at(1, 18, 40), online: false, this: false, chief: false },
  { id: "d3", name: "Work laptop", platform: "Windows", appVersion: "0.22.0", lastSeenAt: null, online: false, this: false, chief: false },
];

/** Says the status changed, and answers it. */
function changed(): CloudStatus {
  const now = structuredClone(st);
  void emit("cloud-changed", now);
  return structuredClone(now);
}

const lastGroup = (code: string) => code.split("-").pop() ?? "";
const norm = (s: unknown) => String(s ?? "").replace(/\s+/g, "").toUpperCase();
const lapsedLine = () => "Your plan ended: Hersir makes no new cloud backups until you renew.";

/** Runs `steps` a moment each, then `done`; a newer run stops it. */
function steps(list: (() => void)[], done: () => void) {
  const mine = ++run;
  list.forEach((f, i) => setTimeout(() => { if (mine === run) { f(); changed(); } }, i * STEP_MS));
  setTimeout(() => { if (mine === run) { done(); changed(); } }, list.length * STEP_MS);
}

export function cloudAnswer(cmd: string, args: Row, restore: RestoreHooks): unknown {
  const signedInFirst = () => (st.signedIn ? null : Promise.reject("Sign in to Hersir Cloud first."));
  switch (cmd) {
    case "cloud_status": return structuredClone(st);
    case "cloud_sign_in": {
      if (st.signedIn) return structuredClone(st);
      if (signIn) return Promise.reject("Signing in is waiting already: finish it in your browser, or cancel it.");
      st.signingIn = true;
      changed();
      return new Promise((resolve, reject) => {
        const timer = setTimeout(() => {
          signIn = null;
          st = { ...st, signedIn: true, signingIn: false, email: EMAIL, plan: activePlan(), key: accountHasKey ? "needed" : "missing",
            recoveryMadeAt: accountHasKey ? Date.now() - 16 * DAY : null };
          resolve(changed());
        }, 2500);
        signIn = { timer, reject };
      });
    }
    case "cloud_cancel_sign_in": {
      if (signIn) { clearTimeout(signIn.timer); signIn.reject("Signing in was cancelled."); signIn = null; }
      st.signingIn = false;
      changed();
      return null;
    }
    case "cloud_sign_out": {
      run++;
      st = signedOut();
      shown = null;
      return changed();
    }
    case "cloud_make_key": {
      const no = signedInFirst();
      if (no) return no;
      if (accountHasKey) return Promise.reject("This account has a recovery code already: type it to use Hersir Cloud on this computer.");
      shown = CODE;
      return { recoveryCode: shown };
    }
    case "cloud_confirm_recovery": {
      if (!shown) return Promise.reject("Make a recovery code first.");
      if (norm(args.lastGroup) !== lastGroup(shown)) return Promise.reject("That isn't the code's last group. Look at the code above again.");
      accountHasKey = true;
      shown = null;
      st = { ...st, key: "here", recoveryMadeAt: Date.now(), backup: { ...noBackup(), on: true, onQuit: true, next: Date.now() + DAY } };
      return changed();
    }
    case "cloud_enter_recovery": {
      const no = signedInFirst();
      if (no) return no;
      if (norm(args.code) !== CODE) return Promise.reject("That recovery code doesn't open this account's backups.");
      st = { ...st, key: "here", backup: { ...noBackup(), last: null } };
      return changed();
    }
    case "cloud_new_recovery": {
      const no = signedInFirst();
      if (no) return no;
      st.recoveryMadeAt = Date.now();
      changed();
      return { recoveryCode: NEW_CODE };
    }
    case "cloud_save_settings": {
      const s = (args.s ?? {}) as Partial<CloudStatus["backup"]>;
      st.backup = { ...st.backup, ...s };
      st.backup.next = st.backup.on && st.backup.every === "daily" ? at(-1, 9, 2) : null;
      return changed();
    }
    case "cloud_back_up_now": {
      const p = st.plan;
      if (!st.backup.on) return Promise.reject("Turn on Cloud Backup first.");
      if (p?.status === "lapsed") return Promise.reject(lapsedLine());
      if (p && p.usedBytes >= p.quotaBytes) return Promise.reject("Hersir Cloud is full: 10.0 of 10 GB.");
      if (st.backup.running) return Promise.reject("A backup is running already.");
      const total = 240 * MB;
      const up = (done: number) => () => { st.backup.running = { step: "uploading", done, total }; };
      st.backup.running = { step: "copying", done: 0, total: 0 };
      st.backup.lastError = null;
      steps([() => { st.backup.running = { step: "chunking", done: 0, total: 0 }; }, up(0), up(112 * MB), up(201 * MB),
        () => { st.backup.running = { step: "registering", done: total, total }; }], () => {
        const madeAt = Date.now();
        st.backup.running = null;
        st.backup.last = { at: madeAt, name: `manual-${madeAt}`, sentBytes: 38 * MB };
        backups = [{ name: `manual-${madeAt}`, madeAt, device: { id: "d1", name: "Desktop", this: true }, reason: "manual", bytes: 1.2 * GB, restorable: true, problem: null }, ...backups];
      });
      return changed();
    }
    case "cloud_list_backups": {
      if (!st.signedIn || st.key !== "here") return Promise.reject("Type your recovery code first.");
      return structuredClone(backups);
    }
    case "cloud_start_restore": {
      const b = backups.find((x) => x.name === args.name);
      const r = restore.status();
      if (!b || !r) return Promise.reject("That backup isn't in Hersir Cloud.");
      if (st.backup.running) return Promise.reject("A backup is running: restore once it is done.");
      const total = b.bytes;
      const down = (done: number) => () => { st.backup.running = { step: "downloading", done, total }; };
      const snapshot = { name: `gizai-${b.reason}-20261125-090200-000-${b.device.id}`, reason: b.reason, madeAt: b.madeAt, deviceId: b.device.id,
        deviceName: b.device.name, thisComputer: b.device.this, appVersion: "0.21.0", schemaVersion: 1, dbBytes: b.bytes, filesCount: 0, filesBytes: 0,
        filesMissing: 0, restorable: true, problem: null };
      const restoreStep = (state: string, step: string | null) => () => {
        st.backup.running = null;
        Object.assign(r, { state, folder: null, snapshot, step, error: null });
        restore.changed();
      };
      down(0)();
      steps([down(total * 0.3), down(total * 0.55), down(total * 0.8), () => { st.backup.running = { step: "checking", done: total, total }; },
        restoreStep("checking", "Checking the database"), restoreStep("saving", "Saving today's data")], () => {
        restoreStep("ready", null)();
        r.savedToday = "gizai-before-restore-now";
        restore.changed();
      });
      changed();
      return structuredClone(r);
    }
    case "cloud_computers": {
      const no = signedInFirst();
      return no ?? structuredClone(computers);
    }
    case "cloud_remove_computer": {
      const c = computers.find((x) => x.id === args.id);
      if (!c) return Promise.reject("That computer isn't on your account.");
      if (c.this) return Promise.reject("Sign out to remove this computer.");
      computers = computers.filter((x) => x.id !== args.id);
      return structuredClone(computers);
    }
    case "cloud_open_account": {
      console.info("Hersir in a browser: Account page opens https://cloud.hersir.dev/account in the browser.");
      return null;
    }
  }
  return Promise.reject(`Hersir in a browser doesn't know ${cmd}.`);
}
