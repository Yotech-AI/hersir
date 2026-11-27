// GA-298 QA: the Board tab's body (components/chat/ChatBoard.tsx; design/islands-chat → Board, Figma frame 3): the board
// of the chat's project (the project of the cards the chat is about, else of the cards its answers link), compact, with
// the chat's cards marked, under a row with the project's dot and name, "the chat's project" and the legend "In this chat";
// without cards in the chat the team's board (the team's name, no legend, every card of the team). A click on a card opens
// it in a Card tab beside the chat (the real lib/chatSide.ts), Ctrl+click its page; a move is saved with moveTask as on the
// Tasks page, and one that fails goes back and says why. The board itself (components/Board.tsx, tested in
// BoardCompact.test.tsx and on the Tasks page) is a stub here that keeps the props it got; the small hook runtime of
// OpenInTerminal.test.tsx stands in for React.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import type { Task } from "../../types";

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

const STATES = [
  { id: "s-todo", name: "To do", category: "ready", sortKey: "a1" },
  { id: "s-doing", name: "In progress", category: "in_progress", sortKey: "a2" },
  { id: "s-test", name: "Testing", category: "testing", sortKey: "a3" },
];
const TEAM = { id: "team1", name: "Software", members: [], states: STATES, labels: [] };
const PROJECTS = [{ id: "p-kade", key: "KADE", name: "Kade portal", color: "var(--c-blue)", teamId: "team1" }, { id: "p-gfw", key: "GFW", name: "Groene Fiets", color: "var(--c-green)", teamId: "team1" }];
const task = (id: string, identifier: string, projectId: string, stateId: string): Task =>
  ({ id, identifier, projectId, title: identifier, stateId, stateName: "", stateCategory: "", sortKey: "a0", labels: [] } as unknown as Task);
const TASKS = [task("k1", "KADE-1", "p-kade", "s-doing"), task("k2", "KADE-2", "p-kade", "s-todo"), task("g1", "GFW-1", "p-gfw", "s-todo")];

const a = vi.hoisted(() => ({ filters: [] as unknown[], moves: [] as unknown[][], moveAnswer: null as null | (() => Promise<void>), went: [] as unknown[], board: null as null | Record<string, unknown> }));
vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(real)) if (typeof real[k] === "function") out[k] = () => new Promise(() => {});
  return {
    ...out,
    listTeams: () => Promise.resolve([TEAM]),
    getTeam: () => Promise.resolve(TEAM),
    listProjects: () => Promise.resolve(PROJECTS),
    listTasks: (filter: { projectId?: string }) => { a.filters.push(filter); return Promise.resolve(TASKS.filter((t) => !filter.projectId || t.projectId === filter.projectId)); },
    moveTask: (...x: unknown[]) => { a.moves.push(x); return a.moveAnswer ? a.moveAnswer() : Promise.resolve(); },
  };
});
// useData as the page uses it: asks again when its keys change
vi.mock("../../lib/useData", async () => {
  const R = await import("react");
  return {
    onWrites: () => () => {},
    useData: <T,>(fetch: () => Promise<T>, deps: unknown[] = []) => {
      const [data, setData] = R.useState<T | null>(null);
      R.useEffect(() => { let on = true; fetch().then((d) => { if (on) setData(d); }); return () => { on = false; }; }, deps); // eslint-disable-line react-hooks/exhaustive-deps
      return { data, error: null, reload: () => {}, setData };
    },
  };
});
vi.mock("../../lib/useLiveRuns", () => ({ useLiveRuns: () => [] }));
vi.mock("../../lib/terminals", async (orig) => ({ ...(await orig<object>()), terminalsNow: () => [], watchTerminals: () => () => {} }));
vi.mock("../../router", async (orig) => ({ ...(await orig<object>()), go: (r: unknown) => { a.went.push(r); } }));
vi.mock("../Board", () => ({ Board: (p: Record<string, unknown>) => { a.board = p; return null; } }));

const store = new Map<string, string>();
vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => { store.set(k, String(v)); }, removeItem: (k: string) => { store.delete(k); } });
vi.stubGlobal("window", Object.assign(new EventTarget(), { innerWidth: 1280, innerHeight: 720 }));
vi.stubGlobal("document", { body: {}, addEventListener: () => {}, removeEventListener: () => {} });
vi.stubGlobal("CSS", { escape: (s: string) => s });

let M: typeof import("./ChatBoard");
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
async function show() {
  root = () => <M.ChatBoardBody />;
  render();
  await settle();
  await settle();
}
const head = () => { const f = one((x) => x.props.className === "chat-board-head"); return f ? textOf(kids(f)) : null; };
const legend = () => one((x) => typeof x.props.className === "string" && x.props.className.startsWith("legend"));
const props = () => a.board as { tasks: Task[]; compact: boolean; marked: Set<string>; states: { id: string }[];
  onOpen: (id: string, e?: { ctrlKey: boolean; metaKey: boolean }) => void; onMove: (id: string, stateId: string, sortKey: string) => Promise<void> };
const side = () => { const x = S.sideNow(); return { tabs: x.tabs.map(S.tabKey), current: x.current }; };

beforeEach(async () => {
  vi.useFakeTimers();
  for (const [k, inst] of insts) { unmount(inst); insts.delete(k); }
  h.effects.length = 0;
  Object.assign(a, { filters: [], moves: [], moveAnswer: null, went: [], board: null });
  store.clear();
  vi.resetModules();
  M = await import("./ChatBoard");
  S = await import("../../lib/chatSide");
  S.showChat("c1");
});
afterEach(() => { root = () => null; render(); vi.useRealTimers(); });

describe("the chat's project's board", () => {
  it("shows the board of the project of the cards the chat is about, compact, with the chat's cards marked", async () => {
    S.setChatCards("c1", "about", ["KADE-1"]);
    S.setChatCards("c1", "named", ["GFW-1", "KADE-2"]);
    await show();
    expect(a.filters.at(-1)).toEqual({ projectId: "p-kade" });
    expect(head()).toBe("Kade portalthe chat's projectIn this chat");
    expect(legend()).toBeDefined();
    expect(props().compact).toBe(true);
    expect(props().tasks.map((t) => t.identifier)).toEqual(["KADE-1", "KADE-2"]);
    expect([...props().marked]).toEqual(["KADE-1", "GFW-1", "KADE-2"]);
    expect(props().states.map((s) => s.id)).toEqual(["s-todo", "s-doing", "s-test"]);
  });
  it("takes the project of the cards its answers link when the chat isn't about cards", async () => {
    S.setChatCards("c1", "named", ["GFW-1"]);
    await show();
    expect(a.filters.at(-1)).toEqual({ projectId: "p-gfw" });
    expect(head()).toBe("Groene Fietsthe chat's projectIn this chat");
    expect(props().tasks.map((t) => t.identifier)).toEqual(["GFW-1"]);
  });
  it("without cards in the chat, shows the team's board: the team's name, every card, no legend", async () => {
    await show();
    expect(a.filters.at(-1)).toEqual({});
    expect(head()).toBe("Software");
    expect(legend()).toBeUndefined();
    expect(props().tasks.map((t) => t.identifier)).toEqual(["KADE-1", "KADE-2", "GFW-1"]);
    expect(props().marked.size).toBe(0);
  });
  it("its tab is named after the project, or Board for the team's", () => {
    expect([M.boardName(PROJECTS[0] as never), M.boardName(null), M.boardName(undefined)]).toEqual(["Kade portal", "Board", "Board"]);
  });
});

describe("on the board", () => {
  it("a click on a card opens it in a Card tab beside the chat; Ctrl+click opens its page", async () => {
    S.setChatCards("c1", "about", ["KADE-1"]);
    await show();
    props().onOpen("k2", { ctrlKey: false, metaKey: false });
    expect(side()).toEqual({ tabs: ["card:KADE-2"], current: "card:KADE-2" });
    props().onOpen("k1", { ctrlKey: true, metaKey: false });
    expect(a.went).toEqual([{ page: "task", id: "k1" }]);
    expect(side().tabs).toEqual(["card:KADE-2"]);
  });
  it("a move is saved with moveTask, as on the Tasks page, and the card shows in its new column at once", async () => {
    S.setChatCards("c1", "about", ["KADE-1"]);
    await show();
    await props().onMove("k2", "s-test", "a5");
    render();
    expect(a.moves).toEqual([["k2", "s-test", "a5"]]);
    expect(props().tasks.find((t) => t.id === "k2")).toMatchObject({ stateId: "s-test", stateName: "Testing", sortKey: "a5" });
  });
  it("a move that fails goes back and says why", async () => {
    S.setChatCards("c1", "about", ["KADE-1"]);
    await show();
    a.moveAnswer = () => Promise.reject(new Error("Testing takes no cards by hand"));
    await props().onMove("k2", "s-test", "a5");
    render();
    expect(props().tasks.find((t) => t.id === "k2")).toMatchObject({ stateId: "s-todo" });
    const alert = one((x) => x.props.role === "alert");
    expect(alert && textOf(kids(alert))).toBe("Couldn't move KADE-2: Testing takes no cards by hand");
  });
});

// GA-372 QA: the board's own page is the Tasks page's board at the project's own address, #/board/project/<id>; nothing is
// kept on this computer for it any more (it used to set the Tasks page's project filter, gizai-task-project, which then
// stuck to the sidebar's Tasks). The team's board (no project) is #/board, every project of the team picked.
describe("the board's own page (GA-372)", () => {
  it("goes to the project's board, #/board/project/<id>, picks the project's team and keeps no project on this computer", () => {
    M.boardOwnPage(PROJECTS[0] as never);
    expect(a.went).toEqual([{ page: "board", project: "p-kade" }]);
    expect(store.get("gizai-team")).toBe("team1");
    expect(store.has("gizai-task-project")).toBe(false);
  });
  it("the team's board goes to #/board, every project's, and leaves an old stored project to the Tasks page to clear", () => {
    store.set("gizai-task-project", "p-gfw");
    M.boardOwnPage(null);
    M.boardOwnPage(undefined);
    expect(a.went).toEqual([{ page: "board" }, { page: "board" }]);
    // the Tasks page clears it as it opens (TasksSearchKeep.test.tsx); this doesn't touch it
    expect(store.get("gizai-task-project")).toBe("p-gfw");
  });
  it("closes the Board tab beside the chat as it goes", () => {
    S.openBoardTab();
    expect(side().tabs.filter((t) => t.startsWith("board"))).toHaveLength(1);
    M.boardOwnPage(PROJECTS[1] as never);
    expect(side().tabs.filter((t) => t.startsWith("board"))).toEqual([]);
    expect(a.went).toEqual([{ page: "board", project: "p-gfw" }]);
  });
});
