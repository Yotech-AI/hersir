// GA-282 QA: the notice after an unexpected end (UnexpectedEnd, mounted in App.tsx above the page). With an unexpectedEnd
// from getLogsInfo (GA-281) it says, word for word, "Hersir ended unexpectedly on <date> at <time>. Its log can help find
// out why: <file>", the date and time from lastWrittenAt in the computer's time zone and the file's name in mono, with Open
// log folder (openLogsFolder) and Dismiss (dismissUnexpectedEnd). Dismissed, it goes at once and doesn't come back at the
// next start (the fake backend clears it as src-tauri's logs.rs does). Without an unexpected end, or when getLogsInfo
// fails, there is nothing. There is no DOM here, so a small hook runtime stands in for React (as in BackupSettings.test.tsx).
import { afterAll, afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { LogsInfo } from "../types";

type Inst = { slots: unknown[]; i: number };
type EffectSlot = { effect: true; deps?: unknown[]; first: boolean; cleanup?: () => void };
const h = vi.hoisted(() => ({
  cur: null as Inst | null,
  dirty: false,
  effects: [] as (() => void)[],
  info: null as unknown,
  infoRefusal: null as string | null,
  calls: [] as string[],
  openRefusal: null as string | null,
  dismissRefusal: null as string | null,
  dismissWait: null as Promise<void> | null,
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
  const useEffect = ((f: () => void | (() => void), deps?: unknown[]) => {
    if (!h.cur) return R.useEffect(f, deps);
    const s = slot<EffectSlot>(() => ({ effect: true, first: true }));
    if (s.first || changed(deps, s.deps)) {
      s.first = false;
      s.deps = deps;
      h.effects.push(() => { s.cleanup?.(); const c = f(); s.cleanup = typeof c === "function" ? c : undefined; });
    }
  }) as typeof R.useEffect;
  const hooks = { useState, useEffect };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});

// The backend: get_logs_info answers what h.info holds; dismiss_unexpected_end clears it (the file goes, as in logs.rs), so a
// next start reads null. Tauri rejects a command with the error's text, so the refusals are strings.
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(real)) if (typeof real[k] === "function") out[k] = () => new Promise(() => {});
  return {
    ...out,
    getLogsInfo: async () => { h.calls.push("getLogsInfo"); if (h.infoRefusal) throw h.infoRefusal; return structuredClone(h.info); },
    openLogsFolder: async () => { h.calls.push("openLogsFolder"); if (h.openRefusal) throw h.openRefusal; },
    dismissUnexpectedEnd: async () => {
      h.calls.push("dismissUnexpectedEnd");
      if (h.dismissWait) await h.dismissWait;
      if (h.dismissRefusal) throw h.dismissRefusal;
      if (h.info) (h.info as LogsInfo).unexpectedEnd = null;
    },
  };
});

const { UnexpectedEnd } = await import("./UnexpectedEnd");
const { logName, localWhen, unexpectedEndText } = await import("../lib/logs");

// ---- the hook runtime ----
let insts = new Map<string, Inst>();
const unmount = (inst: Inst) => { for (const s of inst.slots) if ((s as EffectSlot)?.effect) (s as EffectSlot).cleanup?.(); };
function expand(node: ReactNode, path: string, seen: Set<string>): ReactNode {
  if (Array.isArray(node)) return node.map((n, i) => expand(n, `${path}.${i}`, seen));
  if (!node || typeof node !== "object" || !("props" in node)) return node;
  const el = node as ReactElement<Record<string, unknown>>;
  if (typeof el.type === "function") {
    const key = `${path}>${el.type.name}`;
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
    tree = expand(<UnexpectedEnd />, "", seen);
    for (const [k, inst] of insts) if (!seen.has(k)) { unmount(inst); insts.delete(k); }
    for (const f of h.effects.splice(0)) f();
    if (!h.dirty) return;
  }
  throw new Error("the render never settled");
}
async function settle() { for (let i = 0; i < 4; i++) { await new Promise((r) => setTimeout(r, 0)); render(); } }
/** A start of Hersir: the notice mounted afresh, its effects run and getLogsInfo answered. */
async function start() {
  for (const inst of insts.values()) unmount(inst);
  insts = new Map();
  render();
  await settle();
}

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
const banner = () => findAll(tree, (f) => String(f.props.className ?? "").split(" ").includes("info-banner"));
/** The notice's sentence: its text without the error under it and without its buttons. */
const sentence = () => {
  const [b] = banner();
  const [t] = findAll(kids(b!), (f) => f.props.className === "ib-text");
  return textOf(([] as ReactNode[]).concat(kids(t!)).filter((n) => !(n && typeof n === "object" && "props" in n && (n as ReactElement<{ role?: string }>).props.role === "alert")));
};
const alert = () => findAll(tree, (f) => f.props.role === "alert").map((f) => textOf(kids(f)));
const buttons = () => findAll(tree, (f) => f.type === "button").map((f) => textOf(kids(f)).trim());
function button(name: string) {
  const all = findAll(tree, (f) => f.type === "button" && textOf(kids(f)).trim() === name);
  if (all.length !== 1) throw new Error(`${all.length} buttons called ${name}`);
  return all[0]!.props as { onClick: () => unknown; type?: string };
}
async function click(name: string) { await button(name).onClick(); await settle(); }

const FOLDER = "/home/you/.local/share/gizai/logs";
const FILE = `${FOLDER}/hersir-2026-11-13T14-50-26Z.log`;
const END = { file: FILE, startedAt: "2026-11-13T14:50:26Z", lastWrittenAt: "2026-11-13T14:47:12Z" };
const info = (unexpectedEnd: LogsInfo["unexpectedEnd"]): LogsInfo => ({ folder: FOLDER, current: `${FOLDER}/hersir-2026-11-17T09-00-00Z.log`, unexpectedEnd });
const SENTENCE = "Hersir ended unexpectedly on 13 Nov at 15:47. Its log can help find out why: hersir-2026-11-13T14-50-26Z.log";

// The computer's time zone: Amsterdam, as in the card's example, unless a test says otherwise.
// (Node reads TZ again when it changes; the app's tsconfig has no Node types, so process is reached through globalThis)
const env = (globalThis as unknown as { process: { env: Record<string, string | undefined> } }).process.env;
const TZ = env.TZ;
const setZone = (zone: string | undefined) => { if (zone === undefined) delete env.TZ; else env.TZ = zone; };
beforeEach(() => {
  setZone("Europe/Amsterdam");
  h.info = info(structuredClone(END));
  h.infoRefusal = h.openRefusal = h.dismissRefusal = null;
  h.dismissWait = null;
  h.calls = [];
});
afterEach(() => { for (const inst of insts.values()) unmount(inst); insts = new Map(); tree = null; });
afterAll(() => setZone(TZ));

describe("with an unexpected end", () => {
  it("shows the notice at start, word for word, with the date and time of lastWrittenAt in the local time zone", async () => {
    await start();
    expect(h.calls).toEqual(["getLogsInfo"]);
    expect(banner()).toHaveLength(1);
    expect(sentence()).toBe(SENTENCE);
    expect(buttons()).toEqual(["Open log folder", "Dismiss"]);
    expect(alert()).toEqual([]);
  });

  it("follows the computer's time zone: the same moment is another date and time elsewhere", async () => {
    for (const [zone, when] of [["UTC", "13 Nov at 14:47"], ["America/New_York", "13 Nov at 09:47"], ["Pacific/Auckland", "14 Nov at 03:47"], ["Asia/Kolkata", "13 Nov at 20:17"]]) {
      setZone(zone);
      await start();
      expect(sentence(), zone).toBe(`Hersir ended unexpectedly on ${when}. Its log can help find out why: hersir-2026-11-13T14-50-26Z.log`);
    }
  });

  it("takes the time from lastWrittenAt, not startedAt, in 24 hours and with the day without a leading zero", async () => {
    h.info = info({ file: FILE, startedAt: "2026-03-01T06:00:00Z", lastWrittenAt: "2026-03-04T23:05:00Z" });
    await start();
    expect(sentence()).toBe("Hersir ended unexpectedly on 5 Mar at 00:05. Its log can help find out why: hersir-2026-11-13T14-50-26Z.log");
  });

  it("shows the log's file name in mono, with its full path as the tooltip", async () => {
    await start();
    const mono = findAll(tree, (f) => String(f.props.className ?? "").split(" ").includes("mono"));
    expect(mono.map((f) => [textOf(kids(f)), f.props.title])).toEqual([["hersir-2026-11-13T14-50-26Z.log", FILE]]);
    // the name is the sentence's end, right after "why: "
    expect(sentence().endsWith(`why: ${textOf(kids(mono[0]!))}`)).toBe(true);
  });

  it("names a Windows log by its name too", async () => {
    h.info = info({ ...END, file: "C:\\Users\\you\\AppData\\Roaming\\gizai\\logs\\hersir-2026-11-13T14-50-26Z.log" });
    await start();
    expect(sentence()).toBe(SENTENCE);
  });

  it("is a status in the neutral banner style, not gold: no warn banner and no --needs colour", async () => {
    await start();
    const [b] = banner();
    expect(b!.type).toBe("div");
    expect(b!.props.role).toBe("status");
    expect(String(b!.props.className)).toBe("info-banner unexpected-end");
    const html = renderToStaticMarkup(tree as ReactElement);
    expect(html).not.toMatch(/warn-banner|needs/);
    expect(findAll(tree, (f) => f.type === "button").every((f) => f.props.type === "button")).toBe(true);
  });
});

describe("Open log folder", () => {
  it("opens the logs folder (openLogsFolder) and the notice stays", async () => {
    await start();
    await click("Open log folder");
    expect(h.calls).toEqual(["getLogsInfo", "openLogsFolder"]);
    expect(sentence()).toBe(SENTENCE);
    expect(alert()).toEqual([]);
  });

  it("shows why when the folder can't be opened, under the text, and the notice stays; a next try clears it", async () => {
    await start();
    h.openRefusal = "Hersir couldn't open /home/you/.local/share/gizai/logs: no file manager";
    await click("Open log folder");
    expect(alert()).toEqual(["Hersir couldn't open /home/you/.local/share/gizai/logs: no file manager"]);
    expect(sentence()).toBe(SENTENCE);
    expect(buttons()).toEqual(["Open log folder", "Dismiss"]);
    h.openRefusal = null;
    await click("Open log folder");
    expect(alert()).toEqual([]);
    expect(h.calls.filter((c) => c === "openLogsFolder")).toHaveLength(2);
  });
});

describe("Dismiss", () => {
  it("hides the notice at once and dismisses the unexpected end (dismissUnexpectedEnd)", async () => {
    await start();
    let release = () => {};
    h.dismissWait = new Promise<void>((r) => { release = r; });
    const clicked = button("Dismiss").onClick();
    render();
    // gone before Hersir answers
    expect(banner()).toEqual([]);
    expect(h.calls).toEqual(["getLogsInfo", "dismissUnexpectedEnd"]);
    release();
    await clicked;
    await settle();
    expect(banner()).toEqual([]);
    expect(tree).toBeNull();
  });

  it("stays hidden after a restart", async () => {
    await start();
    await click("Dismiss");
    expect(banner()).toEqual([]);
    await start();
    expect(h.calls).toEqual(["getLogsInfo", "dismissUnexpectedEnd", "getLogsInfo"]);
    expect(banner()).toEqual([]);
    expect(tree).toBeNull();
  });

  it("comes back with why when Hersir couldn't dismiss it, and a next Dismiss works", async () => {
    await start();
    h.dismissRefusal = "Hersir couldn't remove logs/unexpected-end.json: permission denied";
    await click("Dismiss");
    expect(sentence()).toBe(SENTENCE);
    expect(alert()).toEqual(["Hersir couldn't remove logs/unexpected-end.json: permission denied"]);
    h.dismissRefusal = null;
    await click("Dismiss");
    expect(banner()).toEqual([]);
    await start();
    expect(banner()).toEqual([]);
  });
});

describe("without an unexpected end", () => {
  it("shows nothing when unexpectedEnd is null", async () => {
    h.info = info(null);
    await start();
    expect(h.calls).toEqual(["getLogsInfo"]);
    expect(tree).toBeNull();
  });

  it("shows nothing when getLogsInfo fails or answers nothing (the browser preview of an old fixture)", async () => {
    h.infoRefusal = "unknown command get_logs_info";
    await start();
    expect(tree).toBeNull();
    h.infoRefusal = null;
    h.info = null;
    await start();
    expect(tree).toBeNull();
  });

  it("renders nothing on the server before getLogsInfo answers", () => {
    expect(renderToStaticMarkup(<UnexpectedEnd />)).toBe("");
  });
});

describe("the helpers in src/lib/logs.ts", () => {
  it("logName takes the last part of a path, with / or \\", () => {
    expect(logName(FILE)).toBe("hersir-2026-11-13T14-50-26Z.log");
    expect(logName("C:\\logs\\hersir-1.log")).toBe("hersir-1.log");
    expect(logName("hersir-1.log")).toBe("hersir-1.log");
  });
  it("localWhen is the date and time in the local time zone, null for a time that doesn't parse", () => {
    expect(localWhen("2026-11-13T14:47:12Z")).toEqual({ date: "13 Nov", time: "15:47" });
    expect(localWhen("not a time")).toBeNull();
  });
  it("unexpectedEndText is the card's sentence up to the file name", () => {
    expect(unexpectedEndText("2026-11-13T14:47:12Z")).toBe("Hersir ended unexpectedly on 13 Nov at 15:47. Its log can help find out why: ");
  });
});
