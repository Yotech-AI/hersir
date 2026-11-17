// GA-136 QA: the Editor's folder picker as New terminal uses it (TF1; design/terminal §2 and §8): titled "New terminal in",
// the note from 20 open terminals on top of the list, the folder the page shows on top and chosen, a card an agent works
// on off (with the pulse and why), a card that isn't pinned to you asking under its row with Edit anyway, a card pinned to
// you starting at once, and a terminal that couldn't start saying why under its row with the picker still open. The
// Editor's own use (no onPick) is checked to be as it was. There is no DOM here, so a small hook runtime stands in for React
// (as in SidebarBehind.test.tsx), and the portal renders in place. GA-278 QA: the Chat's Open terminal opens this picker
// titled "Open terminal in", with you and the note, and a folder where a terminal can't start says why with it still open.
// GA-296 QA: Open terminal became the top bar's Open, a menu: its New terminal opens that picker while the chat has no
// terminal, and New terminal in… opens it always; a pick opens a Terminal tab in the chat's side island.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import type { EditorFolder } from "../../types";

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
  // subscribes on the first draw and draws again when the store says it changed (OpenInTerminal.test.tsx's)
  const useSyncExternalStore = ((subscribe: (f: () => void) => () => void, snapshot: () => unknown) => {
    if (!h.cur) return snapshot();
    const s = slot(() => ({ effect: true, first: true, sub: undefined as unknown, cleanup: undefined as (() => void) | undefined }));
    if (s.sub !== subscribe) { s.cleanup?.(); s.sub = subscribe; s.cleanup = subscribe(() => { h.dirty = true; }); }
    return snapshot();
  }) as typeof R.useSyncExternalStore;
  const hooks = { useState, useRef, useMemo, useCallback, useEffect, useLayoutEffect: useEffect, useSyncExternalStore };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});
vi.mock("react-dom", async (orig) => ({ ...(await orig<object>()), createPortal: (children: ReactNode) => children }));

const HERD = "/home/you/Herd";
const WT = "/home/you/.local/share/gizai/worktrees";
const folder = (id: string, more: Partial<EditorFolder>): EditorFolder =>
  ({ id, path: id.replace("/home/you", "~"), name: id.split("/").pop()!, kind: "root", projectId: null, taskId: null, readOnly: false, readOnlyWhy: null, ...more });
const FOLDERS: EditorFolder[] = [
  folder(`${HERD}/gizai`, { kind: "project", projectId: "p1" }),
  folder(`${HERD}/invoice-api`, {}),
  folder(`${HERD}/kade-portal`, {}),
  folder(`${WT}/KADE/KADE-41`, { kind: "card", taskId: "k41" }),
  folder(`${WT}/GA/GA-118`, { kind: "card", taskId: "g118", readOnly: true, readOnlyWhy: "QA Agent is working on GA-118" }),
  folder(`${WT}/GA/GA-112`, { kind: "card", taskId: "g112" }),
];
const TASKS = [
  { id: "k41", identifier: "KADE-41", title: "Fix the invoice total", byHand: true, assigneeId: "u1", assigneeName: "Jeffrey", stateCategory: "in_progress" },
  { id: "g118", identifier: "GA-118", title: "Show a run's changes", byHand: false, assigneeId: "qa", assigneeName: "QA Agent", stateCategory: "in_progress" },
  { id: "g112", identifier: "GA-112", title: "Remember the open tabs per folder", byHand: false, assigneeId: "fe", assigneeName: "Frontend Agent", stateCategory: "todo" },
];
const s = vi.hoisted(() => ({ went: [] as unknown[] }));
// GA-278: what terminalOpen answers the Chat's Open terminal, the folders it was asked for, and the open terminals
const chatT = vi.hoisted(() => ({ opens: [] as string[], answer: (_folder: string): Promise<unknown> => new Promise(() => {}), terms: [] as unknown[] }));
vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(real)) if (typeof real[k] === "function") out[k] = () => new Promise(() => {});
  return {
    ...out,
    editorFolders: () => Promise.resolve(FOLDERS),
    getSettings: () => Promise.resolve({ editorRoots: ["~/Herd"] }),
    listTasks: () => Promise.resolve(TASKS),
    listProjects: () => Promise.resolve([{ id: "p1", key: "GA", name: "Gizai", color: "var(--c-blue)" }]),
    terminalOpen: (folder: string) => { chatT.opens.push(folder); return chatT.answer(folder); },
  };
});
vi.mock("../../lib/terminals", async (orig) => ({
  ...(await orig<object>()), useTerminals: () => chatT.terms, noteOpened: () => {}, terminalsNow: () => [], watchTerminals: () => () => {},
}));
vi.mock("../../router", async (orig) => ({ ...(await orig<object>()), go: (r: unknown) => { s.went.push(r); } }));
// GA-297: ChatSide brings the Card tab (its comment box and the pull request's counts load CodeMirror, which needs a DOM);
// no card opens here
vi.mock("../chat/ChatCard", () => ({ CardTab: () => null, ChatCardBody: () => null }));
// GA-298: ChatSide brings the Editor tab (CodeMirror's view, which needs a DOM) and the Board tab; neither shows here, and
// Open a file… opening a file is recorded
vi.mock("../chat/ChatEditor", () => ({
  ChatEditorBody: () => null, EditorCloseAsk: () => null, EditorTab: () => null, closeEditorTabs: () => {}, editorOwnPage: () => {},
  openEditorTab: (...a: unknown[]) => { s.went.push(["openEditorTab", ...a]); },
}));
vi.mock("../chat/ChatBoard", () => ({ BoardTab: () => null, ChatBoardBody: () => null, boardName: () => "Board", boardOwnPage: () => {} }));

const store = new Map<string, string>();
vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => { store.set(k, String(v)); }, removeItem: (k: string) => { store.delete(k); } });
vi.stubGlobal("window", Object.assign(new EventTarget(), { innerWidth: 1280, innerHeight: 720 }));
vi.stubGlobal("document", { body: {}, addEventListener: () => {}, removeEventListener: () => {} });
// GA-298: ChatSide's keys ask whether the key came from a terminal (e.target instanceof Element)
vi.stubGlobal("Element", class {});

const { FolderPicker } = await import("./FolderPicker");
const { ChatSide, OpenButton } = await import("../chat/ChatSide");
const Side = await import("../../lib/chatSide");

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
const fire = (f: Found, handler: string, e: Record<string, unknown> = {}) => {
  (f.props[handler] as (e: unknown) => void)({ preventDefault: () => {}, stopPropagation: () => {}, ...e });
  render();
};
const groups = () => findAll(tree, (f) => f.props.role === "group").map((g) => [g.props["aria-label"], findAll(kids(g), (f) => f.props.role === "option").map((o) => textOf(kids(findAll(kids(o), (x) => x.props.className === "card-id" || x.props.className === "label")[0]!)))]);
const option = (name: string) => findAll(tree, (f) => f.props.role === "option").find((o) => findAll(kids(o), (x) => (x.props.className === "card-id" || x.props.className === "label") && textOf(kids(x)) === name).length > 0)!;
const confirmBox = () => one((f) => f.props.role === "alertdialog");
const failedNote = () => one((f) => f.props.role === "alert");

let picked: string[] = [];
let closed = 0;
async function show(props: Record<string, unknown> = {}) {
  root = () => <FolderPicker at={{ x: 240, y: 100 }} onClose={() => { closed += 1; }} {...props} />;
  render();
  await settle();
}
const terminal = (more: Record<string, unknown> = {}) => show({ title: "New terminal in", you: "u1", onPick: (f: EditorFolder) => { picked.push(f.name); }, ...more });

beforeEach(() => {
  vi.useFakeTimers();
  for (const [k, inst] of insts) { unmount(inst); insts.delete(k); }
  h.effects.length = 0;
  picked = []; closed = 0; s.went = []; store.clear();
});
afterEach(() => { root = () => null; render(); vi.useRealTimers(); });

describe("New terminal's folder picker (design §2)", () => {
  it("is titled 'New terminal in' above 'Find a folder', with the Editor's groups: Projects, In ~/Herd, Cards", async () => {
    await terminal();
    expect(one((f) => f.props.role === "dialog")!.props["aria-label"]).toBe("New terminal in");
    expect(textOf(kids(one((f) => f.props.className === "ed-picker-title")!))).toBe("New terminal in");
    expect(one((f) => f.type === "input")!.props.placeholder).toBe("Find a folder");
    expect(groups()).toEqual([["Projects", ["Gizai"]], ["In ~/Herd", ["invoice-api", "kade-portal"]], ["Cards", ["KADE-41", "GA-118", "GA-112"]]]);
  });
  it("shows the note on top of the list when it has one (from 20 open terminals), and none without", async () => {
    await terminal({ note: "20 terminals are open: each keeps its shell running until you close it." });
    const note = one((f) => f.props.className === "ed-picker-note")!;
    expect(textOf(kids(note))).toBe("20 terminals are open: each keeps its shell running until you close it.");
    expect(note.props.role).toBe("status");
    root = () => null; render();
    await terminal({ note: null });
    expect(one((f) => f.props.className === "ed-picker-note")).toBeUndefined();
  });
  it("puts the folder the page shows on top, chosen, so Enter starts another terminal there", async () => {
    await terminal({ current: `${HERD}/kade-portal` });
    expect(groups()[0]).toEqual(["In ~/Herd", ["kade-portal", "invoice-api"]]);
    expect(option("kade-portal").props["aria-selected"]).toBe(true);
    fire(one((f) => f.type === "input")!, "onKeyDown", { key: "Enter" });
    expect(picked).toEqual(["kade-portal"]);
    expect(closed).toBe(1);
  });
  it("a card an agent works on is off, with the pulse and who works; its tooltip says why; a click does nothing", async () => {
    await terminal();
    const off = option("GA-118");
    expect(off.props["aria-disabled"]).toBe(true);
    expect(off.props.title).toBe("Show a run's changes\nQA Agent is working on GA-118: a terminal there waits until its run ends.");
    const stand = findAll(kids(off), (f) => f.props.className === "stand")[0]!;
    expect(textOf(kids(stand))).toBe("QA Agent is working");
    expect(findAll(kids(stand), (f) => f.props.className === "pulse")).toHaveLength(1);
    fire(off, "onClick");
    expect(picked).toEqual([]);
    expect(closed).toBe(0);
  });
  it("a card that isn't pinned to you asks under its row: 'GA-112 is Frontend Agent's card: …', and Edit anyway starts it", async () => {
    await terminal();
    fire(option("GA-112"), "onClick");
    expect(picked).toEqual([]);
    const ask = confirmBox()!;
    expect(textOf(kids(findAll(kids(ask), (f) => f.props.className === "grow")[0]!))).toBe("GA-112 is Frontend Agent's card: a shell there can change its files, as an edit can.");
    expect(findAll(kids(ask), (f) => f.type === "button").map((b) => textOf(kids(b)))).toEqual(["Edit anyway", "Cancel"]);
    // under its row
    const cards = findAll(tree, (f) => f.props.role === "group" && f.props["aria-label"] === "Cards")[0]!;
    const order = findAll(kids(cards), (f) => f.props.role === "option" || f.props.role === "alertdialog").map((f) => (f.props.role === "option" ? textOf(kids(findAll(kids(f), (x) => x.props.className === "card-id")[0]!)) : "ask"));
    expect(order).toEqual(["KADE-41", "GA-118", "GA-112", "ask"]);
    expect(option("GA-112").props.title).toBe("Remember the open tabs per folder\nGA-112 is Frontend Agent's card");
    fire(findAll(kids(ask), (f) => f.type === "button" && textOf(kids(f)) === "Edit anyway")[0]!, "onClick");
    expect(picked).toEqual(["GA-112"]);
    expect(closed).toBe(1);
  });
  it("Cancel keeps the picker open, without starting anything", async () => {
    await terminal();
    fire(option("GA-112"), "onClick");
    fire(findAll(kids(confirmBox()!), (f) => f.type === "button" && textOf(kids(f)) === "Cancel")[0]!, "onClick");
    expect(confirmBox()).toBeUndefined();
    expect(picked).toEqual([]);
    expect(closed).toBe(0);
  });
  it("a card pinned to you (by hand) starts at once", async () => {
    await terminal();
    expect(textOf(kids(findAll(kids(option("KADE-41")), (f) => f.props.className === "stand")[0]!))).toBe("by hand");
    fire(option("KADE-41"), "onClick");
    expect(confirmBox()).toBeUndefined();
    expect(picked).toEqual(["KADE-41"]);
  });
  it("a terminal that couldn't start says why under its row, with the picker still open; one that started closes it", async () => {
    let answer: string | null = "Couldn't start zsh in ~/Herd/invoice-api: /usr/bin/zsh wasn't found.";
    await terminal({ onPick: (f: EditorFolder) => { picked.push(f.name); return Promise.resolve(answer); } });
    fire(option("invoice-api"), "onClick");
    await settle();
    expect(closed).toBe(0);
    expect(textOf(kids(failedNote()!))).toBe("Couldn't start zsh in ~/Herd/invoice-api: /usr/bin/zsh wasn't found.");
    const list = findAll(tree, (f) => f.props.role === "group" && f.props["aria-label"] === "In ~/Herd")[0]!;
    const order = findAll(kids(list), (f) => f.props.role === "option" || f.props.role === "alert").map((f) => (f.props.role === "option" ? textOf(kids(findAll(kids(f), (x) => x.props.className === "label")[0]!)) : "why"));
    expect(order).toEqual(["invoice-api", "why", "kade-portal"]);
    answer = null;
    fire(option("invoice-api"), "onClick");
    await settle();
    expect(failedNote()).toBeUndefined();
    expect(closed).toBe(1);
    expect(picked).toEqual(["invoice-api", "invoice-api"]);
  });
});

describe("GA-278, GA-296: the Chat's Open opens it", () => {
  const openBtn = () => one((f) => f.type === "button" && typeof f.props.className === "string" && f.props.className.includes("chat-open"));
  const menu = () => one((f) => f.props.role === "menu");
  const items = () => findAll(kids(menu()!), (f) => f.props.role === "menuitem");
  const item = (label: string) => items().find((i) => textOf(kids(i)).startsWith(label) && (label.endsWith("…") || !textOf(kids(i)).includes("…")))!;
  /** The top bar's Open, its menu opened. */
  const openMenu = async () => {
    root = () => <OpenButton youId="u1" />;
    render();
    await settle();
    const btn = openBtn()!;
    expect([btn.props["aria-haspopup"], btn.props["aria-expanded"]]).toEqual(["menu", false]);
    fire(btn, "onClick", { currentTarget: { getBoundingClientRect: () => ({ left: 900, bottom: 40 }) } });
    await settle();
  };
  /** Open, then New terminal (the chat has no terminal yet: the picker). */
  const openTerminal = async () => {
    await openMenu();
    fire(item("New terminal"), "onClick");
    await settle();
  };
  // the tree holds what the components drew: the picker is its dialog
  const picker = () => one((f) => f.props.role === "dialog");
  const classText = (cls: string) => { const f = one((x) => x.props.className === cls); return f ? textOf(kids(f)) : null; };
  const kade = (f: string) => ({ id: "c1", name: "kade-portal", folder: f, folderPath: "~/Herd/kade-portal", taskId: null, shell: "sh", busy: null, ended: null, cols: 80, rows: 24, openedAt: 1 });
  beforeEach(() => {
    chatT.opens = []; chatT.terms = []; chatT.answer = () => new Promise(() => {});
    // the chat on show starts with nothing open beside it
    Side.showChat("c1");
    Side.closeSideTabs(Side.sideNow().tabs.map(Side.tabKey));
  });

  // GA-298 adds Open a file… (Alt+2) and Board (Alt+3) after New terminal (design/islands-chat → The top bar)
  it("Open's menu, under it: New terminal with its key, then (no terminal in the chat yet) Open a file… and Board", async () => {
    await openMenu();
    const m = menu()!;
    expect(m.props["aria-label"]).toBe("Open beside the chat");
    expect(m.props.style, "under the button: its left, 6px under it").toMatchObject({ left: 900, top: 46 });
    expect(openBtn()!.props["aria-expanded"]).toBe(true);
    expect(items().map((i) => textOf(kids(i)))).toEqual(["New terminalCtrl+Shift+`", "Open a file…Alt+2", "BoardAlt+3"]);
    expect([items()[0]!.props.title, items()[0]!.props.disabled]).toEqual(["New terminal in a folder you pick", false]);
  });

  it("New terminal opens the picker titled 'Open terminal in', with you, and no note under 20 open terminals", async () => {
    chatT.terms = Array.from({ length: 19 }, (_, i) => ({ id: `t${i}` }));
    await openTerminal();
    expect(menu(), "the menu closed").toBeUndefined();
    const p = picker()!;
    expect([p.props["aria-label"], classText("ed-picker-title"), classText("ed-picker-note")]).toEqual(["Open terminal in", "Open terminal in", null]);
    expect(groups().map(([g]) => g)).toEqual(["Projects", "In ~/Herd", "Cards"]);
  });

  it("has the note from 20 open terminals, as New terminal's picker", async () => {
    chatT.terms = Array.from({ length: 20 }, (_, i) => ({ id: `t${i}` }));
    await openTerminal();
    expect(classText("ed-picker-note")).toBe("20 terminals are open: each keeps its shell running until you close it.");
  });

  it("a folder where a terminal can't start says why under its row, as a sentence; the picker stays open", async () => {
    chatT.answer = () => Promise.reject(new Error("QA Agent is working on GA-118: a terminal there waits until its run ends"));
    await openTerminal();
    fire(option("invoice-api"), "onClick");
    await settle();
    expect(chatT.opens, "a new terminal in the folder picked (openChatTerminal(folder.id, false))").toEqual([`${HERD}/invoice-api`]);
    expect(picker()).toBeDefined();
    expect(textOf(kids(failedNote()!))).toBe("QA Agent is working on GA-118: a terminal there waits until its run ends.");
    expect(Side.sideNow().tabs, "no tab").toEqual([]);
  });

  it("a card that isn't pinned to you asks first, as New terminal's; one pinned to you starts at once", async () => {
    await openTerminal();
    fire(option("GA-112"), "onClick");
    await settle();
    expect(textOf(kids(findAll(kids(confirmBox()!), (f) => f.props.className === "grow")[0]!))).toMatch(/^GA-112 is Frontend Agent's card: /);
    expect(chatT.opens).toEqual([]);
    fire(option("KADE-41"), "onClick");
    await settle();
    expect(chatT.opens).toEqual([`${WT}/KADE/KADE-41`]);
  });

  it("a pick that starts closes the picker, and the terminal is a tab in the chat's side island, shown", async () => {
    chatT.answer = (f) => Promise.resolve(kade(f));
    await openTerminal();
    fire(option("kade-portal"), "onClick");
    await settle();
    expect(chatT.opens).toEqual([`${HERD}/kade-portal`]);
    expect(picker()).toBeUndefined();
    expect(Side.sideNow()).toMatchObject({ tabs: [{ kind: "terminal", id: "c1" }], current: "terminal:c1", open: true });
  });

  it("with a terminal in the chat: New terminal starts in its folder without the picker, and New terminal in… asks for the folder", async () => {
    chatT.answer = (f) => Promise.resolve(kade(f));
    chatT.terms = [kade(`${HERD}/kade-portal`)];
    Side.openSideTab({ kind: "terminal", id: "c1" });
    await openMenu();
    expect(items().map((i) => textOf(kids(i)))).toEqual(["New terminalCtrl+Shift+`", "New terminal in…", "Open a file…Alt+2", "BoardAlt+3"]);
    expect(item("New terminal").props.title).toBe("New terminal in kade-portal");
    expect(item("New terminal in…").props.title).toBe("New terminal in a folder you pick");
    fire(item("New terminal"), "onClick");
    await settle();
    expect(picker(), "no picker").toBeUndefined();
    expect(chatT.opens, "in the shown terminal's folder").toEqual([`${HERD}/kade-portal`]);
    fire(openBtn()!, "onClick", { currentTarget: { getBoundingClientRect: () => ({ left: 900, bottom: 40 }) } });
    await settle();
    fire(item("New terminal in…"), "onClick");
    await settle();
    expect(picker()?.props["aria-label"]).toBe("Open terminal in");
  });
});

// GA-298 QA: Open's menu (the same as the island's +) adds Open a file… and Board; Alt+2 and Alt+3 open them while the chat has
// none of that kind open (design/islands-chat → The keys). Open a file…: Go to file over the chat project's folder, else the
// folder picker titled "Open a file in" first. The Chat page here is Open and the side island (ChatSide).
describe("GA-298: Open a file… and Board, from Open's menu and Alt+2 and Alt+3", () => {
  const menu = () => one((f) => f.props.role === "menu");
  const item = (label: string) => findAll(kids(menu()!), (f) => f.props.role === "menuitem").find((i) => textOf(kids(i)).startsWith(label))!;
  const page = async () => { root = () => <><OpenButton youId="u1" /><ChatSide youId="u1" /></>; render(); await settle(); };
  const openMenu = async () => {
    const btn = one((f) => f.type === "button" && typeof f.props.className === "string" && f.props.className.includes("chat-open"))!;
    fire(btn, "onClick", { currentTarget: { getBoundingClientRect: () => ({ left: 900, bottom: 40 }) } });
    await settle();
  };
  const pick = async (label: string) => { fire(item(label), "onClick"); await settle(); };
  const alt = async (n: number) => {
    window.dispatchEvent(Object.assign(new Event("keydown", { cancelable: true }), { code: `Digit${n}`, key: String(n), altKey: true, ctrlKey: false, shiftKey: false, metaKey: false }));
    render();
    await settle();
  };
  const filePicker = () => one((f) => f.props.role === "dialog" && f.props["aria-label"] === "Open a file in");
  const goTo = () => one((f) => f.props.role === "dialog" && f.props["aria-label"] === "Go to file");
  const goToIn = () => { const g = goTo(); return g ? findAll(kids(g), (f) => f.type === "input")[0]?.props.placeholder : undefined; };
  const shownSide = () => { const x = Side.sideNow(); return { tabs: x.tabs.map(Side.tabKey), current: x.current, open: x.open }; };
  beforeEach(() => {
    Side.showChat("c1");
    Side.closeSideTabs(Side.sideNow().tabs.map(Side.tabKey));
    Side.setChatCards("c1", "about", []);
    Side.setChatCards("c1", "named", []);
  });
  afterEach(() => { root = () => null; render(); });

  it("Board opens the chat's Board tab beside the chat", async () => {
    await page();
    await openMenu();
    expect(item("Board").props.title).toBe("The board of the chat's project beside the chat");
    await pick("Board");
    expect(shownSide()).toEqual({ tabs: ["board:board"], current: "board:board", open: true });
  });

  it("Open a file… in a chat about a card goes to Go to file over its project's folder", async () => {
    Side.setChatCards("c1", "about", ["GA-5"]);
    await page();
    await openMenu();
    await pick("Open a file…");
    expect(filePicker()).toBeUndefined();
    expect(goToIn()).toBe("Go to file in gizai");
  });

  it("Open a file… in a chat without a project or an Editor tab asks for the folder first, then Go to file over the one picked", async () => {
    await page();
    await openMenu();
    await pick("Open a file…");
    expect(goTo()).toBeUndefined();
    expect(filePicker()).toBeDefined();
    fire(option("invoice-api"), "onClick");
    await settle();
    expect(filePicker()).toBeUndefined();
    expect(goToIn()).toBe("Go to file in invoice-api");
  });

  it("Alt+3 with no Board tab opens it, Alt+3 on the Board hides the island; Alt+4 with no card does nothing; Alt+2 with no Editor tab opens Open a file…", async () => {
    await page();
    await alt(3);
    expect(shownSide()).toEqual({ tabs: ["board:board"], current: "board:board", open: true });
    await alt(3);
    expect(shownSide()).toMatchObject({ tabs: ["board:board"], open: false });
    await alt(4);
    expect(shownSide()).toMatchObject({ tabs: ["board:board"], open: false });
    await alt(3);
    expect(shownSide()).toMatchObject({ current: "board:board", open: true });
    await alt(2);
    expect(filePicker(), "no chat project: the picker first").toBeDefined();
  });

  it("Open a file… asked once doesn't open again by itself when the Chat page draws again (you went to another page and back)", async () => {
    await page();
    await openMenu();
    await pick("Open a file…");
    expect(filePicker()).toBeDefined();
    fire(option("invoice-api"), "onClick");
    await settle();
    expect(goTo()).toBeDefined();
    // another page, then the Chat page again: a new ChatSide
    root = () => null;
    render();
    await settle();
    await page();
    expect([filePicker(), goTo()], "nothing opens by itself").toEqual([undefined, undefined]);
  });
});

describe("the Editor's own picker (no onPick) is as it was", () => {
  it("opens any card's worktree at once, an agent's too (read-only there), without asking", async () => {
    await show();
    expect(one((f) => f.props.className === "ed-picker-title")).toBeUndefined();
    expect(option("GA-118").props["aria-disabled"]).toBeUndefined();
    fire(option("GA-112"), "onClick");
    expect(confirmBox()).toBeUndefined();
    expect(s.went).toEqual([{ page: "editor", id: `${WT}/GA/GA-112` }]);
    expect(closed).toBe(1);
  });
});

// GA-146 QA: card rows with long titles. How tall a row is and where its title wraps is CSS, checked in the real window by
// the cage run (scripts/ui-test.sh, terminal:picker); here: what the rows carry for it, the tooltip and where the picker sits.
describe("card rows with long titles (GA-146)", () => {
  const list = () => one((f) => f.props.role === "listbox")!;
  const idChars = () => (list().props.style as Record<string, unknown>)["--id-chars"];
  const typeQuery = (value: string) => fire(one((f) => f.type === "input")!, "onChange", { target: { value } });

  it("gives the cards' identifiers one column, as wide as the longest card's (KADE-41: 7 characters), kept while you type", async () => {
    await terminal();
    expect(idChars()).toBe(7);
    typeQuery("GA-112");
    expect(groups()).toEqual([["Cards", ["GA-112"]]]);
    expect(idChars()).toBe(7);
    typeQuery("invoice-api");
    expect(idChars()).toBe(7);
  });
  it("marks card rows as cards, so only they grow; project and folder rows keep their path on the right", async () => {
    await terminal();
    const rows = findAll(tree, (f) => f.props.role === "option");
    expect(rows.map((o) => o.props["data-kind"])).toEqual(["project", "root", "root", "card", "card", "card"]);
    const where = (o: Found) => findAll(kids(o), (x) => x.props.className === "where").map((x) => textOf(kids(x)));
    expect(rows.map(where)).toEqual([["~/Herd/gizai"], ["~/Herd/invoice-api"], ["~/Herd/kade-portal"], [], [], []]);
    expect(rows.slice(0, 3).map((o) => o.props.title)).toEqual(["~/Herd/gizai", "~/Herd/invoice-api", "~/Herd/kade-portal"]);
  });
  it("starts a card row's tooltip with its full title, then the path, in New terminal's picker and in the Editor's own", async () => {
    await terminal();
    expect(option("KADE-41").props.title).toBe("Fix the invoice total\n~/.local/share/gizai/worktrees/KADE/KADE-41");
    root = () => null; render();
    await show();
    expect(["KADE-41", "GA-118", "GA-112"].map((n) => option(n).props.title)).toEqual([
      "Fix the invoice total\n~/.local/share/gizai/worktrees/KADE/KADE-41",
      "Show a run's changes\n~/.local/share/gizai/worktrees/GA/GA-118",
      "Remember the open tabs per folder\n~/.local/share/gizai/worktrees/GA/GA-112",
    ]);
  });
  it("stays in the window, 8px from its edges: moved left when it would run off the right, up when it would run off the bottom", async () => {
    const at = { x: 240, y: 100 };
    root = () => <FolderPicker at={{ ...at }} onClose={() => { closed += 1; }} />;
    render();
    await settle();
    // the fake runtime doesn't attach refs: the dialog's ref gets a 480 × 300 box by hand
    const dialog = one((f) => f.props.role === "dialog")!;
    (dialog.props.ref as { current: unknown }).current = { getBoundingClientRect: () => ({ width: 480, height: 300 }) };
    const placed = (x: number, y: number) => {
      root = () => <FolderPicker at={{ x, y }} onClose={() => { closed += 1; }} />;
      render();
      const st = one((f) => f.props.role === "dialog")!.props.style as { left: number; top: number };
      return [st.left, st.top];
    };
    expect(placed(240, 120)).toEqual([240, 120]);
    expect(placed(1000, 120)).toEqual([1280 - 480 - 8, 120]);
    expect(placed(240, 600)).toEqual([240, 720 - 300 - 8]);
    expect(placed(1270, 715)).toEqual([792, 412]);
    expect(placed(2, 3)).toEqual([8, 8]);
  });
});

// Vitest hands a .css file back empty, so the stylesheets are read from disk (as in ChatComposerLayout.test.tsx); node:fs
// comes in through a computed specifier: the build type-checks tests without Node's types.
const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const css = (name: string): string => fs.readFileSync(new URL(`../../styles/${name}`, import.meta.url), "utf8");
/** The declarations of the first block whose selector is exactly `selector`. */
function rule(sheet: string, selector: string): Map<string, string> {
  const start = sheet.indexOf(`\n${selector} {`);
  if (start < 0) throw new Error(`no ${selector} block`);
  const body = sheet.slice(sheet.indexOf("{", start) + 1, sheet.indexOf("}", start)).replace(/\/\*[\s\S]*?\*\//g, "");
  const out = new Map<string, string>();
  for (const m of body.matchAll(/([\w-]+)\s*:\s*([^;]+);/g)) out.set(m[1] ?? "", (m[2] ?? "").replace(/\s+/g, " ").trim());
  return out;
}

describe("the folder picker's styles (GA-146)", () => {
  const components = css("components.css");
  it("make the picker 480px wide, and never wider than the window minus 16px", () => {
    const picker = rule(components, ".pop.ed-picker");
    expect([picker.get("width"), picker.get("max-width")]).toEqual(["480px", "calc(100vw - 16px)"]);
  });
  it("let a card row grow with its title, never shorter than a row; project and folder rows keep the one-line row", () => {
    const row = rule(css("app.css"), ".pop .opt");
    expect([row.get("height"), row.get("align-items")]).toEqual(["calc(30px + var(--ui-box, 0px))", "center"]);
    const card = rule(components, '.ed-picker .opt[data-kind="card"]');
    expect([card.get("height"), card.get("min-height"), card.get("align-items")]).toEqual(["auto", row.get("height"), "flex-start"]);
    const where = rule(components, ".ed-picker .opt .where");
    expect([where.get("white-space"), where.get("text-overflow"), where.get("overflow")]).toEqual(["nowrap", "ellipsis", "hidden"]);
  });
  it("clamp a card's title at two lines, and give the identifiers one column of --id-chars characters", () => {
    const label = rule(components, '.ed-picker .opt[data-kind="card"] > .label');
    expect([label.get("display"), label.get("-webkit-line-clamp"), label.get("line-clamp"), label.get("overflow"), label.get("min-width")]).toEqual(["-webkit-box", "2", "2", "hidden", "0"]);
    const id = rule(components, '.ed-picker .opt[data-kind="card"] > .card-id');
    expect([id.get("width"), id.get("white-space")]).toEqual(["calc(var(--id-chars, 0) * 1ch)", "nowrap"]);
    expect(rule(components, ".ed-picker .opt .card-id").get("font-family")).toBe("var(--font-mono)");
  });
});
