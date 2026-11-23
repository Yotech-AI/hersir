// GA-343 QA, GA-345 QA: Hersir's view for Claude Desktop (src/widget).
// - No network: no fetch, XMLHttpRequest, WebSocket, EventSource or sendBeacon anywhere in the view's code (every file
//   main.tsx imports, the app's Board among them), nor in the built file; the bridge works with each of them made to fail.
// - The bridge: ui/initialize then ui/notifications/initialized, the tool's input and result, the host's context (theme),
//   tools/call through the host, display modes, the host's requests, size-changed from a ResizeObserver.
// - The board in structuredContent ({view, id, data}, GA-345's contract) as the app's Board draws it; every 2 to 5 s
//   board_changes since view_read's seq, and view_read again only when something changed.
// - views.ts reads show's, view_read's and board_changes' results; the stand-in host's made-up board answers as Hersir.
// - The built view is one file under 1 MB; neither build has the stand-in host page.
// GA-349 QA: the view lists no display mode of its own at ui/initialize (that breaks it in Claude); the poll lives in
// App.tsx; the built file may hold a JS `new URL(…)` (react-markdown's vfile), which loads nothing. views.ts' card,
// Chief and change helpers, the board's real order keys, every MarkdownView with openLink, and the stand-in host's card
// and Chief (App.tsx itself as a host drives it: WidgetApp.test.tsx).
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { Bridge, BridgeError, PROTOCOL, reportSize, type ToolResult } from "./bridge";
import { boardOf, statesOf, tasksOf, titleOf, workingOf, WORKING } from "./board";
import { CUT, cardOf, changesOf, chiefOf, jsonOf, keyOf, sentOf, touches, viewOf, type BoardData, type CardData } from "./views";
import { FakeBoard, PROJECT, sentence } from "../widget-host/fakeBoard";
import { FakeCards } from "../widget-host/fakeCard";
import { DESKTOP_THREAD, FakeChief } from "../widget-host/fakeChief";
import { generateKeyBetween } from "fractional-indexing";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const ROOT = new URL("../../", import.meta.url);
const read = (path: string): string => fs.readFileSync(new URL(path, ROOT), "utf8");
const exists = (path: string): boolean => fs.existsSync(new URL(path, ROOT));

// ---- no network ----------------------------------------------------------------------------------------------------

/** The network APIs a view must not use: everything it needs comes through the host. In built (minified) code any mention
 *  counts. */
const NETWORK = /\b(fetch|XMLHttpRequest|WebSocket|EventSource|sendBeacon)\b/g;
/** In source, every mention of these counts… */
const NETWORK_NAMES = /\b(XMLHttpRequest|WebSocket|EventSource|sendBeacon)\b/g;
/** …and fetch as the global one: a call, or read off window, globalThis or self. A property named fetch (`fetch?: string`)
 *  or a file's own binding of that name (a parameter, as lib/share.ts's runFetch has) is no network call. */
const FETCH_CALL = /(?<![.\w$])fetch\s*\(|\b(?:window|globalThis|self)\s*\.\s*fetch\b/g;
const OWN_FETCH = /[(,]\s*fetch\s*[:,)=?]|\b(?:const|let|var|function)\s+fetch\b/;

/** The code without its comments (a comment may say "nothing is fetched"), lines kept in place; `://` in a string stays. */
function code(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, (c) => c.replace(/[^\n]/g, "")).replace(/(^|[^:"'`\\])\/\/.*$/gm, "$1");
}

/** The network APIs a source file uses, each with its line. */
function networkCalls(source: string): string[] {
  const out: string[] = [];
  const text = code(source);
  const ownFetch = OWN_FETCH.test(text);
  text.split("\n").forEach((line, i) => {
    for (const [name] of line.matchAll(NETWORK_NAMES)) out.push(`${i + 1}: ${name}`);
    for (const m of line.matchAll(FETCH_CALL)) if (!ownFetch || /\.\s*fetch/.test(m[0])) out.push(`${i + 1}: fetch`);
  });
  return out;
}

/** The files of the app's own code that the view's entry imports, one by one: relative imports, followed to their files. */
function widgetFiles(entry = "src/widget/main.tsx"): { files: string[]; packages: Set<string> } {
  const files: string[] = [];
  const packages = new Set<string>();
  const todo = [entry];
  while (todo.length) {
    const file = todo.pop() as string;
    if (files.includes(file)) continue;
    files.push(file);
    const dir = file.slice(0, file.lastIndexOf("/") + 1);
    const src = code(read(file));
    const specs = [...src.matchAll(/\b(?:import|export)\s[^"'`;]*?from\s*["']([^"']+)["']|\bimport\s*\(?\s*["']([^"']+)["']/g)].map((m) => m[1] ?? m[2]);
    for (const spec of specs) {
      const path = spec.split("?")[0];
      if (!path.startsWith(".")) { packages.add(path); continue; }
      if (/\.(css|woff2?|svg|png|jpe?g|json)$/.test(path)) continue;
      const base = new URL(path, new URL(dir, ROOT)).href.slice(ROOT.href.length);
      const found = [base, `${base}.ts`, `${base}.tsx`, `${base}/index.ts`, `${base}/index.tsx`].find((p) => /\.(ts|tsx|js|mjs)$/.test(p) && exists(p));
      if (!found) throw new Error(`${file} imports ${spec}, which isn't found`);
      todo.push(found);
    }
  }
  return { files, packages };
}

describe("the board view makes no network call (GA-343)", () => {
  it("the check finds each network API in code, and not in a comment", () => {
    expect(networkCalls(`const r = await fetch("/board");`)).toEqual(["1: fetch"]);
    expect(networkCalls(`const x = new XMLHttpRequest();`)).toEqual(["1: XMLHttpRequest"]);
    expect(networkCalls(`const ws = new WebSocket("ws://127.0.0.1:1");`)).toEqual(["1: WebSocket"]);
    expect(networkCalls(`const es = new EventSource("/feed");`)).toEqual(["1: EventSource"]);
    expect(networkCalls(`navigator.sendBeacon("/x", "y");`)).toEqual(["1: sendBeacon"]);
    expect(networkCalls(`const f = globalThis.fetch;`)).toEqual(["1: fetch"]);
    expect(networkCalls(`window.fetch("/x")`)).toEqual(["1: fetch"]);
    expect(networkCalls(`const a = 1;\nvoid fetch ("/later");`)).toEqual(["2: fetch"]);
    expect(networkCalls(`// nothing is fetched: no fetch here\n/* nor a\nWebSocket */\nconst url = "http://x"; // EventSource\nnew WebSocket(url);`)).toEqual(["5: WebSocket"]);
    // a property, or a file's own binding named fetch, is no network call
    expect(networkCalls(`type Web = { search?: string; fetch?: string | null };`)).toEqual([]);
    expect(networkCalls(`export function runFetch<T>(fetch: () => Promise<T>) { return fetch(); }`)).toEqual([]);
  });

  it("the files the view's entry imports are its own code and the app's Board, and none of them uses a network API", () => {
    const { files, packages } = widgetFiles();
    for (const f of ["src/widget/main.tsx", "src/widget/BoardView.tsx", "src/widget/bridge.ts", "src/widget/board.ts", "src/widget/fonts.ts",
      "src/components/Board.tsx", "src/widget/App.tsx", "src/widget/CardView.tsx", "src/widget/ChiefView.tsx", "src/widget/views.ts",
      "src/components/MarkdownView.tsx"]) expect(files).toContain(f);
    // the stand-in host is no part of it
    expect(files.filter((f) => f.includes("widget-host"))).toEqual([]);
    // @modelcontextprotocol/ext-apps for its types only: never imported as a value
    for (const f of files) {
      for (const m of read(f).matchAll(/import\s+([^;]*?)from\s*["']@modelcontextprotocol\/ext-apps[^"']*["']/g))
        expect(m[1].trim().startsWith("type "), `${f}: ${m[0]}`).toBe(true);
    }
    expect([...packages].filter((p) => p.startsWith("@modelcontextprotocol"))).toEqual(["@modelcontextprotocol/ext-apps"]);
    const found = files.flatMap((f) => networkCalls(read(f)).map((c) => `${f}:${c}`));
    expect(found).toEqual([]);
  });

  it("every file in src/widget is free of network APIs too, also one main.tsx doesn't import yet", () => {
    const dir = new URL("src/widget/", ROOT);
    const found = (fs.readdirSync(dir) as string[]).filter((f) => /\.(ts|tsx)$/.test(f) && !/\.test\.tsx?$/.test(f))
      .flatMap((f) => networkCalls(read(`src/widget/${f}`)).map((c) => `src/widget/${f}:${c}`));
    expect(found).toEqual([]);
  });
});

// ---- the bridge ----------------------------------------------------------------------------------------------------

type Sent = { jsonrpc?: string; id?: number | string; method?: string; params?: Record<string, unknown>; result?: unknown; error?: { code: number; message: string } };

/** The view's window and its host's, as two ends: what the view posts to the host, and a way to deliver the host's. */
function windows() {
  const self = new EventTarget();
  const sent: { msg: Sent; origin: string }[] = [];
  const host = { postMessage: (msg: Sent, origin: string) => sent.push({ msg, origin }) } as unknown as Window;
  const deliver = (data: unknown, source: unknown = host) => {
    const e = new Event("message");
    Object.assign(e, { data, source });
    self.dispatchEvent(e);
  };
  return { self: self as unknown as Window, host, sent, deliver, last: () => sent[sent.length - 1].msg };
}


describe("the view's bridge to the host (GA-343)", () => {
  const network: string[] = [];
  beforeEach(() => {
    network.length = 0;
    const refuse = (name: string) => function () { network.push(name); throw new Error(`network: ${name}`); };
    vi.stubGlobal("fetch", refuse("fetch"));
    vi.stubGlobal("XMLHttpRequest", refuse("XMLHttpRequest"));
    vi.stubGlobal("WebSocket", refuse("WebSocket"));
    vi.stubGlobal("EventSource", refuse("EventSource"));
  });
  afterEach(() => {
    vi.unstubAllGlobals();
    expect(network, "a network API was used").toEqual([]);
  });

  it("asks ui/initialize, says ui/notifications/initialized, and takes the tool's input, result and the host's theme", async () => {
    const w = windows();
    const seen: string[] = [];
    let result: ToolResult | null = null;
    const contexts: unknown[] = [];
    const b = new Bridge(w.host, {
      toolInput: (args) => seen.push(`input ${JSON.stringify(args)}`),
      toolResult: (r) => { result = r; seen.push("result"); },
      context: (c) => contexts.push({ ...c }),
    }, w.self);
    const connected = b.connect({ name: "Hersir board", version: "1" });
    expect(w.sent).toHaveLength(1);
    expect(w.sent[0].origin).toBe("*");
    expect(w.sent[0].msg).toEqual({ jsonrpc: "2.0", id: 1, method: "ui/initialize", params: {
      appInfo: { name: "Hersir board", version: "1" }, appCapabilities: {}, protocolVersion: PROTOCOL } });
    // GA-349: no display modes of its own (listing them breaks ui/initialize in Claude): it takes the host's
    expect(JSON.stringify(w.sent[0].msg)).not.toContain("availableDisplayModes");
    expect(PROTOCOL).toBe("2026-01-26");
    w.deliver({ jsonrpc: "2.0", id: 1, result: { protocolVersion: PROTOCOL, hostContext: { theme: "dark", displayMode: "inline" } } });
    await connected;
    expect(w.last()).toEqual({ jsonrpc: "2.0", method: "ui/notifications/initialized", params: {} });
    expect(contexts).toEqual([{ theme: "dark", displayMode: "inline" }]);
    expect(b.context.theme).toBe("dark");

    w.deliver({ jsonrpc: "2.0", method: "ui/notifications/tool-input", params: { arguments: { project: "GA" } } });
    const board = { content: [{ type: "text", text: "The user sees the board." }], structuredContent: { view: "board" }, isError: false };
    w.deliver({ jsonrpc: "2.0", method: "ui/notifications/tool-result", params: board });
    expect(seen).toEqual(['input {"project":"GA"}', "result"]);
    expect(result).toEqual(board);

    // the theme follows the host: a change is merged into what it said before
    w.deliver({ jsonrpc: "2.0", method: "ui/notifications/host-context-changed", params: { theme: "light" } });
    expect(contexts[contexts.length - 1]).toEqual({ theme: "light", displayMode: "inline" });

    // only the host's messages count: another window's, or one that isn't JSON-RPC 2.0, change nothing
    w.deliver({ jsonrpc: "2.0", method: "ui/notifications/host-context-changed", params: { theme: "dark" } }, {});
    w.deliver({ method: "ui/notifications/host-context-changed", params: { theme: "dark" } });
    w.deliver("not an object");
    expect(b.context.theme).toBe("light");
    expect(contexts).toHaveLength(2);
    b.close();
  });

  it("calls the server's tools through the host, with its answer or its error, and asks for another display mode", async () => {
    const w = windows();
    const b = new Bridge(w.host, {}, w.self);
    const read = b.callTool("view_read", { view: "board", id: "GA" });
    const ask = w.last();
    expect(ask).toEqual({ jsonrpc: "2.0", id: 1, method: "tools/call", params: { name: "view_read", arguments: { view: "board", id: "GA" } } });
    w.deliver({ jsonrpc: "2.0", id: 1, result: { content: [{ type: "text", text: "ok" }], structuredContent: { view: "board" } } });
    await expect(read).resolves.toEqual({ content: [{ type: "text", text: "ok" }], structuredContent: { view: "board" } });

    const failed = b.callTool("view_read", { view: "board" });
    w.deliver({ jsonrpc: "2.0", id: 2, error: { code: -32000, message: "Open Hersir first" } });
    await expect(failed).rejects.toBeInstanceOf(BridgeError);
    await expect(failed).rejects.toMatchObject({ code: -32000, message: "Open Hersir first" });

    // an answer to no request of the view's is ignored
    w.deliver({ jsonrpc: "2.0", id: 99, result: {} });

    const mode = b.requestDisplayMode("fullscreen");
    expect(w.last()).toEqual({ jsonrpc: "2.0", id: 3, method: "ui/request-display-mode", params: { mode: "fullscreen" } });
    w.deliver({ jsonrpc: "2.0", id: 3, result: { mode: "fullscreen" } });
    await expect(mode).resolves.toBe("fullscreen");

    b.sizeChanged(720, 480);
    expect(w.last()).toEqual({ jsonrpc: "2.0", method: "ui/notifications/size-changed", params: { width: 720, height: 480 } });

    // closed: a request still waiting fails, and the host's messages reach nothing any more
    const waiting = b.callTool("view_read", { view: "board" });
    b.close();
    await expect(waiting).rejects.toThrow("the view closed");
    const before = w.sent.length;
    w.deliver({ jsonrpc: "2.0", id: 7, method: "ping" });
    expect(w.sent.length).toBe(before);
  });

  it("answers the host's ping and ui/resource-teardown, and says it doesn't know any other request", () => {
    const w = windows();
    let tornDown = 0;
    const b = new Bridge(w.host, { teardown: () => tornDown++ }, w.self);
    w.deliver({ jsonrpc: "2.0", id: "p1", method: "ping" });
    expect(w.last()).toEqual({ jsonrpc: "2.0", id: "p1", result: {} });
    w.deliver({ jsonrpc: "2.0", id: 5, method: "ui/resource-teardown", params: { reason: "gone" } });
    expect(w.last()).toEqual({ jsonrpc: "2.0", id: 5, result: {} });
    expect(tornDown).toBe(1);
    w.deliver({ jsonrpc: "2.0", id: 6, method: "ui/something-else" });
    expect(w.last()).toMatchObject({ jsonrpc: "2.0", id: 6, error: { code: -32601 } });
    b.close();
  });

  it("a request the host never answers fails after a while, before Claude's 45 s would leave the view waiting forever", async () => {
    vi.useFakeTimers();
    try {
      const w = windows();
      const b = new Bridge(w.host, {}, w.self);
      const call = b.callTool("view_read", { view: "board" });
      const caught = call.catch((e: Error) => e);
      await vi.advanceTimersByTimeAsync(61_000);
      expect(String(await caught)).toContain("no answer to tools/call");
      b.close();
    } finally {
      vi.useRealTimers();
    }
  });

  it("says the view's size from a ResizeObserver, once a frame at most and only when it changed", async () => {
    const observers: { cb: () => void; observed: unknown[]; disconnected: boolean }[] = [];
    vi.stubGlobal("ResizeObserver", class {
      o: { cb: () => void; observed: unknown[]; disconnected: boolean };
      constructor(cb: () => void) { this.o = { cb, observed: [], disconnected: false }; observers.push(this.o); }
      observe(el: unknown) { this.o.observed.push(el); }
      disconnect() { this.o.disconnected = true; }
    });
    let height = 321.2;
    const style = { height: "auto" };
    const heights: string[] = [];
    const html = { style, getBoundingClientRect: () => { heights.push(style.height); return { height }; } };
    const body = {};
    const frames: (() => void)[] = [];
    const doc = { documentElement: html, body } as unknown as Document;
    const win = { innerWidth: 719.5, requestAnimationFrame: (cb: () => void) => frames.push(cb) } as unknown as Window;
    const w = windows();
    const b = new Bridge(w.host, {}, w.self);
    const stop = reportSize(b, doc, win);
    expect(observers).toHaveLength(1);
    expect(observers[0].observed).toEqual([html, body]);
    // one frame for many changes
    observers[0].cb();
    observers[0].cb();
    expect(frames).toHaveLength(1);
    frames.shift()?.();
    expect(w.last()).toEqual({ jsonrpc: "2.0", method: "ui/notifications/size-changed", params: { width: 720, height: 322 } });
    expect(heights).toEqual(["max-content"]);
    expect(style.height).toBe("auto");
    // the same size again: nothing new
    const before = w.sent.length;
    observers[0].cb();
    frames.shift()?.();
    expect(w.sent.length).toBe(before);
    // a new height
    height = 500;
    observers[0].cb();
    frames.shift()?.();
    expect(w.last()).toEqual({ jsonrpc: "2.0", method: "ui/notifications/size-changed", params: { width: 720, height: 500 } });
    stop();
    expect(observers[0].disconnected).toBe(true);
    b.close();
  });
});

// ---- the board -----------------------------------------------------------------------------------------------------

const BOARD: BoardData = {
  team: "Giz team", project: { key: "GA", name: "Giz app" },
  columns: [
    { name: "Backlog", kind: "backlog", total: 1, cards: [
      { identifier: "GA-1", title: "One", project: "Giz app", assignee: null, labels: [], priority: 0, hold: null, running: false, needs_you: false },
    ] },
    { name: "To do", kind: "waiting", total: 3, cards: [
      { identifier: "GA-2", title: "Two", project: "Giz app", assignee: "Backend Agent", labels: [{ name: "bug", color: "#f2706b" }], priority: 2,
        hold: "needs_decision", running: true, needs_you: true },
      { identifier: "GA-3", title: "Three", project: "Giz app", assignee: null, labels: [{ name: "plain", color: null }], priority: 1, hold: null,
        running: false, needs_you: false },
    ] },
    { name: "In progress", kind: "work", total: 0, cards: [] },
  ],
};

describe("the board from structuredContent, as the app's Board draws it (GA-343, GA-345)", () => {
  it("takes only a board's data", () => {
    for (const not of [null, undefined, "board", {}, { cards: [] }, { columns: "x" }]) expect(boardOf(not)).toBeNull();
    expect(boardOf(BOARD)).toEqual(BOARD);
    expect(boardOf({ columns: [] })).toEqual({ team: "", project: null, columns: [] });
  });

  it("gives the Board its columns in board order, by place, with the app's categories for the contract's kinds", () => {
    expect(statesOf(BOARD).map((s) => [s.id, s.name, s.category, s.wipLimit, s.auto])).toEqual([
      ["column-0", "Backlog", "backlog", null, false], ["column-1", "To do", "ready", null, false], ["column-2", "In progress", "in_progress", null, false]]);
    const sortKeys = statesOf(BOARD).map((s) => s.sortKey);
    expect([...sortKeys].sort()).toEqual(sortKeys);
  });

  it("gives the Board its cards with what the board shows, in their column's order, and the running ones as working", () => {
    const [one, two, three] = tasksOf(BOARD);
    expect(one).toMatchObject({ id: "GA-1", identifier: "GA-1", title: "One", projectName: "Giz app", stateId: "column-0", stateName: "Backlog",
      stateCategory: "backlog", hold: null, labels: [], descriptionMd: "", priority: 0, assigneeName: null });
    expect(two).toMatchObject({ id: "GA-2", stateId: "column-1", stateName: "To do", stateCategory: "ready", priority: 2, assigneeName: "Backend Agent",
      hold: "needs_decision", labels: [{ id: "bug", name: "bug", color: "#f2706b" }] });
    expect(three).toMatchObject({ id: "GA-3", stateId: "column-1", labels: [{ name: "plain", color: null }] });
    expect(two.sortKey < three.sortKey).toBe(true);
    expect([...workingOf(BOARD)]).toEqual([["GA-2", WORKING]]);
    expect(titleOf(BOARD)).toBe("Giz app (GA)");
    expect(titleOf({ ...BOARD, project: null })).toBe("Giz team");
  });

  it("asks board_changes every 2 to 5 seconds since the last seq, and view_read only when something changed", async () => {
    // GA-349: the poll moved to App.tsx, for the board, a card and the Chief's thread (stepped through in WidgetApp.test.tsx)
    const src = read("src/widget/App.tsx");
    const poll = Number(/export const POLL_MS = ([\d_]+);/.exec(src)?.[1].replace(/_/g, ""));
    expect(poll).toBeGreaterThanOrEqual(2000);
    expect(poll).toBeLessThanOrEqual(5000);
    expect(src).toContain(`export const VIEW_READ = "view_read";`);
    expect(src).toContain(`export const BOARD_CHANGES = "board_changes";`);
    expect(src).toContain("callTool(BOARD_CHANGES, { since: seq.current })");
    expect(src).toMatch(/callTool\(VIEW_READ, argsOf\(p\)\)/);
    expect(src).toContain("const argsOf = (p: Place) => (p.id ? { view: p.view, id: p.id } : { view: p.view });");
    // the spike's tools are gone from the view
    for (const f of widgetFiles().files) expect(read(f), f).not.toMatch(/\b(show_board|board_read)\b/);
  });
});

// ---- show's, view_read's and board_changes' results (GA-345) ----------------------------------------------------------

describe("the view reads show's, view_read's and board_changes' results as the contract has them (GA-345)", () => {
  it("cuts end with the contract's words", () => {
    expect(CUT).toBe("…(cut: read the rest in Hersir)");
  });

  it("viewOf takes {view, id, seq?, data} for board, card and chief, and nothing else", () => {
    expect(viewOf({ view: "board", id: "GA", data: BOARD })).toEqual({ view: "board", id: "GA", seq: undefined, data: BOARD });
    expect(viewOf({ view: "board", id: null, seq: 1042, data: BOARD })).toEqual({ view: "board", id: null, seq: 1042, data: BOARD });
    const card = { identifier: "GA-12", title: "Fix the login" };
    expect(viewOf({ view: "card", id: "GA-12", seq: 7, data: card })).toEqual({ view: "card", id: "GA-12", seq: 7, data: card });
    const chief = { thread: null, status: "answered", messages: [], draft: null };
    expect(viewOf({ view: "chief", id: null, data: chief })).toEqual({ view: "chief", id: null, seq: undefined, data: chief });
    for (const not of [null, undefined, "board", {}, { view: "column", data: {} }, { view: "board" }, { view: "board", data: "x" }, BOARD])
      expect(viewOf(not)).toBeNull();
  });

  it("changesOf takes board_changes' {seq, cards, board, reload}, and not GA-344's old names", () => {
    expect(changesOf({ seq: 12, cards: ["GA-1"], board: false, reload: false })).toEqual({ seq: 12, cards: ["GA-1"], board: false, reload: false });
    expect(changesOf({ seq: 12, cards: [] })).toEqual({ seq: 12, cards: [], board: false, reload: false });
    expect(changesOf({ counter: 12, tasks: ["GA-1"], board: false, reload: false })).toBeNull();
    expect(changesOf(null)).toBeNull();
  });
});

// ---- the stand-in host's made-up board (GA-345) ---------------------------------------------------------------------

describe("the stand-in host answers view_read and board_changes as Hersir does (GA-345)", () => {
  const BOARD_CARD_KEYS = ["assignee", "hold", "identifier", "labels", "needs_you", "priority", "project", "running", "title"];

  it("its board has the contract's fields, and its sentence is Hersir's", () => {
    const b = new FakeBoard();
    const data = b.data(PROJECT.key);
    expect(data.project).toEqual(PROJECT);
    expect(b.data(null).project).toBeNull();
    for (const c of data.columns) {
      expect(Object.keys(c).sort()).toEqual(["cards", "kind", "name", "total"]);
      expect(c.total).toBe(c.cards.length);
      for (const card of c.cards) {
        expect(Object.keys(card).sort()).toEqual(BOARD_CARD_KEYS);
        expect([null, "needs_decision", "blocked", "stalled"]).toContain(card.hold);
      }
    }
    const n = data.columns.reduce((s, c) => s + c.cards.length, 0);
    expect(sentence(data)).toBe(`The user sees the board of ${PROJECT.name} (${PROJECT.key}) from Hersir in this chat: ${n} cards in ${data.columns.length} columns, 1 on hold.`);
  });

  it("board_changes names the cards changed since a seq, the one changed last first, and none after the latest", () => {
    const b = new FakeBoard();
    const start = b.since(undefined);
    expect(start).toEqual({ seq: b.seq, cards: [], board: false, reload: false });
    const where = () => new Map(b.data(null).columns.flatMap((c) => c.cards.map((card) => [card.identifier, c.name] as const)));
    const before = where();
    b.change();
    const first = [...where()].filter(([id, col]) => before.get(id) !== col).map(([id]) => id);
    expect(first).toHaveLength(1);
    b.change();
    const after = b.since(start.seq);
    expect(after.seq).toBe(start.seq + 2);
    expect(after.cards.length).toBeGreaterThanOrEqual(1);
    expect(after.cards[after.cards.length - 1] === first[0] || after.cards.length === 1).toBe(true);
    expect(b.since(after.seq)).toEqual({ seq: after.seq, cards: [], board: false, reload: false });
    expect(b.since(after.seq + 5).reload).toBe(true);
  });

  it("the host page calls show and answers view_read and board_changes, not the spike's tools", () => {
    const src = read("src/widget-host/main.ts") + read("mcp-host.html");
    expect(src).not.toMatch(/\b(show_board|board_read)\b/);
    expect(src).toContain(`name: "show"`);
    expect(src).toContain(`show {"view": "board", "id": "GA"}`);
    expect(src).toMatch(/view_read/);
    expect(src).toMatch(/board_changes/);
  });
});

// ---- GA-349: the card's and the Chief's data, what a change touches, the board's order keys --------------------------

describe("views.ts reads a card, the Chief's thread and ask_chief's answer, and knows what a change touches (GA-349)", () => {
  it("cardOf takes a card, with lists and texts left out as empty; nothing without an identifier", () => {
    expect(cardOf({ identifier: "GA-12" })).toEqual({
      identifier: "GA-12", title: "", project: null, column: "", assignee: null, labels: [], priority: 0, hold: null, hold_reason: null,
      description_md: "", acceptance_md: null, pr_url: null, comments: [], runs: [], live: null,
    });
    const live = { agent: "QA Agent", output: "Started" };
    expect(cardOf({ identifier: "GA-12", title: "Fix", column: "Review", priority: 3, live, comments: "x" })).toMatchObject({ title: "Fix", column: "Review", priority: 3, live, comments: [] });
    for (const not of [null, undefined, "GA-12", {}, { identifier: 12 }]) expect(cardOf(not)).toBeNull();
  });

  it("chiefOf takes the thread as chief_reply gives it: a status it doesn't know reads as answered, no draft as null", () => {
    const messages = [{ id: "m1", from: "you", time: "2026-11-23T14:05:09Z", text: "Hi" }];
    expect(chiefOf({ thread: "t1", status: "working", messages, draft: "Wri" })).toEqual({ thread: "t1", status: "working", messages, draft: "Wri" });
    expect(chiefOf({ thread: "t1", status: "waiting for you", messages })).toEqual({ thread: "t1", status: "waiting for you", messages, draft: null });
    expect(chiefOf({ thread: null, status: "odd", messages: [] })).toEqual({ thread: null, status: "answered", messages: [], draft: null });
    for (const not of [null, "x", { thread: "t1" }, { messages: "x" }]) expect(chiefOf(not)).toBeNull();
  });

  it("sentOf, jsonOf and keyOf", () => {
    expect(sentOf({ thread: "t1", message: "m1", status: "working" })).toEqual({ thread: "t1", message: "m1" });
    expect(sentOf({ thread: "t1" })).toEqual({ thread: "t1", message: null });
    expect(sentOf({ message: "m1" })).toBeNull();
    expect(jsonOf(`{"a":1}`)).toEqual({ a: 1 });
    expect(jsonOf("Open Hersir first")).toBeUndefined();
    expect(keyOf("GA-12")).toBe("GA");
    expect(keyOf(" WEB-APP-7 ")).toBe("WEB-APP");
    expect(keyOf("GA")).toBeNull();
  });

  it("touches: the card on show, a card of the board's project or on it, any card for every project; board and reload always", () => {
    const c = (cards: string[], more: Partial<{ board: boolean; reload: boolean }> = {}) => ({ seq: 2, cards, board: false, reload: false, ...more });
    expect(touches(c(["ga-12"]), "card", "GA-12")).toBe(true);
    expect(touches(c(["GA-13"]), "card", "GA-12")).toBe(false);
    expect(touches(c([]), "card", "GA-12")).toBe(false);
    expect(touches(c(["GA-99"]), "board", "GA")).toBe(true); // a new card of the project counts
    expect(touches(c(["XY-1"]), "board", "GA")).toBe(false);
    expect(touches(c(["XY-1"]), "board", "GA", (id) => id === "XY-1")).toBe(true); // one on the board on show
    expect(touches(c(["XY-1"]), "board", null)).toBe(true);
    expect(touches(c([]), "board", null)).toBe(false);
    for (const view of ["board", "card", "chief"] as const) {
      expect(touches(c([], { board: true }), view, "GA")).toBe(true);
      expect(touches(c([], { reload: true }), view, "GA")).toBe(true);
    }
    expect(touches(c(["GA-12"]), "chief", null)).toBe(false);
  });

  it("the Board gets real fractional-indexing order keys, so a card dropped between two gets one (it snapped back before)", () => {
    const tasks = tasksOf(BOARD);
    const todo = tasks.filter((t) => t.stateId === "column-1").map((t) => t.sortKey);
    expect(todo).toHaveLength(2);
    // the Board makes a dropped card's key from its neighbours': every pair, and either end, must give one
    expect(generateKeyBetween(todo[0], todo[1]) > todo[0]).toBe(true);
    expect(generateKeyBetween(null, todo[0]) < todo[0]).toBe(true);
    expect(generateKeyBetween(todo[1], null) > todo[1]).toBe(true);
    expect(() => generateKeyBetween("000000", null)).toThrow();
  });

  it("every MarkdownView in the view gets openLink: a link opens through the chat (ui/open-link), never with Tauri", () => {
    for (const f of ["src/widget/CardView.tsx", "src/widget/ChiefView.tsx", "src/widget/BoardView.tsx", "src/widget/App.tsx"]) {
      const uses = [...code(read(f)).matchAll(/<MarkdownView\b[^>]*>/g)].map((m) => m[0]);
      for (const u of uses) expect(u, `${f}: ${u}`).toMatch(/\bopenLink=\{openLink\}/);
    }
    expect(read("src/widget/bridge.ts")).toContain(`this.request("ui/open-link", { url })`);
  });
});

describe("the stand-in host's card and Chief answer as Hersir does (GA-349)", () => {
  const CARD_KEYS: (keyof CardData)[] = ["identifier", "title", "project", "column", "assignee", "labels", "priority", "hold", "hold_reason",
    "description_md", "acceptance_md", "pr_url", "comments", "runs", "live"];

  it("a card has the contract's fields; its comments oldest first, at most 20; its runs newest first, at most 5", () => {
    let now = Date.parse("2026-11-23T14:00:00Z");
    const board = new FakeBoard();
    const cards = new FakeCards(board, () => now);
    const card = cards.data("ga-109") as CardData;
    expect(Object.keys(card).sort()).toEqual([...CARD_KEYS].sort());
    expect(card).toMatchObject({ identifier: "GA-109", column: "Review", hold: "needs_decision", live: null });
    expect(card.hold_reason).toBeTruthy();
    expect(card.pr_url).toMatch(/^https:\/\//);
    for (const c of card.comments) expect(c.at).toMatch(/^\d{4}-\d\d-\d\dT\d\d:\d\d:\d\dZ$/);
    for (let i = 0; i < 25; i++) { now += 1000; expect(cards.comment("GA-109", `Comment ${i}`)).toBeNull(); }
    const many = cards.data("GA-109") as CardData;
    expect(many.comments).toHaveLength(20);
    expect(many.comments.at(-1)?.body_md).toBe("Comment 24");
    for (let i = 0; i < 6; i++) { now += 1000; expect(cards.start("GA-109")).toBeNull(); now += 1000; expect(cards.stop("GA-109")).toBeNull(); }
    const runs = (cards.data("GA-109") as CardData).runs;
    expect(runs).toHaveLength(5);
    expect(runs.map((r) => r.started_at)).toEqual([...runs.map((r) => r.started_at)].sort().reverse());
    expect(cards.data("GA-404")).toBeNull();
    expect(cards.comment("GA-109", "  ")).toMatch(/body_md/);
    expect(cards.stop("GA-109")).toBe("No agent is working on GA-109 right now.");
  });

  it("a run's live output grows, and past 200 lines keeps the last 200 and says it was cut", () => {
    let now = Date.parse("2026-11-23T14:00:00Z");
    const cards = new FakeCards(new FakeBoard(), () => now);
    expect(cards.start("GA-103")).toBeNull();
    expect(cards.start("GA-103")).toMatch(/already has an agent/);
    const lines = () => (cards.data("GA-103") as CardData).live!.output.split("\n");
    expect(lines()).toHaveLength(3);
    now += 5_000;
    expect(lines()).toHaveLength(13);
    now += 200_000;
    expect(lines()).toHaveLength(201);
    expect(lines().at(-1)).toBe(CUT);
  });

  it("the Chief: no thread before the first message; ask_chief makes the chat From Claude Desktop, the draft grows, then its answer", () => {
    let now = Date.parse("2026-11-23T14:00:00Z");
    const chief = new FakeChief(() => now);
    expect(chief.view(null)).toEqual({ thread: null, status: "answered", messages: [], draft: null });
    expect(chief.reply(undefined)).toMatch(/thread/);
    expect(chief.ask("Hi", "other")).toMatch(/There is no chat other/);
    expect(chief.ask("Hi")).toEqual({ thread: DESKTOP_THREAD, message: "msg-1", status: "working" });
    now += 3000;
    const working = chief.reply(DESKTOP_THREAD) as Exclude<ReturnType<FakeChief["reply"]>, string>;
    expect(working.status).toBe("working");
    expect(working.draft?.length).toBeGreaterThan(0);
    expect(Object.keys(working).sort()).toEqual(["draft", "messages", "status", "thread"]);
    now += 6000;
    const done = chief.reply(DESKTOP_THREAD, "msg-1") as Exclude<ReturnType<FakeChief["reply"]>, string>;
    expect(done).toMatchObject({ status: "answered", draft: null });
    expect(done.messages.map((m) => m.from)).toEqual(["Chief"]);
    expect(chief.view(DESKTOP_THREAD)).toMatchObject({ thread: DESKTOP_THREAD, status: "answered" });
  });

  it("the host page shows the board, a card and the Chief's thread, and answers the view's writes", () => {
    const page = read("mcp-host.html");
    for (const v of ["board:GA", "board", "card:GA-109", "card:GA-106", "card:GA-404", "chief"]) expect(page).toContain(`<option value="${v}">`);
    const src = read("src/widget-host/main.ts");
    expect(src).toContain(`const WRITES = ["move_task", "comment_on_task", "start_agent_run", "stop_agent_run", "ask_chief"];`);
    for (const m of [`"chief_reply"`, `"ui/open-link"`, `"ui/request-display-mode"`]) expect(src).toContain(m);
  });
});

// ---- the builds ----------------------------------------------------------------------------------------------------

// npm run build makes both (CI runs it before npm test); a checkout that wasn't built skips these.
const VIEW = "src-tauri/mcp-app/mcp-app.html";

describe("the built view and the app's build (GA-343)", () => {
  it.skipIf(!exists(VIEW))("the view is one HTML file under 1 MB: no script, style or font from elsewhere, and no network API", () => {
    const html = read(VIEW);
    expect(new TextEncoder().encode(html).length).toBeLessThan(1_000_000);
    expect(html.toLowerCase().startsWith("<!doctype html")).toBe(true);
    expect(html.match(/<script[^>]*\ssrc=/gi) ?? []).toEqual([]);
    expect(html.match(/<link[^>]*>/gi) ?? []).toEqual([]);
    // CSS url() of anything but data: (case ignored, as CSS ignores it); a JS `new URL(…)` (vfile's, in react-markdown) loads nothing
    expect(html.match(/(?<!new\s+)\burl\((?!["']?data:)[^)]*\)/gi) ?? []).toEqual([]);
    expect("a{background:url(https://x/y.png)}".match(/(?<!new\s+)\burl\((?!["']?data:)[^)]*\)/gi)).toEqual(["url(https://x/y.png)"]);
    expect("a{background:URL(/y.png)}".match(/(?<!new\s+)\burl\((?!["']?data:)[^)]*\)/gi)).toEqual(["URL(/y.png)"]);
    expect(html.match(/@import/gi) ?? []).toEqual([]);
    expect(html.match(NETWORK) ?? []).toEqual([]);
    // and the stand-in host isn't in it
    expect(html).not.toContain("stand-in host");
  });

  it.skipIf(!exists("dist/index.html"))("the app's build has neither the stand-in host page nor the view", () => {
    const files = fs.readdirSync(new URL("dist/", ROOT), { recursive: true }) as string[];
    expect(files.filter((f) => /mcp-host|mcp-app|widget-host/.test(f))).toEqual([]);
    for (const f of files.filter((f) => /\.(js|html)$/.test(f))) {
      const text = read(`dist/${f}`);
      expect(text.includes("Hersir stand-in host") || text.includes("Auto change: a card moves"), f).toBe(false);
    }
  });
});
