// GA-136 QA: the Terminal page (#/terminal/<id>, TF1; design/terminal §3 and §8) against the card's acceptance criteria:
// the top bar, the folder's tabs (+ without a picker, rename, close, Ctrl+PgDn and Ctrl+PgUp), the ended and couldn't-start
// bars with Start again and Try again, a terminal that leaves the list (exit), Show in Editor, and the empty and closed
// pages. There is no DOM here, so a small hook runtime stands in for React (as in SidebarBehind.test.tsx): components are
// called as functions and keep their hooks by their place in the tree, effects run after each render. The open terminals
// come from a fake store that contract CT's fake updates (open, rename, close); the terminal's view, the folder picker and
// the ⋯ menu are stand-ins that keep their props.
// GA-177 QA: the folder (read-only while an agent works there) is asked again on the window's focus, at most once every 5
// seconds (on Omarchy the focus follows the mouse).
// GA-348 QA: Layout, right of +, shows the folder's terminals side by side in panes; the focused pane's terminal is the
// page's (its tab, the address, the top bar). The panes are kept per folder in src/lib/terminalPanes.ts's store, which
// is emptied before each test.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { createElement, type ReactElement, type ReactNode } from "react";
import { Columns2, Columns3, Columns4, Grid2x2, Square } from "lucide-react";
import type { EditorFolder, TerminalSession } from "../types";

type Inst = { slots: unknown[]; i: number };
type EffectSlot = { effect: true; deps?: unknown[]; first: boolean; cleanup?: () => void };
const h = vi.hoisted(() => ({ cur: null as Inst | null, dirty: false, effects: [] as (() => void)[] }));

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
  const useMemo = ((f: () => unknown, deps: unknown[]) => {
    if (!h.cur) return R.useMemo(f, deps);
    const s = slot(() => ({ deps: undefined as unknown[] | undefined, v: undefined as unknown }));
    if (changed(deps, s.deps)) { s.v = f(); s.deps = deps; }
    return s.v;
  }) as typeof R.useMemo;
  const useCallback = ((f: unknown, deps: unknown[]) => (h.cur ? useMemo(() => f, deps) : R.useCallback(f as () => void, deps))) as typeof R.useCallback;
  const useEffect = ((f: () => void | (() => void), deps?: unknown[]) => {
    if (!h.cur) return R.useEffect(f, deps);
    const s = slot<EffectSlot>(() => ({ effect: true, first: true }));
    if (s.first || changed(deps, s.deps)) {
      s.first = false;
      s.deps = deps;
      h.effects.push(() => { s.cleanup?.(); const c = f(); s.cleanup = typeof c === "function" ? c : undefined; });
    }
  }) as typeof R.useEffect;
  const useSyncExternalStore = ((subscribe: never, snapshot: () => unknown) => (h.cur ? snapshot() : R.useSyncExternalStore(subscribe, snapshot))) as typeof R.useSyncExternalStore;
  const hooks = { useState, useRef, useMemo, useCallback, useEffect, useLayoutEffect: useEffect, useSyncExternalStore };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});

// ---- the terminals: a fake store, and contract CT as a fake that keeps it ----
const s = vi.hoisted(() => ({
  terms: [] as TerminalSession[],
  loaded: true,
  size: null as { cols: number; rows: number } | null,
  folders: [] as EditorFolder[],
  calls: [] as string[],
  shown: [] as [string, unknown][],
  went: [] as unknown[],
  replaced: [] as string[],
  copied: [] as string[],
  cleared: [] as string[],
  next: 10,
  /** Why the next terminalOpen fails, or null. */
  fails: null as string | null,
  /** How often the page asked for the Editor's folders (GA-177). */
  folderAsks: 0,
  /** GA-348: each terminal's size as its view sent it (else `size`), the terminals given the keyboard focus, and how many
   *  opens work before `fails` applies. */
  sizes: {} as Record<string, { cols: number; rows: number }>,
  focused: [] as string[],
  okOpens: 0,
}));
vi.mock("../lib/terminals", async (orig) => ({
  ...(await orig<object>()),
  useTerminals: () => s.terms,
  useTerminalsLoaded: () => s.loaded,
  useTerminalSize: (id: string) => s.sizes[id] ?? s.size,
  focusTerminal: (id: string) => { s.focused.push(id); },
  clearTerminal: (id: string) => { s.cleared.push(id); },
  noteChanged: (x: TerminalSession) => { s.terms = s.terms.map((t) => (t.id === x.id ? x : t)); },
  noteOpened: (x: TerminalSession) => { if (!s.terms.some((t) => t.id === x.id)) s.terms = [...s.terms, x]; },
  showIn: (id: string, place: unknown) => { s.calls.push(`showIn ${id}`); s.shown.push([id, place]); },
}));
const folderName = (folder: string) => folder.split("/").pop()!;
/** The backend's default name: the folder's, then "gizai 2", "gizai 3" (the lowest free number). */
const defaultName = (folder: string, but?: string) => {
  const base = folderName(folder);
  const taken = s.terms.filter((t) => t.id !== but).map((t) => t.name);
  if (!taken.includes(base)) return base;
  for (let n = 2; ; n++) if (!taken.includes(`${base} ${n}`)) return `${base} ${n}`;
};
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(real)) if (typeof real[k] === "function") out[k] = () => new Promise(() => {});
  return {
    ...out,
    editorFolders: () => { s.folderAsks++; return Promise.resolve(s.folders); },
    terminalOpen: (folder: string, cols: number, rows: number, name: string | null) => {
      s.calls.push(`open ${folder} ${cols}x${rows} ${name === null ? "null" : `"${name}"`}`);
      if (s.fails) { if (s.okOpens > 0) s.okOpens--; else return Promise.reject(s.fails); }
      const t = term(`n${s.next++}`, { folder, folderPath: `~/Herd/${folderName(folder)}`, name: name?.trim() || defaultName(folder) });
      return Promise.resolve(t);
    },
    terminalClose: (id: string) => { s.calls.push(`close ${id}`); s.terms = s.terms.filter((t) => t.id !== id); return Promise.resolve(); },
    terminalRename: (id: string, name: string) => {
      s.calls.push(`rename ${id} "${name}"`);
      const t = s.terms.find((x) => x.id === id)!;
      return Promise.resolve({ ...t, name: name.trim() || defaultName(t.folder, id) });
    },
  };
});
vi.mock("../router", async (orig) => ({ ...(await orig<object>()), go: (r: unknown) => { s.calls.push(`go ${JSON.stringify(r)}`); s.went.push(r); } }));
vi.mock("../components/terminal/TerminalView", async (orig) => ({ ...(await orig<object>()), TerminalView: (p: object) => createElement("terminal-view", p) }));
vi.mock("../components/editor/FolderPicker", async (orig) => ({ ...(await orig<object>()), FolderPicker: (p: object) => createElement("folder-picker", p) }));
vi.mock("../components/editor/Menus", async (orig) => ({ ...(await orig<object>()), ContextMenu: (p: object) => createElement("context-menu", p) }));

/** The window: its key listeners are called with plain stand-in events (an Event's target can't be set in Node). */
const heard = new Map<string, Set<(e: unknown) => void>>();
const win = {
  addEventListener: (type: string, f: (e: unknown) => void) => { if (!heard.has(type)) heard.set(type, new Set()); heard.get(type)!.add(f); },
  removeEventListener: (type: string, f: (e: unknown) => void) => { heard.get(type)?.delete(f); },
  innerWidth: 1280, innerHeight: 720,
  location: { hash: "", replace: (u: string) => { s.replaced.push(u); } },
};
vi.stubGlobal("window", win);
vi.stubGlobal("navigator", { userAgent: "Linux", platform: "Linux x86_64", clipboard: { writeText: (t: string) => { s.copied.push(t); return Promise.resolve(); } } });
vi.stubGlobal("document", { hidden: false, querySelector: () => null, body: { style: {} }, documentElement: { style: {} } });
vi.stubGlobal("CSS", { escape: (x: string) => x });
vi.stubGlobal("HTMLButtonElement", class {});
vi.stubGlobal("requestAnimationFrame", (f: () => void) => { f(); return 0; });

const { TerminalPage } = await import("./TerminalPage");
const { resetPanes } = await import("../lib/terminalPanes");
const { awayText } = await import("../components/terminal/TerminalView");
// GA-348: the styles and the docs are read from disk (CSS through ?raw is empty under Vitest). node:fs comes in through a
// computed specifier: the build type-checks tests without Node's types.
const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const fileText = (path: string): string => fs.readFileSync(new URL(path, import.meta.url), "utf8");
const terminalCss = fileText("../styles/terminal.css");
const componentsCss = fileText("../styles/components.css");
const editorAndTerminalDoc = fileText("../../docs/features/editor-and-terminal.md");

// ---- the hook runtime ----
let root: () => ReactNode = () => null;
const insts = new Map<string, Inst>();
const unmount = (inst: Inst) => { for (const x of inst.slots) if ((x as EffectSlot)?.effect) (x as EffectSlot).cleanup?.(); };
function expand(node: ReactNode, path: string, seen: Set<string>): ReactNode {
  if (Array.isArray(node)) return node.map((n, i) => expand(n, `${path}.${(n as { key?: string | null })?.key ?? i}`, seen));
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
  for (let n = 0; n < 40; n++) {
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
const one = (pred: (f: Found) => boolean) => findAll(tree, pred)[0];
const byClass = (c: string) => one((f) => f.props.className === c);
/** An element's handler, called with a stand-in event. */
const fire = (f: Found, handler: string, e: Record<string, unknown> = {}) => {
  const target = { getBoundingClientRect: () => ({ left: 300, right: 420, top: 40, bottom: 72 }), parentElement: null, focus: () => {}, closest: () => null };
  (f.props[handler] as (e: unknown) => void)({ preventDefault: () => {}, stopPropagation: () => {}, button: 0, shiftKey: false, currentTarget: target, target, ...e });
  render();
};
/** A key on the page (the window hears it, as it does from the terminal). */
const key = (k: string, more: Record<string, unknown> = {}) => {
  const e = { key: k, code: k, ctrlKey: false, shiftKey: false, altKey: false, metaKey: false, target: null, defaultPrevented: false, ...more,
    preventDefault() { e.defaultPrevented = true; } };
  for (const f of [...(heard.get("keydown") ?? [])]) f(e);
  render();
};

// the top bar
const crumbs = () => byClass("crumbs ed-crumbs")!;
const where = () => byClass("term-where")!;
const showInEditorBtn = () => one((f) => f.type === "button" && textOf(kids(f)).includes("Show in Editor"))!;
const moreBtn = () => one((f) => f.type === "button" && f.props["aria-label"] === "More")!;
const menu = () => one((f) => f.type === "context-menu")?.props as { label: string; items: ({ label: string; kbd?: string; title?: string; run: () => void } | { sep: true })[]; onClose: () => void } | undefined;
const item = (label: string) => menu()!.items.find((x) => "label" in x && x.label === label) as { label: string; kbd?: string; title?: string; run: () => void };
/** Chooses a menu item as ContextMenu does: the menu closes, then the item runs. */
const choose = (label: string) => { const run = item(label).run; menu()!.onClose(); run(); render(); };
const view = () => one((f) => f.type === "terminal-view")?.props as { id: string; place: unknown; findAsk: number; autoFocus: boolean } | undefined;
const picker = () => one((f) => f.type === "folder-picker")?.props;
// the tabs
const strip = () => one((f) => f.props.role === "tablist")!;
const tabs = () => findAll(kids(strip()), (f) => f.props.role === "tab").map((t) => {
  const name = findAll(kids(t), (f) => f.type === "span" && String(f.props.className).startsWith("name"))[0];
  const right = findAll(kids(t), (f) => f.props.className === "prog" || f.props.className === "state")[0];
  return {
    el: t, id: t.props["data-term"] as string, name: name ? textOf(kids(name)) : "", selected: t.props["aria-selected"] as boolean, tip: t.props.title as string,
    says: right ? textOf(kids(right)) : "", x: findAll(kids(t), (f) => f.type === "button" && f.props.className === "x")[0]!,
  };
});
const tab = (id: string) => tabs().find((t) => t.id === id)!;
const plus = () => findAll(kids(strip()), (f) => f.type === "button" && String(f.props["aria-label"]).startsWith("New terminal in"))[0]!;
const nameBox = () => one((f) => f.type === "input" && f.props["aria-label"] === "Terminal's name");
const ask = () => one((f) => f.props.role === "alertdialog" && String(f.props.className).includes("ed-ask"));
const askButton = (label: string) => findAll(kids(ask()!), (f) => f.type === "button" && textOf(kids(f)) === label)[0]!;
const bar = () => one((f) => String(f.props.className).startsWith("ed-bar file"));
const barButton = (label: string) => findAll(kids(bar()!), (f) => f.type === "button" && textOf(kids(f)).includes(label))[0]!;

// ---- the data: two terminals in gizai, one in kade-portal ----
const GIZAI = "/home/you/Herd/gizai";
const KADE = "/home/you/Herd/kade-portal";
const term = (id: string, more: Partial<TerminalSession> = {}): TerminalSession => ({
  id, name: "gizai", folder: GIZAI, folderPath: "~/Herd/gizai", taskId: null, shell: "zsh", busy: null, ended: null, cols: 80, rows: 24, openedAt: 1, ...more,
});
const three = () => [term("t1"), term("t2", { name: "gizai 2", busy: "npm" }), term("k1", { name: "kade-portal", folder: KADE, folderPath: "~/Herd/kade-portal" })];
const folder = (id: string, more: Partial<EditorFolder> = {}): EditorFolder =>
  ({ id, path: id, name: folderName(id), kind: "root", projectId: null, taskId: null, readOnly: false, readOnlyWhy: null, ...more });

async function show(id?: string) {
  root = () => <TerminalPage id={id} youId="u1" />;
  render();
  await settle();
}

beforeEach(() => {
  vi.useFakeTimers();
  for (const [k, inst] of insts) { unmount(inst); insts.delete(k); }
  h.effects.length = 0;
  s.terms = three(); s.loaded = true; s.size = { cols: 132, rows: 38 }; s.folders = [folder(GIZAI), folder(KADE)];
  s.calls = []; s.shown = []; s.went = []; s.replaced = []; s.copied = []; s.cleared = []; s.next = 10; s.fails = null; s.folderAsks = 0;
  s.sizes = {}; s.focused = []; s.okOpens = 0;
  resetPanes();
});
afterEach(() => { root = () => null; render(); vi.useRealTimers(); });

describe("the top bar (design §3 and §8)", () => {
  it("Terminal / gizai 2, the folder in mono, the shell and its size: '~/Herd/gizai · zsh · 132 × 38'", async () => {
    await show("t2");
    expect(textOf(kids(crumbs()))).toBe("Terminal/gizai 2");
    expect(findAll(kids(crumbs()), (f) => f.type === "a")[0]!.props.href).toBe("#/terminal");
    expect(findAll(kids(crumbs()), (f) => f.type === "b")[0]!.props.title).toBe("Double-click to rename (F2)");
    expect(textOf(kids(where()))).toBe("~/Herd/gizai·zsh·132 × 38");
    expect(findAll(kids(where()), (f) => f.props.className === "path")[0]!.props.title).toBe("~/Herd/gizai");
    expect(findAll(kids(where()), (f) => f.props.className === "size")[0]!.props.title).toBe("132 columns, 38 rows");
  });
  it("the size follows the place it shows in, and is the terminal's own before its view is made", async () => {
    s.size = null;
    await show("t2");
    expect(textOf(kids(where()))).toBe("~/Herd/gizai·zsh·80 × 24");
    s.size = { cols: 120, rows: 12 };
    render();
    expect(textOf(kids(where()))).toBe("~/Herd/gizai·zsh·120 × 12");
  });
  it("Show in Editor (tooltip: Show gizai 2 in gizai's Editor, in its terminal panel) calls showIn with the folder's panel, then opens the Editor on the folder", async () => {
    await show("t2");
    expect(showInEditorBtn().props.title).toBe("Show gizai 2 in gizai's Editor, in its terminal panel");
    fire(showInEditorBtn(), "onClick");
    expect(s.shown).toEqual([["t2", { kind: "panel", folder: GIZAI }]]);
    expect(s.went).toEqual([{ page: "editor", id: GIZAI }]);
    expect(s.calls).toEqual(["showIn t2", `go {"page":"editor","id":"${GIZAI}"}`]);
  });
  it("⋯ (tooltip: More) has Rename (F2), Find (Ctrl Shift F), Clear (empties the scrollback), Copy folder path and Close", async () => {
    await show("t2");
    expect(moreBtn().props.title).toBe("More");
    fire(moreBtn(), "onClick");
    expect(menu()!.items.map((x) => ("sep" in x ? "—" : x.label))).toEqual(["Rename", "Find", "Clear", "Copy folder path", "—", "Close"]);
    expect(item("Rename").kbd).toBe("F2");
    expect(item("Find").kbd).toBe("Ctrl Shift F");
    expect(item("Clear").title).toBe("Empties the scrollback");
  });
  it("⋯ → Find opens find in the view, Clear clears it, Copy folder path copies the folder", async () => {
    await show("t2");
    expect(view()!.findAsk).toBe(0);
    fire(moreBtn(), "onClick"); choose("Find");
    expect(view()!.findAsk).toBe(1);
    fire(moreBtn(), "onClick"); choose("Clear");
    expect(s.cleared).toEqual(["t2"]);
    fire(moreBtn(), "onClick"); choose("Copy folder path");
    expect(s.copied).toEqual([GIZAI]);
  });
  it("the terminal shows in the page's place, with the focus", async () => {
    await show("t2");
    expect(view()).toMatchObject({ id: "t2", place: { kind: "page" }, autoFocus: true });
  });
});

describe("the tabs (acceptance: the folder's terminals; + starts another there; Ctrl+PgDn and Ctrl+PgUp switch)", () => {
  it("are the terminals of the page's folder only, the shown one selected, each with ×, the program or 'ended', and its tooltip", async () => {
    s.terms = [...three(), term("t3", { name: "gizai 3", ended: { code: 2, at: 2, why: "zsh ended with exit code 2" } })];
    await show("t2");
    expect(strip().props["aria-label"]).toBe("Terminals in gizai");
    expect(tabs().map((t) => [t.id, t.name, t.selected, t.says])).toEqual([["t1", "gizai", false, ""], ["t2", "gizai 2", true, "npm"], ["t3", "gizai 3", false, "ended"]]);
    expect(tabs().map((t) => t.tip)).toEqual(["gizai · waiting for a command", "gizai 2 · running npm", "gizai 3 · ended"]);
    expect(tabs().map((t) => t.x.props["aria-label"])).toEqual(["Close gizai", "Close gizai 2", "Close gizai 3"]);
  });
  it("a click on another tab opens it", async () => {
    await show("t2");
    fire(tab("t1").el, "onClick");
    expect(s.went).toEqual([{ page: "terminal", id: "t1" }]);
  });
  it("+ (tooltip: New terminal in gizai (Ctrl+Shift+`)) starts another terminal in the folder, without a picker, and shows it", async () => {
    await show("t2");
    expect(plus().props.title).toBe("New terminal in gizai (Ctrl+Shift+`)");
    fire(plus(), "onClick");
    await settle();
    expect(picker()).toBeUndefined();
    expect(s.calls).toEqual([`open ${GIZAI} 0x0 null`, 'go {"page":"terminal","id":"n10"}']);
    expect(s.terms.find((t) => t.id === "n10")?.name).toBe("gizai 3");
  });
  it("Ctrl+Shift+` does what + does", async () => {
    await show("t1");
    key("~", { code: "Backquote", ctrlKey: true, shiftKey: true });
    await settle();
    expect(s.calls[0]).toBe(`open ${GIZAI} 0x0 null`);
  });
  it("+ is off while an agent works on the card, with why as its tooltip", async () => {
    const CARD = "/home/you/.local/share/gizai/worktrees/GA/GA-118";
    s.terms = [term("c1", { name: "GA-118", folder: CARD, folderPath: "~/.local/share/gizai/worktrees/GA/GA-118", taskId: "g118" })];
    s.folders = [folder(CARD, { kind: "card", taskId: "g118", readOnly: true, readOnlyWhy: "QA Agent is working on GA-118" })];
    await show("c1");
    expect(plus().props["aria-disabled"]).toBe(true);
    expect(plus().props.title).toBe("QA Agent is working on GA-118: a terminal there waits until its run ends.");
    fire(plus(), "onClick");
    key("~", { code: "Backquote", ctrlKey: true, shiftKey: true });
    await settle();
    expect(s.calls).toEqual([]);
  });
  it("Ctrl+PgDn and Ctrl+PgUp step through the folder's tabs, round from the last to the first", async () => {
    s.terms = [...three(), term("t3", { name: "gizai 3" })];
    await show("t2");
    key("PageDown", { ctrlKey: true });
    expect(s.went.pop()).toEqual({ page: "terminal", id: "t3" });
    key("PageUp", { ctrlKey: true });
    expect(s.went.pop()).toEqual({ page: "terminal", id: "t1" });
    await show("t3");
    key("PageDown", { ctrlKey: true });
    expect(s.went.pop()).toEqual({ page: "terminal", id: "t1" });
    // not without Ctrl, and never to another folder's terminal
    key("PageDown");
    expect(s.went).toEqual([]);
  });
  it("← and → on a tab with the focus move to the next tab", async () => {
    await show("t1");
    fire(tab("t1").el, "onKeyDown", { key: "ArrowRight", target: undefined });
    expect(s.went).toEqual([]); // the event's target is the tab itself only when the tab has the focus
    const t1 = tab("t1").el;
    const self = { getBoundingClientRect: () => ({}) };
    (t1.props.onKeyDown as (e: unknown) => void)({ key: "ArrowRight", preventDefault: () => {}, target: self, currentTarget: self });
    render();
    expect(s.went).toEqual([{ page: "terminal", id: "t2" }]);
  });
});

describe("Rename (acceptance: a double-click on a tab or on the page's name; the new name on the tab and the page; an empty name gives the default back)", () => {
  it("a double-click on a tab renames it in place; the tab and the page's name show the new name", async () => {
    await show("t2");
    fire(tab("t2").el, "onDoubleClick");
    expect(nameBox()!.props.value).toBe("gizai 2");
    fire(nameBox()!, "onChange", { target: { value: "dev server" } });
    fire(nameBox()!, "onKeyDown", { key: "Enter" });
    await settle();
    expect(s.calls).toEqual(['rename t2 "dev server"']);
    expect(tab("t2").name).toBe("dev server");
    expect(textOf(kids(crumbs()))).toBe("Terminal/dev server");
  });
  it("a double-click on the page's name renames it there; the tab shows it too", async () => {
    await show("t1");
    fire(findAll(kids(crumbs()), (f) => f.type === "b")[0]!, "onDoubleClick");
    expect(findAll(kids(crumbs()), (f) => f.type === "input")[0]!.props.value).toBe("gizai");
    fire(nameBox()!, "onChange", { target: { value: "tests" } });
    fire(nameBox()!, "onKeyDown", { key: "Enter" });
    await settle();
    expect(textOf(kids(crumbs()))).toBe("Terminal/tests");
    expect(tab("t1").name).toBe("tests");
  });
  it("F2 on the page renames it too (not while typing in the terminal)", async () => {
    await show("t1");
    key("F2", { target: { closest: (sel: string) => (sel.includes(".xterm") ? {} : null) } });
    expect(nameBox()).toBeUndefined();
    key("F2", { target: { closest: () => null } });
    expect(nameBox()!.props.value).toBe("gizai");
  });
  it("⋯ → Rename renames it in place", async () => {
    await show("t1");
    fire(moreBtn(), "onClick"); choose("Rename");
    expect(nameBox()!.props.value).toBe("gizai");
  });
  it("an empty name gives the default back, on the tab and the page", async () => {
    s.terms = [term("t1", { name: "dev server" }), term("t2", { name: "gizai 2" })];
    await show("t1");
    fire(tab("t1").el, "onDoubleClick");
    fire(nameBox()!, "onChange", { target: { value: "   " } });
    fire(nameBox()!, "onKeyDown", { key: "Enter" });
    await settle();
    expect(s.calls).toEqual(['rename t1 "   "']);
    expect(tab("t1").name).toBe("gizai");
    expect(textOf(kids(crumbs()))).toBe("Terminal/gizai");
  });
});

describe("closing a tab (acceptance: asks under the tab while a program runs; one waiting for a command closes at once)", () => {
  it("× on a terminal running npm asks under the tab: 'npm is still running in gizai 2. Close it and end npm?', Cancel and Close", async () => {
    await show("t2");
    fire(tab("t2").x, "onClick", { currentTarget: { parentElement: { getBoundingClientRect: () => ({ left: 300, bottom: 72 }) } } });
    expect(s.calls).toEqual([]);
    expect(ask()!.props["aria-label"]).toBe("Close gizai 2");
    expect(textOf(kids(ask()!))).toBe("npm is still running in gizai 2. Close it and end npm?CancelClose");
    // under the tab
    expect((ask()!.props.style as { left: number; top: number })).toMatchObject({ left: 300, top: 76 });
    fire(askButton("Close"), "onClick");
    expect(s.calls).toEqual(["close t2"]);
    expect(ask()).toBeUndefined();
  });
  it("Cancel keeps it", async () => {
    await show("t2");
    fire(tab("t2").el, "onAuxClick", { button: 1 });
    fire(askButton("Cancel"), "onClick");
    expect(ask()).toBeUndefined();
    expect(s.calls).toEqual([]);
  });
  it("a middle click on a tab waiting for a command closes it at once", async () => {
    await show("t2");
    fire(tab("t1").el, "onAuxClick", { button: 1 });
    expect(s.calls).toEqual(["close t1"]);
    expect(tabs().map((t) => t.id)).toEqual(["t2"]);
  });
  it("⋯ → Close asks the same under the tab while npm runs, and closes at once otherwise", async () => {
    await show("t2");
    fire(moreBtn(), "onClick"); choose("Close");
    expect(textOf(kids(ask()!))).toContain("npm is still running in gizai 2. Close it and end npm?");
    fire(askButton("Cancel"), "onClick");
    await show("t1");
    fire(moreBtn(), "onClick"); choose("Close");
    expect(s.calls).toEqual(["close t1"]);
  });
  it("the shown terminal closing gives way to the tab next to it, else the one before it, else #/terminal", async () => {
    s.terms = [...three(), term("t3", { name: "gizai 3" })];
    await show("t2");
    s.terms = s.terms.filter((t) => t.id !== "t2"); // closed (×, the sidebar) or `exit`: it leaves the list
    render();
    expect(s.replaced).toEqual(["#/terminal/t3"]);
    await show("t3");
    s.terms = s.terms.filter((t) => t.id !== "t3");
    render();
    expect(s.replaced.pop()).toBe("#/terminal/t1");
    await show("t1");
    s.terms = s.terms.filter((t) => t.id !== "t1");
    render();
    expect(s.replaced.pop()).toBe("#/terminal");
  });
});

describe("the end states (acceptance: exit closes; another code shows why, with Start again in the same folder)", () => {
  const ended2 = () => [term("t1"), term("t2", { name: "gizai 2", ended: { code: 2, at: 2, why: "zsh ended with exit code 2" } })];
  it("`exit` (code 0): the terminal leaves the list, so its page gives way to the next tab", async () => {
    await show("t1");
    s.terms = s.terms.filter((t) => t.id !== "t1");
    render();
    expect(s.replaced).toEqual(["#/terminal/t2"]);
  });
  it("another code: 'The shell ended with exit code 2.' under the tabs, with Start again and Close; the tab says 'ended'", async () => {
    s.terms = ended2();
    await show("t2");
    expect(textOf(kids(bar()!))).toBe("The shell ended with exit code 2.Start againClose");
    expect(tab("t2").says).toBe("ended");
    expect(view()?.id).toBe("t2");
  });
  it("a signal: 'The shell was ended by a signal.'", async () => {
    s.terms = [term("t1", { ended: { code: null, at: 2, why: "zsh was ended by SIGKILL" } })];
    await show("t1");
    expect(textOf(kids(bar()!))).toBe("The shell was ended by a signal.Start againClose");
  });
  it("Start again opens a new terminal in the same folder with the same name, shows it, then closes the ended one", async () => {
    s.terms = ended2();
    await show("t2");
    fire(barButton("Start again"), "onClick");
    await settle();
    expect(s.calls).toEqual([`open ${GIZAI} 0x0 "gizai 2"`, 'go {"page":"terminal","id":"n10"}', "close t2"]);
    expect(s.terms.map((t) => [t.id, t.name, t.folder])).toEqual([["t1", "gizai", GIZAI], ["n10", "gizai 2", GIZAI]]);
  });
  it("Close on the bar closes the ended terminal", async () => {
    s.terms = ended2();
    await show("t2");
    fire(barButton("Close"), "onClick");
    expect(s.calls).toEqual(["close t2"]);
  });
  it("a shell that couldn't start: 'Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found.', with Try again and Close", async () => {
    await show("t1");
    s.fails = "Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found";
    fire(plus(), "onClick");
    await settle();
    expect(bar()!.props.role).toBe("alert");
    expect(textOf(kids(bar()!))).toBe("Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found.Try againClose");
    // Try again asks again, and the bar goes when it works
    s.fails = null;
    fire(barButton("Try again"), "onClick");
    await settle();
    expect(s.calls.filter((c) => c.startsWith("open"))).toHaveLength(2);
    expect(s.went.pop()).toEqual({ page: "terminal", id: "n10" });
  });
  it("Close on the couldn't-start bar puts it away", async () => {
    await show("t1");
    s.fails = "Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found";
    fire(plus(), "onClick");
    await settle();
    fire(barButton("Close"), "onClick");
    expect(bar()).toBeUndefined();
    expect(s.calls.filter((c) => c.startsWith("close"))).toEqual([]);
  });
  it("Start again that can't start says why, and Close there closes the ended terminal", async () => {
    s.terms = ended2();
    await show("t2");
    s.fails = "Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found";
    fire(barButton("Start again"), "onClick");
    await settle();
    expect(textOf(kids(bar()!))).toBe("Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found.Try againClose");
    fire(barButton("Close"), "onClick");
    expect(s.calls.filter((c) => c.startsWith("close"))).toEqual(["close t2"]);
  });
});

describe("the pages without a terminal (design §3)", () => {
  it("#/terminal with none open: 'No terminal open', what it is for, and New terminal, which opens the picker titled 'New terminal in'", async () => {
    s.terms = [];
    await show();
    const empty = byClass("empty")!;
    expect(textOf(kids(empty))).toBe("No terminal openStart a shell in a project's folder, a folder in ~/Herd or a card's worktree.New terminal");
    fire(findAll(kids(empty), (f) => f.type === "button")[0]!, "onClick");
    expect(picker()?.title).toBe("New terminal in");
    expect(picker()?.you).toBe("u1");
  });
  it("#/terminal/<id> of a terminal that was closed: 'This terminal isn't open any more.' with New terminal", async () => {
    await show("gone");
    const empty = byClass("empty")!;
    expect(textOf(kids(empty))).toBe("This terminal isn't open any more.New terminal");
    fire(findAll(kids(empty), (f) => f.type === "button")[0]!, "onClick");
    expect(picker()?.title).toBe("New terminal in");
  });
  it("says nothing before the list has come from the backend", async () => {
    s.loaded = false;
    await show("t9");
    expect(byClass("empty")).toBeUndefined();
  });
});

describe("GA-177: a window focus asks for the folder again, at most once every 5 seconds", () => {
  const focus = () => { for (const f of [...(heard.get("focus") ?? [])]) f({ type: "focus" }); };
  async function wait(ms: number) { await vi.advanceTimersByTimeAsync(ms); render(); await settle(); }
  const CARD = "/home/you/.local/share/gizai/worktrees/GA/GA-118";
  const card = (readOnly: boolean) => folder(CARD, { kind: "card", taskId: "g118", readOnly, readOnlyWhy: readOnly ? "QA Agent is working on GA-118" : null });

  it("20 focus events within 3 seconds ask once, not 20 times", async () => {
    await show("t1");
    expect(s.folderAsks).toBe(1);
    await wait(1000);
    for (let i = 0; i < 20; i++) { focus(); await wait(150); }
    expect(s.folderAsks).toBe(2);
  });

  it("a focus more than 5 seconds after the last ask asks at once, and + follows the folder; 5 seconds or less after it, nothing", async () => {
    s.terms = [term("c1", { name: "GA-118", folder: CARD, folderPath: "~/.local/share/gizai/worktrees/GA/GA-118", taskId: "g118" })];
    s.folders = [card(false)];
    await show("c1");
    expect(plus().props["aria-disabled"]).toBeFalsy();
    focus();
    await settle();
    expect(s.folderAsks).toBe(2);
    // an agent starts on the card
    s.folders = [card(true)];
    await wait(5000);
    focus();
    await settle();
    expect(s.folderAsks).toBe(2);
    expect(plus().props["aria-disabled"]).toBeFalsy();
    await wait(1);
    focus();
    await settle();
    expect(s.folderAsks).toBe(3);
    expect(plus().props["aria-disabled"]).toBe(true);
    expect(plus().props.title).toBe("QA Agent is working on GA-118: a terminal there waits until its run ends.");
  });

  it("there is no interval: without a focus the folder isn't asked again", async () => {
    await show("t1");
    await wait(10 * 60_000);
    expect(s.folderAsks).toBe(1);
  });

  it("leaving the page stops listening to the focus", async () => {
    await show("t1");
    root = () => null;
    render();
    focus();
    await wait(10_000);
    focus();
    expect(heard.get("focus")?.size ?? 0).toBe(0);
    expect(s.folderAsks).toBe(1);
  });
});

// ---- GA-348: terminals side by side ----
type LayoutItem = { label: string; icon?: ReactElement; checked?: boolean; disabled?: boolean; title?: string; run: () => void };
const layoutBtn = () => findAll(kids(strip()), (f) => f.type === "button" && f.props["aria-label"] === "Layout")[0]!;
const layoutMenu = () => findAll(tree, (f) => f.type === "context-menu" && f.props.label === "Layout")[0]?.props as { items: LayoutItem[]; onClose: () => void } | undefined;
/** Opens Layout and chooses an item as ContextMenu does (the menu closes, then the item runs); a disabled one can't run. */
function pickNow(label: string) {
  fire(layoutBtn(), "onClick");
  const m = layoutMenu()!;
  const it = m.items.find((x) => x.label === label)!;
  m.onClose();
  if (!it.disabled) it.run();
  render();
}
async function pick(label: string) { pickNow(label); await settle(); }
const panesBox = () => byClass("term-panes");
const paneEls = () => findAll(tree, (f) => /^term-pane( |$)/.test(String(f.props.className)));
const paneOf = (f: Found) => {
  const v = findAll(kids(f), (x) => x.type === "terminal-view")[0];
  const cap = findAll(kids(f), (x) => x.props.className === "term-cap")[0];
  const b = findAll(kids(f), (x) => String(x.props.className).startsWith("ed-bar file"))[0];
  return {
    el: f, id: v?.props.id as string | undefined, on: / on( |$)/.test(String(f.props.className)), wait: String(f.props.className).includes(" wait"),
    view: v?.props as { id: string; place: unknown; findAsk: number; autoFocus: boolean } | undefined,
    cap: cap!, capText: cap ? textOf(kids(cap)) : "", bar: b, barText: b ? textOf(kids(b)) : "", text: textOf(kids(f)),
  };
};
const panes = () => paneEls().map(paneOf);
const paneIds = () => panes().map((p) => p.id ?? null);
const pane = (id: string) => panes().find((p) => p.id === id)!;
/** A click in a pane: its mouse-down (capture) focuses it. */
const clickIn = (id: string) => fire(pane(id).el, "onMouseDownCapture");
const views = () => findAll(tree, (f) => f.type === "terminal-view").map((f) => f.props as { id: string; place: unknown; findAsk: number; autoFocus: boolean });
const tabClass = (id: string) => String(tab(id).el.props.className);
const gizai4 = () => [term("t1"), term("t2", { name: "gizai 2" }), term("t3", { name: "gizai 3" }), term("t4", { name: "gizai 4" }),
  term("k1", { name: "kade-portal", folder: KADE, folderPath: "~/Herd/kade-portal" })];

describe("GA-348: the Layout button and its menu", () => {
  it("sits right of + in the tab header, icon-only like +, aria-label Layout, tooltip 'Layout: One terminal' on a folder's first visit", async () => {
    await show("t2");
    const buttons = findAll(kids(strip()), (f) => f.type === "button" && f.props.className !== "x");
    expect(buttons.map((b) => b.props["aria-label"])).toEqual(["New terminal in gizai", "Layout"]);
    expect(layoutBtn().props.title).toBe("Layout: One terminal");
    expect(String(layoutBtn().props.className)).toContain("btn ghost sm icon-only");
    expect(layoutBtn().props["aria-haspopup"]).toBe("menu");
    expect(findAll(kids(layoutBtn()), (f) => f.type === Square)).toHaveLength(1);
    // the same 26px as +, with no room taken on its left (+ keeps margin-left: auto)
    expect(terminalCss).toContain(".term-page > .term-tabs > .btn.icon-only { width: 26px; height: 26px; margin: 0 0 0 auto; }");
    expect(terminalCss).toContain(".term-page > .term-tabs > .btn.icon-only.term-layout { margin-left: 0; }");
  });
  it("opens a menu (Layout) with the five layouts word for word, each with its icon, the current one checked", async () => {
    await show("t2");
    expect(layoutMenu()).toBeUndefined();
    fire(layoutBtn(), "onClick");
    expect(layoutBtn().props["aria-expanded"]).toBe(true);
    const items = layoutMenu()!.items;
    expect(items.map((x) => x.label)).toEqual(["One terminal", "2 side by side", "3 side by side", "4 side by side", "2 × 2"]);
    expect(items.map((x) => x.icon?.type)).toEqual([Square, Columns2, Columns3, Columns4, Grid2x2]);
    expect(items.map((x) => x.checked)).toEqual([true, false, false, false, false]);
    expect(items.map((x) => !!x.disabled)).toEqual([false, false, false, false, false]);
    layoutMenu()!.onClose();
    render();
    await pick("2 side by side");
    expect(layoutBtn().props.title).toBe("Layout: 2 side by side");
    expect(findAll(kids(layoutBtn()), (f) => f.type === Columns2)).toHaveLength(1);
    fire(layoutBtn(), "onClick");
    expect(layoutMenu()!.items.map((x) => x.checked)).toEqual([false, true, false, false, false]);
  });
  it("each layout's icon on the button: 3 → Columns3, 4 → Columns4, 2 × 2 → Grid2x2", async () => {
    s.terms = gizai4();
    await show("t1");
    for (const [label, icon] of [["3 side by side", Columns3], ["4 side by side", Columns4], ["2 × 2", Grid2x2], ["One terminal", Square]] as const) {
      await pick(label);
      expect(layoutBtn().props.title).toBe(`Layout: ${label}`);
      expect(findAll(kids(layoutBtn()), (f) => f.type === icon)).toHaveLength(1);
    }
  });
});

describe("GA-348: the panes", () => {
  it("two open, 2 side by side: both, the one on the page before on the left, each with its caption; no terminal starts", async () => {
    await show("t2");
    await pick("2 side by side");
    expect(panesBox()!.props["data-layout"]).toBe("two");
    expect(paneIds()).toEqual(["t2", "t1"]);
    expect(s.calls.filter((c) => c.startsWith("open"))).toEqual([]);
    expect(panes().map((p) => [p.capText, p.on])).toEqual([["gizai 2npm", true], ["gizai", false]]);
    expect(panes().map((p) => p.el.props["aria-label"])).toEqual(["gizai 2", "gizai"]);
    expect(panes().map((p) => p.el.props.role)).toEqual(["group", "group"]);
    expect(panes().map((p) => p.cap.props.title)).toEqual(["gizai 2 · running npm", "gizai · waiting for a command"]);
    expect(views().map((v) => [v.id, v.place, v.autoFocus])).toEqual([["t2", { kind: "page" }, true], ["t1", { kind: "page" }, false]]);
    // the page's terminal stays the address's: no history entry, no replace
    expect(s.went).toEqual([]);
    expect(s.replaced).toEqual([]);
    expect(tab("t2").selected).toBe(true);
  });
  it("one open, 3 side by side: two more start in the same folder, named as + names them, and the three show in a row", async () => {
    s.terms = [term("t1"), term("k1", { name: "kade-portal", folder: KADE, folderPath: "~/Herd/kade-portal" })];
    await show("t1");
    pickNow("3 side by side");
    // while they start, their panes wait
    expect(panesBox()!.props["data-layout"]).toBe("three");
    expect(panes().map((p) => [p.id ?? null, p.wait])).toEqual([["t1", false], [null, true], [null, true]]);
    expect(panes()[1]!.text).toBe("Starting a terminal…");
    await settle();
    expect(s.calls.filter((c) => c.startsWith("open"))).toEqual([`open ${GIZAI} 0x0 null`, `open ${GIZAI} 0x0 null`]);
    expect(paneIds()).toEqual(["t1", "n10", "n11"]);
    expect(panes().map((p) => p.capText)).toEqual(["gizai", "gizai 2", "gizai 3"]);
    expect(tabs().map((t) => t.name)).toEqual(["gizai", "gizai 2", "gizai 3"]);
    // the first pane keeps the focus and the address; none went to the history
    expect(panes().map((p) => p.on)).toEqual([true, false, false]);
    expect(s.went).toEqual([]);
    expect(s.replaced).toEqual([]);
    // the menu had the keyboard: the page's terminal gets it back on the next frame (fake timers hold requestAnimationFrame)
    await vi.advanceTimersByTimeAsync(20);
    expect(s.focused).toContain("t1");
  });
  it("2 × 2: four panes, in two rows of two", async () => {
    s.terms = gizai4();
    await show("t3");
    await pick("2 × 2");
    expect(panesBox()!.props["data-layout"]).toBe("grid");
    expect(paneIds()).toEqual(["t3", "t4", "t1", "t2"]);
    expect(terminalCss).toContain('.term-panes[data-layout="grid"] { grid-template-columns: repeat(2, minmax(0, 1fr)); grid-template-rows: repeat(2, minmax(0, 1fr)); }');
  });
  it("equal widths (no dragging), with a 1px var(--line) between the panes", () => {
    expect(terminalCss).toContain(".term-panes { display: grid; flex: 1; min-height: 0; gap: 1px; background: var(--line);");
    for (const [l, n] of [["two", 2], ["three", 3], ["four", 4]] as const) {
      expect(terminalCss).toContain(`.term-panes[data-layout="${l}"] { grid-template-columns: repeat(${n}, minmax(0, 1fr)); }`);
    }
    expect(terminalCss).not.toMatch(/term-pane[^{]*\{[^}]*resize:/);
  });
  it("the focused pane's caption is in text, the others in text-3; a caption is about 24px", () => {
    expect(terminalCss).toMatch(/\.term-cap \{[^}]*height: 24px;[^}]*color: var\(--text-3\);/);
    expect(terminalCss).toContain(".term-pane.on > .term-cap { color: var(--text); }");
  });
  it("each pane shows its own terminal in the page's place, keyed by its id, so its size follows its pane; the top bar shows the focused one's size", async () => {
    s.sizes = { t1: { cols: 61, rows: 30 }, t2: { cols: 59, rows: 30 } };
    await show("t2");
    expect(textOf(kids(where()))).toBe("~/Herd/gizai·zsh·59 × 30");
    await pick("2 side by side");
    const keys = findAll(tree, (f) => f.type === "terminal-view").length;
    expect(keys).toBe(2);
    clickIn("t1");
    expect(textOf(kids(where()))).toBe("~/Herd/gizai·zsh·61 × 30");
    // the panes didn't remount: the same views, the same order
    expect(views().map((v) => v.id)).toEqual(["t2", "t1"]);
  });
});

describe("GA-348: the focused pane is the page's terminal", () => {
  it("a click in another pane: its tab is selected, the address is its id with replace (no history entry), the top bar is about it", async () => {
    await show("t2");
    await pick("2 side by side");
    clickIn("t1");
    expect(s.replaced).toEqual(["#/terminal/t1"]);
    expect(s.went).toEqual([]);
    expect(tab("t1").selected).toBe(true);
    expect(tab("t2").selected).toBe(false);
    expect(textOf(kids(crumbs()))).toBe("Terminal/gizai");
    expect(textOf(kids(where()))).toBe("~/Herd/gizai·zsh·132 × 38");
    expect(panes().map((p) => p.on)).toEqual([false, true]);
    // the router follows the address: nothing moves
    await show("t1");
    expect(paneIds()).toEqual(["t2", "t1"]);
    expect(panes().map((p) => p.on)).toEqual([false, true]);
    expect(s.replaced).toEqual(["#/terminal/t1"]);
  });
  it("a click on a caption focuses that pane and gives its terminal the keyboard", async () => {
    await show("t2");
    await pick("2 side by side");
    s.focused = [];
    fire(pane("t1").cap, "onClick");
    expect(s.focused).toEqual(["t1"]);
    expect(s.replaced).toEqual(["#/terminal/t1"]);
    expect(tab("t1").selected).toBe(true);
  });
  it("a click in a pane whose terminal doesn't have the keyboard gives it the keyboard", async () => {
    await show("t2");
    await pick("2 side by side");
    s.focused = [];
    fire(pane("t1").el, "onClick", { currentTarget: { contains: () => false } });
    expect(s.focused).toEqual(["t1"]);
  });
  it("⋯ → Rename, Find, Clear, Close and Show in Editor act on the focused pane's terminal only", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2" })];
    await show("t2");
    await pick("2 side by side");
    clickIn("t1");
    await show("t1");
    fire(moreBtn(), "onClick"); choose("Find");
    expect(views().map((v) => [v.id, v.findAsk])).toEqual([["t2", 0], ["t1", 1]]);
    fire(moreBtn(), "onClick"); choose("Clear");
    expect(s.cleared).toEqual(["t1"]);
    fire(moreBtn(), "onClick"); choose("Rename");
    expect(nameBox()!.props.value).toBe("gizai");
    fire(nameBox()!, "onKeyDown", { key: "Escape" });
    fire(showInEditorBtn(), "onClick");
    expect(s.shown).toEqual([["t1", { kind: "panel", folder: GIZAI }]]);
    expect(s.went).toEqual([{ page: "editor", id: GIZAI }]);
    fire(moreBtn(), "onClick"); choose("Close");
    expect(s.calls.filter((c) => c.startsWith("close"))).toEqual(["close t1"]);
  });
  it("Show in Editor: the pane says '<name> is in <folder>'s Editor.' with Show here; the other panes keep their terminals on the page", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2" })];
    await show("t2");
    await pick("2 side by side");
    fire(showInEditorBtn(), "onClick");
    expect(s.shown).toEqual([["t2", { kind: "panel", folder: GIZAI }]]);
    expect(awayText(s.terms[1]!, { kind: "panel", folder: GIZAI })).toBe("gizai 2 is in gizai's Editor.");
    // back on the page: both panes still there, each with its own view in the page's place
    await show("t2");
    expect(paneIds()).toEqual(["t2", "t1"]);
    expect(views().every((v) => (v.place as { kind: string }).kind === "page")).toBe(true);
  });
});

describe("GA-348: tabs and keys", () => {
  it("a click on a tab that isn't in a pane shows it in the focused pane (with go); the one it replaces stays open as a tab", async () => {
    s.terms = gizai4();
    await show("t1");
    await pick("2 side by side");
    expect(paneIds()).toEqual(["t1", "t2"]);
    fire(tab("t3").el, "onClick");
    expect(s.went).toEqual([{ page: "terminal", id: "t3" }]);
    await show("t3");
    expect(paneIds()).toEqual(["t3", "t2"]);
    expect(panes().map((p) => p.on)).toEqual([true, false]);
    expect(tabs().map((t) => t.id)).toEqual(["t1", "t2", "t3", "t4"]);
    expect(s.calls.filter((c) => c.startsWith("close"))).toEqual([]);
  });
  it("a click on a tab in another pane focuses that pane (replace, no history entry)", async () => {
    s.terms = gizai4();
    await show("t1");
    await pick("2 side by side");
    fire(tab("t2").el, "onClick");
    expect(s.went).toEqual([]);
    expect(s.replaced).toEqual(["#/terminal/t2"]);
    expect(s.focused).toContain("t2");
    expect(panes().map((p) => p.on)).toEqual([false, true]);
  });
  it("Ctrl+PgDn and Ctrl+PgUp follow the same rule", async () => {
    s.terms = gizai4();
    await show("t1");
    await pick("2 side by side");
    key("PageDown", { ctrlKey: true }); // t2 is in a pane: focus it
    expect(s.went).toEqual([]);
    expect(s.replaced).toEqual(["#/terminal/t2"]);
    await show("t2");
    key("PageDown", { ctrlKey: true }); // t3 isn't: into the focused pane
    expect(s.went).toEqual([{ page: "terminal", id: "t3" }]);
    await show("t3");
    expect(paneIds()).toEqual(["t1", "t3"]);
    key("PageUp", { ctrlKey: true }); // t2 isn't in a pane any more: into the focused pane
    expect(s.went.pop()).toEqual({ page: "terminal", id: "t2" });
    await show("t2");
    expect(paneIds()).toEqual(["t1", "t2"]);
    key("PageUp", { ctrlKey: true }); // t1 is: focus it
    expect(s.went).toEqual([{ page: "terminal", id: "t3" }]);
    expect(s.replaced.pop()).toBe("#/terminal/t1");
  });
  it("a tab in another pane than the focused one has the hover fill; the focused one is the selected pill; others neither", async () => {
    s.terms = gizai4();
    await show("t1");
    expect(tabs().filter((t) => tabClass(t.id).includes("in-pane"))).toEqual([]);
    await pick("3 side by side");
    expect(tabs().map((t) => [t.id, t.selected, tabClass(t.id).includes("in-pane")]))
      .toEqual([["t1", true, false], ["t2", false, true], ["t3", false, true], ["t4", false, false]]);
    expect(terminalCss).toContain(".term-page > .term-tabs > .ed-tab.in-pane { background: var(--hover); }");
  });
  it("+ and Ctrl+Shift+` start a terminal in the folder and show it in the focused pane", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2" })];
    await show("t2");
    await pick("2 side by side");
    clickIn("t1");
    await show("t1");
    fire(plus(), "onClick");
    await settle();
    expect(s.calls.filter((c) => c.startsWith("open"))).toEqual([`open ${GIZAI} 0x0 null`]);
    expect(s.went).toEqual([{ page: "terminal", id: "n10" }]);
    await show("n10");
    expect(paneIds()).toEqual(["t2", "n10"]);
    expect(panes().map((p) => p.on)).toEqual([false, true]);
    key("~", { code: "Backquote", ctrlKey: true, shiftKey: true });
    await settle();
    expect(s.went.pop()).toEqual({ page: "terminal", id: "n11" });
    await show("n11");
    expect(paneIds()).toEqual(["t2", "n11"]);
    expect(tabs().map((t) => t.id)).toEqual(["t1", "t2", "n10", "n11"]);
  });
  it("an address from elsewhere (the sidebar, Own page) follows the tab rule for its folder", async () => {
    s.terms = gizai4();
    await show("t1");
    await pick("2 side by side");
    root = () => null; render(); // another page
    await show("t2"); // in a pane: that pane is focused
    expect(paneIds()).toEqual(["t1", "t2"]);
    expect(panes().map((p) => p.on)).toEqual([false, true]);
    root = () => null; render();
    await show("t4"); // not in a pane: into the focused one
    expect(paneIds()).toEqual(["t1", "t4"]);
    expect(panes().map((p) => p.on)).toEqual([false, true]);
  });
});

describe("GA-348: smaller layouts and terminals that close", () => {
  it("a smaller layout closes no terminal: the ones that drop out are still tabs", async () => {
    s.terms = [term("t1")];
    await show("t1");
    await pick("4 side by side");
    expect(paneIds()).toEqual(["t1", "n10", "n11", "n12"]);
    await pick("2 side by side");
    expect(paneIds()).toEqual(["t1", "n10"]);
    await pick("One terminal");
    expect(panesBox()).toBeUndefined();
    expect(views().map((v) => v.id)).toEqual(["t1"]);
    expect(tabs().map((t) => t.id)).toEqual(["t1", "n10", "n11", "n12"]);
    expect(s.calls.filter((c) => c.startsWith("close"))).toEqual([]);
  });
  it("`exit` in a pane while the folder has a terminal not in a pane: that terminal takes the pane", async () => {
    s.terms = gizai4();
    await show("t1");
    await pick("3 side by side");
    s.terms = s.terms.filter((t) => t.id !== "t2");
    render();
    expect(paneIds()).toEqual(["t1", "t4", "t3"]);
    expect(layoutBtn().props.title).toBe("Layout: 3 side by side");
    expect(s.replaced).toEqual([]);
  });
  it("without one, the pane goes and the layout steps down: 3 → 2 → One terminal", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2" }), term("t3", { name: "gizai 3" })];
    await show("t1");
    await pick("3 side by side");
    s.terms = s.terms.filter((t) => t.id !== "t2");
    render();
    expect(paneIds()).toEqual(["t1", "t3"]);
    expect(layoutBtn().props.title).toBe("Layout: 2 side by side");
    s.terms = s.terms.filter((t) => t.id !== "t3");
    render();
    expect(layoutBtn().props.title).toBe("Layout: One terminal");
    expect(panesBox()).toBeUndefined();
    expect(findAll(tree, (f) => f.props.className === "term-cap")).toEqual([]);
    expect(view()).toMatchObject({ id: "t1", autoFocus: true });
  });
  it("2 × 2 steps down to 3 side by side", async () => {
    s.terms = gizai4();
    await show("t1");
    await pick("2 × 2");
    s.terms = s.terms.filter((t) => t.id !== "t4");
    render();
    expect(layoutBtn().props.title).toBe("Layout: 3 side by side");
    expect(panesBox()!.props["data-layout"]).toBe("three");
    expect(paneIds()).toEqual(["t1", "t2", "t3"]);
  });
  it("the focused pane's terminal closes: the pane that takes its place is focused and the address follows with replace", async () => {
    s.terms = gizai4();
    await show("t1");
    await pick("2 side by side");
    clickIn("t2");
    await show("t2");
    s.terms = s.terms.filter((t) => t.id !== "t2");
    render();
    expect(paneIds()).toEqual(["t1", "t3"]);
    expect(s.replaced.pop()).toBe("#/terminal/t3");
    expect(s.went).toEqual([]);
  });
  it("…and without one, the pane left of it is focused", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2" }), term("t3", { name: "gizai 3" })];
    await show("t1");
    await pick("3 side by side");
    clickIn("t3");
    await show("t3");
    s.terms = s.terms.filter((t) => t.id !== "t3");
    render();
    expect(paneIds()).toEqual(["t1", "t2"]);
    expect(s.replaced.pop()).toBe("#/terminal/t2");
  });
});

describe("GA-348: the bars in the panes", () => {
  it("a shell that ended with another code shows its bar (Start again, Close) in its own pane only", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2", ended: { code: 2, at: 2, why: "zsh ended with exit code 2" } })];
    await show("t1");
    await pick("2 side by side");
    expect(pane("t1").bar).toBeUndefined();
    expect(pane("t2").barText).toBe("The shell ended with exit code 2.Start againClose");
    expect(pane("t2").capText).toBe("gizai 2ended");
    expect(findAll(tree, (f) => String(f.props.className).startsWith("ed-bar file"))).toHaveLength(1);
  });
  it("Start again starts the new shell in that pane, with the same name, and closes the ended one", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2", ended: { code: 2, at: 2, why: "zsh ended with exit code 2" } })];
    await show("t1");
    await pick("2 side by side");
    fire(findAll(kids(pane("t2").bar!), (f) => f.type === "button" && textOf(kids(f)).includes("Start again"))[0]!, "onClick");
    await settle();
    expect(s.calls.filter((c) => c.startsWith("open") || c.startsWith("close"))).toEqual([`open ${GIZAI} 0x0 "gizai 2"`, "close t2"]);
    expect(paneIds()).toEqual(["t1", "n10"]);
    await show("n10");
    expect(paneIds()).toEqual(["t1", "n10"]);
    expect(pane("t1").bar).toBeUndefined();
    expect(pane("n10").bar).toBeUndefined();
  });
  it("Close on a pane's ended bar closes that terminal; the layout steps down", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2", ended: { code: 2, at: 2, why: "zsh ended with exit code 2" } })];
    await show("t1");
    await pick("2 side by side");
    fire(findAll(kids(pane("t2").bar!), (f) => f.type === "button" && textOf(kids(f)) === "Close")[0]!, "onClick");
    expect(s.calls.filter((c) => c.startsWith("close"))).toEqual(["close t2"]);
    render();
    expect(layoutBtn().props.title).toBe("Layout: One terminal");
  });
  it("a terminal that can't start while choosing a layout: the couldn't-start bar with Try again; the panes that got a terminal work", async () => {
    s.terms = [term("t1")];
    await show("t1");
    s.fails = "Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found";
    s.okOpens = 1;
    await pick("3 side by side");
    expect(s.calls.filter((c) => c.startsWith("open"))).toHaveLength(2);
    expect(layoutBtn().props.title).toBe("Layout: 2 side by side");
    expect(paneIds()).toEqual(["t1", "n10"]);
    expect(pane("t1").bar!.props.role).toBe("alert");
    expect(pane("t1").barText).toBe("Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found.Try againClose");
    expect(pane("n10").bar).toBeUndefined();
    // Try again starts the missing one again
    s.fails = null;
    fire(findAll(kids(pane("t1").bar!), (f) => f.type === "button" && textOf(kids(f)).includes("Try again"))[0]!, "onClick");
    await settle();
    expect(s.calls.filter((c) => c.startsWith("open"))).toHaveLength(3);
    expect(layoutBtn().props.title).toBe("Layout: 3 side by side");
    expect(paneIds()).toEqual(["t1", "n10", "n11"]);
    expect(findAll(tree, (f) => String(f.props.className).startsWith("ed-bar file"))).toHaveLength(0);
  });
  it("none could start: One terminal, with the couldn't-start bar; Close puts it away", async () => {
    s.terms = [term("t1")];
    await show("t1");
    s.fails = "Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found";
    await pick("2 × 2");
    expect(layoutBtn().props.title).toBe("Layout: One terminal");
    expect(panesBox()).toBeUndefined();
    expect(textOf(kids(bar()!))).toBe("Couldn't start zsh in ~/Herd/gizai: /usr/bin/zsh wasn't found.Try againClose");
    fire(barButton("Close"), "onClick");
    expect(bar()).toBeUndefined();
  });
  it("while an agent works in the card's worktree, layouts that need more terminals than are open are off, with +'s tooltip", async () => {
    const CARD = "/home/you/.local/share/gizai/worktrees/GA/GA-118";
    const card = (id: string, name: string) => term(id, { name, folder: CARD, folderPath: "~/.local/share/gizai/worktrees/GA/GA-118", taskId: "g118" });
    s.terms = [card("c1", "GA-118"), card("c2", "GA-118 2")];
    s.folders = [folder(CARD, { kind: "card", taskId: "g118", readOnly: true, readOnlyWhy: "QA Agent is working on GA-118" })];
    await show("c1");
    fire(layoutBtn(), "onClick");
    const items = layoutMenu()!.items;
    const why = "QA Agent is working on GA-118: a terminal there waits until its run ends.";
    expect(plus().props.title).toBe(why);
    expect(items.map((x) => [x.label, !!x.disabled, x.title])).toEqual([
      ["One terminal", false, undefined], ["2 side by side", false, undefined], ["3 side by side", true, why], ["4 side by side", true, why], ["2 × 2", true, why],
    ]);
    layoutMenu()!.onClose();
    render();
    await pick("2 side by side");
    expect(paneIds()).toEqual(["c1", "c2"]);
    expect(s.calls.filter((c) => c.startsWith("open"))).toEqual([]);
  });
});

describe("GA-348: kept per folder while Hersir runs; One terminal as before", () => {
  it("leaving the page and coming back shows the same layout and terminals; another folder has its own (One terminal at first)", async () => {
    s.terms = gizai4();
    await show("t1");
    await pick("3 side by side");
    clickIn("t3");
    await show("t3");
    root = () => null; render(); // leave the page
    await show("t3");
    expect(layoutBtn().props.title).toBe("Layout: 3 side by side");
    expect(paneIds()).toEqual(["t1", "t2", "t3"]);
    expect(panes().map((p) => p.on)).toEqual([false, false, true]);
    await show("k1");
    expect(layoutBtn().props.title).toBe("Layout: One terminal");
    expect(panesBox()).toBeUndefined();
    await show("t3");
    expect(layoutBtn().props.title).toBe("Layout: 3 side by side");
    expect(paneIds()).toEqual(["t1", "t2", "t3"]);
  });
  it("a terminal closed while another folder's page shows (the sidebar): back on the folder, its pane went and the layout stepped down", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2" }), term("t3", { name: "gizai 3" }), term("k1", { name: "kade-portal", folder: KADE, folderPath: "~/Herd/kade-portal" })];
    await show("t1");
    await pick("3 side by side");
    await show("k1");
    s.terms = s.terms.filter((t) => t.id !== "t2");
    render();
    await show("t1");
    expect(layoutBtn().props.title).toBe("Layout: 2 side by side");
    expect(paneIds()).toEqual(["t1", "t3"]);
  });
  it("the docs have the Layout bullet word for word, under 'In the sidebar and on its own page'", () => {
    const section = editorAndTerminalDoc.split("### In the sidebar and on its own page")[1]!.split("\n### ")[0]!;
    expect(section).toContain("\n- **Layout** (right of +): the folder's terminals side by side, 2, 3 or 4 in a row, or 2 × 2. It starts the terminals it needs in the folder; a click in a terminal, or on its tab, picks the one you type in. Fewer panes close nothing: the others stay open as tabs.\n");
  });
  it("One terminal looks as before: no captions, no panes, one view with the focus, the bar under the tabs", async () => {
    s.terms = [term("t1"), term("t2", { name: "gizai 2", ended: { code: 2, at: 2, why: "zsh ended with exit code 2" } })];
    await show("t2");
    expect(panesBox()).toBeUndefined();
    expect(paneEls()).toEqual([]);
    expect(findAll(tree, (f) => f.props.className === "term-cap")).toEqual([]);
    expect(views()).toEqual([expect.objectContaining({ id: "t2", place: { kind: "page" }, autoFocus: true })]);
    expect(textOf(kids(bar()!))).toBe("The shell ended with exit code 2.Start againClose");
    expect(tabs().map((t) => [t.id, t.selected, tabClass(t.id).includes("in-pane")])).toEqual([["t1", false, false], ["t2", true, false]]);
  });
});

describe("GA-348: a menu item that is one of a choice", () => {
  it("the check's CSS: 14px, on the right, in text-2", () => {
    expect(componentsCss).toContain(".pop .opt > .icon.check { flex: none; width: 14px; height: 14px; margin-left: auto; color: var(--text-2); }");
  });
});
