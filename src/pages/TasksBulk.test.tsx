// GA-306 QA: the Tasks page after a move or an archive of several cards (the board's Move all, Archive all, and a
// selection's Move to Done and Archive): moved cards show at the bottom of Done at once and archived ones leave the
// board at once; the toast says how many, names the cards the backend skipped (they stay where they were), and has Undo
// for 8 s, which moves the cards back to the bottom of Deploy in their order (move_tasks) or restores them
// (restore_tasks); a command that fails as a whole puts the board back. React itself renders (react-dom/client) into a
// small stand-in DOM (as in components/BoardBulk.test.tsx); the api's bulk commands answer when a test says so, and drag
// and drop is stubbed.
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { BulkResult, Task } from "../types";

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
  declare value?: string;
  declare type?: string;
  declare checked?: boolean;
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
  all(): FakeNode[] { return [this, ...this.childNodes.flatMap((c) => c.all())]; }
  createElement(tag: string) { return new FakeNode(1, tag.toUpperCase(), this); }
  createElementNS(ns: string, tag: string) { return new FakeNode(1, tag, this, ns); }
  createTextNode(t: string) { const n = new FakeNode(3, "#text", this); n.nodeValue = t; return n; }
  getElementById(id: string) { return this.all().find((n) => n.getAttribute("id") === id) ?? null; }
}
const doc = new FakeNode(9, "#document", null);
const body = doc.createElement("body");
doc.appendChild(body);
(doc as unknown as { oninput: null }).oninput = null;
// The window keeps its listeners: the board's Escape and the popover's listen there.
const winListeners = new Map<string, ((e: unknown) => void)[]>();
const win = { document: doc, event: undefined, HTMLIFrameElement: class {}, devicePixelRatio: 1,
  addEventListener(type: string, f: (e: unknown) => void) { winListeners.set(type, [...(winListeners.get(type) ?? []), f]); },
  removeEventListener(type: string, f: (e: unknown) => void) { winListeners.set(type, (winListeners.get(type) ?? []).filter((x) => x !== f)); },
  matchMedia: () => ({ matches: false, addEventListener() {}, removeEventListener() {} }) };
Object.assign(doc, { body, documentElement: body, activeElement: body, defaultView: win, hidden: false });
vi.stubGlobal("window", win);
vi.stubGlobal("document", doc);
vi.stubGlobal("requestAnimationFrame", (f: () => void) => { f(); return 0; });
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
const cls = (n: FakeNode) => (n.getAttribute("class") ?? "").split(" ");

// ---- drag and drop, stubbed ----
vi.mock("@dnd-kit/core", () => ({
  DndContext: ({ children }: { children: unknown }) => children,
  MeasuringStrategy: { Always: 0 }, PointerSensor: class {},
  closestCenter: () => [], getFirstCollision: () => null, pointerWithin: () => [],
  useDroppable: () => ({ setNodeRef: () => {} }), useSensor: () => ({}), useSensors: () => [],
}));
vi.mock("@dnd-kit/sortable", () => ({
  SortableContext: ({ children }: { children: unknown }) => children, arrayMove: (a: unknown[]) => a, verticalListSortingStrategy: {},
  useSortable: () => ({ attributes: { role: "button", tabIndex: 0 }, listeners: {}, setNodeRef: () => {}, transform: null, transition: undefined, isDragging: false }),
}));
vi.mock("@dnd-kit/utilities", () => ({ CSS: { Transform: { toString: () => undefined } } }));

// ---- what the page reads, and the bulk commands ----
type Pending = { resolve: (r: BulkResult) => void; reject: (e: unknown) => void };
const h = vi.hoisted(() => ({
  data: {} as Record<string, unknown>,
  /** The bulk commands called: name and arguments, and how to answer each. */
  bulk: [] as { name: string; args: unknown[]; answer: Pending }[],
}));
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const tags: Record<string, string> = { getTeam: "team", listProjects: "projects", listUsers: "people", listTasks: "tasks", listTeams: "teams" };
  const bulk = new Set(["moveTasks", "archiveTasks", "restoreTasks"]);
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) {
    out[k] = typeof v !== "function" ? v : (...a: unknown[]) => {
      if (bulk.has(k)) return new Promise((resolve, reject) => { h.bulk.push({ name: k, args: a, answer: { resolve, reject } }); });
      return tags[k] ? { tag: tags[k] } : new Promise(() => {});
    };
  }
  return out;
});
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    const r = fetch() as { tag?: string } | null;
    const tag = r && typeof r === "object" && "tag" in r ? r.tag : undefined;
    return { data: tag ? h.data[tag] ?? null : null, error: null, reload: () => {}, setData: () => {} };
  },
}));
vi.mock("../lib/useLiveRuns", () => { const none: never[] = []; return { useLiveRuns: () => none }; });
vi.mock("../router", async (orig) => ({ ...(await orig<Record<string, unknown>>()), go: () => {} }));

const { createRoot } = await import("react-dom/client");
const { TasksPage } = await import("./TasksPage");

// ---- the data: GA-101 … GA-104 in Deploy, GA-201 … GA-203 in Done ----
const states = [
  { id: "s-todo", name: "To do", category: "ready", sortKey: "a1" },
  { id: "s-deploy", name: "Deploy", category: "deploy", sortKey: "a4V", nextStateId: "s-done" },
  { id: "s-done", name: "Done", category: "done", sortKey: "a5" },
];
const team = { id: "team", name: "Software", labels: [], members: [], states, branches: [] };
const card = (n: number, stateId: string, sortKey: string): Task => {
  const s = states.find((x) => x.id === stateId)!;
  return { id: `t${n}`, identifier: `GA-${n}`, projectId: "p1", title: `Card ${n}`, descriptionMd: "", stateId, stateName: s.name, stateCategory: s.category,
    priority: 0, assigneeId: null, assigneeName: null, assigneeKind: null, labels: [], hold: null, bounceCount: 0, failCount: 0, sortKey, testing: true,
    createdAt: 0, updatedAt: 0, byHand: false, screenshots: null } as Task;
};
const tasks = [
  card(101, "s-deploy", "a0"), card(102, "s-deploy", "a1"), card(103, "s-deploy", "a2"), card(104, "s-deploy", "a3"),
  card(201, "s-done", "a0"), card(202, "s-done", "a1"), card(203, "s-done", "a2"),
];

let store: Map<string, string>;
beforeEach(() => {
  store = new Map();
  vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, String(v)), removeItem: (k: string) => void store.delete(k) });
  h.data = { team, projects: [{ id: "p1", name: "Gizai", key: "GA", color: null, teamId: "team" }], people: [], tasks, teams: [{ id: "team", name: "Software" }] };
  h.bulk.length = 0;
  winListeners.clear();
  vi.useFakeTimers();
  container = doc.createElement("div");
  body.appendChild(container);
  root = createRoot(container as unknown as HTMLElement);
  act(() => { root!.render(<TasksPage initialView="board" onNewTask={() => {}} />); });
});
afterEach(async () => {
  await act(async () => { root!.unmount(); });
  root = null;
  container.parentNode?.removeChild(container);
  vi.useRealTimers();
});

let container: FakeNode;
let root: ReturnType<typeof createRoot> | null = null;
const col = (name: string) => container.all().find((n) => n.getAttribute("data-col") === name)!;
/** A column's cards, top to bottom (Done is a rail until it's opened: then its count only). */
const cards = (name: string) => col(name).all().map((n) => n.getAttribute("data-card")).filter(Boolean);
const labelled = (root: FakeNode, label: string) => root.all().filter((n) => n.getAttribute("aria-label") === label);
const button = (root: FakeNode, text: string) => root.all().find((n) => n.nodeName === "BUTTON" && n.textContent.trim() === text);
const click = (n: FakeNode, extra: Record<string, unknown> = {}) => act(() => { fire(n, "click", extra); });
const tick = (id: string) => click(labelled(container, `Select ${id}`)[0]);
const toast = () => container.all().find((n) => cls(n).includes("toast"));
const toastText = () => { const t = toast(); return t ? t.childNodes.filter((c) => c.nodeType === 3).map((c) => c.textContent).join("") : null; };
const undo = () => click(button(toast()!, "Undo")!);
const openDone = () => click(col("Done"));
/** Answers the latest bulk command. */
async function answer(r: BulkResult | Error) {
  const last = h.bulk[h.bulk.length - 1];
  await act(async () => { if (r instanceof Error) last.answer.reject(r.message); else last.answer.resolve(r); });
}
const calls = () => h.bulk.map((c) => [c.name, ...c.args]);

describe("moving several cards from Deploy", () => {
  it("they show at the bottom of Done at once; the toast says how many, and Undo puts them back at the bottom of Deploy in their order", async () => {
    openDone();
    tick("GA-103");
    tick("GA-101");
    click(button(col("Deploy"), "Move to Done")!);
    expect(calls()).toEqual([["moveTasks", ["t101", "t103"], "s-done"]]);
    // before the backend answers
    expect(cards("Done")).toEqual(["GA-201", "GA-202", "GA-203", "GA-101", "GA-103"]);
    expect(cards("Deploy")).toEqual(["GA-102", "GA-104"]);
    await answer({ done: ["t101", "t103"], skipped: [] });
    expect(toastText()).toBe("Moved 2 cards to Done");
    expect(button(toast()!, "Undo")).toBeDefined();

    undo();
    expect(calls()[1]).toEqual(["moveTasks", ["t101", "t103"], "s-deploy"]);
    expect(cards("Deploy")).toEqual(["GA-102", "GA-104", "GA-101", "GA-103"]);
    expect(cards("Done")).toEqual(["GA-201", "GA-202", "GA-203"]);
    expect(toast()).toBeUndefined();
    await answer({ done: ["t101", "t103"], skipped: [] });
    expect(toast()).toBeUndefined();
  });

  it("Move all takes every card; one card is named", async () => {
    click(labelled(container, "Move all to Done").find((n) => n.nodeName === "BUTTON")!);
    click(button(container, "Move 4 cards")!);
    expect(calls()).toEqual([["moveTasks", ["t101", "t102", "t103", "t104"], "s-done"]]);
    expect(cards("Deploy")).toEqual([]);
    await answer({ done: ["t101", "t102", "t103", "t104"], skipped: [] });
    expect(toastText()).toBe("Moved 4 cards to Done");
    undo();
    expect(cards("Deploy")).toEqual(["GA-101", "GA-102", "GA-103", "GA-104"]);
    await answer({ done: ["t101", "t102", "t103", "t104"], skipped: [] });

    tick("GA-102");
    click(button(col("Deploy"), "Move to Done")!);
    await answer({ done: ["t102"], skipped: [] });
    expect(toastText()).toBe("Moved GA-102 to Done");
  });

  it("a card an agent is working on stays in Deploy, and the toast names it; the others go", async () => {
    tick("GA-101");
    tick("GA-102");
    tick("GA-103");
    click(button(col("Deploy"), "Move to Done")!);
    await answer({ done: ["t101", "t103"], skipped: [{ identifier: "GA-102", reason: "An agent is working on this card" }] });
    expect(cards("Deploy")).toEqual(["GA-102", "GA-104"]);
    expect(toastText()).toBe("Moved 2 cards to Done. GA-102 wasn't: An agent is working on this card.");
    undo();
    expect(calls()[1]).toEqual(["moveTasks", ["t101", "t103"], "s-deploy"]);
  });

  it("when the command fails as a whole, the board goes back and the toast says why", async () => {
    tick("GA-101");
    tick("GA-102");
    click(button(col("Deploy"), "Move to Done")!);
    expect(cards("Deploy")).toEqual(["GA-103", "GA-104"]);
    await answer(new Error("database is locked"));
    expect(cards("Deploy")).toEqual(["GA-101", "GA-102", "GA-103", "GA-104"]);
    expect(toastText()).toBe("Couldn't move 2 cards to Done: database is locked");
    expect(button(toast()!, "Undo")).toBeUndefined();
  });

  it("Undo is there for 8 s", async () => {
    tick("GA-101");
    click(button(col("Deploy"), "Move to Done")!);
    await answer({ done: ["t101"], skipped: [] });
    act(() => { vi.advanceTimersByTime(7_900); });
    expect(button(toast()!, "Undo")).toBeDefined();
    act(() => { vi.advanceTimersByTime(100); });
    expect(toast()).toBeUndefined();
  });
});

describe("archiving several cards of Done", () => {
  it("they leave the board at once; the toast says how many, and Undo restores them to the bottom of Done", async () => {
    openDone();
    tick("GA-201");
    tick("GA-203");
    click(button(col("Done"), "Archive")!);
    expect(calls()).toEqual([["archiveTasks", ["t201", "t203"]]]);
    expect(cards("Done")).toEqual(["GA-202"]);
    await answer({ done: ["t201", "t203"], skipped: [] });
    expect(toastText()).toBe("Archived 2 cards");
    undo();
    expect(calls()[1]).toEqual(["restoreTasks", ["t201", "t203"]]);
    expect(cards("Done")).toEqual(["GA-202", "GA-201", "GA-203"]);
  });

  it("Archive all, with a card an agent is working on: it stays, and the toast names it", async () => {
    openDone();
    click(labelled(container, "Archive all").find((n) => n.nodeName === "BUTTON")!);
    click(button(container, "Archive 3 cards")!);
    expect(calls()).toEqual([["archiveTasks", ["t201", "t202", "t203"]]]);
    expect(cards("Done")).toEqual([]);
    await answer({ done: ["t201", "t203"], skipped: [{ identifier: "GA-202", reason: "An agent is working on this card" }] });
    expect(cards("Done")).toEqual(["GA-202"]);
    expect(toastText()).toBe("Archived 2 cards. GA-202 wasn't: An agent is working on this card.");
    undo();
    expect(calls()[1]).toEqual(["restoreTasks", ["t201", "t203"]]);
  });

  it("one card: 'GA-12 archived'; a failure puts it back", async () => {
    openDone();
    tick("GA-202");
    click(button(col("Done"), "Archive")!);
    await answer({ done: ["t202"], skipped: [] });
    expect(toastText()).toBe("GA-202 archived");
    tick("GA-201");
    click(button(col("Done"), "Archive")!);
    expect(cards("Done")).toEqual(["GA-203"]);
    await answer(new Error("database is locked"));
    expect(cards("Done")).toEqual(["GA-201", "GA-203"]);
    expect(toastText()).toBe("Couldn't archive GA-201: database is locked");
  });
});
