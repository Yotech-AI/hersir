// GA-219 QA: Restore in Settings → General → Backups and in the sidebar (RestoreNotice), on GA-218's contract BR. Restore on each
// snapshot that can be restored, why on one that can't; Restore from another folder…; the confirm, whose Cancel changes nothing;
// the steps, a refusal and a failure with their reasons; ready in both places with Restart now and Cancel restore; the result
// after the start until Dismiss. There is no DOM here, so a small hook runtime stands in for React (as in
// BackupSettings.test.tsx), with a fake backend that refuses and sends `restore-changed` as src-tauri's restore.rs does: the
// screens follow that event, not the commands' answers.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import type { BackupEntry, BackupStatus, RestoreStatus } from "../types";

type Inst = { slots: unknown[]; i: number };
type EffectSlot = { effect: true; deps?: unknown[]; first: boolean; cleanup?: () => void };
const h = vi.hoisted(() => ({
  cur: null as Inst | null,
  dirty: false,
  effects: [] as (() => void)[],
  backup: null as unknown,
  list: [] as unknown[],
  other: [] as unknown[],
  restore: null as unknown,
  calls: { start: [] as [string, string][], cancel: 0, restart: 0, dismiss: 0, update: 0, list: [] as (string | null)[] },
  backupListeners: new Set<(s: unknown) => void>(),
  restoreListeners: new Set<(s: unknown) => void>(),
  picked: null as string | null,
  pickerOpts: [] as unknown[],
  pickRefusal: null as string | null,
  otherRefusal: null as string | null,
  cancelRefusal: null as string | null,
  dismissRefusal: null as string | null,
  restartRefusal: null as string | null,
  live: [] as unknown[],
  heldStatus: [] as (() => void)[],
  holdStatus: false,
}));

vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const slot = <T,>(make: () => T): T => {
    const c = h.cur!;
    if (c.i === c.slots.length) c.slots.push(make());
    return c.slots[c.i++] as T;
  };
  const changed = (deps?: unknown[], prev?: unknown[]) => !deps || !prev || deps.length !== prev.length || deps.some((d, i) => !Object.is(d, prev[i]));
  const useState = ((init: unknown) => {
    if (!h.cur) return R.useState(init);
    const box = slot(() => {
      const b = { v: typeof init === "function" ? (init as () => unknown)() : init, set: (x: unknown) => {
        const v = typeof x === "function" ? (x as (p: unknown) => unknown)(b.v) : x;
        if (!Object.is(v, b.v)) { b.v = v; h.dirty = true; }
      } };
      return b;
    });
    return [box.v, box.set];
  }) as typeof R.useState;
  const useRef = ((init: unknown) => (h.cur ? slot(() => ({ current: init })) : R.useRef(init))) as typeof R.useRef;
  const useEffect = ((f: () => void | (() => void), deps?: unknown[]) => {
    if (!h.cur) return R.useEffect(f, deps);
    const s = slot<EffectSlot>(() => ({ effect: true, first: true }));
    if (s.first || changed(deps, s.deps)) {
      s.first = false;
      s.deps = deps;
      h.effects.push(() => { s.cleanup?.(); const c = f(); s.cleanup = typeof c === "function" ? c : undefined; });
    }
  }) as typeof R.useEffect;
  const hooks = { useState, useRef, useEffect };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: async (o: unknown) => { h.pickerOpts.push(o); if (h.pickRefusal) throw h.pickRefusal; return h.picked; } }));

// The backend. Tauri rejects a command with the error's text, so the refusals are strings.
const rs = () => h.restore as RestoreStatus;
const bs = () => h.backup as BackupStatus;
const sendRestore = () => { const x = structuredClone(rs()); for (const cb of [...h.restoreListeners]) cb(x); };
const sendBackups = () => { const x = structuredClone(bs()); for (const cb of [...h.backupListeners]) cb(x); };
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(real)) if (typeof real[k] === "function") out[k] = () => new Promise(() => {});
  return {
    ...out,
    backupStatus: async () => structuredClone(bs()),
    listBackups: async (folder: string | null) => {
      h.calls.list.push(folder);
      if (folder === null) return bs().settings.folder ? structuredClone(h.list) : [];
      if (h.otherRefusal) throw h.otherRefusal;
      return structuredClone(h.other);
    },
    onBackupsChanged: (cb: (s: unknown) => void) => { h.backupListeners.add(cb); return Promise.resolve(() => { h.backupListeners.delete(cb); }); },
    restoreStatus: () => new Promise((res) => {
      const now = structuredClone(rs());
      if (h.holdStatus) h.heldStatus.push(() => res(now)); else res(now);
    }),
    startRestore: async (folder: string, name: string) => {
      h.calls.start.push([folder, name]);
      if (bs().running) throw "A backup is running: restore once it is done";
      if (["checking", "saving", "staging"].includes(rs().state)) throw "A restore is running already";
      Object.assign(rs(), { state: "checking", folder: `${folder}/Gizai backups`, snapshot: null, step: "Checking the snapshot", error: null, savedToday: null });
      sendRestore();
      return structuredClone(rs());
    },
    cancelRestore: async () => {
      h.calls.cancel++;
      if (h.cancelRefusal) throw h.cancelRefusal;
      Object.assign(rs(), { state: "none", folder: null, snapshot: null, step: null, error: null, savedToday: null });
      sendRestore();
      return structuredClone(rs());
    },
    restartToRestore: async () => {
      h.calls.restart++;
      if (h.restartRefusal) throw h.restartRefusal;
      if (rs().state !== "ready") throw "No restore is ready: restore a snapshot first";
    },
    dismissRestoreResult: async () => {
      h.calls.dismiss++;
      if (h.dismissRefusal) throw h.dismissRefusal;
      rs().result = null;
      sendRestore();
      return structuredClone(rs());
    },
    onRestoreChanged: (cb: (s: unknown) => void) => { h.restoreListeners.add(cb); return Promise.resolve(() => { h.restoreListeners.delete(cb); }); },
    liveRuns: async () => structuredClone(h.live),
    restartGizai: async () => { h.calls.update++; },
  };
});

const { BackupSettings } = await import("./BackupSettings");
const { RestoreNotice } = await import("./RestoreNotice");
const { RestartButton } = await import("./UpdateNotice");

// ---- the hook runtime ----
let root: () => ReactNode = () => null;
const insts = new Map<string, Inst>();
const unmount = (inst: Inst) => { for (const s of inst.slots) if ((s as EffectSlot)?.effect) (s as EffectSlot).cleanup?.(); };
const keyOf = (n: unknown) => (n && typeof n === "object" && "key" in n ? (n as { key: string | null }).key : null);
function expand(node: ReactNode, path: string, seen: Set<string>): ReactNode {
  if (Array.isArray(node)) return node.map((n, i) => expand(n, `${path}.${keyOf(n) ?? i}`, seen));
  if (!node || typeof node !== "object" || !("props" in node)) return node;
  const el = node as ReactElement<Record<string, unknown>>;
  if (typeof el.type === "function") {
    const key = `${path}>${el.type.name}${el.key !== null ? `#${el.key}` : ""}`;
    seen.add(key);
    let inst = insts.get(key);
    if (!inst) insts.set(key, (inst = { slots: [], i: 0 }));
    inst.i = 0;
    h.cur = inst;
    let out: ReactNode;
    try { out = (el.type as (p: unknown) => ReactNode)(el.props); } finally { h.cur = null; }
    return expand(out, key, seen);
  }
  return { ...el, props: { ...el.props, children: expand(el.props.children as ReactNode, `${path}>${String(el.type)}`, seen) } } as ReactNode;
}
let tree: ReactNode = null;
function render() {
  for (let n = 0; n < 30; n++) {
    h.dirty = false;
    const seen = new Set<string>();
    tree = expand(root(), "", seen);
    for (const [k, inst] of insts) if (!seen.has(k)) { unmount(inst); insts.delete(k); }
    for (const f of h.effects.splice(0)) f();
    if (!h.dirty) return;
  }
  throw new Error("the render never settled");
}
async function settle() { for (let i = 0; i < 4; i++) { await vi.advanceTimersByTimeAsync(0); render(); } }

type Found = { type: unknown; props: Record<string, unknown> };
function findAll(node: ReactNode, pred: (f: Found) => boolean, out: Found[] = []): Found[] {
  if (Array.isArray(node)) node.forEach((n) => findAll(n, pred, out));
  else if (node && typeof node === "object" && "props" in node) {
    const el = node as ReactElement<Record<string, unknown>>;
    const f = { type: el.type, props: el.props };
    if (pred(f)) out.push(f);
    findAll(el.props.children as ReactNode, pred, out);
  }
  return out;
}
function textOf(node: ReactNode): string {
  if (typeof node === "string" || typeof node === "number") return `${node}`;
  if (Array.isArray(node)) return node.map(textOf).join("");
  if (node && typeof node === "object" && "props" in node) return textOf((node as ReactElement<{ children?: ReactNode }>).props.children);
  return "";
}
const kids = (f: Found) => f.props.children as ReactNode;

/** The section or the sidebar's notice, as rendered side by side. */
type Where = "section" | "sidebar";
const area = (where: Where) => kids(findAll(tree, (f) => f.props["data-where"] === where)[0]!);
const text = (where: Where) => textOf(area(where));
function buttonIn(where: Where, name: string) {
  const all = findAll(area(where), (f) => f.type === "button" && textOf(kids(f)).trim() === name);
  if (all.length !== 1) throw new Error(`${all.length} buttons called ${name} in the ${where}`);
  return all[0]!.props as { onClick: () => unknown; disabled?: boolean };
}
const hasButton = (where: Where, name: string) => findAll(area(where), (f) => f.type === "button" && textOf(kids(f)).trim() === name).length > 0;
async function press(where: Where, name: string) { await buttonIn(where, name).onClick(); await settle(); }

/** A list's rows, by the list's aria-label: what each says, and its Restore when it has one. */
const THIS_FOLDER = "Backups in this folder", OTHER_FOLDER = "Backups in the other folder";
function rows(label: string) {
  const ul = findAll(tree, (f) => f.type === "ul" && f.props["aria-label"] === label);
  return ul.flatMap((u) => findAll(kids(u), (f) => f.type === "li")).map((li) => {
    const b = findAll(kids(li), (f) => f.type === "button");
    return {
      what: textOf(kids(findAll(kids(li), (f) => f.props.className === "bk-what")[0]!)),
      why: findAll(kids(li), (f) => f.props.className === "warn").map((f) => textOf(kids(f))).join(""),
      restore: b[0]?.props as { onClick: () => unknown; disabled?: boolean; "aria-label"?: string } | undefined,
      buttons: b.map((f) => textOf(kids(f)).trim()),
    };
  });
}
/** Whether each Restore in the list of the folder in Settings is off. */
const restoresOff = () => rows(THIS_FOLDER).flatMap((x) => (x.restore ? [!!x.restore.disabled] : []));
function field(label: string): Found {
  const all = findAll(tree, (f) => f.type === "div" && /^field( |$)/.test(String(f.props.className))
    && textOf((findAll(kids(f), (n) => n.type === "label")[0]?.props.children ?? "") as ReactNode) === label);
  if (all.length !== 1) throw new Error(`${all.length} fields called ${label}`);
  return all[0]!;
}
const hasField = (label: string) => findAll(tree, (f) => f.type === "label" && textOf(kids(f)) === label).length > 0;
/** The red line under a field. */
function errorUnder(label: string): string {
  return findAll(kids(field(label)), (f) => f.props.className === "error").map((f) => textOf(kids(f))).join("");
}
const dialogs = () => findAll(tree, (f) => f.props.role === "alertdialog");
const dialog = () => {
  const d = dialogs()[0]!;
  return { title: textOf(kids(findAll(kids(d), (f) => f.type === "h2")[0]!)), text: textOf(kids(findAll(kids(d), (f) => f.type === "p")[0]!)),
    buttons: findAll(kids(d), (f) => f.type === "button"), props: d.props };
};
const dialogButton = (name: string) => dialog().buttons.find((b) => textOf(kids(b)) === name)!.props as { onClick: () => unknown };
/** What a status or an alert says (the section's banner has its Dismiss inside it). */
const roleIn = (where: Where, role: string) => findAll(area(where), (f) => f.props.role === role).map((f) => textOf(kids(f)).replace(/Dismiss$/, ""));

// local times, so "today" and "yesterday" don't depend on the time zone the tests run in
const NOW = new Date(2026, 9, 27, 14, 0).getTime();
const at = (day: number, hh: number, mm = 0) => new Date(2026, 9, day, hh, mm).getTime();
const entry = (over: Partial<BackupEntry>): BackupEntry => ({
  name: "gizai-daily-20261027-110000-000-ce82a749", reason: "daily", madeAt: at(27, 11), deviceId: "me", deviceName: "this machine",
  thisComputer: true, appVersion: "0.15.0", schemaVersion: 40, dbBytes: 4_456_448, filesCount: 14, filesBytes: 3_276_800, filesMissing: 0,
  restorable: true, problem: null, ...over,
});
const NEWER = "A newer Hersir made it (0.16.0): update Hersir to restore it";
const LIST: BackupEntry[] = [
  entry({}),
  entry({ name: "gizai-manual-20261026-090000-000-ce82a749", reason: "manual", madeAt: at(26, 9) }),
  entry({ name: "gizai-daily-20261025-080000-000-0199f3a2", madeAt: at(25, 8), deviceId: "laptop", deviceName: "work-laptop", thisComputer: false,
    appVersion: "0.16.0", schemaVersion: 41, restorable: false, problem: NEWER }),
  entry({ name: "gizai-weekly-20261021-080000-000-77aa0011", reason: "weekly", madeAt: at(21, 8), deviceId: "nas", deviceName: "nas", thisComputer: false }),
];
const USB: BackupEntry[] = [
  entry({ name: "gizai-quit-20261024-180000-000-0199f3a2", reason: "quit", madeAt: at(24, 18), deviceId: "laptop", deviceName: "work-laptop", thisComputer: false }),
  entry({ name: "gizai-daily-20261023-080000-000-0199f3a2", madeAt: at(23, 8), deviceId: "laptop", deviceName: "work-laptop", thisComputer: false, filesMissing: 3, restorable: false, problem: "3 files are missing" }),
];
const STATUS: BackupStatus = {
  settings: { folder: "/home/you/Dropbox", every: "daily", onQuit: false, keep: 10 },
  folder: "/home/you/Dropbox/Gizai backups", folderProblem: null, running: null,
  last: { name: LIST[0]!.name, madeAt: at(27, 11), bytes: 1_153_434 }, lastError: null, next: at(28, 11),
};
const OFF: BackupStatus = { ...STATUS, settings: { ...STATUS.settings, folder: null }, folder: null, last: null, next: null };
const NONE: RestoreStatus = { state: "none", folder: null, snapshot: null, step: null, error: null, savedToday: null, result: null };
const AFTER = "After a restart, Hersir shows your data as it was then: what changed since, also until the restart, is not kept. "
  + "Keys and passwords stay as they are on this computer. Agents are paused afterwards.";
const OK_RESULT = { ok: true, at: NOW, snapshot: LIST[1]!.name, fromComputer: "work-laptop", message: "Hersir restored it." };
const OK_TEXT = "Restored the backup of yesterday 09:00 from work-laptop. Agents are paused: resume them in Settings → Agents and runs. Today's data was saved as a backup first.";
const FAILED_RESULT = { ok: false, at: NOW, snapshot: LIST[1]!.name, fromComputer: "work-laptop",
  message: `Hersir couldn't open the restored data of ${LIST[1]!.name}, so it started with today's data: file is not a database.` };
const FAILED_TEXT = `The restore failed: Hersir couldn't open the restored data of ${LIST[1]!.name}, so it started with today's data: file is not a database. Hersir started with today's data.`;
const READY: RestoreStatus = { ...NONE, state: "ready", folder: STATUS.folder, snapshot: LIST[1]!, savedToday: "gizai-before-restore-20261027-135900-000-ce82a749" };

async function show({ backup = STATUS, list = LIST, restore = NONE }: { backup?: BackupStatus; list?: BackupEntry[]; restore?: RestoreStatus } = {}) {
  h.backup = structuredClone(backup);
  h.list = structuredClone(list);
  h.restore = structuredClone(restore);
  root = () => (
    <>
      <div data-where="section"><BackupSettings /></div>
      <div data-where="sidebar"><RestoreNotice /></div>
    </>
  );
  render();
  await settle();
}
/** The backend moves on: a step, a state or how it ended, with `restore-changed`. */
async function backendSays(change: Partial<RestoreStatus>) {
  Object.assign(rs(), change);
  sendRestore();
  await settle();
}
/** Presses Restore on a row, then Restore in the confirm. */
async function restoreRow(label: string, i: number) {
  await rows(label)[i]!.restore!.onClick();
  await settle();
  await dialogButton("Restore").onClick();
  await settle();
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
  for (const [k, inst] of insts) { unmount(inst); insts.delete(k); }
  h.effects.length = 0; h.pickerOpts.length = 0; h.backupListeners.clear(); h.restoreListeners.clear(); h.heldStatus.length = 0;
  h.calls = { start: [], cancel: 0, restart: 0, dismiss: 0, update: 0, list: [] };
  h.picked = null; h.pickRefusal = null; h.otherRefusal = null; h.cancelRefusal = null; h.dismissRefusal = null; h.restartRefusal = null;
  h.other = structuredClone(USB); h.live = []; h.holdStatus = false;
});
afterEach(() => { root = () => null; render(); vi.useRealTimers(); });

describe("Restore on each snapshot in the list", () => {
  it("one that can be restored has Restore, one that can't says why and has none", async () => {
    await show();
    const r = rows(THIS_FOLDER);
    expect(r.map((x) => x.buttons)).toEqual([["Restore"], ["Restore"], [], ["Restore"]]);
    expect(r.map((x) => x.why)).toEqual(["", "", NEWER, ""]);
    for (const x of r) if (x.restore) expect(x.restore.disabled).toBe(false);
  });

  it.each(["Its manifest can't be read", "Its database file is missing", "1 file is missing", "3 files are missing"])("the backend's %j shows, without Restore", async (problem) => {
    await show({ list: [entry({ restorable: false, problem })] });
    expect(rows(THIS_FOLDER)).toMatchObject([{ why: problem, buttons: [] }]);
  });

  it("one the backend gives no reason for still says it can't be restored", async () => {
    await show({ list: [entry({ restorable: false, problem: null })] });
    expect(rows(THIS_FOLDER)).toMatchObject([{ why: "It can't be restored.", buttons: [] }]);
  });

  it("a screen reader hears which backup each Restore is", async () => {
    await show();
    expect(rows(THIS_FOLDER)[0]!.restore!["aria-label"]).toBe("Restore the backup of today 11:00");
    expect(rows(THIS_FOLDER)[1]!.restore!["aria-label"]).toBe("Restore the backup of yesterday 09:00");
  });

  it("is off while a backup runs, on again after it", async () => {
    await show();
    Object.assign(bs(), { running: { reason: "daily", startedAt: NOW, step: "database" } });
    sendBackups();
    await settle();
    expect(restoresOff()).toEqual([true, true, true]);
    Object.assign(bs(), { running: null });
    sendBackups();
    await settle();
    expect(restoresOff()).toEqual([false, false, false]);
  });

  it("is off while a restore runs, and on again when it fails or is ready (another restore takes a ready one's place)", async () => {
    await show();
    for (const state of ["checking", "saving", "staging"] as const) {
      await backendSays({ state, step: "A step" });
      expect(restoresOff(), state).toEqual([true, true, true]);
    }
    for (const state of ["ready", "failed"] as const) {
      await backendSays({ state, step: null });
      expect(restoresOff(), state).toEqual([false, false, false]);
    }
  });

  it("is off the same way in another folder's list", async () => {
    await show();
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    expect(rows(OTHER_FOLDER)[0]!.restore!.disabled).toBe(false);
    await backendSays({ state: "checking", step: "A step" });
    expect(rows(OTHER_FOLDER)[0]!.restore!.disabled).toBe(true);
  });

  it("starts with the folder in Settings and the snapshot's name, only after the confirm", async () => {
    await show();
    await rows(THIS_FOLDER)[1]!.restore!.onClick();
    await settle();
    expect(h.calls.start).toEqual([]);
    await dialogButton("Restore").onClick();
    await settle();
    expect(h.calls.start).toEqual([["/home/you/Dropbox", LIST[1]!.name]]);
    expect(dialogs()).toEqual([]);
  });

  it("the list is read again when a before-restore snapshot is made, so today's data shows in it", async () => {
    await show();
    const before = h.calls.list.length;
    Object.assign(bs(), { running: { reason: "before-restore", startedAt: NOW, step: "database" } });
    sendBackups();
    await settle();
    h.list = [entry({ name: "gizai-before-restore-20261027-140000-000-ce82a749", reason: "before-restore", madeAt: NOW }), ...LIST];
    Object.assign(bs(), { running: null, last: { name: "gizai-before-restore-20261027-140000-000-ce82a749", madeAt: NOW, bytes: 1 } });
    sendBackups();
    await settle();
    expect(h.calls.list.length).toBeGreaterThan(before);
    expect(rows(THIS_FOLDER)[0]!.what).toBe("Before a restore");
  });
});

describe("Restore from another folder…", () => {
  it("is there with no folder in Settings, and opens the system's folder picker", async () => {
    await show({ backup: OFF, list: [] });
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    expect(h.pickerOpts).toEqual([{ directory: true, multiple: false, title: "Choose a folder with backups" }]);
  });

  it("lists that folder's snapshots, also another computer's, each with Restore or why not", async () => {
    await show();
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    expect(h.calls.list).toContain("/media/you/USB");
    expect(text("section")).toContain("/media/you/USB");
    const r = rows(OTHER_FOLDER);
    expect(r.map((x) => x.what)).toEqual(["On quit · from work-laptop", "Daily · from work-laptop"]);
    expect(r[0]).toMatchObject({ buttons: ["Restore"], why: "" });
    expect(r[1]).toMatchObject({ buttons: [], why: "3 files are missing" });
    // the folder in Settings keeps its own list
    expect(rows(THIS_FOLDER)).toHaveLength(LIST.length);
  });

  it("a Restore there restores from that folder", async () => {
    await show();
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    await restoreRow(OTHER_FOLDER, 0);
    expect(h.calls.start).toEqual([["/media/you/USB", USB[0]!.name]]);
  });

  it("a picker that is closed changes nothing", async () => {
    await show();
    await press("section", "Restore from another folder…");
    expect(h.calls.list).toEqual([null]);
    expect(rows(OTHER_FOLDER)).toEqual([]);
  });

  it("says so when the folder holds no backups", async () => {
    h.other = [];
    await show();
    h.picked = "/media/you/empty";
    await press("section", "Restore from another folder…");
    expect(text("section")).toContain("No backups in this folder.");
  });

  it("a folder that can't be read says why", async () => {
    h.otherRefusal = "The folder isn't there: is the disk plugged in?";
    await show();
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    expect(text("section")).toContain("The folder isn't there: is the disk plugged in?");
    expect(rows(OTHER_FOLDER)).toEqual([]);
  });

  it("a picker that fails says why", async () => {
    h.pickRefusal = "The dialog couldn't open";
    await show();
    await press("section", "Restore from another folder…");
    expect(text("section")).toContain("The dialog couldn't open");
  });

  it("the same folder picked again is read again, as a USB disk may have changed", async () => {
    await show();
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    const reads = h.calls.list.filter((f) => f === "/media/you/USB").length;
    h.other = [USB[0]!];
    await press("section", "Restore from another folder…");
    expect(h.calls.list.filter((f) => f === "/media/you/USB").length).toBe(reads + 1);
    expect(rows(OTHER_FOLDER)).toHaveLength(1);
  });

  it("is read again once today's data is saved there, and the X closes it", async () => {
    await show({ backup: OFF, list: [] });
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    const saved = entry({ name: "gizai-before-restore-20261027-140000-000-ce82a749", reason: "before-restore", madeAt: NOW });
    h.other = [saved, ...USB];
    await backendSays({ state: "staging", step: "Copying the database", savedToday: saved.name });
    expect(rows(OTHER_FOLDER)[0]!.what).toBe("Before a restore");
    const close = findAll(area("section"), (f) => f.type === "button" && f.props["aria-label"] === "Close this folder's backups")[0]!;
    await (close.props.onClick as () => void)();
    await settle();
    expect(rows(OTHER_FOLDER)).toEqual([]);
    expect(text("section")).not.toContain("/media/you/USB");
  });
});

describe("the confirm", () => {
  it("shows the card's title and text: the backup's reason, time and computer, and the folder in Settings; Cancel and Restore", async () => {
    await show();
    await rows(THIS_FOLDER)[1]!.restore!.onClick();
    await settle();
    expect(dialog().title).toBe("Restore this backup?");
    expect(dialog().text).toBe(`By hand, yesterday 09:00, from this computer. Hersir first saves today's data as a backup in /home/you/Dropbox/Gizai backups. ${AFTER}`);
    expect(dialog().buttons.map((b) => textOf(kids(b)))).toEqual(["Cancel", "Restore"]);
    expect(dialog().buttons.map((b) => b.props.className)).toEqual(["btn ghost", "btn primary"]);
  });

  it("another computer's backup in the list names that computer", async () => {
    await show();
    await rows(THIS_FOLDER)[3]!.restore!.onClick();
    await settle();
    expect(dialog().text).toMatch(/^Weekly, 21 Oct, 08:00, from nas\. /);
  });

  it("another folder's backup goes to the backups folder in Settings when there is one", async () => {
    await show();
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    await rows(OTHER_FOLDER)[0]!.restore!.onClick();
    await settle();
    expect(dialog().text).toContain("from work-laptop. Hersir first saves today's data as a backup in /home/you/Dropbox/Gizai backups. ");
  });

  it("with no backups folder in Settings, today's data goes to the folder restored from", async () => {
    await show({ backup: OFF, list: [] });
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    await rows(OTHER_FOLDER)[0]!.restore!.onClick();
    await settle();
    expect(dialog().text).toContain("Hersir first saves today's data as a backup in /media/you/USB. ");
  });

  it("with a backups folder that can't be used now, today's data goes to the folder restored from", async () => {
    await show({ backup: { ...STATUS, folderProblem: "The folder isn't there: is the disk plugged in?" } });
    h.picked = "/media/you/USB";
    await press("section", "Restore from another folder…");
    await rows(OTHER_FOLDER)[0]!.restore!.onClick();
    await settle();
    expect(dialog().text).toContain("as a backup in /media/you/USB. ");
  });

  it("Cancel changes nothing: no restore starts, and the section is as it was", async () => {
    await show();
    const before = text("section");
    await rows(THIS_FOLDER)[0]!.restore!.onClick();
    await settle();
    await dialogButton("Cancel").onClick();
    await settle();
    expect(dialogs()).toEqual([]);
    expect(h.calls).toMatchObject({ start: [], cancel: 0, restart: 0, dismiss: 0 });
    expect(rs()).toEqual(NONE);
    expect(text("section")).toBe(before);
    expect(hasField("Restore")).toBe(false);
  });

  it("Escape and a click beside it cancel too; a click in it doesn't", async () => {
    await show();
    await rows(THIS_FOLDER)[0]!.restore!.onClick();
    await settle();
    const stop = vi.fn();
    (dialog().props.onKeyDown as (e: unknown) => void)({ key: "Escape", preventDefault: stop, stopPropagation: stop });
    await settle();
    expect(dialogs()).toEqual([]);
    await rows(THIS_FOLDER)[0]!.restore!.onClick();
    await settle();
    const scrim = findAll(tree, (f) => f.props.className === "ask-scrim")[0]!.props.onMouseDown as (e: unknown) => void;
    scrim({ target: "in the box", currentTarget: "the scrim" });
    await settle();
    expect(dialogs()).toHaveLength(1);
    scrim({ target: "the scrim", currentTarget: "the scrim" });
    await settle();
    expect(dialogs()).toEqual([]);
    expect(h.calls.start).toEqual([]);
  });
});

describe("while it runs", () => {
  it("each step shows, in the section and in the sidebar", async () => {
    await show();
    await restoreRow(THIS_FOLDER, 0);
    const steps: [RestoreStatus["state"], string][] = [
      ["checking", "Checking the snapshot"], ["checking", "Checking the database"], ["checking", "Checking files (3 of 14)"],
      ["saving", "Saving today's data"], ["staging", "Copying the database"], ["staging", "Copying files (120 of 300)"],
    ];
    for (const [state, step] of steps) {
      await backendSays({ state, step, snapshot: LIST[0]! });
      expect(roleIn("section", "status"), step).toEqual([`${step}…`]);
      expect(text("sidebar"), step).toBe(`${step}…`);
    }
  });

  it("the sidebar's step opens Settings, where the section is", async () => {
    await show();
    await backendSays({ state: "checking", step: "Checking the database" });
    const link = findAll(area("sidebar"), (f) => f.type === "a")[0]!;
    expect(link.props.href).toBe("#/settings");
  });

  it("no step yet: it still says it started", async () => {
    await show();
    await backendSays({ state: "checking", step: null });
    expect(roleIn("section", "status")).toEqual(["Starting the restore…"]);
  });

  it("Cancel restore stops it, and the steps go in both places", async () => {
    await show();
    await backendSays({ state: "saving", step: "Saving today's data" });
    await press("section", "Cancel restore");
    expect(h.calls.cancel).toBe(1);
    expect(hasField("Restore")).toBe(false);
    expect(text("sidebar")).toBe("");
  });

  it.each([
    "Made by a newer Hersir (backup format 2). Update Hersir first.",
    "Made by a newer Hersir (database version 99). Update Hersir first.",
    "1 file of the snapshot is missing from /media/you/USB/Gizai backups/files",
    "12 files of the snapshot are missing from /media/you/USB/Gizai backups/files",
    "The snapshot's database file is missing: /media/you/USB/Gizai backups/snapshots/x.db.zst",
    "Hersir can't save today's data first: no room. Choose a backups folder in Settings → General → Backups.",
  ])("a failure shows its reason: %s", async (why) => {
    await show();
    await restoreRow(THIS_FOLDER, 0);
    await backendSays({ state: "failed", step: null, error: why });
    expect(roleIn("section", "alert")).toEqual([`The restore failed: ${why}`]);
    expect(roleIn("section", "status")).toEqual([]);
    // the sidebar says so, and has the reason in its tooltip and the section's link
    expect(text("sidebar")).toBe("The restore failed");
    const link = findAll(area("sidebar"), (f) => f.type === "a")[0]!;
    expect([link.props.title, link.props.href]).toEqual([why, "#/settings"]);
  });

  it("a failure stays until Dismiss, which clears it with cancel_restore, the only way the backend has", async () => {
    await show();
    await backendSays({ state: "failed", step: null, error: "1 file of the snapshot is missing from /x" });
    await settle();
    expect(roleIn("section", "alert")).toHaveLength(1);
    await press("section", "Dismiss");
    expect(h.calls.cancel).toBe(1);
    expect(hasField("Restore")).toBe(false);
    expect(text("sidebar")).toBe("");
  });

  it.each([["a backup runs", "A backup is running: restore once it is done"], ["another restore runs", "A restore is running already"]])("a refusal while %s shows the backend's reason under Restore", async (what, why) => {
    await show();
    await rows(THIS_FOLDER)[0]!.restore!.onClick();
    await settle();
    // it starts between the question and the answer: the Restore buttons were on when it was asked
    if (what === "a backup runs") Object.assign(bs(), { running: { reason: "daily", startedAt: NOW, step: "database" } });
    else Object.assign(rs(), { state: "checking", step: "Checking the snapshot" });
    await dialogButton("Restore").onClick();
    await settle();
    expect(h.calls.start).toHaveLength(1);
    expect(errorUnder("Restore")).toBe(why);
  });

  it("the next Restore clears an earlier refusal", async () => {
    await show();
    await rows(THIS_FOLDER)[0]!.restore!.onClick();
    await settle();
    Object.assign(bs(), { running: { reason: "daily", startedAt: NOW, step: "database" } });
    await dialogButton("Restore").onClick();
    await settle();
    Object.assign(bs(), { running: null });
    sendBackups();
    await settle();
    expect(errorUnder("Restore")).not.toBe("");
    await restoreRow(THIS_FOLDER, 0);
    expect(errorUnder("Restore")).toBe("");
    expect(roleIn("section", "status")).toEqual(["Checking the snapshot…"]);
  });
});

describe("ready", () => {
  it("shows in the section and in the sidebar with the time of the backup, Restart now and Cancel restore", async () => {
    await show({ restore: READY });
    expect(roleIn("section", "status")).toEqual(["Restore ready: yesterday 09:00"]);
    expect(roleIn("sidebar", "status")).toEqual(["Restore ready: yesterday 09:00"]);
    for (const where of ["section", "sidebar"] as const) {
      expect(hasButton(where, "Restart now"), where).toBe(true);
      expect(hasButton(where, "Cancel restore"), where).toBe(true);
    }
  });

  it("comes from the steps' last event, without a reload", async () => {
    await show();
    await restoreRow(THIS_FOLDER, 1);
    await backendSays({ state: "ready", step: null, snapshot: LIST[1]!, savedToday: "gizai-before-restore-20261027-135900-000-ce82a749" });
    expect(text("sidebar")).toContain("Restore ready: yesterday 09:00");
  });

  it("whose snapshot can't be found now (the USB disk is out) still shows", async () => {
    await show({ restore: { ...READY, snapshot: null } });
    expect(roleIn("sidebar", "status")).toEqual(["Restore ready"]);
    expect(hasButton("sidebar", "Restart now")).toBe(true);
  });

  it.each(["section", "sidebar"] as const)("Restart now in the %s calls restart_to_restore, not the update's restart", async (where) => {
    await show({ restore: READY });
    await press(where, "Restart now");
    expect(h.calls.restart).toBe(1);
    expect(h.calls.update).toBe(0);
    // it shows it is busy until Hersir quits
    expect(findAll(area(where), (f) => f.type === "button" && textOf(kids(f)).trim() === "Restarting…")).toHaveLength(1);
  });

  it.each(["section", "sidebar"] as const)("Cancel restore in the %s removes the notice from both", async (where) => {
    await show({ restore: READY });
    await press(where, "Cancel restore");
    expect(h.calls.cancel).toBe(1);
    expect(h.calls.restart).toBe(0);
    expect(hasField("Restore")).toBe(false);
    expect(text("sidebar")).toBe("");
    expect(text("section")).not.toContain("Restore ready");
  });

  it("a Restart now the backend refuses says why, and can be pressed again", async () => {
    await show({ restore: READY });
    h.restartRefusal = "Couldn't restart Hersir: no such file";
    await press("sidebar", "Restart now");
    expect(roleIn("sidebar", "alert")).toEqual(["Couldn't restart Hersir: no such file"]);
    expect(buttonIn("sidebar", "Restart now").disabled).toBe(false);
  });

  it("a Cancel restore the backend refuses says why, and the notice stays", async () => {
    await show({ restore: READY });
    h.cancelRefusal = "Couldn't remove the staged restore: permission denied";
    await press("sidebar", "Cancel restore");
    expect(roleIn("sidebar", "alert")).toEqual(["Couldn't remove the staged restore: permission denied"]);
    expect(hasButton("sidebar", "Restart now")).toBe(true);
    await press("section", "Cancel restore");
    expect(errorUnder("Restore")).toBe("Couldn't remove the staged restore: permission denied");
  });

  it("with agents at work, Restart now asks once more first", async () => {
    h.live = [{ runId: "r1" }];
    await show({ restore: READY });
    const ask = "An agent is at work: restarting stops it. Restart anyway?";
    await press("sidebar", "Restart now");
    expect(h.calls.restart).toBe(0);
    await press("sidebar", ask);
    expect(h.calls.restart).toBe(1);
  });

  it("without `restart`, RestartButton is still the update's: the update's restart, not the restore's", async () => {
    h.restore = structuredClone(READY);
    root = () => <div data-where="section"><RestartButton className="x" label="Restart to use 1.2.3" /></div>;
    render();
    await settle();
    await press("section", "Restart to use 1.2.3");
    expect([h.calls.update, h.calls.restart]).toEqual([1, 0]);
  });
});

describe("after the start", () => {
  it("ok: the card's text in a banner at the top of the section and in the sidebar", async () => {
    await show({ restore: { ...NONE, result: OK_RESULT } });
    expect(roleIn("section", "status")).toEqual([OK_TEXT]);
    expect(roleIn("sidebar", "status")).toEqual([OK_TEXT]);
    // above the section's fields
    expect(text("section").indexOf(OK_TEXT)).toBeLessThan(text("section").indexOf("Folder"));
    expect(findAll(area("section"), (f) => String(f.props.className).includes("bk-result ok"))).toHaveLength(1);
  });

  it("failed: the card's text, as an alert, in both", async () => {
    await show({ restore: { ...NONE, result: FAILED_RESULT } });
    expect(roleIn("section", "alert")).toEqual([FAILED_TEXT]);
    expect(roleIn("sidebar", "alert")).toEqual([FAILED_TEXT]);
    expect(findAll(area("section"), (f) => String(f.props.className).includes("bk-result failed"))).toHaveLength(1);
  });

  it("stays when the section is left and opened again, until Dismiss", async () => {
    await show({ restore: { ...NONE, result: OK_RESULT } });
    root = () => <div data-where="sidebar"><RestoreNotice /></div>;
    render();
    await settle();
    expect(text("sidebar")).toContain(OK_TEXT);
    expect(h.calls.dismiss).toBe(0);
    await show({ restore: rs() });
    expect(roleIn("section", "status")).toEqual([OK_TEXT]);
  });

  it.each(["section", "sidebar"] as const)("Dismiss in the %s calls dismiss_restore_result and removes it from both", async (where) => {
    await show({ restore: { ...NONE, result: OK_RESULT } });
    await press(where, "Dismiss");
    expect(h.calls.dismiss).toBe(1);
    expect(h.calls.cancel).toBe(0);
    expect(text("section")).not.toContain("Restored the backup");
    expect(text("sidebar")).toBe("");
  });

  it("a Dismiss the backend refuses says why, and the result stays", async () => {
    await show({ restore: { ...NONE, result: FAILED_RESULT } });
    h.dismissRefusal = "database is locked";
    await press("sidebar", "Dismiss");
    expect(roleIn("sidebar", "alert")).toEqual([FAILED_TEXT, "database is locked"]);
    await press("section", "Dismiss");
    expect(errorUnder("Restore")).toBe("database is locked");
    expect(text("section")).toContain(FAILED_TEXT);
  });

  it("a new restore doesn't hide a result that is still there", async () => {
    await show({ restore: { ...NONE, result: OK_RESULT } });
    await restoreRow(THIS_FOLDER, 0);
    expect(roleIn("section", "status")).toEqual([OK_TEXT, "Checking the snapshot…"]);
    expect(text("sidebar")).toContain(OK_TEXT);
    expect(text("sidebar")).toContain("Checking the snapshot…");
  });
});

describe("with nothing to show", () => {
  it("no restore and no result: no Restore field and nothing in the sidebar", async () => {
    await show();
    expect(hasField("Restore")).toBe(false);
    expect(text("sidebar")).toBe("");
    expect(hasButton("section", "Restart now")).toBe(false);
    expect(hasButton("section", "Cancel restore")).toBe(false);
    expect(hasButton("section", "Dismiss")).toBe(false);
  });

  it("the sidebar shows nothing before the status comes", async () => {
    h.holdStatus = true;
    await show();
    expect(text("sidebar")).toBe("");
  });
});

describe("restore-changed", () => {
  it("wins over a status answer that is older than the event", async () => {
    h.holdStatus = true;
    await show();
    await backendSays({ state: "ready", step: null, snapshot: LIST[1]! });
    expect(text("sidebar")).toContain("Restore ready");
    h.holdStatus = false;
    for (const f of h.heldStatus.splice(0)) f();
    await settle();
    expect(text("sidebar")).toContain("Restore ready");
    expect(roleIn("section", "status")).toEqual(["Restore ready: yesterday 09:00"]);
  });

  it("each place listens once and stops when it goes", async () => {
    await show();
    expect(h.restoreListeners.size).toBe(2);
    root = () => null;
    render();
    expect(h.restoreListeners.size).toBe(0);
  });
});
