// GA-190 QA: the Tasks page keeps its filter on this device (localStorage) once typing in the search pauses, not per
// keystroke; a pick in Filters writes at once; leaving the page writes what still waits, so nothing is lost. By design the
// search text itself is kept only while the page shows: what is stored is the filter without its text. GA-372: the project
// is no longer kept at all; it is the page's address (#/tasks/project/<id>), so the page here is drawn as App draws it
// (Routed below), its project following the address the page goes to. React itself renders (react-dom/client) into a small stand-in DOM (there is no DOM library in
// the project), with the real search box, Filters popover and task list; the data hooks answer at once, and timers are
// fake. Typing goes through React's own listeners, an input event per character.
import { act, useState, type ReactNode } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Task } from "../types";

// ---- a stand-in DOM, as much of it as React DOM uses (as in components/StreamParses.test.tsx) ----
const HTML = "http://www.w3.org/1999/xhtml";
/** GA-300: the Tasks toolbar's width, and what it needs at fold level 0, 1 and 2; wide enough by default. */
const bar = { have: 0, need: [0, 0, 0] };
type Listener = { f: (e: unknown) => void; capture: boolean };
class FakeNode {
  childNodes: FakeNode[] = [];
  parentNode: FakeNode | null = null;
  attrs = new Map<string, string>();
  listeners = new Map<string, Listener[]>();
  style: Record<string, unknown> = { setProperty(k: string, v: string) { (this as Record<string, unknown>)[k] = v; }, removeProperty() {} };
  nodeValue: string | null = null;
  // data-* through dataset, and widths, as the Tasks toolbar measures itself to fold (GA-300, useFold): the toolbar is
  // bar.have wide and needs bar.need[its data-fold] (its CSS isn't here); everything else fits
  dataset: Record<string, string> = {};
  get scrollWidth() { return this.isBar() ? bar.need[Number(this.dataset.fold ?? 0)] ?? 0 : 0; }
  get clientWidth() { return this.isBar() ? bar.have : 0; }
  isBar() { return (this.getAttribute("class") ?? "").split(" ").includes("island-toolbar"); }
  // an <input>'s value and type, as React sets and reads them
  declare value?: string;
  declare type?: string;
  // a <select>'s options, as React marks the one picked (the Team select, with two teams: GA-372's tests)
  get options(): FakeNode[] { return this.all().filter((n) => n.nodeName === "OPTION"); }
  constructor(public nodeType: number, public nodeName: string, public ownerDocument: FakeNode | null, public namespaceURI = HTML) {}
  get tagName() { return this.nodeName; }
  get firstChild(): FakeNode | null { return this.childNodes[0] ?? null; }
  get lastChild(): FakeNode | null { return this.childNodes[this.childNodes.length - 1] ?? null; }
  get nextSibling(): FakeNode | null { const p = this.parentNode; return p ? p.childNodes[p.childNodes.indexOf(this) + 1] ?? null : null; }
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
  /** This node and every node under it, in document order. */
  all(): FakeNode[] { return [this, ...this.childNodes.flatMap((c) => c.all())]; }
  createElement(tag: string) { return new FakeNode(1, tag.toUpperCase(), this); }
  createElementNS(ns: string, tag: string) { return new FakeNode(1, tag, this, ns); }
  createTextNode(t: string) { const n = new FakeNode(3, "#text", this); n.nodeValue = t; return n; }
  getElementById(id: string) { return this.all().find((n) => n.getAttribute("id") === id) ?? null; }
}
const doc = new FakeNode(9, "#document", null);
const body = doc.createElement("body");
doc.appendChild(body);
// React DOM checks, as it loads, whether the browser has the input event (`"oninput" in document`): it does here
(doc as unknown as { oninput: null }).oninput = null;
/** The window's own listeners (the page's Ctrl+B), by event type. */
const onWindow = new Map<string, Set<(e: unknown) => void>>();
const win = { document: doc, event: undefined, HTMLIFrameElement: class {}, devicePixelRatio: 1,
  addEventListener(type: string, f: (e: unknown) => void) { onWindow.set(type, (onWindow.get(type) ?? new Set()).add(f)); },
  removeEventListener(type: string, f: (e: unknown) => void) { onWindow.get(type)?.delete(f); },
  // a team picked (lib/team's pickTeam) tells every page that shows one
  dispatchEvent(e: { type: string }) { for (const f of onWindow.get(e.type) ?? []) f(e); return true; },
  // GA-372: an address with a project that isn't there is replaced by every project's
  location: { hash: "", replace(to: string) { h.calls.push(["replace", [to]]); } },
  matchMedia: () => ({ matches: false, addEventListener() {}, removeEventListener() {} }) };
Object.assign(doc, { body, documentElement: body, activeElement: body, defaultView: win, hidden: false });
vi.stubGlobal("window", win);
vi.stubGlobal("document", doc);
(globalThis as { IS_REACT_ACT_ENVIRONMENT?: boolean }).IS_REACT_ACT_ENVIRONMENT = true;

/** An event at `target`, through the listeners on the way as a browser calls them (capture down, then bubble up). */
function fire(target: FakeNode, type: string) {
  const path: FakeNode[] = [];
  for (let x: FakeNode | null = target; x; x = x.parentNode) path.push(x);
  let stopped = false;
  const e = { type, target, bubbles: true, button: 0, defaultPrevented: false, timeStamp: 0,
    preventDefault() { e.defaultPrevented = true; }, stopPropagation() { stopped = true; } };
  for (const n of [...path].reverse()) for (const l of n.listeners.get(type) ?? []) if (l.capture && !stopped) l.f(e);
  for (const n of path) for (const l of n.listeners.get(type) ?? []) if (!l.capture && !stopped) l.f(e);
}
const cls = (n: FakeNode) => (n.getAttribute("class") ?? "").split(" ");
const byLabel = (root: FakeNode, label: string) => root.all().filter((n) => n.getAttribute("aria-label") === label);
const buttonsNamed = (root: FakeNode, text: string) => root.all().filter((n) => n.nodeName === "BUTTON" && n.textContent.trim() === text);

// ---- what the page reads ----
const h = vi.hoisted(() => ({
  /** What the data hooks answer, by the api call they make. */
  data: {} as Record<string, unknown>,
  calls: [] as [string, unknown[]][],
  /** The address the page went to last (go): Routed follows it, as App follows the hash. */
  go: null as null | ((r: { page: string; project?: string }) => void),
  /** The cards the board was drawn with last (a stand-in: the real one needs a browser's drag and drop). */
  board: null as null | Task[],
}));
// The api: the reads the page makes answer with a tag the data hooks look up; anything else never settles.
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const tags: Record<string, string> = { getTeam: "team", listProjects: "projects", listUsers: "people", listTasks: "tasks", listTeams: "teams" };
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) {
    out[k] = typeof v !== "function" ? v : (...a: unknown[]) => { h.calls.push([k, a]); return tags[k] ? { tag: tags[k] } : new Promise(() => {}); };
  }
  return out;
});
// The data hooks answer at once, with the same objects on each render.
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    const r = fetch() as { tag?: string } | null;
    const tag = r && typeof r === "object" && "tag" in r ? r.tag : undefined;
    return { data: tag ? h.data[tag] ?? null : null, error: null, reload: () => {}, setData: () => {} };
  },
}));
vi.mock("../components/Board", () => ({ Board: ({ tasks }: { tasks: Task[] }) => { h.board = tasks; return null; } }));
vi.mock("../lib/useLiveRuns", () => { const none: never[] = []; return { useLiveRuns: () => none }; });
vi.mock("../router", async (orig) => ({ ...(await orig<Record<string, unknown>>()), go: (...a: unknown[]) => { h.calls.push(["go", a]); h.go?.(a[0] as { page: string; project?: string }); } }));

const { createRoot } = await import("react-dom/client");
const { TasksPage, KEEP_AFTER_MS } = await import("./TasksPage");

// ---- the data ----
const team = { id: "team", name: "Software", labels: [{ id: "l1", name: "Bug", color: null }],
  members: [{ actorId: "be", name: "Backend Agent", kind: "agent", roleKey: "backend", status: "active", handle: "be", isLead: false, allowedTools: [], chatEnabled: false }],
  states: [{ id: "todo", name: "Todo", category: "unstarted", sortKey: "a" }, { id: "ip", name: "In progress", category: "in_progress", sortKey: "b" }] };
const projects = [{ id: "p1", name: "Gizai", key: "GA", color: "#4f46e5" }, { id: "p2", name: "Website", key: "WEB", color: null }];
const people = [{ id: "u1", name: "Jeffrey" }];
const card = (id: string, title: string, over: Partial<Task>): Task => ({ id, identifier: id.toUpperCase(), title, descriptionMd: "", stateId: "todo",
  stateName: "Todo", stateCategory: "unstarted", priority: 0, assigneeId: null, assigneeName: null, assigneeKind: null, labels: [], hold: null,
  bounceCount: 0, failCount: 0, sortKey: "a", testing: true, createdAt: 0, updatedAt: 0, projectId: "p1", byHand: false, screenshots: null, ...over } as Task);
const tasks = [
  card("ga-1", "Fix the login page", { assigneeId: "u1", assigneeName: "Jeffrey", assigneeKind: "person", updatedAt: 6 }),
  card("ga-2", "Login page copy", { stateId: "ip", stateName: "In progress", stateCategory: "in_progress", updatedAt: 5 }),
  card("web-3", "Landing page", { projectId: "p2", updatedAt: 4 }),
  card("ga-4", "Logout button", { assigneeId: "be", assigneeName: "Backend Agent", assigneeKind: "agent", updatedAt: 3 }),
  card("web-5", "Login page for partners", { projectId: "p2", assigneeId: "u1", assigneeName: "Jeffrey", assigneeKind: "person", updatedAt: 2 }),
];

// ---- localStorage, every write recorded ----
let store: Map<string, string>;
let ls: { getItem: ReturnType<typeof vi.fn>; setItem: ReturnType<typeof vi.fn>; removeItem: ReturnType<typeof vi.fn> };
const writes = () => [...ls.setItem.mock.calls.map(([k, v]) => ["set", k, v]), ...ls.removeItem.mock.calls.map(([k]) => ["remove", k])];
const setsOf = (k: string) => ls.setItem.mock.calls.filter(([key]) => key === k).map(([, v]) => v);
const removesOf = (k: string) => ls.removeItem.mock.calls.filter(([key]) => key === k).length;

beforeEach(() => {
  store = new Map();
  ls = {
    getItem: vi.fn((k: string) => store.get(k) ?? null),
    setItem: vi.fn((k: string, v: string) => void store.set(k, String(v))),
    removeItem: vi.fn((k: string) => void store.delete(k)),
  };
  vi.stubGlobal("localStorage", ls);
  h.data = { team, projects, people, tasks };
  h.calls.length = 0;
  h.go = null;
  h.board = null;
  // on before the page mounts, so its setTimeout is the fake one
  vi.useFakeTimers();
});
afterEach(async () => {
  if (root) await unmount();
  vi.useRealTimers();
});

// ---- the page ----
let container: FakeNode;
let root: ReturnType<typeof createRoot> | null = null;
function mount(el: ReactNode) {
  container = doc.createElement("div");
  body.appendChild(container);
  root = createRoot(container as unknown as HTMLElement);
  act(() => { root!.render(el); });
}
/** The page as App draws it (GA-372): its project is the address's, and a go() from it (a project picked in Filters, All
 *  projects, Clear filters) changes the address and with it the project. The page itself stays, as App keys it by page. */
function Routed({ start, view = "list", onNewTask = () => {} }: { start?: string; view?: "list" | "board"; onNewTask?: (stateId?: string) => void }) {
  const [project, setProject] = useState(start);
  h.go = (r) => setProject(r.project);
  return <TasksPage initialView={view} project={project} onNewTask={onNewTask} />;
}
/** The Tasks page, the list: every project's (#/tasks), or `project`'s (#/tasks/project/<id>). */
const mountTasks = (project?: string) => mount(<Routed start={project} />);
/** The addresses the page went to. */
const went = () => h.calls.filter(([k]) => k === "go").map(([, a]) => a[0]);
async function unmount() {
  await act(async () => { root!.unmount(); });
  root = null;
  container.parentNode?.removeChild(container);
}
const search = () => byLabel(container, "Search tasks")[0]!;
/** Types one character more: the box holds `value`, and the browser sends an input event. */
function key(value: string) {
  const box = search();
  act(() => { box.type = "search"; box.value = value; fire(box, "input"); });
}
const wait = (ms: number) => act(() => { vi.advanceTimersByTime(ms); });
/** The task rows shown, by identifier. */
const rows = () => container.all().filter((n) => cls(n).includes("task-row")).map((n) => n.all().find((c) => cls(c).includes("id"))!.textContent);
/** What the page's search should show for `q` (as lib/taskView matches it: identifier and title, any case). */
const matching = (q: string, among = tasks) => among.filter((t) => `${t.identifier} ${t.title}`.toLowerCase().includes(q.toLowerCase())).map((t) => t.identifier);
const filtersButton = () => container.all().find((n) => n.nodeName === "BUTTON" && /^Filters/.test(n.textContent.trim()))!;
const openFilters = () => act(() => { fire(filtersButton(), "click"); });
const pick = (name: string) => act(() => { fire(buttonsNamed(byLabel(container, "Filters").find((n) => n.getAttribute("role") === "menu")!, name)[0]!, "click"); });

const TEXT = "login page";
const GAP = 50;
/** Types `text` a character at a time, `gap` ms apart; checks after each character that nothing was written yet and the list
 *  shows what matches. */
function typeQuickly(text: string, gap = GAP, among = tasks) {
  for (let i = 1; i <= text.length; i++) {
    if (i > 1) wait(gap);
    key(text.slice(0, i));
    expect(search().value).toBe(text.slice(0, i));
    expect(rows().sort()).toEqual(matching(text.slice(0, i), among).sort());
    expect(writes(), `after ${i} characters`).toEqual([]);
  }
}

describe("the Tasks page keeps its filter once typing pauses (GA-190)", () => {
  it("writes nothing while 10 characters are typed 50 ms apart, the list filtering at each; once typing pauses for KEEP_AFTER_MS it writes once, the filter without its text", () => {
    expect(KEEP_AFTER_MS).toBe(400);
    mountTasks();
    expect(rows().sort()).toEqual(tasks.map((t) => t.identifier).sort());
    // opening the page writes nothing
    expect(writes()).toEqual([]);

    expect(TEXT).toHaveLength(10);
    typeQuickly(TEXT);
    expect(rows().sort()).toEqual(["GA-1", "GA-2", "WEB-5"]);
    // 450 ms of typing, nothing written

    // a pause just short of KEEP_AFTER_MS: still nothing
    wait(KEEP_AFTER_MS - 1);
    expect(writes()).toEqual([]);
    // then the write, once: the filter without its search text (nothing else is on: an empty filter); no project is kept
    // (GA-372), and with none stored there is nothing to clear
    wait(1);
    expect(setsOf("gizai-task-filter")).toEqual(["{}"]);
    expect(setsOf("gizai-task-project")).toEqual([]);
    expect(removesOf("gizai-task-project")).toBe(0);
    expect(writes()).toHaveLength(1);
    expect(store.get("gizai-task-filter")).not.toContain("login");
    // nothing more as time goes on, and the search still shows what was typed
    wait(10_000);
    expect(writes()).toHaveLength(1);
    expect(search().value).toBe(TEXT);
    expect(rows().sort()).toEqual(["GA-1", "GA-2", "WEB-5"]);
  });

  it("on a project's tasks with an assignee kept, typing writes the assignee again once it pauses (and never the text, nor the project)", () => {
    store.set("gizai-task-filter", JSON.stringify({ assigneeId: "u1" }));
    mountTasks("p1");
    expect(filtersButton().textContent).toBe("Filters: 2");
    expect(rows()).toEqual(["GA-1"]);
    expect(writes()).toEqual([]);

    typeQuickly("fix the l", 30, tasks.filter((t) => t.projectId === "p1" && t.assigneeId === "u1"));
    expect(writes()).toEqual([]);
    wait(KEEP_AFTER_MS);
    expect(setsOf("gizai-task-filter")).toEqual([JSON.stringify({ assigneeId: "u1" })]);
    expect(setsOf("gizai-task-project")).toEqual([]);
    expect(writes()).toHaveLength(1);
    wait(10_000);
    expect(writes()).toHaveLength(1);
    // the project stays the address's: nothing went anywhere
    expect(went()).toEqual([]);
  });

  it("a pick in Filters writes at once, without waiting for the timer, and leaves no write waiting; a project is the address, never written", () => {
    mountTasks();
    openFilters();
    expect(byLabel(container, "Filters").filter((n) => n.getAttribute("role") === "menu")).toHaveLength(1);
    expect(writes()).toEqual([]);

    // a project (GA-372): its address, and nothing kept for it
    pick("Gizai");
    expect(went()).toEqual([{ page: "tasks", project: "p1" }]);
    expect(setsOf("gizai-task-project")).toEqual([]);
    expect(rows().sort()).toEqual(["GA-1", "GA-2", "GA-4"]);
    expect(filtersButton().textContent).toBe("Filters: 1");

    // an assignee: at once, the filter without the project
    ls.setItem.mockClear();
    pick("Jeffrey");
    expect(setsOf("gizai-task-filter")).toEqual([JSON.stringify({ assigneeId: "u1" })]);
    expect(writes()).toHaveLength(1);
    expect(rows()).toEqual(["GA-1"]);
    expect(filtersButton().textContent).toBe("Filters: 2");
    // the project stays: no second address
    expect(went()).toEqual([{ page: "tasks", project: "p1" }]);

    // the timer that a change starts was flushed with it: nothing more later
    wait(10_000);
    expect(writes()).toHaveLength(1);
    expect(store.get("gizai-task-filter")).toBe(JSON.stringify({ assigneeId: "u1" }));
    expect(store.has("gizai-task-project")).toBe(false);
  });

  it("a pick in Filters while typed text waits writes at once, once, and no second write follows when the timer would have run", () => {
    mountTasks();
    typeQuickly("log", 20);
    expect(writes()).toEqual([]);
    openFilters();
    pick("Jeffrey");
    expect(setsOf("gizai-task-filter")).toEqual([JSON.stringify({ assigneeId: "u1" })]);
    expect(writes()).toHaveLength(1);
    // Jeffrey's cards with "log" in them, of every project
    expect(rows().sort()).toEqual(["GA-1", "WEB-5"]);
    wait(10_000);
    expect(writes()).toHaveLength(1);
    expect(search().value).toBe("log");
    // then a project: its address, the text and the assignee kept on the page
    pick("Website");
    expect(went()).toEqual([{ page: "tasks", project: "p2" }]);
    expect(rows()).toEqual(["WEB-5"]);
    expect(search().value).toBe("log");
    expect(setsOf("gizai-task-project")).toEqual([]);
  });

  it("leaving the page while a write waits writes it then (nothing is lost), and nothing more when the timers run", async () => {
    mountTasks("p1");
    typeQuickly(TEXT, GAP, tasks.filter((t) => t.projectId === "p1"));
    wait(KEEP_AFTER_MS - 100);
    expect(writes()).toEqual([]);

    await unmount();
    expect(setsOf("gizai-task-filter")).toEqual(["{}"]);
    expect(setsOf("gizai-task-project")).toEqual([]);
    expect(removesOf("gizai-task-project")).toBe(0);
    expect(writes()).toHaveLength(1);
    // the timer was cleared with it
    expect(vi.getTimerCount()).toBe(0);
    vi.advanceTimersByTime(10_000);
    expect(writes()).toHaveLength(1);
  });

  it("leaving the page right after a single keystroke writes too, and with nothing waiting, leaving writes nothing", async () => {
    mountTasks();
    key("x");
    await unmount();
    expect(setsOf("gizai-task-filter")).toEqual(["{}"]);
    expect(writes()).toHaveLength(1);

    ls.setItem.mockClear();
    ls.removeItem.mockClear();
    mountTasks();
    typeQuickly("lo");
    wait(KEEP_AFTER_MS);
    expect(writes()).toHaveLength(1);
    await unmount();
    expect(writes()).toHaveLength(1);
    vi.advanceTimersByTime(10_000);
    expect(writes()).toHaveLength(1);
  });

  it("a page opened again reads back what was kept: the picks in Filters but not the project (GA-372), nor the search text", async () => {
    mountTasks();
    openFilters();
    pick("Gizai");
    pick("Jeffrey");
    key("f");
    key("fi");
    key("fix");
    expect(rows()).toEqual(["GA-1"]);
    await unmount();
    expect(store.get("gizai-task-filter")).toBe(JSON.stringify({ assigneeId: "u1" }));
    expect(store.has("gizai-task-project")).toBe(false);
    const before = writes().length;

    // Tasks again (#/tasks, as the sidebar's Tasks opens it): every project's, Jeffrey's
    mountTasks();
    expect(search().value).toBe("");
    expect(filtersButton().textContent).toBe("Filters: 1");
    const crumbs = () => container.all().find((n) => cls(n).includes("crumbs"))!.textContent;
    expect(crumbs()).not.toContain("Gizai");
    expect(rows().sort()).toEqual(["GA-1", "WEB-5"]);
    openFilters();
    const ticks = () => byLabel(container, "Filters").find((n) => n.getAttribute("role") === "menu")!.all()
      .filter((n) => n.nodeName === "BUTTON" && n.all().some((c) => (c.getAttribute("class") ?? "").includes("tick"))).map((n) => n.textContent);
    expect(ticks()).toEqual(["All projects", "Jeffrey"]);
    await unmount();

    // a project's tasks: that project, and the assignee still kept
    mountTasks("p2");
    expect(filtersButton().textContent).toBe("Filters: 2");
    expect(crumbs()).toContain("Website/");
    expect(rows()).toEqual(["WEB-5"]);
    openFilters();
    expect(ticks()).toEqual(["Website", "Jeffrey"]);
    // opening it writes nothing
    expect(writes()).toHaveLength(before);
  });
});

// GA-372 QA: a project's tasks have their own address, #/tasks/project/<id> (on the board #/board/project/<id>, in the Inbox
// #/inbox/project/<id>); #/tasks is every project's, so the sidebar's Tasks always shows every project. The project shows
// in the crumbs, its name linking to its page and × (All projects) to every project's. Filters → a project goes to its
// address, All projects and Clear filters to #/tasks. Nothing of the project is kept on this computer, and a project kept
// there by an older Hersir (gizai-task-project) is cleared once.
describe("a project's tasks have their own address (GA-372)", () => {
  const crumbs = () => container.all().find((n) => cls(n).includes("crumbs"))!;
  const links = () => crumbs().all().filter((n) => n.nodeName === "A").map((n) => [n.textContent, n.getAttribute("href"), n.getAttribute("aria-label")]);
  const menu = () => byLabel(container, "Filters").find((n) => n.getAttribute("role") === "menu")!;
  const ticked = () => menu().all().filter((n) => n.nodeName === "BUTTON" && n.all().some((c) => cls(c).includes("tick"))).map((n) => n.textContent);

  it("#/tasks/project/<id> shows only that project's cards, its name in the crumbs before Tasks, and × back to every project's", () => {
    mountTasks("p1");
    expect(rows().sort()).toEqual(["GA-1", "GA-2", "GA-4"]);
    expect(crumbs().textContent).toBe("Gizai/Tasks3");
    // the name links to the project's page; × is All projects, to #/tasks
    expect(links()).toEqual([["Gizai", "#/project/p1", null], ["", "#/tasks", "All projects"]]);
    expect(crumbs().all().find((n) => n.getAttribute("aria-label") === "All projects")!.getAttribute("title")).toBe("All projects");
    expect(filtersButton().textContent).toBe("Filters: 1");
    openFilters();
    expect(ticked()).toEqual(["Gizai", "Anyone"]);
    // nothing was written or went anywhere to show it
    expect(writes()).toEqual([]);
    expect(went()).toEqual([]);
  });

  it("#/tasks shows every project's cards, with no project in the crumbs and All projects ticked", () => {
    mountTasks();
    expect(rows().sort()).toEqual(tasks.map((t) => t.identifier).sort());
    expect(crumbs().textContent).toBe("Tasks5");
    expect(links()).toEqual([]);
    expect(filtersButton().textContent).toBe("Filters");
    openFilters();
    expect(ticked()).toEqual(["All projects", "Anyone"]);
  });

  it("a project picked in Filters goes to its address; another project to that one; All projects and Clear filters go to #/tasks", () => {
    mountTasks();
    openFilters();
    pick("Gizai");
    expect(went()).toEqual([{ page: "tasks", project: "p1" }]);
    expect(rows().sort()).toEqual(["GA-1", "GA-2", "GA-4"]);
    expect(crumbs().textContent).toBe("Gizai/Tasks3");
    // the same project again: it is there already
    pick("Gizai");
    expect(went()).toHaveLength(1);
    pick("Website");
    expect(went()).toEqual([{ page: "tasks", project: "p1" }, { page: "tasks", project: "p2" }]);
    expect(rows().sort()).toEqual(["WEB-3", "WEB-5"]);
    expect(crumbs().textContent).toBe("Website/Tasks2");
    pick("All projects");
    expect(went()[2]).toEqual({ page: "tasks" });
    expect(rows()).toHaveLength(5);
    expect(crumbs().textContent).toBe("Tasks5");

    // Clear filters: every project's, and the other filters gone too
    pick("Website");
    pick("Jeffrey");
    expect(rows()).toEqual(["WEB-5"]);
    act(() => { fire(byLabel(container, "Clear filters")[0]!, "click"); });
    expect(went().at(-1)).toEqual({ page: "tasks" });
    expect(rows()).toHaveLength(5);
    expect(filtersButton().textContent).toBe("Filters");
    // never kept on this computer
    expect(setsOf("gizai-task-project")).toEqual([]);
    for (const v of setsOf("gizai-task-filter")) expect(JSON.parse(v as string)).not.toHaveProperty("projectId");
  });

  it("a person or label picked on a project's tasks keeps that project: the page stays at its address", () => {
    mountTasks("p2");
    openFilters();
    pick("Jeffrey");
    pick("Bug");
    expect(went()).toEqual([]);
    expect(crumbs().textContent).toContain("Website/");
    expect(store.get("gizai-task-filter")).toBe(JSON.stringify({ assigneeId: "u1", labelIds: ["l1"] }));
  });

  it("on the board, × and a project picked in Filters stay on the board: #/board and #/board/project/<id>", () => {
    store.set("gizai-tasks-view", JSON.stringify("board"));
    mount(<Routed start="p1" view="board" />);
    expect(h.board!.map((t) => t.identifier).sort()).toEqual(["GA-1", "GA-2", "GA-4"]);
    expect(links()).toEqual([["Gizai", "#/project/p1", null], ["", "#/board", "All projects"]]);
    openFilters();
    pick("Website");
    expect(went()).toEqual([{ page: "board", project: "p2" }]);
    expect(h.board!.map((t) => t.identifier).sort()).toEqual(["WEB-3", "WEB-5"]);
    pick("All projects");
    expect(went()[1]).toEqual({ page: "board" });
  });

  it("in the Inbox, × goes to #/inbox and a project picked in Filters to #/inbox/project/<id>", () => {
    mount(<TasksPage inboxFor="u1" project="p1" onNewTask={() => {}} />);
    expect(links()).toEqual([["Gizai", "#/project/p1", null], ["", "#/inbox", "All projects"]]);
    expect(crumbs().textContent.startsWith("Gizai/Inbox")).toBe(true);
    openFilters();
    pick("Website");
    expect(went()).toEqual([{ page: "inbox", project: "p2" }]);
  });

  it("the List and Board switch keeps the scope: a project's tasks stay that project's, every project's stay every project's", () => {
    mountTasks("p1");
    const board = byLabel(container, "Board")[0]!, list = byLabel(container, "List")[0]!;
    act(() => { fire(board, "click"); });
    expect(board.getAttribute("aria-pressed")).toBe("true");
    expect(h.board!.map((t) => t.identifier).sort()).toEqual(["GA-1", "GA-2", "GA-4"]);
    expect(store.get("gizai-tasks-view")).toBe(JSON.stringify("board"));
    expect(crumbs().textContent).toBe("Gizai/Tasks3");
    act(() => { fire(list, "click"); });
    expect(rows().sort()).toEqual(["GA-1", "GA-2", "GA-4"]);
    // Ctrl+B both ways
    const ctrlB = () => act(() => { for (const f of onWindow.get("keydown") ?? []) f({ key: "b", ctrlKey: true, metaKey: false, altKey: false, shiftKey: false, target: body, preventDefault() {} }); });
    h.board = null;
    ctrlB();
    expect(store.get("gizai-tasks-view")).toBe(JSON.stringify("board"));
    expect(h.board!.map((t) => t.identifier).sort()).toEqual(["GA-1", "GA-2", "GA-4"]);
    expect(crumbs().textContent).toBe("Gizai/Tasks3");
    ctrlB();
    expect(rows().sort()).toEqual(["GA-1", "GA-2", "GA-4"]);
    // the switch never changes the address
    expect(went()).toEqual([]);
    expect(setsOf("gizai-task-project")).toEqual([]);
  });

  it("the List and Board switch on every project's tasks stays every project's", async () => {
    mountTasks();
    act(() => { fire(byLabel(container, "Board")[0]!, "click"); });
    expect(h.board).toHaveLength(5);
    act(() => { fire(byLabel(container, "List")[0]!, "click"); });
    expect(rows()).toHaveLength(5);
    expect(crumbs().textContent).toBe("Tasks5");
    expect(went()).toEqual([]);
  });

  it("a project kept by an older Hersir (gizai-task-project) is cleared once as the page opens, and not shown", async () => {
    store.set("gizai-task-project", "p2");
    store.set("gizai-task-filter", JSON.stringify({ labelIds: ["l1"] }));
    mountTasks();
    expect(removesOf("gizai-task-project")).toBe(1);
    expect(store.has("gizai-task-project")).toBe(false);
    // every project's, the rest of the filter as it was kept
    expect(crumbs().textContent).not.toContain("Website");
    expect(filtersButton().textContent).toBe("Filters: 1");
    expect(went()).toEqual([]);
    await unmount();
    // the next time there is nothing left to clear, and nothing is written back
    mountTasks();
    expect(removesOf("gizai-task-project")).toBe(1);
    openFilters();
    pick("Website");
    await unmount();
    expect(setsOf("gizai-task-project")).toEqual([]);
    expect(store.has("gizai-task-project")).toBe(false);
  });

  it("a project's address with a project that isn't there (removed, another data folder's) is replaced by every project's", () => {
    mountTasks("gone");
    expect(h.calls.filter(([k]) => k === "replace")).toEqual([["replace", ["#/tasks"]]]);
    expect(crumbs().textContent).not.toContain("/");
  });

  it("with two teams, arriving at a project's address picks its team, once; every project's tasks and the Inbox leave the pick as it is", () => {
    h.data = { ...h.data, teams: [{ id: "team", name: "Software" }, { id: "web", name: "Web" }],
      projects: [{ ...projects[0], teamId: "team" }, { ...projects[1], teamId: "web" }] };
    store.set("gizai-team", "team");
    mountTasks();
    expect(store.get("gizai-team")).toBe("team");
    // Website is the Web team's
    act(() => { h.go!({ page: "tasks", project: "p2" }); });
    expect(setsOf("gizai-team")).toEqual(["web"]);
    // once: a team picked after that stays picked, on the same project's address
    act(() => { store.set("gizai-team", "team"); win.dispatchEvent({ type: "gizai-team-changed" }); });
    expect(setsOf("gizai-team")).toEqual(["web"]);
    // the project of the team picked: nothing to pick
    act(() => { h.go!({ page: "tasks", project: "p1" }); });
    expect(setsOf("gizai-team")).toEqual(["web"]);
    act(() => { h.go!({ page: "tasks" }); });
    expect(setsOf("gizai-team")).toEqual(["web"]);
  });

  it("the Inbox at a project's address picks no team: it shows every team's cards", () => {
    h.data = { ...h.data, teams: [{ id: "team", name: "Software" }, { id: "web", name: "Web" }],
      projects: [{ ...projects[0], teamId: "team" }, { ...projects[1], teamId: "web" }] };
    store.set("gizai-team", "team");
    mount(<TasksPage inboxFor="u1" project="p2" onNewTask={() => {}} />);
    expect(setsOf("gizai-team")).toEqual([]);
  });

  it("New task on the page asks App for it (App picks the address's project, AppProjectTasks.test.tsx), a column's + with its column", () => {
    const asked: (string | undefined)[] = [];
    mount(<Routed start="p1" onNewTask={(s) => asked.push(s)} />);
    act(() => { fire(buttonsNamed(container, "New task")[0]!, "click"); });
    expect(asked).toEqual([undefined]);
  });
});

// GA-300 QA: the toolbar (the list's island header) folds when it doesn't fit, measured before it is drawn (useFold): level 1
// turns Filters, Sort and Group into icon buttons named in their tooltips, level 2 New task too (its tooltip with its key N).
// Here the toolbar is as wide as bar.have and needs bar.need[level] (the CSS that makes each level narrower is checked in
// src/lib/useFold.test.ts, and the real 900 × 600 window in the self-test's appearance probe). Folded, every action works.
describe("the Tasks toolbar folds to fit a narrow island (GA-300)", () => {
  afterEach(() => { bar.have = 0; bar.need = [0, 0, 0]; });
  const toolbar = () => container.all().find((n) => cls(n).includes("island-toolbar"))!;
  const button = (text: string) => container.all().find((n) => n.nodeName === "BUTTON" && n.textContent.trim() === text);
  const titles = () => ["New task", "Sort", "Group"].map((t) => [t, button(t)?.getAttribute("title") ?? null]);
  const menu = (label: string) => byLabel(container, label).find((n) => n.getAttribute("role") === "menu");

  it("draws it as before while it fits: no data-fold, every button with its name and no tooltip of its own", () => {
    bar.have = 900; bar.need = [708, 640, 600];
    mountTasks();
    expect(toolbar().getAttribute("data-fold")).toBeNull();
    expect(titles()).toEqual([["New task", null], ["Sort", null], ["Group", null]]);
    expect(filtersButton().getAttribute("title")).toBeNull();
  });

  it("a little short: Filters, Sort and Group fold (data-fold 1), each named in its tooltip; New task keeps its label", () => {
    bar.have = 660; bar.need = [708, 640, 600];
    mountTasks();
    expect(toolbar().getAttribute("data-fold")).toBe("1");
    expect(titles()).toEqual([["New task", null], ["Sort", "Sort"], ["Group", "Group"]]);
    expect(filtersButton().getAttribute("title")).toBe("Filters");
    // the names stay in the buttons for screen readers (the CSS draws them at no size)
    for (const b of [filtersButton(), button("Sort")!, button("Group")!]) expect(cls(b)).toContain("fold");
    expect(cls(button("New task")!)).toContain("fold-2");
  });

  it("at 900 × 600 (the list's island 634 wide where 708 was needed, GA-290 QA): New task folds too, its tooltip with its key", () => {
    bar.have = 634; bar.need = [708, 680, 600];
    mountTasks();
    expect(toolbar().getAttribute("data-fold")).toBe("2");
    expect(titles()).toEqual([["New task", "New task (N)"], ["Sort", "Sort"], ["Group", "Group"]]);
    // nothing on it goes: the search, the view switch, Filters, Sort, Group and Archived cards are all there
    expect(byLabel(container, "Search tasks")).toHaveLength(1);
    expect(byLabel(container, "Board")).toHaveLength(1);
    expect(byLabel(container, "List")).toHaveLength(1);
    expect(byLabel(container, "Archived cards")).toHaveLength(1);
  });

  it("folded, every action still works: Filters filters and keeps its count, Sort and Group open, New task and Clear filters run", () => {
    bar.have = 634; bar.need = [708, 680, 600];
    let asked = 0;
    mount(<Routed onNewTask={() => { asked += 1; }} />);
    openFilters();
    pick("Gizai");
    expect(rows().sort()).toEqual(["GA-1", "GA-2", "GA-4"]);
    // the count after the icon (data-count, drawn by ::after) and in its tooltip; measured again with the Clear button there
    expect(filtersButton().getAttribute("data-count")).toBe("1");
    expect(filtersButton().getAttribute("title")).toBe("Filters: 1");
    expect(toolbar().getAttribute("data-fold")).toBe("2");
    act(() => { fire(button("Sort")!, "click"); });
    expect(menu("Sort")).toBeDefined();
    act(() => { fire(button("Group")!, "click"); });
    expect(menu("Group")).toBeDefined();
    act(() => { fire(button("New task")!, "click"); });
    expect(asked).toBe(1);
    act(() => { fire(byLabel(container, "Clear filters")[0]!, "click"); });
    expect(rows().sort()).toEqual(tasks.map((t) => t.identifier).sort());
    expect(filtersButton().getAttribute("data-count")).toBeNull();
    expect(filtersButton().getAttribute("title")).toBe("Filters");
  });

  it("measures again when what it holds changes, and unfolds when it fits again", () => {
    bar.have = 634; bar.need = [708, 680, 600];
    mountTasks();
    expect(toolbar().getAttribute("data-fold")).toBe("2");
    // the island grows (here: a window made wider), and a filter changes what the toolbar holds
    bar.have = 900;
    openFilters();
    pick("Gizai");
    expect(toolbar().getAttribute("data-fold")).toBeNull();
    expect(titles()).toEqual([["New task", null], ["Sort", null], ["Group", null]]);
    expect(filtersButton().getAttribute("title")).toBeNull();
  });
});
