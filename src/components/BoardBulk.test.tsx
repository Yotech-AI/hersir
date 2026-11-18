// GA-306 QA: the Tasks page's board in Deploy and Done: Move all (Deploy, to its next column) and Archive all (an open
// Done) in the column's head, each asking first with the number of cards; a checkbox on each card to tick a selection,
// one column at a time, with Shift+click, Space, Escape and Clear selection; and with a selection, the head's "N
// selected" and its action, on exactly the ticked cards. React itself renders (react-dom/client) into a small stand-in
// DOM (there is no DOM library in the project; as in pages/TasksSearchKeep.test.tsx). Drag and drop is stubbed: each
// card's drag listener only records that a drag would start, so a test can see the checkbox never starts one.
import { act } from "react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Task, WorkflowState } from "../types";

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
const fireWindow = (type: string, extra: Record<string, unknown>) =>
  act(() => { for (const f of winListeners.get(type) ?? []) f({ type, preventDefault() {}, ...extra }); });
const cls = (n: FakeNode) => (n.getAttribute("class") ?? "").split(" ");

// ---- drag and drop, stubbed: a card's drag starts on its pointerdown listener, which records it ----
const h = vi.hoisted(() => ({ drags: [] as string[] }));
vi.mock("@dnd-kit/core", () => ({
  DndContext: ({ children }: { children: unknown }) => children,
  MeasuringStrategy: { Always: 0 }, PointerSensor: class {},
  closestCenter: () => [], getFirstCollision: () => null, pointerWithin: () => [],
  useDroppable: () => ({ setNodeRef: () => {} }), useSensor: () => ({}), useSensors: () => [],
}));
vi.mock("@dnd-kit/sortable", () => ({
  SortableContext: ({ children }: { children: unknown }) => children, arrayMove: (a: unknown[]) => a, verticalListSortingStrategy: {},
  useSortable: ({ id }: { id: string }) => ({ attributes: { role: "button", tabIndex: 0 }, listeners: { onPointerDown: () => { h.drags.push(id); } },
    setNodeRef: () => {}, transform: null, transition: undefined, isDragging: false }),
}));
vi.mock("@dnd-kit/utilities", () => ({ CSS: { Transform: { toString: () => undefined } } }));

const { createRoot } = await import("react-dom/client");
const { Board } = await import("./Board");

// ---- the board ----
const states: WorkflowState[] = [
  { id: "s-todo", name: "To do", category: "ready", sortKey: "a1" },
  { id: "s-review", name: "Review", category: "review", sortKey: "a4" },
  { id: "s-deploy", name: "Deploy", category: "deploy", sortKey: "a4V", nextStateId: "s-done" },
  { id: "s-done", name: "Done", category: "done", sortKey: "a5" },
  { id: "s-cancelled", name: "Cancelled", category: "cancelled", sortKey: "a6" },
] as WorkflowState[];
const at = (id: string) => states.find((s) => s.id === id)!;
const card = (n: number, stateId: string, i: number): Task => {
  const s = at(stateId);
  return { id: `t${n}`, identifier: `GA-${n}`, projectId: "p1", title: `Card ${n}`, descriptionMd: "", stateId, stateName: s.name, stateCategory: s.category,
    priority: 0, labels: [], bounceCount: 0, failCount: 0, sortKey: `k${String(i).padStart(3, "0")}`, testing: true, createdAt: 0, updatedAt: 0, byHand: false,
    screenshots: null } as Task;
};
// GA-1 in To do, GA-2 in Review, GA-101 … GA-160 in Deploy (60: ten past the 50 drawn), GA-201 … GA-203 in Done
const deploy = Array.from({ length: 60 }, (_, i) => card(101 + i, "s-deploy", i));
const done = [card(201, "s-done", 0), card(202, "s-done", 1), card(203, "s-done", 2)];
const all = [card(1, "s-todo", 0), card(2, "s-review", 0), ...deploy, ...done];

type Props = Partial<Parameters<typeof Board>[0]>;
let container: FakeNode;
let root: ReturnType<typeof createRoot> | null = null;
let calls: { open: string[]; moveMany: [string[], string, string][]; archiveMany: string[][]; archive: string[] };
let props: Parameters<typeof Board>[0];
function draw(over: Props = {}) {
  props = {
    tasks: all, states, onMove: () => {}, onOpen: (id: string) => { calls.open.push(id); },
    onArchive: (t: Task) => { calls.archive.push(t.identifier); },
    onMoveMany: (ts: Task[], from: string, to: string) => { calls.moveMany.push([ts.map((t) => t.identifier), from, to]); },
    onArchiveMany: (ts: Task[]) => { calls.archiveMany.push(ts.map((t) => t.identifier)); },
    ...over,
  } as Parameters<typeof Board>[0];
  act(() => { root!.render(<Board {...props} />); });
}
beforeEach(() => {
  calls = { open: [], moveMany: [], archiveMany: [], archive: [] };
  h.drags.length = 0;
  winListeners.clear();
  container = doc.createElement("div");
  body.appendChild(container);
  root = createRoot(container as unknown as HTMLElement);
});
afterEach(async () => {
  await act(async () => { root!.unmount(); });
  root = null;
  container.parentNode?.removeChild(container);
});

const col = (name: string) => container.all().find((n) => n.getAttribute("data-col") === name)!;
const head = (name: string) => col(name).all().find((n) => cls(n).includes("col-head"))!;
const count = (name: string) => head(name).all().find((n) => cls(n).includes("n"))!.textContent;
const labelled = (root: FakeNode, label: string) => root.all().filter((n) => n.getAttribute("aria-label") === label);
const button = (root: FakeNode, text: string) => root.all().find((n) => n.nodeName === "BUTTON" && n.textContent.trim() === text);
const cardEl = (id: string) => container.all().find((n) => n.getAttribute("data-card") === id)!;
const check = (id: string) => labelled(cardEl(id), `Select ${id}`)[0];
const picked = () => container.all().filter((n) => cls(n).includes("card") && cls(n).includes("picked")).map((n) => n.getAttribute("data-card"));
const click = (n: FakeNode, extra: Record<string, unknown> = {}) => act(() => { fire(n, "click", extra); });
const tick = (id: string, shiftKey = false) => click(check(id), { shiftKey });
const openDone = () => click(col("Done"));
/** The head's Move all or Archive all: the button in its popover's wrapper (the click lands on the wrapper's span). */
const askButton = (label: string) => labelled(container, label).find((n) => n.nodeName === "BUTTON");
const menu = () => container.all().find((n) => n.getAttribute("role") === "menu");

describe("Move all and Archive all in the column's head", () => {
  it("Deploy has Move all to its next column, an open Done Archive all; no other column has either", () => {
    draw();
    expect(askButton("Move all to Done")?.getAttribute("title")).toBe("Move all to Done");
    expect(head("Deploy").all().some((n) => n === askButton("Move all to Done"))).toBe(true);
    expect(askButton("Archive all")).toBeUndefined(); // Done is a closed rail
    openDone();
    expect(head("Done").all().some((n) => n === askButton("Archive all"))).toBe(true);
    for (const name of ["To do", "Review"]) {
      expect(labelled(head(name), "Move all to Done")).toHaveLength(0);
      expect(labelled(head(name), "Archive all")).toHaveLength(0);
    }
    expect(container.all().filter((n) => /^(Move all|Archive all)/.test(n.getAttribute("aria-label") ?? "") && n.nodeName === "BUTTON")).toHaveLength(2);
  });

  it("sits in the head's buttons before Collapse", () => {
    draw();
    openDone();
    const add = head("Done").all().find((n) => cls(n).includes("add"))!;
    const order = add.all().filter((n) => n.nodeName === "BUTTON").map((n) => n.getAttribute("aria-label") ?? n.textContent.trim());
    expect(order.indexOf("Archive all")).toBeGreaterThanOrEqual(0);
    expect(order.indexOf("Archive all")).toBeLessThan(order.indexOf("Collapse"));
  });

  it("is not on an empty Deploy or Done, nor on the compact board, nor where the page offers no bulk action", () => {
    draw({ tasks: [card(1, "s-todo", 0)] });
    openDone();
    expect(askButton("Move all to Done")).toBeUndefined();
    expect(askButton("Archive all")).toBeUndefined();
    draw({ compact: true });
    expect(askButton("Move all to Done")).toBeUndefined();
    expect(container.all().some((n) => cls(n).includes("card-check"))).toBe(false);
    draw({ compact: false, onMoveMany: undefined, onArchiveMany: undefined });
    openDone();
    expect(askButton("Move all to Done")).toBeUndefined();
    expect(askButton("Archive all")).toBeUndefined();
    expect(container.all().some((n) => cls(n).includes("card-check"))).toBe(false);
  });

  it("names Deploy's next column, and without one the team's first Done", () => {
    const shipped = { id: "s-shipped", name: "Shipped", category: "done", sortKey: "a7" } as WorkflowState;
    draw({ states: [...states.map((s) => (s.id === "s-deploy" ? { ...s, nextStateId: "s-shipped" } : s)), shipped] });
    expect(askButton("Move all to Shipped")).toBeDefined();
    draw({ states: states.map((s) => (s.id === "s-deploy" ? { ...s, nextStateId: null } : s)) });
    expect(askButton("Move all to Done")).toBeDefined();
  });

  it("Move all asks first with the number of cards; Cancel changes nothing", () => {
    draw();
    expect(count("Deploy")).toBe("60");
    expect(col("Deploy").all().filter((n) => n.getAttribute("data-card"))).toHaveLength(50); // 50 drawn, Show more
    click(askButton("Move all to Done")!);
    expect(menu()?.textContent).toContain("Move all 60 cards in Deploy to Done?");
    expect(button(menu()!, "Move 60 cards")).toBeDefined();
    click(button(menu()!, "Cancel")!);
    expect(menu()).toBeUndefined();
    expect(calls.moveMany).toEqual([]);
  });

  it("confirmed, Move all moves every card in Deploy, those past the 50 drawn too, in their order", () => {
    draw();
    click(askButton("Move all to Done")!);
    click(button(menu()!, "Move 60 cards")!);
    expect(menu()).toBeUndefined();
    expect(calls.moveMany).toEqual([[deploy.map((t) => t.identifier), "s-deploy", "s-done"]]);
  });

  it("Archive all asks first with the number of cards, and archives every card in Done", () => {
    draw();
    openDone();
    click(askButton("Archive all")!);
    expect(menu()?.textContent).toContain("Archive all 3 cards in Done? The bin on the Tasks page keeps them, and Restore puts one back.");
    click(button(menu()!, "Cancel")!);
    expect(calls.archiveMany).toEqual([]);
    click(askButton("Archive all")!);
    click(button(menu()!, "Archive 3 cards")!);
    expect(calls.archiveMany).toEqual([["GA-201", "GA-202", "GA-203"]]);
  });

  it("with a filter or a search on, takes only the cards the column shows, and asks with the head's count", () => {
    // the page hands the board only the cards that match: here GA-103, GA-150 and GA-160 of Deploy
    const shown = all.filter((t) => t.stateId !== "s-deploy" || ["GA-103", "GA-150", "GA-160"].includes(t.identifier));
    draw({ tasks: shown });
    expect(count("Deploy")).toBe("3");
    click(askButton("Move all to Done")!);
    expect(menu()?.textContent).toContain("Move all 3 cards in Deploy to Done?");
    click(button(menu()!, "Move 3 cards")!);
    expect(calls.moveMany).toEqual([[["GA-103", "GA-150", "GA-160"], "s-deploy", "s-done"]]);
  });

  it("one card: names it", () => {
    draw({ tasks: [card(12, "s-deploy", 0), card(13, "s-done", 0)] });
    click(askButton("Move all to Done")!);
    expect(menu()?.textContent).toContain("Move GA-12 to Done?");
    click(button(menu()!, "Cancel")!);
    openDone();
    click(askButton("Archive all")!);
    expect(menu()?.textContent).toContain("Archive GA-13?");
  });
});

describe("the checkbox on a card in Deploy and Done", () => {
  it("is first in the card's top row, before the ID, labelled with it; cards in other columns have none", () => {
    draw();
    openDone();
    for (const id of ["GA-101", "GA-150", "GA-201"]) {
      const box = check(id)!;
      expect(box.nodeName).toBe("INPUT");
      expect(box.type ?? box.getAttribute("type")).toBe("checkbox");
      expect(cls(box)).toContain("card-check");
      const top = box.parentNode!;
      expect(cls(top)).toContain("top");
      expect(top.childNodes[0]).toBe(box);
      expect(cls(top.childNodes[1])).toContain("id");
    }
    for (const id of ["GA-1", "GA-2"]) expect(cardEl(id).all().some((n) => cls(n).includes("card-check")), id).toBe(false);
  });

  it("ticks the card without opening it or starting a drag, and shows the selection in the head", () => {
    draw();
    act(() => { fire(check("GA-102"), "pointerdown"); });
    expect(h.drags).toEqual([]);
    tick("GA-102");
    expect(calls.open).toEqual([]);
    expect(picked()).toEqual(["GA-102"]);
    expect(cls(col("Deploy"))).toContain("selecting");
    expect(count("Deploy")).toBe("1 selected");
    expect(button(head("Deploy"), "Move to Done")).toBeDefined();
    expect(labelled(head("Deploy"), "Clear selection")).toHaveLength(1);
    expect(askButton("Move all to Done")).toBeUndefined();
    // the card itself still opens, and starts a drag
    click(cardEl("GA-103"));
    expect(calls.open).toEqual(["t103"]);
    act(() => { fire(cardEl("GA-103"), "pointerdown"); });
    expect(h.drags).toEqual(["t103"]);
    // ticked again, it's unticked
    tick("GA-102");
    expect(picked()).toEqual([]);
    expect(count("Deploy")).toBe("60");
  });

  it("Space on the card ticks it; Enter opens it", () => {
    draw();
    act(() => { fire(cardEl("GA-104"), "keydown", { key: " " }); });
    expect(picked()).toEqual(["GA-104"]);
    expect(calls.open).toEqual([]);
    act(() => { fire(cardEl("GA-104"), "keydown", { key: "Enter" }); });
    expect(calls.open).toEqual(["t104"]);
    // a key on the checkbox stays there: Enter on it doesn't open the card
    act(() => { fire(check("GA-105"), "keydown", { key: "Enter" }); });
    expect(calls.open).toEqual(["t104"]);
  });

  it("Shift+click ticks every card between the last one ticked and this one, in that column", () => {
    draw();
    tick("GA-102");
    tick("GA-106", true);
    expect(picked()).toEqual(["GA-102", "GA-103", "GA-104", "GA-105", "GA-106"]);
    expect(count("Deploy")).toBe("5 selected");
    // and upwards from the last one ticked
    tick("GA-110");
    tick("GA-108", true);
    expect(picked()).toEqual(["GA-102", "GA-103", "GA-104", "GA-105", "GA-106", "GA-108", "GA-109", "GA-110"]);
  });

  it("one column at a time: ticking a card in the other column clears the first", () => {
    draw();
    openDone();
    tick("GA-101");
    tick("GA-102");
    tick("GA-202");
    expect(picked()).toEqual(["GA-202"]);
    expect(count("Deploy")).toBe("60");
    expect(count("Done")).toBe("1 selected");
    expect(button(head("Done"), "Archive")).toBeDefined();
    expect(cls(col("Deploy"))).not.toContain("selecting");
  });

  it("Escape clears the selection, and so does Clear selection", () => {
    draw();
    tick("GA-101");
    tick("GA-103");
    fireWindow("keydown", { key: "Escape" });
    expect(picked()).toEqual([]);
    expect(count("Deploy")).toBe("60");
    tick("GA-101");
    click(labelled(head("Deploy"), "Clear selection")[0]);
    expect(picked()).toEqual([]);
    expect(askButton("Move all to Done")).toBeDefined();
  });

  it("Move to Done and Archive act on exactly the ticked cards and clear the selection", () => {
    draw();
    openDone();
    tick("GA-110");
    tick("GA-103");
    click(button(head("Deploy"), "Move to Done")!);
    expect(calls.moveMany).toEqual([[["GA-103", "GA-110"], "s-deploy", "s-done"]]);
    expect(picked()).toEqual([]);
    tick("GA-203");
    tick("GA-201");
    click(button(head("Done"), "Archive")!);
    expect(calls.archiveMany).toEqual([["GA-201", "GA-203"]]);
    expect(picked()).toEqual([]);
    expect(calls.archive).toEqual([]);
  });

  it("a card that leaves the board leaves the selection", () => {
    draw();
    tick("GA-101");
    tick("GA-102");
    draw({ tasks: all.filter((t) => t.identifier !== "GA-102") });
    expect(picked()).toEqual(["GA-101"]);
    expect(count("Deploy")).toBe("1 selected");
    draw({ tasks: all.filter((t) => t.identifier !== "GA-101" && t.identifier !== "GA-102") });
    expect(picked()).toEqual([]);
    expect(count("Deploy")).toBe("58");
  });

  it("the single card's Archive button still archives one card", () => {
    draw();
    openDone();
    click(labelled(cardEl("GA-202"), "Archive GA-202")[0]);
    expect(calls.archive).toEqual(["GA-202"]);
    expect(calls.open).toEqual([]);
  });
});

// The stylesheets on disk (Vitest hands .css back empty): the checkbox hides until the card is hovered or focused, shows
// on touch screens and on every card of a column with a selection, and is our own checkbox as it is; a ticked card has
// the accent border and the accent-soft background.
describe("the checkbox's and a ticked card's styles", async () => {
  const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
  const read = (f: string): string => fs.readFileSync(new URL(`../styles/${f}`, import.meta.url), "utf8").replace(/\/\*[\s\S]*?\*\//g, "");
  const app = read("app.css"), components = read("components.css");
  /** The declarations of the rule whose selector list holds `selector`, in `css`. */
  const decl = (css: string, selector: string) => [...css.matchAll(/([^{}]+)\{([^{}]*)\}/g)]
    .filter((m) => m[1].split(",").map((s) => s.trim()).includes(selector)).map((m) => m[2].replace(/\s+/g, " ").trim());

  it("hidden until the card is hovered or focused, shown on touch screens, with a selection, and when ticked", () => {
    expect(decl(app, ".card .card-check").join(";")).toMatch(/opacity: 0/);
    for (const shown of [".card:hover .card-check", ".card:focus-within .card-check", ".col.selecting .card .card-check", ".card .card-check:checked"]) {
      expect(decl(app, shown).join(";"), shown).toMatch(/opacity: 1/);
    }
    expect(app).toMatch(/@media \(hover: none\) \{ \.card \.card-check \{ opacity: 1; \} \}/);
  });

  it("is our own checkbox: no new look of its own", () => {
    for (const d of decl(app, ".card .card-check")) expect(d).not.toMatch(/appearance|border|background|width|height/);
    expect(components).toContain('input[type="checkbox"] { appearance: none;');
  });

  it("a ticked card has the accent border and the accent-soft background", () => {
    const picked = decl(components, ".card.picked").join(";");
    expect(picked).toMatch(/border-color: var\(--accent\)/);
    expect(picked).toMatch(/var\(--accent-soft\)/);
  });
});
