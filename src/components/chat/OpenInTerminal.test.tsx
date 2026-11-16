// GA-279 QA: Open in terminal under a Chief answer that asks you to run commands. The row under a message: nothing without a
// command; while the Editor's folders load, a hidden button's room; then a quiet button per folder the message names (at most
// 3, in the order named) with the terminal icon, "Open in terminal", the folder in mono and its tooltip; Open in terminal…
// with the folder picker titled "Open terminal in" when it names none of the Editor's folders. A click goes through GA-278's
// real openChatTerminal(folder.id, true) with terminalOpen faked: the panel shows the terminal with the focus, a second click
// shows the same one while it waits for a command, and a new one starts while a program runs in it or after it ended. A
// terminal that can't start says why under the button (role alert). Nothing is ever typed into a terminal. There is no DOM
// here, so the small hook runtime of FolderPickerTerminal.test.tsx stands in for React (with useSyncExternalStore and memo),
// and the picker's portal renders in place. Each test imports the modules afresh, so the folders and the chat's terminals
// start empty.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import type { EditorFolder, TerminalSession } from "../../types";

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
const DOQ = folder(`${HOME}/Herd/doq-studio`, { kind: "project", projectId: "p1" });
const SHOP = folder(`${HOME}/Herd/shop`);
const KADE = folder(`${HOME}/Herd/kade-portal`);
const API = folder(`${HOME}/Herd/invoice-api`);
const CARD = folder(`${HOME}/.local/share/gizai/worktrees/GA/GA-279`, { kind: "card", taskId: "t279", name: "GA-279" });
const FOLDERS = [DOQ, SHOP, KADE, API, CARD];

// The api, faked: every call is noted; the Editor's folders answer when the test says so; terminalOpen answers per folder.
const a = vi.hoisted(() => ({
  calls: [] as string[],
  folders: [] as unknown[],
  folderAsks: 0,
  hold: false,
  waiting: [] as (() => void)[],
  opens: [] as string[],
  answer: null as null | ((folder: string) => Promise<unknown>),
  n: 0,
}));
// The terminals' list (TF0's store), faked: what terminalOpen started, busy or ended as the test says.
const tl = vi.hoisted(() => ({ list: [] as TerminalSession[], subs: new Set<() => void>() }));
vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const k of Object.keys(real)) if (typeof real[k] === "function") out[k] = () => { a.calls.push(k); return new Promise(() => {}); };
  return {
    ...out,
    editorFolders: () => {
      a.calls.push("editorFolders");
      a.folderAsks += 1;
      return a.hold ? new Promise((ok) => { a.waiting.push(() => ok(a.folders)); }) : Promise.resolve(a.folders);
    },
    getSettings: () => Promise.resolve({ editorRoots: ["~/Herd"] }),
    listTasks: () => Promise.resolve([{ id: "t279", identifier: "GA-279", title: "Open in terminal", byHand: true, assigneeId: "u1", assigneeName: "You", stateCategory: "in_progress" }]),
    listProjects: () => Promise.resolve([{ id: "p1", name: "Doq studio", color: "var(--c-blue)" }]),
    terminalOpen: (dir: string, cols: number, rows: number, name: string | null) => {
      a.calls.push("terminalOpen");
      a.opens.push(dir);
      if (a.answer) return a.answer(dir);
      void cols; void rows; void name;
      const s: TerminalSession = { id: `t${++a.n}`, name: "sh", folder: dir, folderPath: dir.replace(HOME, "~"), taskId: null, shell: "sh", busy: null,
        ended: null, cols: 80, rows: 24, openedAt: a.n };
      return Promise.resolve(s);
    },
  };
});
vi.mock("../../lib/terminals", async (orig) => ({
  ...(await orig<object>()),
  terminalsNow: () => tl.list,
  watchTerminals: (g: () => void) => { tl.subs.add(g); return () => { tl.subs.delete(g); }; },
  noteOpened: (s: TerminalSession) => { if (!tl.list.some((x) => x.id === s.id)) { tl.list = [...tl.list, s]; tl.subs.forEach((g) => g()); } },
  useTerminals: () => tl.list,
}));
vi.mock("../../router", async (orig) => ({ ...(await orig<object>()), go: () => {} }));

const store = new Map<string, string>();
vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => { store.set(k, String(v)); }, removeItem: (k: string) => { store.delete(k); } });
vi.stubGlobal("window", Object.assign(new EventTarget(), { innerWidth: 1280, innerHeight: 720 }));
vi.stubGlobal("document", { body: {} });

let M: typeof import("./OpenInTerminal");
let C: typeof import("../../lib/chatTerminal");

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
const rowOf = () => one((f) => f.props.className === "msg-term");
const rowButtons = () => {
  const row = one((f) => f.props.className === "msg-term-row");
  return row ? findAll(kids(row), (f) => f.type === "button") : [];
};
const labels = () => rowButtons().map((b) => textOf(kids(b)));
const iconIn = (b: Found) => findAll(kids(b), (f) => typeof f.type === "object" && f.props.className === "icon").length;
const why = () => one((f) => f.props.role === "alert");
const picker = () => one((f) => f.props.role === "dialog");
const option = (name: string) => findAll(tree, (f) => f.props.role === "option").find((o) => findAll(kids(o), (x) => (x.props.className === "card-id" || x.props.className === "label") && textOf(kids(x)) === name).length > 0);

async function show(md: string, more: { hold?: boolean } = {}) {
  a.hold = !!more.hold;
  root = () => <M.OpenInTerminal md={md} you="u1" />;
  render();
  await settle();
}
/** Several rows at once, as the chat draws them, one per message. */
async function showAll(mds: string[]) {
  root = () => <>{mds.map((md, i) => <M.OpenInTerminal key={i} md={md} you="u1" />)}</>;
  render();
  await settle();
}
const shown = () => C.useChatTerminals();
const session = (id: string) => tl.list.find((s) => s.id === id)!;
const setSession = (id: string, more: Partial<TerminalSession>) => { tl.list = tl.list.map((s) => (s.id === id ? { ...s, ...more } : s)); tl.subs.forEach((g) => g()); };

// The card's Why, as the Chief wrote it
const WHY = "Before you test, in `~/Herd/doq-studio`:\n\n1. Run `php artisan migrate`.\n2. Run `npm run build`.";

beforeEach(async () => {
  vi.useFakeTimers();
  for (const [k, inst] of insts) { unmount(inst); insts.delete(k); }
  h.effects.length = 0;
  Object.assign(a, { calls: [], folders: FOLDERS, folderAsks: 0, hold: false, waiting: [], opens: [], answer: null, n: 0 });
  tl.list = [];
  tl.subs.clear();
  store.clear();
  vi.resetModules();
  M = await import("./OpenInTerminal");
  C = await import("../../lib/chatTerminal");
});
afterEach(() => { root = () => null; render(); vi.useRealTimers(); });

describe("which messages get Open in terminal", () => {
  it("none for a message that only mentions commands or files, or has no code", async () => {
    for (const md of ["QA ran `npm test` and it passed.", "The change is in `src/api.ts`.", "All done: GA-279 is in Review.",
      "Run npm test in ~/Herd/doq-studio.", "In `~/Herd/doq-studio`:\n\n```php\nArtisan::call('migrate');\n```", ""]) {
      await show(md);
      expect(tree, md).toBeNull();
    }
    expect(a.folderAsks, "a message without a command doesn't ask for the folders").toBe(0);
  });

  it("a row for the card's Why, a ```sh block and a block with $ prompts", async () => {
    for (const md of [WHY, "In `~/Herd/doq-studio`:\n\n```sh\nphp artisan migrate\nnpm run build\n```",
      "In `~/Herd/doq-studio`:\n\n```sh\n$ php artisan migrate\n$ npm run build\n```"]) {
      await show(md);
      expect(rowOf(), md).toBeDefined();
      expect(labels(), md).toEqual(["Open in terminal~/Herd/doq-studio"]);
    }
  });
});

describe("a button per folder the message names", () => {
  it("is one quiet Open in terminal ~/Herd/doq-studio for the card's Why: the icon, the words, the folder in mono, its tooltip", async () => {
    await show(WHY);
    const [b, ...more] = rowButtons();
    expect(more).toEqual([]);
    expect(b!.props.className).toBe("btn ghost sm");
    expect(b!.props.type).toBe("button");
    expect(b!.props.title).toBe("Open a terminal in ~/Herd/doq-studio beside the chat");
    expect(iconIn(b!), "the terminal icon").toBe(1);
    const mono = findAll(kids(b!), (f) => f.props.className === "mono");
    expect(mono.map((m) => textOf(kids(m)))).toEqual(["~/Herd/doq-studio"]);
    expect(textOf(kids(b!))).toBe("Open in terminal~/Herd/doq-studio");
  });

  it("opens ~/Herd/doq-studio for a subfolder or a file in it, and for its absolute path", async () => {
    for (const md of ["In `~/Herd/doq-studio/resources/embed-test`: run `php -S 127.0.0.1:8081`.",
      `In \`${HOME}/Herd/doq-studio\`: run \`npm test\`.`, `Open ${HOME}/Herd/doq-studio/app/Kernel.php and run \`php artisan test\`.`]) {
      await show(md);
      expect(labels(), md).toEqual(["Open in terminal~/Herd/doq-studio"]);
      await click(rowButtons()[0]!);
    }
    expect(a.opens[0], "the terminal starts in the folder, not the subfolder").toBe(DOQ.id);
  });

  it("is two buttons for two folders, in the order named; never more than 3", async () => {
    await show("In `~/Herd/shop`: run `npm run build`. Then in `~/Herd/doq-studio`: run `php artisan migrate`.");
    expect(labels()).toEqual(["Open in terminal~/Herd/shop", "Open in terminal~/Herd/doq-studio"]);
    await show("Run `npm ci` in ~/Herd/doq-studio, ~/Herd/kade-portal, ~/Herd/invoice-api and ~/Herd/shop.");
    expect(labels()).toEqual(["Open in terminal~/Herd/doq-studio", "Open in terminal~/Herd/kade-portal", "Open in terminal~/Herd/invoice-api"]);
  });

  it("doesn't take a URL's path or a card's worktree as a folder: Open in terminal… instead", async () => {
    await show("Run `npm run dev`, then open http://127.0.0.1:8081/Herd/doq-studio/page.html.");
    expect(labels()).toEqual(["Open in terminal…"]);
    await show("In `~/.local/share/gizai/worktrees/GA/GA-279`: run `npm test`.");
    expect(labels()).toEqual(["Open in terminal…"]);
  });
});

describe("the row from the message's first draw", () => {
  it("keeps a hidden button's room while the folders load, then shows the button", async () => {
    await show(WHY, { hold: true });
    const wait = one((f) => typeof f.props.className === "string" && (f.props.className as string).includes("msg-term-wait"));
    expect(wait, "a hidden button's room").toBeDefined();
    expect(wait!.props.className).toBe("btn ghost sm msg-term-wait");
    expect(wait!.props["aria-hidden"]).toBeTruthy();
    expect(textOf(kids(wait!)), "as tall as the button: its icon and words").toBe("Open in terminal");
    expect(iconIn(wait!)).toBe(1);
    expect(rowButtons(), "nothing to click yet").toEqual([]);
    a.waiting.splice(0).forEach((ok) => ok());
    await settle();
    expect(labels()).toEqual(["Open in terminal~/Herd/doq-studio"]);
    expect(one((f) => typeof f.props.className === "string" && (f.props.className as string).includes("msg-term-wait"))).toBeUndefined();
  });

  it("shows Open in terminal… at once for a message that names no path, without asking for the folders", async () => {
    await show("Run `npm test`.", { hold: true });
    expect(labels()).toEqual(["Open in terminal…"]);
    expect(a.folderAsks).toBe(0);
  });

  it("asks for the folders once for all the rows of a chat", async () => {
    await showAll([WHY, "In `~/Herd/shop`: run `npm ci`.", "QA ran `npm test`.", "Then in ~/Herd/kade-portal run `composer install`."]);
    expect(a.folderAsks).toBe(1);
    const rows = findAll(tree, (f) => f.props.className === "msg-term-row").map((r) => findAll(kids(r), (x) => x.type === "button").map((b) => textOf(kids(b))));
    expect(rows).toEqual([["Open in terminal~/Herd/doq-studio"], ["Open in terminal~/Herd/shop"], ["Open in terminal~/Herd/kade-portal"]]);
  });
});

describe("a click", () => {
  it("opens the chat's terminal panel with a terminal in the folder, with the focus", async () => {
    await show(WHY);
    const before = shown().focus;
    await click(rowButtons()[0]!);
    expect(a.opens).toEqual([DOQ.id]);
    const t = shown();
    expect([t.ids, t.current, t.open]).toEqual([["t1"], "t1", true]);
    expect(t.focus, "the terminal takes the focus").toBeGreaterThan(before);
    expect(session("t1").folder).toBe(DOQ.id);
    expect(why()).toBeUndefined();
  });

  it("a second click shows the same terminal while it waits for a command", async () => {
    await show(WHY);
    await click(rowButtons()[0]!);
    C.hideChatPanel();
    const focus = shown().focus;
    await click(rowButtons()[0]!);
    expect(a.opens, "no new terminal").toEqual([DOQ.id]);
    expect([shown().ids, shown().current, shown().open]).toEqual([["t1"], "t1", true]);
    expect(shown().focus).toBeGreaterThan(focus);
  });

  it("starts a new tab in that folder while a program runs in it, and after it ended", async () => {
    await show(WHY);
    await click(rowButtons()[0]!);
    setSession("t1", { busy: "npm run build" });
    await click(rowButtons()[0]!);
    expect(a.opens).toEqual([DOQ.id, DOQ.id]);
    expect([shown().ids, shown().current]).toEqual([["t1", "t2"], "t2"]);
    setSession("t2", { ended: { code: 0 } as unknown as TerminalSession["ended"] });
    await click(rowButtons()[0]!);
    expect(a.opens).toEqual([DOQ.id, DOQ.id, DOQ.id]);
    expect(shown().current).toBe("t3");
    // t3 waits for a command: the next click shows it
    await click(rowButtons()[0]!);
    expect(a.opens).toHaveLength(3);
    expect(shown().current).toBe("t3");
  });

  it("opens each folder's own terminal under a message that names two", async () => {
    await show("In `~/Herd/shop`: run `npm run build`. Then in `~/Herd/doq-studio`: run `php artisan migrate`.");
    await click(rowButtons()[1]!);
    await click(rowButtons()[0]!);
    expect(a.opens).toEqual([DOQ.id, SHOP.id]);
    await click(rowButtons()[1]!);
    expect(a.opens, "doq-studio's terminal waits for a command: it shows again").toHaveLength(2);
    expect(shown().current).toBe("t1");
  });

  it("never types or runs anything: the terminal starts in the folder and waits", async () => {
    await show(WHY);
    await click(rowButtons()[0]!);
    await click(rowButtons()[0]!);
    expect(a.calls.filter((c) => c !== "editorFolders" && c !== "terminalOpen"), "no other call (terminalInput, …)").toEqual([]);
  });

  it("says why under the button when a terminal can't start, and the next click starts afresh", async () => {
    await show(WHY);
    a.answer = () => Promise.reject(new Error("The folder ~/Herd/doq-studio is gone"));
    await click(rowButtons()[0]!);
    const alert = why()!;
    expect(alert.props.className).toBe("msg-term-why");
    expect(textOf(kids(alert))).toBe("The folder ~/Herd/doq-studio is gone.");
    const parts = findAll(tree, (f) => f.props.className === "msg-term-row" || f.props.role === "alert").map((f) => f.props.className);
    expect(parts, "under the button").toEqual(["msg-term-row", "msg-term-why"]);
    a.answer = null;
    await click(rowButtons()[0]!);
    expect(why()).toBeUndefined();
    expect(shown().current).toBe("t1");
  });

  it("starts one terminal for a double click while the first is still starting", async () => {
    await show(WHY);
    let started: (s: TerminalSession) => void = () => {};
    a.answer = (dir) => new Promise((ok) => { started = (s) => ok({ ...s, folder: dir }); });
    await click(rowButtons()[0]!);
    expect(rowButtons()[0]!.props["aria-busy"]).toBe(true);
    await click(rowButtons()[0]!);
    expect(a.opens).toEqual([DOQ.id]);
    started({ id: "t9", name: "sh", folder: "", folderPath: "~/Herd/doq-studio", taskId: null, shell: "sh", busy: null, ended: null, cols: 80, rows: 24, openedAt: 9 });
    await settle();
    expect(rowButtons()[0]!.props["aria-busy"]).toBeUndefined();
    expect(shown().current).toBe("t9");
  });
});

describe("Open in terminal… when no folder resolves", () => {
  it("is one quiet button that opens the folder picker titled 'Open terminal in' under it", async () => {
    await show("Run `npm test` in your checkout.");
    const [b, ...more] = rowButtons();
    expect(more).toEqual([]);
    expect(b!.props.className).toBe("btn ghost sm");
    expect(textOf(kids(b!))).toBe("Open in terminal…");
    expect(iconIn(b!)).toBe(1);
    expect(picker()).toBeUndefined();
    await click(b!);
    const p = picker()!;
    expect(p.props["aria-label"]).toBe("Open terminal in");
    expect(textOf(kids(one((f) => f.props.className === "ed-picker-title")!))).toBe("Open terminal in");
    expect(rowButtons()[0]!.props["aria-expanded"]).toBe(true);
  });

  it("a pick opens the terminal in that folder beside the chat", async () => {
    await show("```sh\nnpm ci\n```");
    await click(rowButtons()[0]!);
    await click(option("kade-portal")!);
    expect(a.opens).toEqual([KADE.id]);
    expect([shown().current, shown().open]).toEqual(["t1", true]);
    expect(picker(), "the picker closes after a pick that starts").toBeUndefined();
    // the terminal waits for a command: picking the same folder again shows it
    await click(rowButtons()[0]!);
    await click(option("kade-portal")!);
    expect(a.opens).toHaveLength(1);
  });

  it("says why under the folder's row when a terminal can't start there, with the picker still open", async () => {
    await show("Run `npm test`.");
    a.answer = () => Promise.reject(new Error("No such folder"));
    await click(rowButtons()[0]!);
    await click(option("shop")!);
    expect(a.opens).toEqual([SHOP.id]);
    expect(picker(), "still open").toBeDefined();
    expect(textOf(kids(why()!))).toBe("No such folder.");
  });
});
