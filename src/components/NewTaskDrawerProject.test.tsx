// GA-372 QA: the New task drawer's project. Opened with a project (on a project's tasks, #/tasks/project/<id>, App passes
// the address's project; a project's page passes its own), it picks that project, also when the team picked elsewhere
// is another, and doesn't remember it. Opened without one, it picks its own last pick, kept under a key of its own
// (gizai-new-task-project), no longer the Tasks page's old project filter (gizai-task-project). A project picked in the
// drawer is remembered when the task is created; a task that couldn't be created remembers nothing. The hook runtime of
// NewTaskDrawerFiles.test.tsx stands in for React (components called as functions, effects after each render); the api
// and the router are fakes that record what they are given; localStorage is a Map.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import type { Project, Team } from "../types";

type Inst = { slots: unknown[]; i: number };
type EffectSlot = { effect: true; deps?: unknown[]; first: boolean; cleanup?: () => void };
const h = vi.hoisted(() => ({
  cur: null as Inst | null,
  dirty: false,
  effects: [] as (() => void)[],
  calls: [] as [string, unknown[]][],
  answers: {} as Record<string, (...a: unknown[]) => unknown>,
}));

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
  const hooks = { useState, useRef, useMemo, useCallback, useEffect, useLayoutEffect: useEffect };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});
vi.mock("@tauri-apps/api/webview", () => ({ getCurrentWebview: () => ({ onDragDropEvent: () => Promise.resolve(() => {}) }) }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: () => Promise.resolve(null) }));
vi.mock("../router", async (orig) => ({ ...(await orig<Record<string, unknown>>()), go: (r: unknown) => { h.calls.push(["go", [r]]); } }));
vi.mock("./MarkdownEditor", () => ({ MarkdownEditor: () => null }));
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) {
    out[k] = typeof v !== "function" ? v : (...a: unknown[]) => { h.calls.push([k, a]); return Promise.resolve().then(() => h.answers[k]?.(...a)); };
  }
  return out;
});
vi.stubGlobal("window", { devicePixelRatio: 1, addEventListener: () => {}, removeEventListener: () => {}, dispatchEvent: () => true });
const store = new Map<string, string>();
const sets: [string, string][] = [];
vi.stubGlobal("localStorage", {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => { sets.push([k, String(v)]); store.set(k, String(v)); },
  removeItem: (k: string) => { store.delete(k); },
});

const { NewTaskDrawer, LAST_PROJECT_KEY } = await import("./NewTaskDrawer");

const project = (id: string, name: string, teamId: string, status = "active") => ({ id, key: id.toUpperCase(), name, status, teamId } as Project);
const KADE = project("p-kade", "Kade portal", "t-soft"), GFW = project("p-gfw", "Groene Fiets", "t-soft"), SITE = project("p-site", "Website", "t-web");
const OLD = project("p-old", "Old portal", "t-soft", "archived");
const team = (id: string, name: string): Team => ({ id, name, members: [], labels: [],
  states: [{ id: `${id}-backlog`, name: "Backlog", category: "backlog", sortKey: "a0" }, { id: `${id}-todo`, name: "To do", category: "ready", sortKey: "a1" }] } as Team);
const SOFT = team("t-soft", "Software"), WEB = team("t-web", "Web");
const defaults: Record<string, (...a: unknown[]) => unknown> = {
  listProjects: () => [KADE, GFW, SITE, OLD], listTeams: () => [SOFT, WEB], getTeam: (id) => (id === "t-web" ? WEB : SOFT), listUsers: () => [],
  createTask: () => "t-new", onRowsChanged: () => () => {},
};

/** The drawer on screen, opened as App or a page opens it. */
const ui = { open: false, preset: undefined as string | undefined, closed: 0 };
function Root() {
  return ui.open ? <NewTaskDrawer projectId={ui.preset} onClose={() => { ui.closed++; ui.open = false; }} /> : null;
}

// ---- the hook runtime ----
const insts = new Map<string, Inst>();
const unmount = (inst: Inst) => { for (const s of inst.slots) if ((s as EffectSlot)?.effect) (s as EffectSlot).cleanup?.(); };
const keyOf = (n: unknown) => (n && typeof n === "object" && "key" in n ? (n as { key: string | null }).key : null);
function expand(node: ReactNode, path: string, seen: Set<string>): ReactNode {
  if (Array.isArray(node)) return node.map((n, i) => expand(n, `${path}.${keyOf(n) ?? i}`, seen));
  if (!node || typeof node !== "object" || !("props" in node)) return node;
  const el = node as ReactElement<Record<string, unknown>>;
  if (typeof el.type === "function") {
    const key = `${path}>${el.type.name}`;
    seen.add(key);
    let inst = insts.get(key);
    if (!inst) insts.set(key, (inst = { slots: [], i: 0 }));
    inst.i = 0;
    h.cur = inst;
    let out: ReactNode;
    try { out = (el.type as (p: unknown) => ReactNode)(el.props); } finally { h.cur = null; }
    return expand(out, key, seen);
  }
  const p = el.props;
  if (p.ref && typeof p.ref === "object") (p.ref as { current: unknown }).current = { getBoundingClientRect: () => ({ left: 0, top: 0, right: 0, bottom: 0 }), scrollIntoView: () => {} };
  return { ...el, props: { ...p, children: expand(p.children as ReactNode, `${path}>${String(el.type)}`, seen) } } as ReactNode;
}
let tree: ReactNode = null;
function render() {
  for (let n = 0; n < 30; n++) {
    h.dirty = false;
    const seen = new Set<string>();
    tree = expand(<Root />, "", seen);
    for (const [k, inst] of insts) if (!seen.has(k)) { unmount(inst); insts.delete(k); }
    for (const f of h.effects.splice(0)) f();
    if (!h.dirty) return;
  }
  throw new Error("the render never settled");
}
async function settle() { for (let i = 0; i < 3; i++) { await new Promise((r) => setTimeout(r, 0)); render(); } }

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
const byId = (id: string) => findAll(tree, (f) => f.props.id === id)[0].props;
/** The project the drawer shows picked. */
const picked = () => byId("t-project").value as string;
/** The projects it offers, by id. */
const offered = () => findAll(tree, (f) => f.type === "option" && String(f.props.value).startsWith("p-")).map((f) => f.props.value);
const choose = (id: string) => { (byId("t-project").onChange as (e: unknown) => void)({ target: { value: id } }); render(); };
const typeTitle = (v: string) => { (byId("t-title").onChange as (e: unknown) => void)({ target: { value: v } }); render(); };
const strings = (p: Record<string, unknown>) => [p.children].flat(Infinity).filter((c) => typeof c === "string").join("");
async function create() {
  const b = findAll(tree, (f) => f.type === "button" && strings(f.props) === "Create task");
  expect(b).toHaveLength(1);
  await (b[0].props.onClick as () => unknown)();
  await settle();
}
const created = () => h.calls.filter(([k]) => k === "createTask").map(([, a]) => (a[0] as { projectId: string }).projectId);
const remembered = () => sets.filter(([k]) => k === LAST_PROJECT_KEY).map(([, v]) => v);

async function openDrawer(preset?: string) {
  ui.open = true;
  ui.preset = preset;
  render();
  await settle();
}

beforeEach(() => {
  for (const inst of insts.values()) unmount(inst);
  insts.clear();
  h.effects.length = 0;
  h.calls.length = 0;
  h.answers = { ...defaults };
  Object.assign(ui, { open: false, preset: undefined, closed: 0 });
  store.clear();
  sets.length = 0;
  tree = null;
});

describe("New task on a project's tasks (GA-372)", () => {
  it("picks the project it is opened with, not the drawer's last pick nor the old kept project", async () => {
    store.set(LAST_PROJECT_KEY, "p-gfw");
    store.set("gizai-task-project", "p-gfw");
    await openDrawer("p-kade");
    expect(picked()).toBe("p-kade");
    expect(offered()).toEqual(["p-kade", "p-gfw", "p-site"]);
  });

  it("picks it when it is on another team than the one picked on the board", async () => {
    store.set("gizai-team", "t-soft");
    await openDrawer("p-site");
    expect(picked()).toBe("p-site");
    // its team's columns: Web's
    expect(findAll(tree, (f) => f.type === "option" && String(f.props.value).startsWith("t-")).map((f) => f.props.value)).toEqual(["t-web-backlog", "t-web-todo"]);
  });

  it("creates the task in that project and doesn't remember it: the page's pick isn't the drawer's", async () => {
    store.set(LAST_PROJECT_KEY, "p-gfw");
    await openDrawer("p-kade");
    typeTitle("Fix the login");
    await create();
    expect(created()).toEqual(["p-kade"]);
    expect(remembered()).toEqual([]);
    expect(store.get(LAST_PROJECT_KEY)).toBe("p-gfw");
    expect(ui.closed).toBe(1);
  });

  it("another project picked in the drawer over the page's is remembered when the task is created", async () => {
    await openDrawer("p-kade");
    choose("p-gfw");
    typeTitle("Fix the login");
    await create();
    expect(created()).toEqual(["p-gfw"]);
    expect(remembered()).toEqual(["p-gfw"]);
  });
});

describe("New task elsewhere remembers its own last pick (GA-372)", () => {
  it("picks the project last picked in the drawer, kept under gizai-new-task-project", async () => {
    store.set(LAST_PROJECT_KEY, "p-gfw");
    await openDrawer();
    expect(picked()).toBe("p-gfw");
  });

  it("no longer reads the Tasks page's old project filter (gizai-task-project)", async () => {
    store.set("gizai-task-project", "p-gfw");
    await openDrawer();
    // the first project of the team shown, as with nothing remembered
    expect(picked()).toBe("p-kade");
  });

  it("with nothing remembered, picks the first project of the team picked on the board", async () => {
    await openDrawer();
    expect(picked()).toBe("p-kade");
    for (const inst of insts.values()) unmount(inst);
    insts.clear();
    store.set("gizai-team", "t-web");
    ui.open = false;
    render();
    await openDrawer();
    expect(picked()).toBe("p-site");
  });

  it("a project picked in the drawer is remembered on Create task, and the next New task picks it", async () => {
    await openDrawer();
    choose("p-gfw");
    typeTitle("Fix the login");
    await create();
    expect(created()).toEqual(["p-gfw"]);
    expect(remembered()).toEqual(["p-gfw"]);
    // never under the Tasks page's old key
    expect(store.has("gizai-task-project")).toBe(false);
    await openDrawer();
    expect(picked()).toBe("p-gfw");
  });

  it("the project it picked itself is remembered too, once the task is created", async () => {
    await openDrawer();
    typeTitle("Fix the login");
    await create();
    expect(remembered()).toEqual(["p-kade"]);
  });

  it("a task that can't be created remembers nothing, and Cancel remembers nothing", async () => {
    h.answers.createTask = () => { throw new Error("The database is locked"); };
    await openDrawer();
    choose("p-gfw");
    typeTitle("Fix the login");
    await create();
    expect(remembered()).toEqual([]);
    expect(ui.closed).toBe(0);
    const cancel = findAll(tree, (f) => f.type === "button" && strings(f.props) === "Cancel")[0];
    (cancel.props.onClick as () => void)();
    expect(remembered()).toEqual([]);
  });

  it("a remembered project that is archived now, or of another team than the one picked, gives way to the team's first", async () => {
    store.set(LAST_PROJECT_KEY, "p-old");
    await openDrawer();
    expect(picked()).toBe("p-kade");
    ui.open = false;
    render();
    store.set(LAST_PROJECT_KEY, "p-site");
    store.set("gizai-team", "t-soft");
    await openDrawer();
    expect(picked()).toBe("p-kade");
  });
});
