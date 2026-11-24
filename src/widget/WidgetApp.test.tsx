// GA-349 QA: Hersir's view in Claude Desktop's chat (src/widget/App.tsx with BoardView, CardView and ChiefView), as a host
// drives it. React itself renders (react-dom/client) into a small stand-in DOM: there is no DOM library in the project (as in
// components/BoardBulk.test.tsx). The host is a stand-in for Claude Desktop that speaks the bridge's JSON-RPC as
// src/widget-host/main.ts does, and answers every tool from the stand-in host's made-up Hersir (fakeBoard.ts, fakeCard.ts,
// fakeChief.ts). Timers are fake, so the view's poll (3 s; 1.5 s while the Chief works) is stepped through exactly.
// The app's Board (dnd-kit) is a stand-in that draws each column with its cards and keeps its props: a test drops a card
// with its onMove and opens one with its onOpen, as the Board does after a drag or a click. The drag itself is checked in
// the stand-in host page (GA-349's QA comment).
// - Display modes: none of its own at ui/initialize, nothing asked on load, full screen only after Open board or Open full
//   screen, and the view draws in the mode the host returns.
// - The board inline (columns with their total, Needs you in gold, Agents working in teal), the whole board, a drop that
//   calls move_task, a click on a card and Back.
// - The card (everything its page shows, the live output while a run works) and its Comment, Move to, Run and Stop; an
//   isError answer by its button.
// - The Chief's thread: ask_chief, chief_reply while it works, its draft as it comes.
// - Polling: board_changes every 3 s since the last seq, view_read only when that touches the view; a change in Hersir shows
//   within 5 s. The theme follows the host's.
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { FakeBoard, PROJECT } from "../widget-host/fakeBoard";
import { FakeCards } from "../widget-host/fakeCard";
import { DESKTOP_THREAD, FakeChief } from "../widget-host/fakeChief";
import type { BoardData } from "./views";

// ---- a stand-in DOM, as much of it as React DOM uses ----
const HTML = "http://www.w3.org/1999/xhtml";
type Listener = { f: (e: unknown) => void; capture: boolean };
class FakeNode {
  childNodes: FakeNode[] = [];
  parentNode: FakeNode | null = null;
  attrs = new Map<string, string>();
  listeners = new Map<string, Listener[]>();
  style: Record<string, unknown> = { setProperty(k: string, v: string) { (this as Record<string, unknown>)[k] = v; }, removeProperty() {} };
  nodeValue: string | null = null;
  dataset: Record<string, string> = {};
  scrollTop = 0;
  scrollHeight = 0;
  clientHeight = 0;
  declare value?: string;
  declare type?: string;
  declare checked?: boolean;
  constructor(public nodeType: number, public nodeName: string, public ownerDocument: FakeNode | null, public namespaceURI = HTML) {}
  get tagName() { return this.nodeName; }
  get firstChild(): FakeNode | null { return this.childNodes[0] ?? null; }
  get lastChild(): FakeNode | null { return this.childNodes[this.childNodes.length - 1] ?? null; }
  get nextSibling(): FakeNode | null { const p = this.parentNode; return p ? p.childNodes[p.childNodes.indexOf(this) + 1] ?? null : null; }
  get previousSibling(): FakeNode | null { const p = this.parentNode; return p ? p.childNodes[p.childNodes.indexOf(this) - 1] ?? null : null; }
  get textContent(): string { return this.nodeType === 3 ? this.nodeValue ?? "" : this.childNodes.map((c) => c.textContent).join(""); }
  set textContent(t: string) {
    for (const c of this.childNodes) c.parentNode = null;
    this.childNodes = [];
    if (t) this.appendChild(doc.createTextNode(t));
  }
  appendChild(c: FakeNode) { return this.insertBefore(c, null); }
  insertBefore(c: FakeNode, ref: FakeNode | null) {
    c.parentNode?.removeChild(c);
    const i = ref ? this.childNodes.indexOf(ref) : -1;
    if (i < 0) this.childNodes.push(c); else this.childNodes.splice(i, 0, c);
    c.parentNode = this;
    return c;
  }
  removeChild(c: FakeNode) { this.childNodes.splice(this.childNodes.indexOf(c), 1); c.parentNode = null; return c; }
  setAttribute(k: string, v: unknown) { this.attrs.set(k, String(v)); }
  setAttributeNS(_ns: string, k: string, v: unknown) { this.attrs.set(k, String(v)); }
  removeAttribute(k: string) { this.attrs.delete(k); }
  removeAttributeNS(_ns: string, k: string) { this.attrs.delete(k); }
  getAttribute(k: string) { return this.attrs.get(k) ?? null; }
  hasAttribute(k: string) { return this.attrs.has(k); }
  addEventListener(type: string, f: (e: unknown) => void, opt?: boolean | { capture?: boolean }) {
    const capture = typeof opt === "boolean" ? opt : !!opt?.capture;
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), { f, capture }]);
  }
  removeEventListener(type: string, f: (e: unknown) => void) { this.listeners.set(type, (this.listeners.get(type) ?? []).filter((l) => l.f !== f)); }
  contains(n: FakeNode | null) { for (let x = n; x; x = x.parentNode) if (x === this) return true; return false; }
  closest() { return null; }
  scrollIntoView() {}
  focus() {}
  blur() {}
  getBoundingClientRect() { return { top: 0, left: 0, right: 0, bottom: 0, width: 0, height: 0 }; }
  all(): FakeNode[] { return [this, ...this.childNodes.flatMap((c) => c.all())]; }
  createElement(tag: string) { return new FakeNode(1, tag.toUpperCase(), this); }
  createElementNS(ns: string, tag: string) { return new FakeNode(1, tag, this, ns); }
  createTextNode(t: string) { const n = new FakeNode(3, "#text", this); n.nodeValue = t; return n; }
  getElementById(id: string) { return this.all().find((n) => n.getAttribute("id") === id) ?? null; }
}
const doc = new FakeNode(9, "#document", null);
const html = doc.createElement("html");
const body = doc.createElement("body");
doc.appendChild(html);
html.appendChild(body);
(doc as unknown as { oninput: null }).oninput = null;
// The view's window: the bridge listens for the host's messages on it; its parent is the host.
const winListeners = new Map<string, ((e: unknown) => void)[]>();
const opened: string[] = [];
const prefersLight = { value: false };
const win = {
  document: doc, event: undefined, HTMLIFrameElement: class {}, devicePixelRatio: 1, innerWidth: 720, parent: null as unknown,
  addEventListener(type: string, f: (e: unknown) => void) { winListeners.set(type, [...(winListeners.get(type) ?? []), f]); },
  removeEventListener(type: string, f: (e: unknown) => void) { winListeners.set(type, (winListeners.get(type) ?? []).filter((x) => x !== f)); },
  matchMedia: (q: string) => ({ matches: q.includes("light") && prefersLight.value, addEventListener() {}, removeEventListener() {} }),
  requestAnimationFrame: (f: () => void) => { f(); return 0; },
  open: (url: string) => { opened.push(url); return null; },
};
Object.assign(doc, { body, documentElement: html, activeElement: body, defaultView: win, hidden: false, visibilityState: "visible" });
vi.stubGlobal("window", win);
vi.stubGlobal("document", doc);
vi.stubGlobal("requestAnimationFrame", win.requestAnimationFrame);
vi.stubGlobal("ResizeObserver", class { observe() {} disconnect() {} });
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

/** An event at `target`, through the listeners on the way as a browser calls them (capture down, then bubble up). */
function fire(target: FakeNode, type: string, extra: Record<string, unknown> = {}) {
  const path: FakeNode[] = [];
  for (let x: FakeNode | null = target; x; x = x.parentNode) path.push(x);
  let stopped = false;
  const e = { type, target, bubbles: true, button: 0, defaultPrevented: false, timeStamp: 0, shiftKey: false, ctrlKey: false, metaKey: false, altKey: false, ...extra,
    preventDefault() { e.defaultPrevented = true; }, stopPropagation() { stopped = true; } };
  for (const n of [...path].reverse()) for (const l of n.listeners.get(type) ?? []) if (l.capture && !stopped) l.f(e);
  for (const n of path) for (const l of n.listeners.get(type) ?? []) if (!l.capture && !stopped) l.f(e);
  return e;
}

// ---- the app's Board, as a stand-in that keeps its props ----
type BoardProps = {
  tasks: { id: string; identifier: string; stateId: string }[]; states: { id: string; name: string }[];
  working?: Map<string, string>; onMove: (id: string, stateId: string) => void; onOpen: (id: string) => void;
};
const h = vi.hoisted(() => ({ board: null as null | BoardProps }));
vi.mock("../components/Board", async () => {
  const { createElement } = await import("react");
  return {
    Board: (p: BoardProps) => {
      h.board = p;
      return createElement("div", { "data-board": "" }, p.states.map((s) => createElement("div", { key: s.id, "data-col": s.name },
        p.tasks.filter((t) => t.stateId === s.id).map((t) => createElement("div", { key: t.id, "data-card": t.identifier }, t.identifier)))));
    },
  };
});

const { createRoot } = await import("react-dom/client");
const { App, POLL_MS, CHIEF_POLL_MS } = await import("./App");

// ---- the host: Claude Desktop's side of the bridge, answering from the stand-in host's made-up Hersir ----
type Msg = { jsonrpc: "2.0"; id?: number | string; method?: string; params?: Record<string, unknown>; result?: unknown; error?: unknown };
type Args = Record<string, unknown>;
type Result = { content: { type: string; text: string }[]; isError?: boolean; structuredContent?: unknown };
const failed = (text: string): Result => ({ content: [{ type: "text", text }], isError: true });
const answered = (value: unknown): Result => ({ content: [{ type: "text", text: JSON.stringify(value) }], isError: false });
const WRITES = ["move_task", "comment_on_task", "start_agent_run", "stop_agent_run", "ask_chief"];

class Host {
  sent: Msg[] = [];
  board = new FakeBoard();
  cards = new FakeCards(this.board);
  chief = new FakeChief();
  theme: string | undefined = "dark";
  mode = "inline";
  /** What the host gives on ui/request-display-mode: the mode asked, or (refuse) inline. */
  refuseFullscreen = false;
  failWrites = false;
  /** show's arguments for this tool call. */
  show: Args = { view: "board", id: PROJECT.key };
  /** A tool's own answer instead of the made-up Hersir's (by name). */
  override: Record<string, (a: Args) => Result> = {};
  readonly win = { postMessage: (m: Msg) => { this.sent.push(m); void Promise.resolve().then(() => this.onMessage(m)); } };

  deliver(m: Msg) {
    for (const f of winListeners.get("message") ?? []) f({ data: m, source: this.win });
  }
  notify(method: string, params: unknown) { this.deliver({ jsonrpc: "2.0", method, params: params as Record<string, unknown> }); }
  /** The tools/call the view made, by name: their arguments, in order. */
  calls(name?: string): Args[] {
    return this.sent.filter((m) => m.method === "tools/call" && (!name || m.params?.name === name)).map((m) => m.params?.arguments as Args);
  }
  /** The names of the tools/call the view made, in order. */
  names(): string[] { return this.sent.filter((m) => m.method === "tools/call").map((m) => String(m.params?.name)); }
  requests(method: string): Msg[] { return this.sent.filter((m) => m.method === method); }

  /** show's or view_read's answer, as Hersir gives it. */
  view(tool: "show" | "view_read", a: Args): Result {
    const id = typeof a.id === "string" ? a.id : null;
    const out = (view: string, vid: string | null, data: unknown, text: string): Result => ({
      content: [{ type: "text", text }], isError: false,
      structuredContent: tool === "view_read" ? { view, id: vid, seq: this.board.seq, data } : { view, id: vid, data },
    });
    if (a.view === "board") {
      const data = this.board.data(id === null ? null : PROJECT.key);
      return out("board", data.project?.key ?? null, data, "The user sees the board.");
    }
    if (a.view === "card") {
      const data = id ? this.cards.data(id) : null;
      return data ? out("card", data.identifier, data, `${data.identifier} – ${data.title}, in ${data.column}.`) : failed(`There is no card ${id} in Hersir.`);
    }
    const data = this.chief.view(id);
    return typeof data === "string" ? failed(data) : out("chief", data.thread, data, "The user sees the Chief's chat.");
  }

  tool(name: string, a: Args): Result {
    if (this.override[name]) return this.override[name](a);
    if (name === "show" || name === "view_read") return this.view(name, a);
    if (name === "board_changes") {
      const c = this.board.since(a.since as number | undefined);
      return { content: [{ type: "text", text: JSON.stringify(c) }], structuredContent: c, isError: false };
    }
    if (name === "chief_reply") {
      const r = this.chief.reply(a.thread as string | undefined, a.after as string | undefined);
      return typeof r === "string" ? failed(r) : answered(r);
    }
    if (!WRITES.includes(name)) throw new Error(`unknown tool ${name}`);
    if (this.failWrites) return failed(`Hersir didn't do ${name}: refused.`);
    if (name === "ask_chief") {
      const s = this.chief.ask(String(a.message ?? ""), a.thread as string | undefined);
      return typeof s === "string" ? failed(s) : answered(s);
    }
    const task = String(a.task ?? "");
    const why = name === "move_task" ? this.board.move(task, String(a.column ?? ""))
      : name === "comment_on_task" ? this.cards.comment(task, String(a.body_md ?? ""))
      : name === "start_agent_run" ? this.cards.start(task, a.agent as string | undefined)
      : this.cards.stop(task);
    return why ? failed(why) : answered({ task, done: name });
  }

  onMessage(m: Msg) {
    const answer = (result: unknown) => this.deliver({ jsonrpc: "2.0", id: m.id, result });
    if (m.method === "ui/initialize") {
      return answer({ protocolVersion: "2026-01-26", hostInfo: { name: "test host", version: "1" }, hostCapabilities: { serverTools: {}, openLinks: {} },
        hostContext: { ...(this.theme ? { theme: this.theme } : {}), displayMode: this.mode, availableDisplayModes: ["inline", "fullscreen"] } });
    }
    if (m.method === "ui/notifications/initialized") {
      this.notify("ui/notifications/tool-input", { arguments: this.show });
      this.notify("ui/notifications/tool-result", this.tool("show", this.show));
      return;
    }
    if (m.method === "tools/call") return answer(this.tool(String(m.params?.name), (m.params?.arguments ?? {}) as Args));
    if (m.method === "ui/request-display-mode") {
      const want = m.params?.mode === "fullscreen" && !this.refuseFullscreen ? "fullscreen" : "inline";
      if (want !== this.mode) {
        this.mode = want;
        this.notify("ui/notifications/host-context-changed", { displayMode: want });
      }
      return answer({ mode: want });
    }
    if (m.method === "ui/open-link") return answer({});
    // size-changed and the rest: nothing to answer
  }
}

let host: Host;
let container: FakeNode;
let root: ReturnType<typeof createRoot> | null = null;

/** Time passes (the poll's timers run) and the host's answers come in. */
async function wait(ms = 0) {
  await act(async () => { await vi.advanceTimersByTimeAsync(ms); });
  await act(async () => { await vi.advanceTimersByTimeAsync(0); });
}
async function mount(show: Args = { view: "board", id: PROJECT.key }) {
  host.show = show;
  await act(async () => { root!.render(<App />); });
  await wait();
}

const cls = (n: FakeNode) => (n.getAttribute("class") ?? "").split(" ").filter(Boolean);
const find = (c: string, from: FakeNode = container) => from.all().filter((n) => cls(n).includes(c));
const one = (c: string, from: FakeNode = container) => find(c, from)[0];
const text = (c: string, from: FakeNode = container) => one(c, from)?.textContent ?? null;
const button = (label: string, from: FakeNode = container) =>
  from.all().find((n) => n.nodeName === "BUTTON" && n.textContent.trim() === label);
const buttons = (from: FakeNode = container) => from.all().filter((n) => n.nodeName === "BUTTON").map((n) => n.textContent.trim());
async function click(n: FakeNode | undefined) {
  if (!n) throw new Error("nothing to click");
  await act(async () => { fire(n, "click"); });
  await wait();
}
async function type(box: FakeNode, value: string) {
  await act(async () => { box.value = value; fire(box, "input"); });
}
const textarea = () => container.all().find((n) => n.nodeName === "TEXTAREA");
/** The board's summary: the column chips as "name n". */
const chips = () => find("mcp-column").map((c) => `${text("name", c)} ${text("n", c)}`);
const listed = (kind: "needs" | "live") => {
  const list = container.all().find((n) => cls(n).includes("mcp-list") && cls(n).includes(kind));
  return list ? find("id", list).map((n) => n.textContent) : [];
};
/** The whole board (the Board stand-in): its columns, each with its cards. */
const whole = () => container.all().filter((n) => n.getAttribute("data-col") !== null)
  .map((c) => `${c.getAttribute("data-col")}: ${c.childNodes.map((x) => x.getAttribute("data-card")).join(",")}`);
const columnOf = (id: string) => host.board.find(id)?.column;
/** The card view's property, by its label (Column, Assignee, …). */
const prop = (label: string) => {
  const dt = container.all().find((n) => n.nodeName === "DT" && n.textContent === label);
  return dt?.nextSibling?.textContent ?? null;
};
/** The error shown right after an element with class `after` (the button's block), or null. */
const errorAfter = (after: string) => {
  const e = find("mcp-error").find((n) => n.previousSibling && cls(n.previousSibling).includes(after));
  return e?.textContent ?? null;
};

beforeEach(() => {
  vi.useFakeTimers({ toFake: ["setTimeout", "clearTimeout", "setInterval", "clearInterval", "Date"] });
  vi.setSystemTime(new Date("2026-11-23T14:00:00Z"));
  host = new Host();
  win.parent = host.win;
  winListeners.clear();
  opened.length = 0;
  prefersLight.value = false;
  h.board = null;
  html.attrs.clear();
  container = doc.createElement("div");
  body.appendChild(container);
  root = createRoot(container as unknown as HTMLElement);
});
afterEach(async () => {
  await act(async () => { root!.unmount(); });
  root = null;
  container.parentNode?.removeChild(container);
  vi.useRealTimers();
});

// ---- display modes and the theme ----

describe("display modes: the host's, full screen only after a click (GA-349)", () => {
  it("lists no display mode of its own at ui/initialize, and asks for none on load nor while it polls", async () => {
    await mount();
    const init = host.requests("ui/initialize");
    expect(init).toHaveLength(1);
    expect(init[0].params?.appCapabilities).toEqual({});
    expect(host.requests("ui/notifications/initialized")).toHaveLength(1);
    await wait(4 * POLL_MS);
    expect(host.requests("ui/request-display-mode")).toEqual([]);
    for (const show of [{ view: "card", id: "GA-109" }, { view: "chief" }, { view: "board" }]) {
      await act(async () => { root!.unmount(); });
      root = createRoot(container as unknown as HTMLElement);
      host.sent.length = 0;
      await mount(show);
      await wait(2 * POLL_MS);
      expect(host.requests("ui/request-display-mode"), JSON.stringify(show)).toEqual([]);
    }
  });

  it("Open board asks for full screen on its click and draws the whole board; Back to chat asks for inline and the summary is back", async () => {
    await mount();
    expect(find("mcp-summary")).toHaveLength(1);
    expect(container.all().some((n) => n.getAttribute("data-board") !== null)).toBe(false);
    await click(button("Open board"));
    expect(host.requests("ui/request-display-mode").map((m) => m.params)).toEqual([{ mode: "fullscreen" }]);
    expect(cls(one("mcp-app"))).toContain("fullscreen");
    expect(find("mcp-summary")).toHaveLength(0);
    expect(whole().map((c) => c.split(":")[0])).toEqual(["Backlog", "To do", "In progress", "Testing", "Review", "Done"]);
    expect(whole()[1]).toBe("To do: GA-103,GA-104,GA-105,GA-114");
    expect(button("Open board")).toBeUndefined();
    await click(button("Back to chat"));
    expect(host.requests("ui/request-display-mode").map((m) => m.params)).toEqual([{ mode: "fullscreen" }, { mode: "inline" }]);
    expect(cls(one("mcp-app"))).toContain("inline");
    expect(find("mcp-summary")).toHaveLength(1);
    expect(button("Open board")).toBeDefined();
  });

  it("when the host keeps it inline, Open board draws the whole board inline, with Show the summary", async () => {
    host.refuseFullscreen = true;
    await mount();
    await click(button("Open board"));
    expect(host.requests("ui/request-display-mode").map((m) => m.params)).toEqual([{ mode: "fullscreen" }]);
    expect(cls(one("mcp-app"))).toContain("inline");
    expect(whole()).toHaveLength(6);
    await click(button("Show the summary"));
    expect(find("mcp-summary")).toHaveLength(1);
  });

  it("a card and the Chief's thread ask for full screen only on Open full screen; a card that isn't there offers none", async () => {
    await mount({ view: "card", id: "GA-109" });
    expect(host.requests("ui/request-display-mode")).toEqual([]);
    await click(button("Open full screen"));
    expect(host.requests("ui/request-display-mode").map((m) => m.params)).toEqual([{ mode: "fullscreen" }]);
    expect(cls(one("mcp-app"))).toContain("fullscreen");
    expect(one("mcp-card")).toBeDefined();

    await act(async () => { root!.unmount(); });
    root = createRoot(container as unknown as HTMLElement);
    host = new Host();
    win.parent = host.win;
    await mount({ view: "card", id: "GA-404" });
    expect(text("mcp-note")).toBe("There is no card GA-404 in Hersir.");
    expect(button("Open full screen")).toBeUndefined();
  });
});

describe("the theme follows the host's, dark first (GA-349)", () => {
  it("takes the host's theme at ui/initialize and each change after it", async () => {
    host.theme = "light";
    await mount();
    expect(html.getAttribute("data-theme")).toBe("light");
    await act(async () => { host.notify("ui/notifications/host-context-changed", { theme: "dark" }); });
    expect(html.getAttribute("data-theme")).toBe("dark");
    expect((html.style as { colorScheme?: string }).colorScheme).toBe("dark");
  });

  it("dark when the host says no theme and the system has no light preference; light only with one", async () => {
    host.theme = undefined;
    await mount();
    expect(html.getAttribute("data-theme")).toBe("dark");
    await act(async () => { root!.unmount(); });
    root = createRoot(container as unknown as HTMLElement);
    prefersLight.value = true;
    await mount();
    expect(html.getAttribute("data-theme")).toBe("light");
  });
});

// ---- the board ----

/** A board whose To do column has more cards than were sent (Hersir sends 50 at most). */
const BIG: BoardData = {
  team: "My team", project: { key: "GA", name: "Hersir" },
  columns: [
    { name: "To do", kind: "waiting", total: 60, cards: [
      { identifier: "GA-1", title: "Waits for you", project: "Hersir", assignee: null, labels: [], priority: 0, hold: "needs_decision", running: false, needs_you: true },
      { identifier: "GA-2", title: "Plain", project: "Hersir", assignee: null, labels: [], priority: 0, hold: null, running: false, needs_you: false },
    ] },
    { name: "In progress", kind: "work", total: 1, cards: [
      { identifier: "GA-3", title: "Worked on", project: "Hersir", assignee: "Backend Agent", labels: [], priority: 0, hold: null, running: true, needs_you: false },
    ] },
    { name: "Review", kind: "review", total: 1, cards: [
      { identifier: "GA-4", title: "Your review", project: "Hersir", assignee: null, labels: [], priority: 0, hold: null, running: false, needs_you: true },
    ] },
  ],
};

describe("the board inline: each column with its count, what needs you and what agents work on (GA-349)", () => {
  it("draws each column with its total, Needs you and Agents working, and Open board", async () => {
    host.override.show = () => ({ content: [{ type: "text", text: "The board." }], structuredContent: { view: "board", id: "GA", data: BIG } });
    host.override.view_read = () => ({ content: [{ type: "text", text: "The board." }], structuredContent: { view: "board", id: "GA", seq: 5, data: BIG } });
    await mount();
    // a column's count is its total, not the cards sent
    expect(chips()).toEqual(["To do 60", "In progress 1", "Review 1"]);
    expect(listed("needs")).toEqual(["GA-1", "GA-4"]);
    expect(listed("live")).toEqual(["GA-3"]);
    expect(text("mcp-title")).toBe("Hersir (GA)");
    const why = find("mcp-row-why").map((n) => n.textContent);
    expect(why).toEqual(["On hold: needs your decision", "In Review", "In progress · Backend Agent"]);
    expect(button("Open board")).toBeDefined();
    expect(button("Back")).toBeUndefined();
  });

  it("gold only for what needs you, teal only for what an agent works on now", async () => {
    await mount();
    const needs = container.all().find((n) => cls(n).includes("mcp-list") && cls(n).includes("needs"))!;
    const live = container.all().find((n) => cls(n).includes("mcp-list") && cls(n).includes("live"))!;
    // the made-up board: GA-109 on hold, GA-110 in Review; GA-106 and GA-107 worked on
    expect(listed("needs").sort()).toEqual(["GA-109", "GA-110"]);
    expect(listed("live").sort()).toEqual(["GA-106", "GA-107"]);
    // every pulse (teal) is in the Agents working list or its heading; every gold mark in Needs you's
    const section = (n: FakeNode) => { let x: FakeNode | null = n; while (x && !cls(x).includes("mcp-section")) x = x.parentNode; return x; };
    for (const p of find("pulse")) expect(section(p)?.contains(live)).toBe(true);
    for (const d of find("mcp-dot")) expect(needs.contains(d)).toBe(true);
    for (const n of container.all().filter((x) => cls(x).includes("needs") || cls(x).includes("live"))) {
      const s = section(n);
      const kind = cls(n).includes("needs") ? needs : live;
      expect(s?.contains(kind), cls(n).join(" ")).toBe(true);
    }
  });

  it("a card dropped in another column moves there at once, calls move_task, and stays after the next read", async () => {
    await mount();
    await click(button("Open board"));
    const inProgress = h.board!.states.find((s) => s.name === "In progress")!;
    expect(whole()[1]).toBe("To do: GA-103,GA-104,GA-105,GA-114");
    await act(async () => { h.board!.onMove("GA-103", inProgress.id); });
    expect(whole()[1]).toBe("To do: GA-104,GA-105,GA-114");
    expect(whole()[2]).toMatch(/^In progress: .*GA-103$/);
    await wait();
    expect(host.calls("move_task")).toEqual([{ task: "GA-103", column: "In progress" }]);
    // read again right after the write, and the card keeps its new column
    expect(host.names().slice(-2)).toEqual(["move_task", "view_read"]);
    expect(columnOf("GA-103")).toBe("In progress");
    await wait(POLL_MS);
    expect(whole()[2]).toMatch(/GA-103/);
    expect(whole()[1]).toBe("To do: GA-104,GA-105,GA-114");
    // a drop in its own column moves nothing
    await act(async () => { h.board!.onMove("GA-114", h.board!.states.find((s) => s.name === "To do")!.id); });
    await wait();
    expect(host.calls("move_task")).toHaveLength(1);
  });

  it("a drop that Hersir refuses says why above the board, and the card goes back", async () => {
    await mount();
    await click(button("Open board"));
    host.failWrites = true;
    const testing = h.board!.states.find((s) => s.name === "Testing")!;
    await act(async () => { h.board!.onMove("GA-114", testing.id); });
    await wait();
    expect(host.calls("move_task")).toEqual([{ task: "GA-114", column: "Testing" }]);
    expect(text("mcp-error")).toBe("Couldn't move GA-114 to Testing: Hersir didn't do move_task: refused.");
    expect(whole()[1]).toBe("To do: GA-103,GA-104,GA-105,GA-114");
    expect(whole()[3]).not.toMatch(/GA-114/);
  });
});

describe("polling: board_changes every 3 s, view_read only when it touches the view (GA-349)", () => {
  it("asks board_changes since the last read's seq every 3 s, and reads the board again only after a change", async () => {
    expect(POLL_MS).toBe(3000);
    await mount();
    expect(host.names()).toEqual([]);
    // show's result has no seq: the first poll reads the view
    await wait(POLL_MS);
    expect(host.names()).toEqual(["view_read"]);
    expect(host.calls("view_read")).toEqual([{ view: "board", id: "GA" }]);
    await wait(POLL_MS);
    await wait(POLL_MS);
    expect(host.names()).toEqual(["view_read", "board_changes", "board_changes"]);
    expect(host.calls("board_changes")).toEqual([{ since: 1000 }, { since: 1000 }]);
    // nothing sooner than 3 s
    await wait(POLL_MS - 100);
    expect(host.names()).toHaveLength(3);
    await wait(100);
    expect(host.names()).toHaveLength(4);
  });

  it("a card moved in Hersir shows in the board's summary within 5 s", async () => {
    await mount();
    await wait(POLL_MS);
    const before = chips();
    // the made-up board's first change: GA-104 (To do) moves on to In progress, where an agent works on it
    host.board.change();
    const moved = "GA-104";
    expect(columnOf(moved)).toBe("In progress");
    await wait(POLL_MS);
    expect(host.names().slice(-2)).toEqual(["board_changes", "view_read"]);
    expect(chips()).not.toEqual(before);
    expect(chips()).toContain("In progress 3");
    expect(chips()).toContain("To do 3");
    expect(listed("live")).toContain(moved);
    // and the next poll asks since the new seq
    await wait(POLL_MS);
    expect(host.calls("board_changes").at(-1)).toEqual({ since: 1001 });
  });

  it("a board of another project, or a card that is on no board on show, isn't read again", async () => {
    await mount();
    await wait(POLL_MS);
    host.override.board_changes = () => {
      const c = { seq: 1003, cards: ["XY-1"], board: false, reload: false };
      return { content: [{ type: "text", text: JSON.stringify(c) }], structuredContent: c };
    };
    await wait(POLL_MS);
    expect(host.names().slice(-1)).toEqual(["board_changes"]);
    // too much changed: read again
    host.override.board_changes = () => {
      const c = { seq: 1004, cards: [], board: false, reload: true };
      return { content: [{ type: "text", text: JSON.stringify(c) }], structuredContent: c };
    };
    await wait(POLL_MS);
    expect(host.names().slice(-2)).toEqual(["board_changes", "view_read"]);
  });
});

// ---- a card ----

describe("a card: what its page shows, and its live output while a run works (GA-349)", () => {
  it("draws the title, column, assignee, labels, priority, hold and its reason, description, acceptance criteria, pull request, comments and runs", async () => {
    await mount({ view: "card", id: "GA-109" });
    expect(text("mcp-title")).toBe("GA-109");
    expect(text("mcp-card-title")).toBe("Keyboard shortcut for a new card");
    expect(prop("Column")).toBe("Review");
    expect(prop("Assignee")).toContain("QA Agent");
    expect(prop("Priority")).toBe("None");
    expect(prop("Labels")).toBe("backend");
    expect(prop("Pull request")).toBe("PR #309");
    const hold = one("mcp-hold");
    expect(hold.textContent).toContain("On hold: needs your decision");
    // the reason and the texts drawn as Markdown (the app's MarkdownView: a .prose block)
    expect(find("prose", hold)).toHaveLength(1);
    expect(hold.all().some((n) => n.nodeName === "STRONG" && n.textContent === "N")).toBe(true);
    const blocks = find("mcp-block");
    const block = (title: string) => blocks.find((b) => b.childNodes[0]?.textContent.startsWith(title))!;
    expect(block("Description").all().some((n) => n.nodeName === "H2" && n.textContent === "Why")).toBe(true);
    expect(block("Description").all().some((n) => n.nodeName === "CODE" && n.textContent === "Ctrl+N")).toBe(true);
    const criteria = block("Acceptance criteria").all().filter((n) => n.nodeName === "LI").map((n) => n.textContent.trim());
    expect(criteria).toEqual(["The button shows on the page", "A click does what its label says", "Texts in sentence case"]);
    const comments = find("comment", block("Comments"));
    expect(comments.map((c) => text("by", c)?.split(/\d|yesterday/)[0])).toEqual(["Jefsev", "Frontend Agent"]);
    expect(comments[1].all().some((n) => n.nodeName === "OL")).toBe(true);
    expect(find("mcp-run", block("Runs")).map((r) => r.textContent)).toEqual(["Frontend AgentReady for testingended 3h agoAdded the button and its shortcut. Pushed; the tests are QA's."]);
    // no agent works on it: no live output, Run and not Stop
    expect(one("mcp-output")).toBeUndefined();
    expect(buttons()).toEqual(expect.arrayContaining(["Comment", "Move to", "Run", "Open full screen"]));
    expect(button("Stop")).toBeUndefined();
  });

  it("a card no agent works on, with none of the optional parts, says so plainly", async () => {
    // GA-110: no assignee, no hold, no acceptance criteria (every fifth card), comments and a run
    await mount({ view: "card", id: "GA-110" });
    expect(prop("Assignee")).toBe("No one");
    expect(one("mcp-hold")).toBeUndefined();
    const acceptance = find("mcp-block").find((b) => b.childNodes[0]?.textContent === "Acceptance criteria")!;
    expect(acceptance.childNodes[1].textContent).toBe("None.");
  });

  it("while an agent works on it: its live output, a teal Live badge and Stop; read every 3 s for its new lines", async () => {
    await mount({ view: "card", id: "GA-106" });
    const out = () => text("mcp-output") ?? "";
    expect(out().split("\n")[0]).toBe("Started · claude-opus-5-5");
    expect(text("mcp-live")).toContain("is working");
    expect(cls(one("badge", one("task-head")))).toEqual(["badge", "live"]);
    expect(button("Stop")).toBeDefined();
    expect(button("Run")).toBeUndefined();
    const lines = out().split("\n").length;
    await wait(POLL_MS);
    await wait(POLL_MS);
    // board_changes doesn't count a live run's output: the card is read every time instead
    expect(host.names()).toEqual(["view_read", "view_read"]);
    expect(out().split("\n").length).toBeGreaterThan(lines);
  });

  it("a comment made in Hersir shows in the open card within 5 s", async () => {
    await mount({ view: "card", id: "GA-109" });
    await wait(POLL_MS);
    const count = () => find("comment").length;
    const before = count();
    host.cards.comment("GA-109", "Written in Hersir.", "Jefsev");
    await wait(POLL_MS);
    expect(host.names().slice(-2)).toEqual(["board_changes", "view_read"]);
    expect(count()).toBe(before + 1);
    expect(find("comment").at(-1)?.textContent).toContain("Written in Hersir.");
  });

  it("the pull request and a web link in the Markdown open through the chat (ui/open-link), not in the view", async () => {
    await mount({ view: "card", id: "GA-109" });
    const links = container.all().filter((n) => n.nodeName === "A");
    const pr = links.find((a) => a.textContent.startsWith("PR #309"))!;
    await click(pr);
    const design = links.find((a) => a.textContent === "the design")!;
    await click(design);
    expect(host.requests("ui/open-link").map((m) => m.params)).toEqual([
      { url: "https://github.com/example/hersir/pull/309" }, { url: "https://example.com/design" }]);
    expect(opened).toEqual([]);
  });
});

describe("a card's Comment, Move to, Run and Stop (GA-349)", () => {
  it("Comment posts with comment_on_task, the card is read again at once, and the comment shows", async () => {
    await mount({ view: "card", id: "GA-109" });
    const before = find("comment").length;
    await click(button("Comment"));
    expect(button("Post comment")?.attrs.has("disabled")).toBe(true);
    await type(textarea()!, "  Go with **N**.  ");
    await click(button("Post comment"));
    expect(host.calls("comment_on_task")).toEqual([{ task: "GA-109", body_md: "Go with **N**." }]);
    // within the same tick: no 3 s wait
    expect(host.names()).toEqual(["comment_on_task", "view_read"]);
    expect(find("comment")).toHaveLength(before + 1);
    expect(find("comment").at(-1)?.all().some((n) => n.nodeName === "STRONG" && n.textContent === "N")).toBe(true);
    expect(textarea()).toBeUndefined();
  });

  it("Move to lists the card's board's other columns, and a pick calls move_task; the card shows its new column", async () => {
    await mount({ view: "card", id: "GA-109" });
    await click(button("Move to"));
    // the card's board isn't on show: its columns are read once, from its project's board
    expect(host.calls("view_read")).toEqual([{ view: "board", id: "GA" }]);
    const picks = one("mcp-move").all().filter((n) => n.nodeName === "BUTTON").map((n) => n.textContent);
    expect(picks).toEqual(["Backlog", "To do", "In progress", "Testing", "Done"]);
    await click(button("Testing", one("mcp-move")));
    expect(host.calls("move_task")).toEqual([{ task: "GA-109", column: "Testing" }]);
    expect(host.names().slice(-2)).toEqual(["move_task", "view_read"]);
    expect(prop("Column")).toBe("Testing");
    expect(one("mcp-move")).toBeUndefined();
  });

  it("Move to on a card opened from its board takes the columns from that board", async () => {
    await mount();
    await click(find("mcp-row").find((r) => r.textContent.includes("GA-109")));
    const reads = host.calls("view_read").length;
    await click(button("Move to"));
    expect(host.calls("view_read")).toHaveLength(reads);
    expect(one("mcp-move").all().filter((n) => n.nodeName === "BUTTON").map((n) => n.textContent)).not.toContain("Review");
  });

  it("Run starts the agent with start_agent_run and Stop stops it with stop_agent_run, each shown at once", async () => {
    await mount({ view: "card", id: "GA-109" });
    await click(button("Run"));
    expect(host.calls("start_agent_run")).toEqual([{ task: "GA-109" }]);
    expect(host.names()).toEqual(["start_agent_run", "view_read"]);
    expect(one("mcp-output")).toBeDefined();
    expect(text("mcp-run")).toContain("Working");
    await click(button("Stop"));
    expect(host.calls("stop_agent_run")).toEqual([{ task: "GA-109" }]);
    expect(host.names().slice(-2)).toEqual(["stop_agent_run", "view_read"]);
    expect(one("mcp-output")).toBeUndefined();
    expect(button("Run")).toBeDefined();
    expect(text("mcp-run")).toContain("Stopped by you.");
  });

  it("an isError answer shows its text by its button, and a comment's text stays to try again", async () => {
    await mount({ view: "card", id: "GA-109" });
    host.failWrites = true;
    await click(button("Run"));
    expect(errorAfter("mcp-actions")).toBe("Hersir didn't do start_agent_run: refused.");
    await click(button("Move to"));
    await click(button("Testing", one("mcp-move")));
    expect(errorAfter("mcp-move")).toBe("Hersir didn't do move_task: refused.");
    expect(prop("Column")).toBe("Review");
    await click(button("Comment"));
    await type(textarea()!, "Try this");
    await click(button("Post comment"));
    expect(errorAfter("mcp-compose")).toBe("Hersir didn't do comment_on_task: refused.");
    expect(textarea()?.value).toBe("Try this");
    expect(find("mcp-error").map((e) => e.getAttribute("role"))).toEqual(["alert", "alert", "alert"]);
    // no read after a refused write
    expect(host.calls("view_read")).toEqual([{ view: "board", id: "GA" }]);
    // the same button again, now that Hersir takes it: its error goes
    host.failWrites = false;
    await click(button("Run"));
    expect(errorAfter("mcp-actions")).toBeNull();
  });
});

// ---- a click on a card, and Back ----

describe("a click on a card on the board opens it here, and Back returns to the board (GA-349)", () => {
  it("from the summary: the card's view, then Back to the summary", async () => {
    await mount();
    await click(find("mcp-row").find((r) => r.textContent.includes("GA-110")));
    expect(host.calls("view_read").at(-1)).toEqual({ view: "card", id: "GA-110" });
    expect(text("mcp-card-title")).toBe("Drag cards between teams");
    expect(text("mcp-title")).toBe("GA-110");
    await click(button("Back"));
    expect(host.calls("view_read").at(-1)).toEqual({ view: "board", id: "GA" });
    expect(find("mcp-summary")).toHaveLength(1);
    expect(button("Back")).toBeUndefined();
  });

  it("from the whole board: the card's view, then Back to the whole board", async () => {
    await mount();
    await click(button("Open board"));
    await act(async () => { h.board!.onOpen("GA-107"); });
    await wait();
    expect(text("mcp-card-title")).toBe("Label colours in the board's filter");
    expect(cls(one("mcp-app"))).toContain("fullscreen");
    await click(button("Back"));
    expect(whole()).toHaveLength(6);
  });

  it("an answer for the place left behind is dropped: Back while the card is read keeps the board", async () => {
    await mount();
    await wait(POLL_MS);
    // the card's read is answered only after Back
    let release: (() => void) | null = null;
    const slow = host.onMessage.bind(host);
    host.onMessage = (m: Msg) => {
      if (m.method === "tools/call" && (m.params?.arguments as Args)?.view === "card") { release = () => slow(m); return; }
      slow(m);
    };
    await act(async () => { fire(find("mcp-row").find((r) => r.textContent.includes("GA-110"))!, "click"); });
    await wait();
    await click(button("Back"));
    await act(async () => { release?.(); });
    await wait();
    expect(find("mcp-summary")).toHaveLength(1);
    expect(one("mcp-card")).toBeUndefined();
  });
});

// ---- the Chief's thread ----

describe("the Chief's thread: its messages, ask_chief, and the reply as it comes in (GA-349)", () => {
  it("before the first message: no thread and no messages; Send calls ask_chief with the message alone", async () => {
    await mount({ view: "chief" });
    expect(text("mcp-title")).toBe("Chief");
    expect(text("mcp-empty")).toMatch(/^No messages yet\./);
    expect(text("mcp-chief-head")).toBe("Answered");
    expect(button("Send")?.attrs.has("disabled")).toBe(true);
    await type(textarea()!, "What is left for 0.20.0?");
    await click(button("Send"));
    expect(host.calls("ask_chief")).toEqual([{ message: "What is left for 0.20.0?" }]);
    expect(textarea()?.value).toBe("");
    expect(find("mcp-msg").filter((m) => cls(m).includes("from-you")).map((m) => text("mcp-plain", m))).toEqual(["What is left for 0.20.0?"]);
    expect(text("mcp-chief-head")).toBe("The Chief is working");
  });

  it("while the Chief works: chief_reply {thread, wait_seconds: 0} every 1.5 s, its draft growing; then its answer, and no more chief_reply", async () => {
    expect(CHIEF_POLL_MS).toBe(1500);
    await mount({ view: "chief" });
    await type(textarea()!, "What is left?");
    await click(button("Send"));
    const draft = () => one("draft")?.textContent ?? null;
    const lengths: number[] = [];
    for (let i = 0; i < 4; i++) {
      await wait(CHIEF_POLL_MS);
      lengths.push(draft()?.length ?? -1);
    }
    const replies = host.calls("chief_reply");
    expect(replies.length).toBeGreaterThanOrEqual(3);
    for (const r of replies) expect(r).toEqual({ thread: DESKTOP_THREAD, wait_seconds: 0 });
    expect(lengths.every((n) => n > 0)).toBe(true);
    expect(lengths[3]).toBeGreaterThan(lengths[0]);
    expect(cls(one("badge", one("mcp-chief-head")))).toEqual(["badge", "live"]);
    // the turn ends (8 s in the made-up Chief)
    await wait(6000);
    expect(draft()).toBeNull();
    expect(text("mcp-chief-head")).toBe("Answered");
    const chiefSaid = find("mcp-msg").filter((m) => cls(m).includes("from-chief"));
    expect(chiefSaid).toHaveLength(1);
    expect(chiefSaid[0].textContent).toContain("Here is what I see on the board");
    const n = host.calls("chief_reply").length;
    await wait(2 * POLL_MS);
    expect(host.calls("chief_reply")).toHaveLength(n);
    expect(host.names().slice(-2)).toEqual(["view_read", "view_read"]);
  });

  it("a message typed in Hersir's chat, and the Chief at work on it, show within 5 s", async () => {
    await mount({ view: "chief" });
    await wait(POLL_MS);
    host.chief.fromHersir("A message typed in Hersir.");
    await wait(POLL_MS);
    expect(find("mcp-msg").some((m) => m.textContent.includes("A message typed in Hersir."))).toBe(true);
    expect(one("draft")).toBeDefined();
    // the read that first sees the Chief working waits a whole poll before chief_reply (App reads chiefRef before React drew)
    await wait(POLL_MS);
    expect(host.calls("chief_reply").at(-1)).toEqual({ thread: DESKTOP_THREAD, wait_seconds: 0 });
  });

  it("a thread named by show: ask_chief goes to it; an error answer shows its text by Send, and the message stays", async () => {
    host.chief.ask("First.");
    await wait(9000);
    await mount({ view: "chief", id: DESKTOP_THREAD });
    expect(find("mcp-msg").length).toBe(2);
    host.override.ask_chief = () => failed("The Chief can't answer: it has no model set.");
    await type(textarea()!, "Second.");
    await click(button("Send"));
    expect(host.calls("ask_chief")).toEqual([{ message: "Second.", thread: DESKTOP_THREAD }]);
    expect(errorAfter("mcp-compose-acts")).toBe("The Chief can't answer: it has no model set.");
    expect(textarea()?.value).toBe("Second.");
  });
});

// ---- Hersir closed ----

describe("Hersir closed (GA-349)", () => {
  it("show's error is said, and the poll reads again until Hersir answers", async () => {
    const NOT = "Open Hersir first: Claude Desktop reaches your board through the Hersir app on this computer, and it isn't running.";
    host.override.show = () => failed(NOT);
    host.override.view_read = () => failed(NOT);
    await mount();
    expect(text("mcp-note")).toBe(NOT);
    await wait(POLL_MS);
    expect(host.names()).toEqual(["view_read"]);
    delete host.override.view_read;
    // slower after a failure, but it reads again
    await wait(2 * POLL_MS);
    expect(host.names().at(-1)).toBe("view_read");
    expect(text("mcp-note")).toBeNull();
    expect(chips()).toHaveLength(6);
  });
});
