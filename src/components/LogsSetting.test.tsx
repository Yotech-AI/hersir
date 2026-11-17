// GA-282 QA: Settings → General → Data → Hersir's logs (LogsSetting): the logs folder from getLogsInfo (GA-281) in mono,
// `<data folder>/logs` until it answers or when it fails, an Open folder button (openLogsFolder) and the card's hint word for
// word; a failure to open shows under the row instead of the hint. Where the row sits in Settings is in SettingsTabs.test.tsx.
// There is no DOM here, so a small hook runtime stands in for React (as in BackupSettings.test.tsx).
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";

type Inst = { slots: unknown[]; i: number };
type EffectSlot = { effect: true; deps?: unknown[]; first: boolean; cleanup?: () => void };
const h = vi.hoisted(() => ({
  cur: null as Inst | null,
  dirty: false,
  effects: [] as (() => void)[],
  folder: "" as string,
  infoRefusal: null as string | null,
  openRefusal: null as string | null,
  calls: [] as string[],
}));

vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const slot = <T,>(make: () => T): T => {
    const c = h.cur!;
    if (c.i === c.slots.length) c.slots.push(make());
    return c.slots[c.i++] as T;
  };
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
    if (s.first) { s.first = false; s.deps = deps; h.effects.push(() => { const c = f(); s.cleanup = typeof c === "function" ? c : undefined; }); }
  }) as typeof R.useEffect;
  const hooks = { useState, useEffect };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});

vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(real)) if (typeof real[k] === "function") out[k] = () => new Promise(() => {});
  return {
    ...out,
    getLogsInfo: async () => {
      h.calls.push("getLogsInfo");
      if (h.infoRefusal) throw h.infoRefusal;
      return { folder: h.folder, current: `${h.folder}/hersir-2026-11-17T09-00-00Z.log`, unexpectedEnd: null };
    },
    openLogsFolder: async () => { h.calls.push("openLogsFolder"); if (h.openRefusal) throw h.openRefusal; },
  };
});

const { LogsSetting } = await import("./LogsSetting");

// ---- the hook runtime ----
let inst: Inst = { slots: [], i: 0 };
let tree: ReactNode = null;
const DATA = "/home/you/.local/share/gizai";
function expand(node: ReactNode): ReactNode {
  if (Array.isArray(node)) return node.map(expand);
  if (!node || typeof node !== "object" || !("props" in node)) return node;
  const el = node as ReactElement<Record<string, unknown>>;
  if (el.type === LogsSetting) {
    inst.i = 0;
    h.cur = inst;
    let out: ReactNode;
    try { out = LogsSetting(el.props as { dataDir: string }); } finally { h.cur = null; }
    return expand(out);
  }
  if (typeof el.type === "function") return expand((el.type as (p: unknown) => ReactNode)(el.props));
  return { ...el, props: { ...el.props, children: expand(el.props.children as ReactNode) } } as ReactNode;
}
function render() {
  for (let n = 0; n < 30; n++) {
    h.dirty = false;
    tree = expand(<LogsSetting dataDir={DATA} />);
    for (const f of h.effects.splice(0)) f();
    if (!h.dirty) return;
  }
  throw new Error("the render never settled");
}
async function settle() { for (let i = 0; i < 4; i++) { await new Promise((r) => setTimeout(r, 0)); render(); } }
async function mount() { inst = { slots: [], i: 0 }; render(); await settle(); }

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
const html = () => renderToStaticMarkup(tree as ReactElement);
const folderShown = () => findAll(tree, (f) => String(f.props.className ?? "").split(" ").includes("mono")).map((f) => textOf(kids(f)));
/** What shows under the row (Field's hint, warning or error), as "kind: text". */
const under = () => findAll(tree, (f) => f.type === "span" && ["hint", "warn", "error"].includes(String(f.props.className))).map((f) => `${f.props.className}: ${textOf(kids(f))}`);
const openButton = () => {
  const all = findAll(tree, (f) => f.type === "button" && textOf(kids(f)).trim() === "Open folder");
  expect(all).toHaveLength(1);
  return all[0]!.props as { onClick: () => unknown };
};

const HINT = "One file per start; the 20 newest are kept. If Hersir crashed, send us the file of that start.";

beforeEach(() => {
  h.folder = `${DATA}/logs`;
  h.infoRefusal = h.openRefusal = null;
  h.calls = [];
});

describe("Hersir's logs in Settings", () => {
  it("is a wide field labelled Hersir's logs with the folder in mono, Open folder and the hint word for word", async () => {
    h.folder = "/srv/hersir-data/logs";
    await mount();
    expect(h.calls).toEqual(["getLogsInfo"]);
    expect(html()).toMatch(/^<div class="field wide"><label>Hersir&#x27;s logs<\/label><span class="mono bk-folder">\/srv\/hersir-data\/logs<\/span><div class="bk-actions"><button type="button" class="btn sm"><svg[^>]*>.*<\/svg>Open folder<\/button><\/div><span class="hint">/);
    expect(folderShown()).toEqual(["/srv/hersir-data/logs"]);
    expect(under()).toEqual([`hint: ${HINT}`]);
  });

  it("shows <data folder>/logs until getLogsInfo answers, and when it fails", async () => {
    expect(renderToStaticMarkup(<LogsSetting dataDir={DATA} />)).toContain(`<span class="mono bk-folder">${DATA}/logs</span>`);
    h.infoRefusal = "unknown command get_logs_info";
    await mount();
    expect(folderShown()).toEqual([`${DATA}/logs`]);
    expect(under()).toEqual([`hint: ${HINT}`]);
  });

  it("shows the folder as the backend writes it, also on Windows", async () => {
    h.folder = "C:\\Users\\you\\AppData\\Roaming\\gizai\\logs";
    await mount();
    expect(folderShown()).toEqual(["C:\\Users\\you\\AppData\\Roaming\\gizai\\logs"]);
  });

  it("Open folder opens the logs folder (openLogsFolder)", async () => {
    await mount();
    await openButton().onClick();
    await settle();
    expect(h.calls).toEqual(["getLogsInfo", "openLogsFolder"]);
    expect(under()).toEqual([`hint: ${HINT}`]);
  });

  it("shows why under the row when the folder can't be opened, and the hint again after a next try works", async () => {
    await mount();
    h.openRefusal = "Hersir couldn't open the folder: no file manager";
    await openButton().onClick();
    await settle();
    expect(under()).toEqual(["error: Hersir couldn't open the folder: no file manager"]);
    h.openRefusal = null;
    await openButton().onClick();
    await settle();
    expect(under()).toEqual([`hint: ${HINT}`]);
  });
});
