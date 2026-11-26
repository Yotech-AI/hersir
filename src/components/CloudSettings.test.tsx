// GA-360 QA: Settings → Hersir Cloud (CloudSettings.tsx, CloudRestore.tsx) against GA-317's design (design/hersir-cloud.md:
// sheet A's states A1 to A9 and frame 1, "Every text, word for word") and GA-359's commands, which are mocked here. Each
// state shows from its CloudStatus: signed out, signing in, the recovery code (which can't be skipped: nothing backs up
// before its last group is typed back), signed in, a backup running, a failure, the quota full, a lapsed plan, Make a new
// recovery code, the computers with Remove…, the account page, and Restore from Hersir Cloud through to Restart now. The
// texts are the design's, typed out here rather than taken from src/lib/cloud.ts, so a changed word fails. Refusals are
// the commands' sentences, shown as they are. Dark and light: the new styles use only the theme's variables, each set in
// both themes. There is no DOM here, so a small hook runtime stands in for React (as in BackupSettings.test.tsx), with a
// fake Hersir Cloud that keeps the status and sends `cloud-changed` as research 5.4 says.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import type { CloudBackup, CloudComputer, CloudStatus, RestoreStatus } from "../types";

type Inst = { slots: unknown[]; i: number };
type EffectSlot = { effect: true; deps?: unknown[]; first: boolean; cleanup?: () => void };
const h = vi.hoisted(() => ({
  cur: null as Inst | null,
  dirty: false,
  effects: [] as (() => void)[],
  status: null as unknown,
  afterSignIn: null as unknown,
  list: [] as unknown[],
  computers: [] as unknown[],
  restore: null as unknown,
  calls: [] as [string, unknown[]][],
  cloudCbs: new Set<(s: unknown) => void>(),
  restoreCbs: new Set<(s: unknown) => void>(),
  refuse: {} as Record<string, string>,
  signIn: null as null | { done: () => void; fail: (why: string) => void },
  opened: [] as string[],
  copied: [] as string[],
  files: [] as { name: string; blob: Blob }[],
  shown: true,
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
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: async (url: string) => { h.opened.push(url); } }));
// Restart now asks first while agents work; none do here
vi.mock("../lib/useLiveRuns", () => ({ useLiveRuns: () => [] }));
vi.mock("./chat/useChat", () => ({ useChatLive: () => [] }));

// ---- the fake Hersir Cloud (GA-359's side of research 5.4). Tauri rejects a command with the error's text. ----
const st = () => h.status as CloudStatus;
const clone = <T,>(x: T): T => structuredClone(x);
const sendCloud = () => { const x = clone(st()); for (const cb of [...h.cloudCbs]) cb(x); };
const sendRestore = () => { const x = clone(h.restore); for (const cb of [...h.restoreCbs]) cb(x); };
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(real)) if (typeof real[k] === "function") out[k] = () => new Promise(() => {});
  const cmd = (name: string, f: (...a: never[]) => unknown) => async (...a: unknown[]) => {
    h.calls.push([name, a]);
    if (h.refuse[name]) throw h.refuse[name];
    return (f as (...x: unknown[]) => unknown)(...a);
  };
  return {
    ...out,
    cloudStatus: cmd("cloudStatus", () => clone(st())),
    onCloudChanged: (cb: (s: unknown) => void) => { h.cloudCbs.add(cb); return Promise.resolve(() => { h.cloudCbs.delete(cb); }); },
    // opens the browser and waits until the test says how the sign-in went
    cloudSignIn: cmd("cloudSignIn", () => new Promise((resolve, reject) => {
      st().signingIn = true;
      sendCloud();
      h.signIn = {
        done: () => { h.signIn = null; h.status = clone(h.afterSignIn); sendCloud(); resolve(clone(st())); },
        fail: (why) => { h.signIn = null; st().signingIn = false; sendCloud(); reject(why); },
      };
    })),
    cloudCancelSignIn: cmd("cloudCancelSignIn", () => { h.signIn?.fail("The sign-in was cancelled."); }),
    cloudSignOut: cmd("cloudSignOut", () => { h.status = clone(SIGNED_OUT); sendCloud(); return clone(st()); }),
    cloudMakeKey: cmd("cloudMakeKey", () => ({ recoveryCode: CODE })),
    cloudConfirmRecovery: cmd("cloudConfirmRecovery", (group: string) => {
      if (group.toUpperCase() !== "Q8YC") throw "That isn't the code's last group.";
      Object.assign(st(), { key: "here", recoveryMadeAt: at(25, 14) });
      Object.assign(st().backup, { on: true, next: at(25, 15) });
      sendCloud();
      return clone(st());
    }),
    cloudEnterRecovery: cmd("cloudEnterRecovery", (code: string) => {
      if (code !== CODE) throw WRONG_CODE;
      st().key = "here";
      sendCloud();
      return clone(st());
    }),
    cloudNewRecovery: cmd("cloudNewRecovery", () => { st().recoveryMadeAt = at(25, 14); sendCloud(); return { recoveryCode: NEW_CODE }; }),
    cloudSaveSettings: cmd("cloudSaveSettings", (s: object) => {
      Object.assign(st().backup, s);
      if (st().backup.every === "manual") st().backup.next = null;
      sendCloud();
      return clone(st());
    }),
    cloudBackUpNow: cmd("cloudBackUpNow", () => {
      if (st().backup.running) throw "A backup is running already.";
      st().backup.running = { step: "copying", done: 0, total: 0 };
      sendCloud();
      return clone(st());
    }),
    cloudListBackups: cmd("cloudListBackups", () => clone(h.list)),
    cloudStartRestore: cmd("cloudStartRestore", () => clone(h.restore)),
    cloudComputers: cmd("cloudComputers", () => clone(h.computers)),
    cloudRemoveComputer: cmd("cloudRemoveComputer", (id: string) => { h.computers = h.computers.filter((c) => (c as CloudComputer).id !== id); return clone(h.computers); }),
    cloudOpenAccount: cmd("cloudOpenAccount", () => undefined),
    restoreStatus: cmd("restoreStatus", () => clone(h.restore)),
    onRestoreChanged: (cb: (s: unknown) => void) => { h.restoreCbs.add(cb); return Promise.resolve(() => { h.restoreCbs.delete(cb); }); },
    cancelRestore: cmd("cancelRestore", () => { h.restore = clone(NO_RESTORE); sendRestore(); return clone(h.restore); }),
    restartToRestore: cmd("restartToRestore", () => undefined),
  };
});

const { CloudSettings } = await import("./CloudSettings");
// Vitest hands a .css file back empty, even with ?raw, so the styles are read from disk (as codeTheme.test.ts does); node:fs
// comes in through a computed specifier, as the build type-checks tests without Node's types.
const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const readCss = (path: string): string => fs.readFileSync(new URL(`../styles/${path}`, import.meta.url), "utf8");
const appCss = readCss("app.css");
const onboardingCss = readCss("onboarding.css");
const tokensCss = readCss("tokens.css");

// ---- the hook runtime (BackupSettings.test.tsx's) ----
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
const classes = (f: Found) => String(f.props.className ?? "").split(" ");
const byClass = (c: string, at: ReactNode = tree) => findAll(at, (f) => classes(f).includes(c));
const all = () => textOf(tree);
/** The sections' titles, in order. */
const sections = () => findAll(tree, (f) => f.type === "section").map((s) => textOf(kids(findAll(kids(s), (f) => f.type === "h3")[0]!)));
/** A section's text under its title. */
const sectionText = (title: string) => {
  const s = findAll(tree, (f) => f.type === "section" && textOf(kids(findAll(kids(f), (n) => n.type === "h3")[0]!)) === title);
  if (s.length !== 1) throw new Error(`${s.length} sections called ${title}`);
  return textOf(kids(findAll(kids(s[0]!), (f) => f.type === "p")[0]!));
};
const isField = (f: Found) => f.type === "div" && /^field( |$)/.test(String(f.props.className));
const labelOf = (f: Found) => textOf((findAll(kids(f), (n) => n.type === "label")[0]?.props.children ?? "") as ReactNode);
/** The fields' labels, in order (Field's, not the restore steps'). */
const fields = () => findAll(tree, (f) => isField(f) && findAll(kids(f), (n) => n.type === "label").length > 0 && !findAll(kids(f), (n) => classes(n).includes("cloud-steps")).length)
  .map(labelOf);
function field(label: string): Found {
  const found = findAll(tree, (f) => isField(f) && labelOf(f) === label);
  if (found.length !== 1) throw new Error(`${found.length} fields called ${label}`);
  return found[0]!;
}
/** What shows under a field (its hint, warning or error, its last child), as "kind: text", or "". */
function under(label: string): string {
  const c = kids(field(label));
  const last = Array.isArray(c) ? c[c.length - 1] : null;
  if (!last || typeof last !== "object" || !("props" in last)) return "";
  const el = last as ReactElement<{ className?: string; children?: ReactNode }>;
  return el.type === "span" && ["hint", "warn", "error"].includes(String(el.props.className)) ? `${el.props.className}: ${textOf(el.props.children)}` : "";
}
/** A field's text without its label and what shows under it. */
const fieldText = (label: string) => {
  const c = kids(field(label));
  const rest = Array.isArray(c) ? c.slice(1) : [];
  const body = under(label) ? rest.slice(0, -1) : rest;
  return textOf(body as ReactNode);
};
type Btn = { onClick: () => unknown; disabled?: boolean; className?: string; "aria-label"?: string };
const buttons = (name: string, at: ReactNode = tree) => findAll(at, (f) => f.type === "button" && textOf(kids(f)).trim() === name).map((f) => f.props as Btn);
function button(name: string, at: ReactNode = tree): Btn {
  const found = buttons(name, at);
  if (found.length !== 1) throw new Error(`${found.length} buttons called ${name}`);
  return found[0]!;
}
async function click(b: Btn) { await b.onClick(); await settle(); }
type Input = { value: string; disabled?: boolean; placeholder?: string; onChange: (e: unknown) => void; onKeyDown: (e: unknown) => void; checked?: boolean; className?: string };
const input = (id: string) => {
  const found = findAll(tree, (f) => f.type === "input" && f.props.id === id);
  if (found.length !== 1) throw new Error(`${found.length} inputs #${id}`);
  return found[0]!.props as unknown as Input;
};
async function type(id: string, value: string) { input(id).onChange({ target: { value } }); render(); await settle(); }
async function enter(id: string) { input(id).onKeyDown({ key: "Enter", preventDefault() {} }); await settle(); }
const radios = () => findAll(tree, (f) => f.type === "label" && findAll(kids(f), (n) => n.type === "input" && n.props.type === "radio").length === 1)
  .map((l) => ({ label: textOf(kids(l)), input: findAll(kids(l), (n) => n.type === "input")[0]!.props as unknown as Input }));
const checkbox = () => {
  const l = findAll(tree, (f) => f.type === "label" && findAll(kids(f), (n) => n.type === "input" && n.props.type === "checkbox").length === 1);
  return { label: textOf(kids(l[0]!)), input: findAll(kids(l[0]!), (n) => n.type === "input")[0]!.props as unknown as Input };
};
const statusLines = () => findAll(tree, (f) => f.props.role === "status").map((f) => textOf(kids(f)));
const alerts = () => findAll(tree, (f) => f.props.role === "alert").map((f) => textOf(kids(f)));
const ask = () => {
  const d = findAll(tree, (f) => f.props.role === "alertdialog");
  if (d.length > 1) throw new Error("two asks");
  return d[0] ? { title: textOf(kids(findAll(kids(d[0]), (f) => f.type === "h2")[0]!)), text: textOf(kids(findAll(kids(d[0]), (f) => f.type === "p")[0]!)),
    buttons: findAll(kids(d[0]), (f) => f.type === "button").map((b) => textOf(kids(b))), node: kids(d[0]) } : null;
};
/** The restore steps: each as "state: title". */
const steps = () => byClass("cloud-step").map((li) => `${["on", "done", "later"].find((c) => classes(li).includes(c))}: ${textOf(kids(byClass("title", kids(li))[0]!))}`);
const stepBody = (n: number) => textOf(kids(byClass("body", kids(byClass("cloud-step")[n - 1]!))[0]!));
const stepsNode = () => kids(byClass("cloud-steps")[0]!);
/** A list's rows: when | what | size | Restore or why it can't be restored. */
const rows = (at: ReactNode = tree) => findAll(at, (f) => f.type === "ul" && classes(f).includes("backup-list") && !classes(f).includes("cloud-computers"))
  .flatMap((ul) => findAll(kids(ul), (f) => f.type === "li")).map((li) => {
    const span = (c: string) => byClass(c, kids(li)).map((f) => textOf(kids(f))).join("");
    const why = byClass("warn", kids(li)).map((f) => textOf(kids(f)));
    return [span("bk-when"), span("bk-what"), span("bk-size"), ...(why.length ? why : [span("bk-do")])].join(" | ");
  });
const computerRows = () => findAll(tree, (f) => f.type === "ul" && classes(f).includes("cloud-computers"))
  .flatMap((ul) => findAll(kids(ul), (f) => f.type === "li")).map((li) => [textOf(kids(byClass("bk-when", kids(li))[0]!)), textOf(kids(byClass("bk-what", kids(li))[0]!)),
    textOf(kids(byClass("bk-do", kids(li))[0]!))].join(" | "));
const called = (name: string) => h.calls.filter(([n]) => n === name).map(([, a]) => a);

// ---- the account and its data: local times, so "today" and "yesterday" don't depend on the time zone ----
const NOW = new Date(2026, 10, 25, 14, 0).getTime();
const at = (day: number, hh: number, mm = 0) => new Date(2026, 10, day, hh, mm).getTime();
const MB = 1024 ** 2;
const GB = 1024 ** 3;
const CODE = "HC-7K2Q-M4VB-9TXD-2HWN-PL6R-Q8YC";
const NEW_CODE = "HC-A1B2-C3D4-E5F6-G7H8-J9K0-ZX42";
const WRONG_CODE = "That recovery code doesn't open this account's backups.";
// Decision 2's warning, word for word (design/hersir-cloud.md, Every text)
const WARNING = "Hersir encrypts your data on this computer before it leaves. Only your computers and this recovery code can open it. "
  + "If you lose this code and every computer signed in to Hersir Cloud, your cloud backups can't be opened by anyone, Hersir included.";
const PLAN = { name: "personal" as const, status: "active" as const, validUntil: new Date(2027, 10, 9).getTime(), readOnlyUntil: null,
  quotaBytes: 10 * GB, usedBytes: Math.round(1.4 * GB) };
const backup = (over: Partial<CloudStatus["backup"]> = {}): CloudStatus["backup"] => ({ on: true, every: "daily", onQuit: false, running: null,
  last: { at: at(25, 9, 2), name: "hc-daily-1", sentBytes: 38 * MB }, next: at(26, 9, 2), lastError: null, ...over });
const SIGNED_OUT: CloudStatus = { signedIn: false, email: null, signingIn: false, plan: null, key: "missing", recoveryMadeAt: null,
  backup: backup({ on: false, last: null, next: null }) };
const SIGNED_IN: CloudStatus = { signedIn: true, email: "you@example.com", signingIn: false, plan: PLAN, key: "here",
  recoveryMadeAt: new Date(2026, 10, 9, 10, 0).getTime(), backup: backup() };
/** The account's first computer, just signed in: no key made yet. */
const FIRST: CloudStatus = { ...SIGNED_IN, key: "missing", recoveryMadeAt: null, backup: backup({ on: false, last: null, next: null }) };
/** Another computer on an account with a key. */
const NEEDED: CloudStatus = { ...SIGNED_IN, key: "needed", backup: backup({ on: false, last: null, next: null }) };
const cloudBackup = (over: Partial<CloudBackup>): CloudBackup => ({ name: "hc-daily-1", madeAt: at(25, 9, 2), device: { id: "pc", name: "PC", this: true },
  reason: "daily", bytes: 38 * MB, restorable: true, problem: null, ...over });
// in no order: the screen puts the newest first
const LIST: CloudBackup[] = [
  cloudBackup({ name: "hc-quit-2", reason: "quit", madeAt: at(24, 18, 40), device: { id: "laptop", name: "Laptop", this: false }, bytes: 36 * MB }),
  cloudBackup({}),
  cloudBackup({ name: "hc-before-restore-4", reason: "before-restore", madeAt: at(20, 12) }),
  cloudBackup({ name: "hc-manual-3", reason: "manual", madeAt: at(23, 8, 15), device: { id: "laptop", name: "Laptop", this: false }, bytes: 40 * MB,
    restorable: false, problem: "A newer Hersir made it (0.22.0): update Hersir to restore it" }),
];
const COMPUTERS: CloudComputer[] = [
  { id: "pc", name: "PC", platform: "Linux", appVersion: "0.21.0", lastSeenAt: NOW, online: true, this: true, chief: true },
  { id: "laptop", name: "Laptop", platform: "macOS", appVersion: "0.21.0", lastSeenAt: at(24, 18, 40), online: false, this: false, chief: false },
];
const NO_RESTORE: RestoreStatus = { state: "none", folder: null, snapshot: null, step: null, error: null, savedToday: null, result: null };

async function show(status: CloudStatus, opts: { shown?: boolean; afterSignIn?: CloudStatus } = {}) {
  h.status = clone(status);
  h.afterSignIn = clone(opts.afterSignIn ?? SIGNED_IN);
  h.shown = opts.shown ?? true;
  root = () => <CloudSettings shown={h.shown} />;
  render();
  await settle();
}
/** The fake backend's status changed: `cloud-changed` goes out. */
async function changed(f: (s: CloudStatus) => void) { f(st()); sendCloud(); await settle(); }
async function restoreChanged(r: Partial<RestoreStatus>) { h.restore = { ...NO_RESTORE, ...r }; sendRestore(); await settle(); }

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(NOW);
  for (const [k, inst] of insts) { unmount(inst); insts.delete(k); }
  h.effects.length = 0; h.calls.length = 0; h.cloudCbs.clear(); h.restoreCbs.clear();
  h.opened.length = 0; h.copied.length = 0; h.files.length = 0;
  h.refuse = {}; h.signIn = null;
  h.list = clone(LIST); h.computers = clone(COMPUTERS); h.restore = clone(NO_RESTORE);
  vi.stubGlobal("navigator", { clipboard: { writeText: async (t: string) => { h.copied.push(t); } } });
  const a = { href: "", download: "", click() { h.files.push({ name: a.download, blob: blobs.get(a.href)! }); }, remove() {} };
  const blobs = new Map<string, Blob>();
  vi.stubGlobal("document", { createElement: () => a, body: { appendChild: () => {} } });
  vi.spyOn(URL, "createObjectURL").mockImplementation((b) => { const u = `blob:${blobs.size}`; blobs.set(u, b as Blob); return u; });
  vi.spyOn(URL, "revokeObjectURL").mockImplementation(() => {});
});
afterEach(() => { root = () => null; render(); vi.useRealTimers(); vi.unstubAllGlobals(); vi.restoreAllMocks(); });

describe("A1 · signed out", () => {
  it("shows the plan and Sign in, Create an account and the hint, word for word, then Restore from Hersir Cloud", async () => {
    await show(SIGNED_OUT);
    expect(sections()).toEqual(["Hersir Cloud", "Restore from Hersir Cloud"]);
    expect(sectionText("Hersir Cloud")).toBe("Encrypted backups of your data, kept off your computer. Restore them on a new one.");
    expect(fields()).toEqual(["Personal plan", "Account"]);
    expect(fieldText("Personal plan")).toBe("€5 a month or €48 a year, VAT included. Hersir can't read what it stores.");
    const account = kids(field("Account"));
    expect(buttons("Sign in", account).map((b) => b.className)).toEqual(["btn primary"]);
    expect(button("Create an account").className).toBe("btn");
    expect(under("Account")).toBe("hint: Signing in opens your browser.");
    expect(called("cloudSignIn")).toEqual([]);
  });

  it("Create an account opens cloud.hersir.dev/register in the browser", async () => {
    await show(SIGNED_OUT);
    await click(button("Create an account"));
    expect(h.opened).toEqual(["https://cloud.hersir.dev/register"]);
  });

  it("has Restore from Hersir Cloud below: its text, step 1 on accent, step 2's field off, step 3 later", async () => {
    await show(SIGNED_OUT);
    expect(sectionText("Restore from Hersir Cloud")).toBe("Your data from a backup in Hersir Cloud. It replaces what is on this computer, which Hersir saves first.");
    expect(steps()).toEqual(["on: Sign in", "later: Your recovery code", "later: Pick a backup"]);
    expect(stepBody(1)).toBe("Sign inSign inOpens your browser: your password, then your authenticator's code.");
    const code = input("cloud-settings-restore-code");
    expect(code.disabled).toBe(true);
    expect(code.placeholder).toBe("HC-____-____-____-____-____-____");
    expect(buttons("Check")).toEqual([]);
    expect(stepBody(3)).toBe("Pick a backup");
  });

  it("reads Hersir Cloud's status, but not the backups, the computers or a key, while signed out", async () => {
    await show(SIGNED_OUT);
    expect(called("cloudStatus")).toHaveLength(1);
    for (const c of ["cloudListBackups", "cloudComputers", "cloudMakeKey"]) expect(called(c)).toEqual([]);
  });

  it("shows why the status can't be read, as the command says it", async () => {
    h.refuse.cloudStatus = "Hersir Cloud's settings can't be read: the database is busy.";
    await show(SIGNED_OUT);
    expect(fields()).toEqual(["Hersir Cloud"]);
    expect(under("Hersir Cloud")).toBe("error: Hersir Cloud's settings can't be read: the database is busy.");
  });
});

describe("A2 · signing in", () => {
  it("shows the spinner's waiting line, Cancel and the hint, from signingIn", async () => {
    await show({ ...SIGNED_OUT, signingIn: true });
    expect(fields()).toEqual(["Account"]);
    expect(statusLines()).toEqual(["Waiting for you to sign in in your browser…"]);
    expect(byClass("spin").length).toBe(1);
    expect(button("Cancel").className).toBe("btn sm ghost");
    expect(under("Account")).toBe("hint: Sign in with your password and your authenticator's code. Didn't a tab open? Cancel and sign in again.");
    expect(buttons("Sign in")).toEqual([]);
  });

  it("Sign in opens the browser and waits; once signed in, the tab follows the account", async () => {
    await show(SIGNED_OUT);
    await click(button("Sign in", kids(field("Account"))));
    expect(called("cloudSignIn")).toHaveLength(1);
    expect(statusLines()).toEqual(["Waiting for you to sign in in your browser…"]);
    // the restore's steps wait while the account's own Sign in does
    expect(sections()).toEqual(["Hersir Cloud"]);
    h.signIn!.done();
    await settle();
    expect(sections()).toEqual(["Hersir Cloud", "Cloud Backup"]);
    expect(fieldText("Account")).toContain("you@example.com · Personal, renews 9 Nov 2027");
  });

  it("Cancel stops the wait (cloud_cancel_sign_in) and shows no error; Sign in is back", async () => {
    await show(SIGNED_OUT);
    await click(button("Sign in", kids(field("Account"))));
    await click(button("Cancel"));
    expect(called("cloudCancelSignIn")).toEqual([[]]);
    expect(under("Account")).toBe("hint: Signing in opens your browser.");
    expect(all()).not.toContain("cancelled");
    expect(buttons("Sign in", kids(field("Account")))).toHaveLength(1);
  });

  it("shows a sign-in's refusal under Account, as it is", async () => {
    await show(SIGNED_OUT);
    await click(button("Sign in", kids(field("Account"))));
    h.signIn!.fail("Hersir Cloud couldn't be reached.");
    await settle();
    expect(under("Account")).toBe("error: Hersir Cloud couldn't be reached.");
  });

  it("started from Restore from Hersir Cloud, the waiting line shows only in step 1", async () => {
    await show(SIGNED_OUT);
    await click(button("Sign in", stepsNode()));
    expect(called("cloudSignIn")).toHaveLength(1);
    expect(fields()).toEqual(["Personal plan"]);
    expect(fieldText("Personal plan")).toBe("€5 a month or €48 a year, VAT included. Hersir can't read what it stores.");
    expect(statusLines()).toEqual(["Waiting for you to sign in in your browser…"]);
    expect(stepBody(1)).toContain("Waiting for you to sign in in your browser…");
    expect(buttons("Cancel", stepsNode())).toHaveLength(1);
  });
});

describe("A3 · the recovery code and its warning (the account's first computer)", () => {
  it("shows Save your recovery code with Needs you, decision 2's warning word for word, the code in mono, Copy and Save as a file…", async () => {
    await show(FIRST);
    expect(sections()).toEqual(["Hersir Cloud", "Cloud Backup"]);
    expect(sectionText("Cloud Backup")).toBe("A copy of this computer's data in Hersir Cloud, encrypted before it leaves.");
    expect(called("cloudMakeKey")).toEqual([[]]);
    const save = byClass("cloud-save-code")[0]!;
    expect(textOf(kids(findAll(kids(save), (f) => f.type === "h4")[0]!))).toBe("Save your recovery code");
    expect(byClass("badge", kids(save)).map((b) => [b.props.className, textOf(kids(b))])).toEqual([["badge needs", "Needs you"]]);
    expect(textOf(kids(byClass("cloud-warning")[0]!))).toBe(`${WARNING} Hersir won't show it again.`);
    expect(byClass("cloud-code-text").map((c) => [classes(c).includes("mono"), textOf(kids(c))])).toEqual([[true, CODE]]);
    expect(buttons("Copy")).toHaveLength(1);
    expect(buttons("Save as a file…")).toHaveLength(1);
    expect(fields()).toEqual(["Account", "Type its last group to show you have it"]);
  });

  it("can't be skipped: no Back up now, no settings, no backups and no Restore until the last group is typed back", async () => {
    await show(FIRST);
    for (const b of ["Back up now", "Account page", "Make a new recovery code", "Restore"]) expect(buttons(b)).toEqual([]);
    expect(radios()).toEqual([]);
    expect(findAll(tree, (f) => f.type === "input" && f.props.type === "checkbox")).toEqual([]);
    expect(buttons("Sign out")).toHaveLength(1);
    expect(button("Turn on Cloud Backup").disabled).toBe(true);
    for (const c of ["cloudBackUpNow", "cloudSaveSettings", "cloudListBackups", "cloudConfirmRecovery"]) expect(called(c)).toEqual([]);
    // events while it waits don't make a second code
    await changed(() => {});
    await changed(() => {});
    expect(called("cloudMakeKey")).toHaveLength(1);
    expect(textOf(kids(byClass("cloud-code-text")[0]!))).toBe(CODE);
  });

  it("refuses a wrong last group with the design's sentence and doesn't send it", async () => {
    await show(FIRST);
    await type("cloud-last-group", "Q8YD");
    expect(button("Turn on Cloud Backup").disabled).toBe(false);
    await click(button("Turn on Cloud Backup"));
    expect(under("Type its last group to show you have it")).toBe("error: That isn't the code's last group. Look at the code above again.");
    expect(called("cloudConfirmRecovery")).toEqual([]);
    expect(buttons("Back up now")).toEqual([]);
    // typing again clears it
    await type("cloud-last-group", "Q8Y");
    expect(under("Type its last group to show you have it")).toBe("");
  });

  it("the right group (any case, with spaces) turns Cloud Backup on; the code is never shown again", async () => {
    await show(FIRST);
    await type("cloud-last-group", " q8yc ");
    await enter("cloud-last-group");
    expect(called("cloudConfirmRecovery")).toEqual([["q8yc"]]);
    expect(byClass("cloud-code-text")).toEqual([]);
    expect(fields()).toEqual(["Account", "Storage", "Recovery code", "Your computers", "Back up", "Last backup", "Backups"]);
    expect(fieldText("Recovery code")).toBe("Made on 25 Nov 2026Make a new recovery code");
    expect(button("Back up now").disabled).toBe(false);
    expect(called("cloudMakeKey")).toHaveLength(1);
  });

  it("Copy copies the code and says Copied for a moment; Save as a file… saves it with the warning", async () => {
    await show(FIRST);
    await click(button("Copy"));
    expect(h.copied).toEqual([CODE]);
    expect(buttons("Copied")).toHaveLength(1);
    await vi.advanceTimersByTimeAsync(2100);
    render();
    expect(buttons("Copy")).toHaveLength(1);
    await click(button("Save as a file…"));
    expect(h.files.map((f) => f.name)).toEqual(["Hersir Cloud recovery code.txt"]);
    const saved = await h.files[0]!.blob.text();
    expect(saved).toContain(CODE);
    expect(saved).toContain(WARNING);
  });

  it("shows cloud_make_key's refusal as it is, with Try again", async () => {
    h.refuse.cloudMakeKey = "Hersir Cloud couldn't be reached.";
    await show(FIRST);
    expect(alerts()).toContain("Hersir Cloud couldn't be reached.");
    delete h.refuse.cloudMakeKey;
    await click(button("Try again"));
    expect(textOf(kids(byClass("cloud-code-text")[0]!))).toBe(CODE);
  });

  it("makes no key until the tab is opened (Settings keeps every tab mounted)", async () => {
    await show(FIRST, { shown: false });
    expect(called("cloudMakeKey")).toEqual([]);
    expect(called("cloudComputers")).toEqual([]);
    h.shown = true;
    render();
    await settle();
    expect(called("cloudMakeKey")).toHaveLength(1);
    // closed again: what it read stays
    h.shown = false;
    render();
    await settle();
    expect(textOf(kids(byClass("cloud-code-text")[0]!))).toBe(CODE);
    expect(called("cloudMakeKey")).toHaveLength(1);
  });
});

describe("frame 1 · signed in", () => {
  it("shows the account, storage, recovery code and computers, then Cloud Backup, word for word", async () => {
    await show(SIGNED_IN);
    expect(sections()).toEqual(["Hersir Cloud", "Cloud Backup"]);
    expect(sectionText("Hersir Cloud")).toBe("Encrypted backups of your data, kept off your computer. Restore them on a new one. Hersir can't read what it stores.");
    expect(sectionText("Cloud Backup")).toBe("A copy of this computer's data in Hersir Cloud, encrypted before it leaves. Each backup is kept 7 days, then one a day up to 90 days.");
    expect(fields()).toEqual(["Account", "Storage", "Recovery code", "Your computers", "Back up", "Last backup", "Backups"]);
    expect(fieldText("Account")).toBe("you@example.com · Personal, renews 9 Nov 2027Account pageSign out");
    expect(fieldText("Storage")).toBe("1.4 of 10 GB");
    const meter = findAll(tree, (f) => f.props.role === "meter")[0]!;
    expect(meter.props["aria-valuetext"]).toBe("1.4 of 10 GB");
    expect(classes(meter)).toEqual(["cloud-meter"]);
    expect(fieldText("Recovery code")).toBe("Made on 9 Nov 2026Make a new recovery code");
    expect(under("Recovery code")).toBe("hint: Hersir never keeps it on a computer. A new code replaces the old one, which stops working.");
    expect(radios().map((r) => [r.label, r.input.checked])).toEqual([["Daily", true], ["Only when I ask", false]]);
    expect(checkbox().label).toBe("Also when I quit Hersir completely");
    expect(checkbox().input.checked).toBe(false);
    expect(fieldText("Last backup")).toBe("Today 09:02 · 38 MB sent · next tomorrow 09:02Back up now");
    expect(button("Back up now").disabled).toBe(false);
    expect(under("Backups")).toBe("hint: Restoring saves today's data first and finishes when Hersir restarts.");
  });

  it("lists the backups newest first, each with the computer that made it, Restore or why it can't be restored", async () => {
    await show(SIGNED_IN);
    expect(rows()).toEqual([
      "Today 09:02 | Daily · this computer | 38 MB | Restore",
      "Yesterday 18:40 | On quit · Laptop | 36 MB | Restore",
      "Mon 23 Nov, 08:15 | By hand · Laptop | 40 MB | A newer Hersir made it (0.22.0): update Hersir to restore it",
      "Fri 20 Nov, 12:00 | Before a restore · this computer | 38 MB | Restore",
    ]);
  });

  it("lists the computers, this one first without Remove…, the others with Remove…", async () => {
    await show(SIGNED_IN);
    expect(computerRows()).toEqual([
      "PC · this computer | Linux · Hersir 0.21.0 · online | ",
      "Laptop | macOS · Hersir 0.21.0 · seen yesterday 18:40 | Remove…",
    ]);
    expect(under("Your computers")).toBe("hint: Each has its own sign-in. Remove one you no longer use, or lost: it is signed out at once.");
  });

  it("saves Back up at once: Only when I ask (and then there is no next), Daily, and On quit", async () => {
    await show(SIGNED_IN);
    radios()[1]!.input.onChange({});
    await settle();
    expect(called("cloudSaveSettings")).toEqual([[{ every: "manual" }]]);
    expect(radios().map((r) => r.input.checked)).toEqual([false, true]);
    expect(fieldText("Last backup")).toBe("Today 09:02 · 38 MB sentBack up now");
    checkbox().input.onChange({ target: { checked: true } });
    await settle();
    radios()[0]!.input.onChange({});
    await settle();
    expect(called("cloudSaveSettings")).toEqual([[{ every: "manual" }], [{ onQuit: true }], [{ every: "daily" }]]);
    expect(checkbox().input.checked).toBe(true);
  });

  it("shows a refusal to save under Back up, as it is", async () => {
    h.refuse.cloudSaveSettings = "Sign in to Hersir Cloud first.";
    await show(SIGNED_IN);
    checkbox().input.onChange({ target: { checked: true } });
    await settle();
    expect(under("Back up")).toBe("error: Sign in to Hersir Cloud first.");
    expect(checkbox().input.checked).toBe(false);
  });

  it("Cloud Backup off on this computer: Turn on Cloud Backup saves on", async () => {
    await show({ ...SIGNED_IN, backup: backup({ on: false }) });
    expect(fieldText("Cloud Backup")).toBe("Off on this computer.Turn on Cloud Backup");
    expect(button("Back up now").disabled).toBe(true);
    await click(button("Turn on Cloud Backup"));
    expect(called("cloudSaveSettings")).toEqual([[{ on: true }]]);
    expect(radios().map((r) => r.label)).toEqual(["Daily", "Only when I ask"]);
  });

  it("No backup yet, and a next one already due", async () => {
    await show({ ...SIGNED_IN, backup: backup({ last: null, next: at(25, 13) }) });
    expect(fieldText("Last backup")).toBe("No backup yet · next within the hour" + "Back up now");
  });

  it("Account page opens the account page (cloud_open_account), and Sign out signs out", async () => {
    await show(SIGNED_IN);
    await click(button("Account page"));
    expect(called("cloudOpenAccount")).toEqual([[]]);
    await click(button("Sign out"));
    expect(called("cloudSignOut")).toEqual([[]]);
    expect(sections()).toEqual(["Hersir Cloud", "Restore from Hersir Cloud"]);
    expect(rows()).toEqual([]);
  });

  it("shows cloud_open_account's refusal under Account, as it is", async () => {
    h.refuse.cloudOpenAccount = "Hersir couldn't open your browser.";
    await show(SIGNED_IN);
    await click(button("Account page"));
    expect(under("Account")).toBe("error: Hersir couldn't open your browser.");
  });

  it("reads the backups and computers only once the tab is opened", async () => {
    await show(SIGNED_IN, { shown: false });
    expect(called("cloudListBackups")).toEqual([]);
    expect(called("cloudComputers")).toEqual([]);
    h.shown = true;
    render();
    await settle();
    expect(called("cloudListBackups")).toHaveLength(1);
    expect(called("cloudComputers")).toHaveLength(1);
    expect(rows()).toHaveLength(4);
  });
});

describe("A5 · a backup running", () => {
  it("Back up now starts one: the button waits and the step shows, each in the design's words, then the list is read again", async () => {
    await show(SIGNED_IN);
    expect(called("cloudListBackups")).toHaveLength(1);
    await click(button("Back up now"));
    expect(called("cloudBackUpNow")).toEqual([[]]);
    expect(button("Back up now").disabled).toBe(true);
    expect(statusLines()).toEqual(["Copying your data…"]);
    expect(byClass("spin")).toHaveLength(1);
    const say: [CloudStatus["backup"]["running"], string][] = [
      [{ step: "chunking", done: 0, total: 0 }, "Encrypting…"],
      [{ step: "uploading", done: 112 * MB, total: 240 * MB }, "Uploading: 112 of 240 MB…"],
      [{ step: "registering", done: 0, total: 0 }, "Finishing…"],
    ];
    for (const [running, line] of say) {
      await changed((s) => { s.backup.running = running; });
      expect(statusLines()).toEqual([line]);
      expect(button("Back up now").disabled).toBe(true);
    }
    // the rows can't be restored while it runs
    expect(buttons("Restore").every((b) => b.disabled)).toBe(true);
    h.list = [cloudBackup({ name: "hc-manual-5", reason: "manual", madeAt: at(25, 14), bytes: 39 * MB }), ...LIST];
    await changed((s) => { s.backup.running = null; s.backup.last = { at: at(25, 14), name: "hc-manual-5", sentBytes: 2 * MB }; });
    expect(statusLines()).toEqual([]);
    expect(button("Back up now").disabled).toBe(false);
    expect(fieldText("Last backup")).toBe("Today 14:00 · 2.0 MB sent · next tomorrow 09:02Back up now");
    expect(rows()[0]).toBe("Today 14:00 | By hand · this computer | 39 MB | Restore");
  });

  it("shows from the status alone, as another window started it", async () => {
    await show({ ...SIGNED_IN, backup: backup({ running: { step: "uploading", done: 5 * MB, total: 1200 * MB } }) });
    expect(button("Back up now").disabled).toBe(true);
    expect(statusLines()).toEqual(["Uploading: 5 of 1,200 MB…"]);
  });

  it("shows Back up now's refusal under Last backup, as it is", async () => {
    h.refuse.cloudBackUpNow = "Hersir Cloud is full: delete some backups on the account page.";
    await show(SIGNED_IN);
    await click(button("Back up now"));
    expect(under("Last backup")).toBe("error: Hersir Cloud is full: delete some backups on the account page.");
    expect(button("Back up now").disabled).toBe(false);
  });
});

describe("A6 · a failure", () => {
  it("shows the failure under Last backup as a failed line, with the message as it came", async () => {
    await show({ ...SIGNED_IN, backup: backup({ lastError: { at: at(25, 9, 2), message: "Hersir Cloud couldn't be reached." } }) });
    expect(alerts()).toEqual(["Today 09:02: the backup failed. Hersir Cloud couldn't be reached. Hersir tries again at the next backup, or back up now."]);
    expect(findAll(kids(field("Last backup")), (f) => f.props.role === "alert").map((f) => f.props.className)).toEqual(["gh-line failed"]);
    expect(button("Back up now").disabled).toBe(false);
  });
});

describe("A7 · the quota full", () => {
  const FULL: CloudStatus = { ...SIGNED_IN, plan: { ...PLAN, usedBytes: 10 * GB } };
  it("shows the meter full in danger with its words and hint, Back up now off, and Account page under Last backup", async () => {
    await show(FULL);
    expect(classes(findAll(tree, (f) => f.props.role === "meter")[0]!)).toEqual(["cloud-meter", "full"]);
    expect(alerts()).toEqual(["Hersir Cloud is full: 10.0 of 10 GB."]);
    expect(under("Storage")).toBe("hint: Backups stop until there is room. Older backups go after 90 days, or delete some on the account page.");
    expect(button("Back up now").disabled).toBe(true);
    expect(buttons("Account page", kids(field("Last backup")))).toHaveLength(1);
    await click(buttons("Account page", kids(field("Last backup")))[0]!);
    expect(called("cloudOpenAccount")).toEqual([[]]);
    // the backups can still be restored
    expect(buttons("Restore").filter((b) => !b.disabled)).toHaveLength(3);
  });
});

describe("A8 · a lapsed plan", () => {
  const LAPSED: CloudStatus = { ...SIGNED_IN, plan: { ...PLAN, status: "lapsed", validUntil: new Date(2026, 11, 9).getTime(), readOnlyUntil: new Date(2027, 1, 7).getTime() } };
  it("says the plan ended, offers Renew on the account page, a warning line and the hint; the backups still restore", async () => {
    await show(LAPSED);
    expect(fieldText("Account")).toBe("you@example.com · Personal ended on 9 Dec 2026Renew on the account pageSign outYour plan ended on 9 Dec 2026: backups can be restored until 7 Feb 2027.");
    expect(findAll(kids(field("Account")), (f) => f.props.role === "status").map((f) => f.props.className)).toEqual(["gh-line warning"]);
    expect(under("Account")).toBe("hint: Hersir makes no new cloud backups until you renew. Your data on this computer is untouched.");
    expect(buttons("Account page")).toEqual([]);
    await click(button("Renew on the account page"));
    expect(called("cloudOpenAccount")).toEqual([[]]);
    expect(button("Back up now").disabled).toBe(true);
    expect(rows()[0]).toBe("Today 09:02 | Daily · this computer | 38 MB | Restore");
    expect(buttons("Restore").filter((b) => !b.disabled)).toHaveLength(3);
  });

  it("past_due changes nothing on screen", async () => {
    await show({ ...SIGNED_IN, plan: { ...PLAN, status: "past_due" } });
    expect(fieldText("Account")).toBe("you@example.com · Personal, renews 9 Nov 2027Account pageSign out");
    expect(under("Account")).toBe("");
    expect(button("Back up now").disabled).toBe(false);
  });
});

describe("A4 · Make a new recovery code", () => {
  it("asks first with decision 2's warning; Cancel makes none", async () => {
    await show(SIGNED_IN);
    await click(button("Make a new recovery code"));
    expect(ask()?.title).toBe("Make a new recovery code?");
    expect(ask()?.text).toBe(`${WARNING} Your current code stops working once the new one is shown.`);
    expect(ask()?.buttons).toEqual(["Cancel", "Make a new code"]);
    await click(button("Cancel", ask()!.node));
    expect(ask()).toBeNull();
    expect(called("cloudNewRecovery")).toEqual([]);
  });

  it("then shows the new code in the Recovery code field, with its last group and Done", async () => {
    await show(SIGNED_IN);
    await click(button("Make a new recovery code"));
    await click(button("Make a new code", ask()!.node));
    expect(called("cloudNewRecovery")).toEqual([[]]);
    expect(ask()).toBeNull();
    expect(fieldText("Recovery code")).toBe(`Save your new recovery code. Your old code no longer works.${NEW_CODE}CopySave as a file…Done`);
    expect(under("Recovery code")).toBe("hint: Type its last group to show you have it.");
    expect(button("Done").disabled).toBe(true);
    await type("cloud-new-group", "ZX43");
    await click(button("Done"));
    expect(under("Recovery code")).toBe("error: That isn't the code's last group. Look at the code above again.");
    expect(byClass("cloud-code-text")).toHaveLength(1);
    await type("cloud-new-group", "zx42");
    await click(button("Done"));
    expect(byClass("cloud-code-text")).toEqual([]);
    expect(fieldText("Recovery code")).toBe("Made on 25 Nov 2026Make a new recovery code");
  });

  it("shows cloud_new_recovery's refusal under Recovery code, as it is", async () => {
    h.refuse.cloudNewRecovery = "Hersir Cloud couldn't be reached.";
    await show(SIGNED_IN);
    await click(button("Make a new recovery code"));
    await click(button("Make a new code", ask()!.node));
    expect(under("Recovery code")).toBe("error: Hersir Cloud couldn't be reached.");
  });
});

describe("the computers, with Remove…", () => {
  it("Remove… asks first (no lost-or-stolen question, decision 10), then removes it", async () => {
    await show(SIGNED_IN);
    expect(buttons("Remove…").map((b) => b["aria-label"])).toEqual(["Remove Laptop…"]);
    await click(button("Remove…"));
    expect(ask()?.title).toBe("Remove Laptop?");
    expect(ask()?.text).toBe("It is signed out of Hersir Cloud at once and can no longer back up to it or restore from it. Its own data stays on it.");
    expect(ask()?.buttons).toEqual(["Cancel", "Remove"]);
    await click(button("Cancel", ask()!.node));
    expect(called("cloudRemoveComputer")).toEqual([]);
    await click(button("Remove…"));
    await click(button("Remove", ask()!.node));
    expect(called("cloudRemoveComputer")).toEqual([["laptop", false]]);
    expect(computerRows()).toEqual(["PC · this computer | Linux · Hersir 0.21.0 · online | "]);
  });

  it("shows cloud_remove_computer's refusal under Your computers, as it is", async () => {
    h.refuse.cloudRemoveComputer = "Laptop can't be removed now: Hersir Cloud couldn't be reached.";
    await show(SIGNED_IN);
    await click(button("Remove…"));
    await click(button("Remove", ask()!.node));
    expect(under("Your computers")).toBe("error: Laptop can't be removed now: Hersir Cloud couldn't be reached.");
    expect(computerRows()).toHaveLength(2);
  });
});

describe("A9 · Restore from Hersir Cloud, in the tab", () => {
  it("from a backup's row: asks, downloads, GA-218's steps, then Restart now", async () => {
    await show(SIGNED_IN);
    await click(buttons("Restore")[0]!);
    expect(ask()?.title).toBe("Restore this backup?");
    expect(ask()?.text).toBe("Daily, today 09:02, from this computer. Hersir first saves today's data as a backup. After a restart, Hersir shows your data as "
      + "it was then: what changed since, also until the restart, is not kept. Keys and passwords stay as they are on this computer. Agents are paused afterwards.");
    expect(ask()?.buttons).toEqual(["Cancel", "Restore"]);
    await click(button("Restore", ask()!.node));
    expect(called("cloudStartRestore")).toEqual([["hc-daily-1"]]);
    await changed((s) => { s.backup.running = { step: "downloading", done: 640 * MB, total: 1200 * MB }; });
    expect(fields()).toContain("Restore");
    expect(fieldText("Restore")).toBe("Downloading: 640 of 1,200 MB…Cancel restore");
    expect(button("Back up now").disabled).toBe(true);
    expect(buttons("Restore").every((b) => b.disabled)).toBe(true);
    await changed((s) => { s.backup.running = { step: "checking", done: 0, total: 0 }; });
    expect(fieldText("Restore")).toBe("Checking the backup…Cancel restore");
    await changed((s) => { s.backup.running = null; });
    await restoreChanged({ state: "saving", step: "Saving today's data" });
    expect(fieldText("Restore")).toBe("Saving today's data…Cancel restore");
    const snapshot = { name: "hc-daily-1", reason: "daily", madeAt: at(25, 9, 2), deviceId: "pc", deviceName: "PC", thisComputer: true, appVersion: "0.21.0",
      schemaVersion: 41, dbBytes: 1, filesCount: 0, filesBytes: 0, filesMissing: 0, restorable: true, problem: null } as const;
    await restoreChanged({ state: "ready", snapshot, savedToday: "gizai-before-restore-x" });
    expect(fieldText("Restore")).toBe("Restore ready: today 09:02Restart nowCancel restore");
    await click(button("Restart now"));
    expect(called("restartToRestore")).toEqual([[]]);
  });

  it("Cancel restore cancels it, and a failed restore shows its reason with Dismiss", async () => {
    await show(SIGNED_IN);
    await restoreChanged({ state: "staging", step: "Copying files (3 of 14)" });
    await click(button("Cancel restore"));
    expect(called("cancelRestore")).toEqual([[]]);
    expect(fields()).not.toContain("Restore");
    await restoreChanged({ state: "failed", error: "The backup's database is damaged." });
    expect(fieldText("Restore")).toBe("The restore failed: The backup's database is damaged.Dismiss");
  });

  it("shows cloud_start_restore's refusal under Restore, as it is", async () => {
    h.refuse.cloudStartRestore = "Another restore is running.";
    await show(SIGNED_IN);
    await click(buttons("Restore")[0]!);
    await click(button("Restore", ask()!.node));
    expect(under("Restore")).toBe("error: Another restore is running.");
  });

  it("signed out, step by step: sign in, the recovery code, the backups newest first, then Restart now", async () => {
    await show(SIGNED_OUT, { afterSignIn: NEEDED });
    await click(button("Sign in", stepsNode()));
    h.signIn!.done();
    await settle();
    // signed in: the account above, the restore's steps stay open in place of Cloud Backup
    expect(sections()).toEqual(["Hersir Cloud", "Restore from Hersir Cloud"]);
    expect(steps()).toEqual(["done: Sign in", "on: Your recovery code", "later: Pick a backup"]);
    expect(stepBody(1)).toBe("Sign inDone.Signed in as you@example.com");
    expect(stepBody(3)).toBe("Pick a backupYour backups show once the code is right.");
    expect(input("cloud-settings-restore-code").disabled).toBe(false);
    expect(button("Check").disabled).toBe(true);
    await type("cloud-settings-restore-code", "HC-0000-0000-0000-0000-0000-0000");
    await click(button("Check"));
    expect(alerts()).toEqual([WRONG_CODE]);
    expect(called("cloudListBackups")).toEqual([]);
    await type("cloud-settings-restore-code", CODE);
    await click(button("Check"));
    expect(called("cloudEnterRecovery")).toEqual([["HC-0000-0000-0000-0000-0000-0000"], [CODE]]);
    expect(steps()).toEqual(["done: Sign in", "done: Your recovery code", "on: Pick a backup"]);
    expect(stepBody(2)).toBe("Your recovery codeDone.Accepted.");
    expect(rows(stepsNode())).toEqual([
      "Today 09:02 | Daily · this computer | 38 MB | Restore",
      "Yesterday 18:40 | On quit · Laptop | 36 MB | Restore",
      "Mon 23 Nov, 08:15 | By hand · Laptop | 40 MB | A newer Hersir made it (0.22.0): update Hersir to restore it",
      "Fri 20 Nov, 12:00 | Before a restore · this computer | 38 MB | Restore",
    ]);
    expect(stepBody(3)).toContain("Hersir restarts to finish, as a restore from a folder does.");
    await click(buttons("Restore", stepsNode())[1]!);
    expect(ask()?.text.startsWith("On quit, yesterday 18:40, from Laptop. Hersir first saves today's data as a backup.")).toBe(true);
    await click(button("Restore", ask()!.node));
    expect(called("cloudStartRestore")).toEqual([["hc-quit-2"]]);
    await changed((s) => { s.backup.running = { step: "downloading", done: 640 * MB, total: 1200 * MB }; });
    expect(statusLines()).toEqual(["Downloading: 640 of 1,200 MB…"]);
    await changed((s) => { s.backup.running = null; });
    const snapshot = { name: "hc-quit-2", reason: "quit", madeAt: at(24, 18, 40), deviceId: "laptop", deviceName: "Laptop", thisComputer: false, appVersion: "0.21.0",
      schemaVersion: 41, dbBytes: 1, filesCount: 0, filesBytes: 0, filesMissing: 0, restorable: true, problem: null } as const;
    await restoreChanged({ state: "ready", snapshot, savedToday: "gizai-before-restore-x" });
    expect(statusLines()).toEqual(["Restore ready: yesterday 18:40"]);
    await click(button("Restart now"));
    expect(called("restartToRestore")).toEqual([[]]);
  });

  it("on a computer whose key is needed, Cloud Backup asks for the recovery code first, and shows a wrong one's refusal as it is", async () => {
    await show(NEEDED);
    expect(fields()).toEqual(["Account", "Storage", "Your computers", "Your recovery code"]);
    expect(under("Your recovery code")).toBe("hint: This account has a recovery code already. Type it to back up this computer and to see your backups.");
    expect(buttons("Back up now")).toEqual([]);
    expect(called("cloudListBackups")).toEqual([]);
    await type("cloud-enter-code", "HC-1111-1111-1111-1111-1111-1111");
    await enter("cloud-enter-code");
    expect(under("Your recovery code")).toBe(`error: ${WRONG_CODE}`);
    await type("cloud-enter-code", ` ${CODE} `);
    await click(button("Check"));
    expect(called("cloudEnterRecovery").at(-1)).toEqual([CODE]);
    expect(fields()).toEqual(["Account", "Storage", "Recovery code", "Your computers", "Cloud Backup", "Last backup", "Backups"]);
    expect(rows()).toHaveLength(4);
  });

  it("an account with no key yet has nothing to restore: step 2 says so", async () => {
    await show(SIGNED_OUT, { afterSignIn: FIRST });
    await click(button("Sign in", stepsNode()));
    h.signIn!.done();
    await settle();
    // the first computer goes to Save your recovery code, which can't be skipped
    expect(sections()).toEqual(["Hersir Cloud", "Cloud Backup"]);
    expect(byClass("cloud-save-code")).toHaveLength(1);
  });
});

describe("dark and light (GA-317's sheet A: 384:416 dark, 389:2445 light)", () => {
  // the rules GA-360 added: app.css from its Hersir Cloud comment to the end of the cloud rules, and onboarding.css's strip
  const cloudCss = () => {
    const start = appCss.indexOf("/* Settings → Hersir Cloud and Restore from Hersir Cloud (GA-360");
    expect(start).toBeGreaterThan(-1);
    const rest = appCss.slice(start);
    const lines = rest.split("\n");
    // up to the first rule after it that isn't about Hersir Cloud (Coding CLIs' list)
    const end = lines.findIndex((l) => l.startsWith(".cli-list"));
    const onb = onboardingCss.slice(onboardingCss.indexOf("/* Restore from Hersir Cloud (GA-360"));
    return `${lines.slice(0, end < 0 ? undefined : end).join("\n")}\n${onb}`;
  };
  /** The `--name: value` declarations of the block whose selector is exactly `sel`. */
  const block = (sel: string) => {
    const i = tokensCss.indexOf(`${sel} {`);
    expect(i).toBeGreaterThan(-1);
    const body = tokensCss.slice(tokensCss.indexOf("{", i) + 1, tokensCss.indexOf("}", i));
    return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1]!, m[2]!.trim()]));
  };
  const root = block(":root");
  const dark = block(':root, [data-theme="dark"]');
  const light = block('[data-theme="light"]');
  /** A variable's value in a theme: light sets only what differs, the rest comes from dark; aliases are followed. */
  const value = (theme: "dark" | "light", name: string): string | undefined => {
    const v = (theme === "light" ? light.get(name) : undefined) ?? dark.get(name) ?? root.get(name);
    const alias = v && /^var\((--[\w-]+)\)$/.exec(v);
    return alias ? value(theme, alias[1]!) : v;
  };
  const isColour = (v: string | undefined) => !!v && /^(#[0-9a-f]{3,8}|rgba?\()/i.test(v);

  it("uses no colour of its own: every colour is a theme variable", () => {
    const css = cloudCss();
    expect(css).toContain(".cloud-meter");
    expect(css).toContain(".onb-cloud");
    expect(css.replace(/\/\*[\s\S]*?\*\//g, "").match(/#[0-9a-fA-F]{3,8}\b|rgba?\(|hsla?\(/g)).toBeNull();
  });

  it("every variable it uses is set for dark and for light, and the surfaces and text change with the theme", () => {
    const used = [...new Set([...cloudCss().matchAll(/var\((--[\w-]+)\)/g)].map((m) => m[1]!))];
    expect(used.length).toBeGreaterThan(10);
    for (const v of used) {
      expect(value("dark", v), `${v} in dark`).toBeDefined();
      expect(value("light", v), `${v} in light`).toBeDefined();
    }
    const colours = used.filter((v) => isColour(value("dark", v)));
    for (const v of ["--raised", "--line", "--line-2", "--text", "--text-2", "--text-3", "--accent", "--danger", "--success", "--warning", "--needs"])
      expect(colours, `the new styles use ${v}`).toContain(v);
    for (const v of ["--raised", "--line", "--line-2", "--text", "--text-2", "--text-3"]) expect(value("light", v), v).not.toBe(value("dark", v));
  });

  it("keeps gold (needs) for Save your recovery code alone, and the steps' colours as the design says", () => {
    const css = cloudCss();
    const needs = css.split("\n").filter((l) => l.includes("var(--needs"));
    expect(needs).toEqual([".cloud-needs .icon { color: var(--needs); }"]);
    expect(css).not.toContain("var(--live");
    expect(css).toMatch(/\.cloud-step\.on > \.num \{[^}]*background: var\(--accent\)/);
    expect(css).toMatch(/\.cloud-step\.done > \.num \{[^}]*color: var\(--success\)/);
    expect(css).toMatch(/\.cloud-meter\.full > span \{[^}]*background: var\(--danger\)/);
    expect(css).toMatch(/\.cloud-meter \{[^}]*width: 240px;[^}]*height: 6px;[^}]*border: 1px solid var\(--line-2\)[^}]*background: var\(--raised\)/);
    expect(css).toMatch(/\.onb-cloud \{[^}]*border: 1px solid var\(--line\);[^}]*border-radius: 6px;[^}]*background: var\(--raised\)/);
  });
});

// ---- GA-371 QA: Personal's price is VAT included (€5 a month, €48 a year), the same in the app, the design and the docs ----
describe("GA-371 · the Personal plan's price, VAT included", () => {
  const read = (path: string): string => fs.readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
  const flat = (s: string) => s.replace(/\s+/g, " ");

  it("the app's words are the card's, and the design's Every text gives them word for word", async () => {
    const { PLAN_PRICE } = await import("../lib/cloud");
    expect(PLAN_PRICE).toBe("€5 a month or €48 a year, VAT included. Hersir can't read what it stores.");
    const design = flat(read("design/hersir-cloud.md"));
    expect(design).toContain(`field **Personal plan**: **${PLAN_PRICE}**`);
    expect(design).toContain("**Personal ·** *€48 a year, VAT included* **· renews** *date*");
    expect(design).not.toContain("excl. VAT");
  });

  it("docs/settings.md's Hersir Cloud section says VAT included, and no longer excl. VAT", () => {
    const docs = flat(read("docs/settings.md"));
    expect(docs).toContain("It needs an account with the Personal plan, €5 a month or €48 a year, VAT included.");
    expect(docs).not.toContain("excl. VAT");
  });
});
