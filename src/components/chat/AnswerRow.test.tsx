// GA-298 QA: the row of buttons under a Chief answer (components/chat/AnswerRow.tsx; design/islands-chat → What opens, and
// when; Figma frame 2). One row under the whole answer (its text and tool messages), quiet buttons in this order: Open GA-12
// (square-kanban) for each card its chips link, at most three, then "and 2 more" with the rest in a menu; Open board
// (columns-3) after a create_task or move_task that ended without an error; Open in Editor (file-code) with the file and line
// in mono for each path:line that is a file in the chat project's folder or one of the Editor's folders (asked with
// editorVersions), at most three; then GA-279's Open in terminal, last. No row for an answer with none of these. Each button
// opens its tab in the chat's side island (the real lib/chatSide.ts; the Editor tab's openEditorTab is recorded, since
// CodeMirror needs a DOM), Ctrl+click on a card its page; nothing opens by itself. There is no DOM here, so the small hook
// runtime of OpenInTerminal.test.tsx stands in for React. Each test imports the modules afresh.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import type { ChatMessage, EditorFolder } from "../../types";

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
  // subscribes on the first draw (again when `subscribe` changes) and draws again when the store says it changed
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

const HOME = "/home/you";
const folder = (id: string, more: Partial<EditorFolder> = {}): EditorFolder =>
  ({ id, path: id.replace(HOME, "~"), name: id.split("/").pop()!, kind: "root", projectId: null, taskId: null, readOnly: false, readOnlyWhy: null, ...more });
const DOQ = folder(`${HOME}/Herd/doq-studio`, { kind: "project", projectId: "p-doq" });
const KADE = folder(`${HOME}/Herd/kade-portal`, { kind: "project", projectId: "p-kade" });
const SHOP = folder(`${HOME}/Herd/shop`);
const CARD = folder(`${HOME}/.local/share/gizai/worktrees/GA/GA-279`, { kind: "card", taskId: "t279", name: "GA-279" });
const PROJECTS = [{ id: "p-doq", key: "DOQ", name: "Doq studio" }, { id: "p-kade", key: "KADE", name: "Kade portal" }];

// The api, faked: the Editor's folders, the projects, and which files are there (editorVersions: "" for none)
const a = vi.hoisted(() => ({ files: new Set<string>(), versionAsks: [] as [string, string[]][], opened: [] as unknown[][], items: [] as unknown[][] }));
vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(real)) if (typeof real[k] === "function") out[k] = () => new Promise(() => {});
  return {
    ...out,
    editorFolders: () => Promise.resolve([DOQ, KADE, SHOP, CARD]),
    getSettings: () => Promise.resolve({ editorRoots: ["~/Herd"] }),
    listProjects: () => Promise.resolve(PROJECTS),
    editorVersions: (dir: string, paths: string[]) => {
      a.versionAsks.push([dir, paths]);
      return Promise.resolve(paths.map((p) => (a.files.has(`${dir}/${p}`) ? "v1" : "")));
    },
  };
});
vi.mock("../../lib/useData", () => ({ onWrites: () => () => {}, useData: () => ({ data: null, error: null, reload: () => {}, setData: () => {} }) }));
vi.mock("../../lib/terminals", async (orig) => ({ ...(await orig<object>()), terminalsNow: () => [], watchTerminals: () => () => {}, noteOpened: () => {}, useTerminals: () => [] }));
vi.mock("../../router", async (orig) => ({ ...(await orig<object>()), go: () => {} }));
// the Editor tab (CodeMirror needs a DOM) and a card's page: what they were asked to open
vi.mock("./ChatEditor", () => ({ openEditorTab: (...x: unknown[]) => { a.opened.push(x); } }));
vi.mock("../../lib/openItem", () => ({ openItem: (...x: unknown[]) => { a.items.push(x); return Promise.resolve(); } }));

const store = new Map<string, string>();
vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => { store.set(k, String(v)); }, removeItem: (k: string) => { store.delete(k); } });
vi.stubGlobal("window", Object.assign(new EventTarget(), { innerWidth: 1280, innerHeight: 720 }));
vi.stubGlobal("document", { body: {}, addEventListener: () => {}, removeEventListener: () => {} });

let M: typeof import("./AnswerRow");
let S: typeof import("../../lib/chatSide");

// ---- the hook runtime (FolderPickerTerminal.test.tsx's, with memo) ----
let root: () => ReactNode = () => null;
const insts = new Map<string, Inst>();
const unmount = (inst: Inst) => { for (const x of inst.slots) if ((x as EffectSlot)?.effect) (x as EffectSlot).cleanup?.(); };
const MEMO = Symbol.for("react.memo");
function expand(node: ReactNode, path: string, seen: Set<string>): ReactNode {
  if (Array.isArray(node)) return node.map((n, i) => expand(n, `${path}.${(n as { key?: string | null })?.key ?? i}`, seen));
  if (!node || typeof node !== "object" || !("props" in node)) return node;
  const el = node as ReactElement<Record<string, unknown>>;
  const memo = el.type as unknown as { $$typeof?: symbol; type?: unknown };
  const type = typeof el.type === "object" && memo?.$$typeof === MEMO ? memo.type : el.type;
  if (typeof type === "function") {
    const key = `${path}>${type.name}${el.key !== null ? `#${el.key}` : ""}`;
    seen.add(key);
    let inst = insts.get(key);
    if (!inst) insts.set(key, (inst = { slots: [], i: 0 }));
    inst.i = 0;
    h.cur = inst;
    let out: ReactNode;
    try { out = (type as (p: unknown) => ReactNode)(el.props); } finally { h.cur = null; }
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
const click = async (f: Found, e: Record<string, unknown> = {}) => {
  (f.props.onClick as (e: unknown) => void)({ preventDefault: () => {}, stopPropagation: () => {},
    currentTarget: { getBoundingClientRect: () => ({ left: 300, bottom: 400, top: 370, right: 520, width: 220, height: 30 }) }, ...e });
  render();
  await settle();
};
const msg = (n: number, role: string, bodyMd: string | null, more: Partial<ChatMessage> = {}): ChatMessage =>
  ({ id: `m${n}`, threadId: "c1", role, bodyMd, createdAt: n, authorName: "Chief", ...more });
const said = (n: number, md: string) => msg(n, "agent", md);
const called = (n: number, name: string, tool: Record<string, unknown>, body = `Called ${name}`) => msg(n, "tool", body, { toolName: `mcp__gizai__${name}`, tool });

async function show(items: ChatMessage[]) {
  root = () => <M.AnswerRow items={items} you="u1" />;
  render();
  await settle();
  await settle();
}
const rowOf = () => one((f) => f.props.className === "msg-term");
const rowButtons = () => {
  const row = one((f) => f.props.className === "msg-term-row");
  return row ? findAll(kids(row), (f) => f.type === "button") : [];
};
const labels = () => rowButtons().map((b) => textOf(kids(b)));
const button = (label: string) => rowButtons().find((b) => textOf(kids(b)) === label)!;
const icons = () => rowButtons().map((b) => findAll(kids(b), (f) => typeof f.type === "object" && f.props.className === "icon")
  .map((f) => (f.type as { displayName?: string }).displayName ?? "?").join("+"));
const side = () => { const s = S.sideNow(); return { tabs: s.tabs.map(S.tabKey), current: s.current, open: s.open }; };

// An answer with all of it: four cards, a card made, a file and line, a command
const ALL = [
  said(1, "I made [GA-1](gizai:task/GA-1) and moved [GA-2](gizai:task/GA-2); [GA-3](gizai:task/GA-3) and [GA-4](gizai:task/GA-4) wait."),
  called(2, "create_task", { input: { title: "x" }, result: "{\"identifier\":\"GA-1\"}", isError: false }),
  said(3, "The change is in `src/api.ts:12`. In `~/Herd/doq-studio`: run `npm test`."),
];

beforeEach(async () => {
  vi.useFakeTimers();
  for (const [k, inst] of insts) { unmount(inst); insts.delete(k); }
  h.effects.length = 0;
  a.files = new Set([`${DOQ.id}/src/api.ts`]);
  a.versionAsks = []; a.opened = []; a.items = [];
  store.clear();
  vi.resetModules();
  M = await import("./AnswerRow");
  S = await import("../../lib/chatSide");
  S.showChat("c1");
});
afterEach(() => { root = () => null; render(); vi.useRealTimers(); });

describe("which answers get a row", () => {
  it("none for an answer with no linked card, no card made or moved, no file in a known folder and no command", async () => {
    await show([
      said(1, "[Kade](gizai:project/KADE) is on the board; GA-12 is only written here, and `nowhere/missing.ts:4` isn't a file."),
      called(2, "list_tasks", { result: "[]" }),
      called(3, "move_task", { result: "no such column", isError: true }),
      called(4, "create_task", { input: { title: "still running" } }),
    ]);
    expect(rowOf()).toBeUndefined();
  });
  it("reads the answer's text messages only: a chip or a file and line in a tool call's text makes no button", async () => {
    await show([called(1, "get_task", { result: "{}" }, "[GA-7](gizai:task/GA-7) in src/api.ts:12")]);
    expect(rowOf()).toBeUndefined();
    expect(a.versionAsks).toEqual([]);
  });
  it("keeps a hidden button's room while it asks whether a named file is there, then has none when it isn't", async () => {
    root = () => <M.AnswerRow items={[said(1, "Look at `nowhere/missing.ts:4`.")]} you="u1" />;
    render();
    const wait = one((f) => typeof f.props.className === "string" && f.props.className.includes("msg-term-wait"));
    expect(wait?.props["aria-hidden"]).toBe(true);
    expect(textOf(kids(wait!))).toBe("Open in Editor");
    await settle();
    await settle();
    expect(rowOf()).toBeUndefined();
  });
  it("is one row under the whole answer, in the notes' order: cards, and N more, Open board, Open in Editor, Open in terminal", async () => {
    await show(ALL);
    expect(findAll(tree, (f) => f.props.className === "msg-term")).toHaveLength(1);
    expect(labels()).toEqual(["Open GA-1", "Open GA-2", "Open GA-3", "and 1 more", "Open board", "Open in Editorapi.ts:12", "Open in terminal~/Herd/doq-studio"]);
    expect(icons()).toEqual(["SquareKanban", "SquareKanban", "SquareKanban", "ChevronDown", "Columns3", "FileCode", "Terminal"]);
    expect(rowButtons().every((b) => b.props.className === "btn ghost sm"), "quiet and small, as Open in terminal").toBe(true);
  });
  it("opens nothing by itself", async () => {
    await show(ALL);
    expect(side()).toMatchObject({ tabs: [], current: null });
    expect([a.opened, a.items]).toEqual([[], []]);
  });
});

describe("Open GA-12", () => {
  const FIVE = said(1, "[GA-1](gizai:task/GA-1), [GA-2](gizai:task/GA-2), [ga-3](gizai:task/ga-3), [GA-1](gizai:task/GA-1) again, [GA-4](gizai:task/GA-4) and [GA-5](gizai:task/GA-5).");
  it("is a button for each card the answer links, in order, each once, three at most, then 'and 2 more'", async () => {
    await show([FIVE]);
    expect(labels()).toEqual(["Open GA-1", "Open GA-2", "Open GA-3", "and 2 more"]);
    expect(button("Open GA-1").props.title).toBe("Open GA-1 beside the chat (Ctrl+click: its page)");
    expect([button("and 2 more").props.title, button("and 2 more").props["aria-haspopup"], button("and 2 more").props["aria-expanded"]]).toEqual(["GA-4, GA-5", "menu", false]);
  });
  it("opens the card in a Card tab beside the chat; Ctrl+click opens its page instead", async () => {
    await show([FIVE]);
    await click(button("Open GA-2"));
    expect(side()).toEqual({ tabs: ["card:GA-2"], current: "card:GA-2", open: true });
    await click(button("Open GA-1"), { ctrlKey: true, metaKey: false });
    expect(a.items).toEqual([["task", "GA-1"]]);
    expect(side().tabs, "no tab for a Ctrl+click").toEqual(["card:GA-2"]);
    await click(button("Open GA-2"));
    expect(side().tabs, "a second click shows its tab, no second one").toEqual(["card:GA-2"]);
  });
  it("'and 2 more' opens a menu of the rest; picking one opens it beside the chat and closes the menu", async () => {
    await show([FIVE]);
    await click(button("and 2 more"));
    const menu = one((f) => f.props.role === "menu");
    expect(menu?.props["aria-label"]).toBe("More cards");
    const items = findAll(kids(menu!), (f) => f.props.role === "menuitem");
    expect(items.map((i) => textOf(kids(i)))).toEqual(["Open GA-4", "Open GA-5"]);
    expect(button("and 2 more").props["aria-expanded"]).toBe(true);
    (items[1]!.props.onClick as () => void)();
    render();
    await settle();
    expect(side()).toEqual({ tabs: ["card:GA-5"], current: "card:GA-5", open: true });
    expect(one((f) => f.props.role === "menu")).toBeUndefined();
  });
});

describe("Open board", () => {
  it("shows after a create_task or move_task that ended without an error, and opens the chat's Board tab", async () => {
    await show([said(1, "Moved it."), called(2, "move_task", { input: {}, result: "{}" })]);
    expect(labels()).toEqual(["Open board"]);
    expect(button("Open board").props.title).toBe("Open the board beside the chat");
    await click(button("Open board"));
    expect(side()).toEqual({ tabs: ["board:board"], current: "board:board", open: true });
    await click(button("Open board"));
    expect(side().tabs, "one Board tab per chat").toEqual(["board:board"]);
  });
  it("isn't there after a call that failed or hasn't ended, or after another tool", async () => {
    await show([said(1, "Tried."), called(2, "move_task", { result: "no such column", isError: true }), called(3, "update_task", { result: "{}" })]);
    expect(rowOf()).toBeUndefined();
  });
});

describe("Open in Editor", () => {
  const MD = "See `src/api.ts:12`, ~/Herd/shop/app/Cart.php:7, `missing.ts:3`, ~/.local/share/gizai/worktrees/GA/GA-279/src/api.ts:2, app/a.ts:9 and src/api.ts:40.";
  beforeEach(() => {
    a.files = new Set([`${DOQ.id}/src/api.ts`, `${KADE.id}/src/api.ts`, `${SHOP.id}/app/Cart.php`, `${SHOP.id}/app/a.ts`, `${CARD.id}/src/api.ts`]);
  });
  it("is a button per named file that is there, at most three: a path from a folder in the chat project's folder first", async () => {
    S.setChatCards("c1", "about", ["KADE-1"]);
    await show([said(1, MD)]);
    expect(labels()).toEqual(["Open in Editorapi.ts:12", "Open in EditorCart.php:7", "Open in Editora.ts:9"]);
    expect(rowButtons().map((b) => b.props.title)).toEqual([
      "~/Herd/kade-portal/src/api.ts, line 12", "~/Herd/shop/app/Cart.php, line 7", "~/Herd/shop/app/a.ts, line 9",
    ]);
    const mono = findAll(kids(rowButtons()[0]!), (f) => f.props.className === "mono");
    expect(mono.map((m) => textOf(kids(m)))).toEqual(["api.ts:12"]);
    expect(a.versionAsks.some(([d]) => d === CARD.id), "a card's worktree is never looked in").toBe(false);
  });
  it("without a chat project, looks in the projects' folders first, then the Editor's other folders", async () => {
    await show([said(1, "In `src/api.ts:12` and app/a.ts:9.")]);
    expect(rowButtons().map((b) => b.props.title)).toEqual(["~/Herd/doq-studio/src/api.ts, line 12", "~/Herd/shop/app/a.ts, line 9"]);
  });
  it("opens the file at its line in an Editor tab beside the chat", async () => {
    S.setChatCards("c1", "named", ["KADE-9"]);
    await show([said(1, MD)]);
    await click(button("Open in EditorCart.php:7"));
    await click(button("Open in Editorapi.ts:12"));
    expect(a.opened).toEqual([[SHOP.id, "app/Cart.php", 7], [KADE.id, "src/api.ts", 12]]);
  });
});

describe("Open in terminal (GA-279's) in the row", () => {
  it("is the row's last, after the answer's other buttons, from any of the answer's texts", async () => {
    await show([said(1, "Moved [GA-9](gizai:task/GA-9)."), called(2, "list_tasks", { result: "[]" }), said(3, "Before you test, in `~/Herd/doq-studio`: run `npm test`.")]);
    expect(labels()).toEqual(["Open GA-9", "Open in terminal~/Herd/doq-studio"]);
  });
  it("is alone in the row when the answer has nothing else", async () => {
    await show([said(1, "Run `composer install` in your checkout.")]);
    expect(labels()).toEqual(["Open in terminal…"]);
  });
});
