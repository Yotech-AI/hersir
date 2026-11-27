// GA-372 QA: App and a project's tasks. App hands the address's project (#/tasks/project/<id>, #/board/project/<id>,
// #/inbox/project/<id>) to the Tasks page, which stays the same page (keyed by page) as the project changes. New task
// (the page's button, a column's +, the N key, the sidebar, its toolbar and Search) opens the drawer with that project on a
// project's tasks, and without one elsewhere (the drawer then picks its own, NewTaskDrawerProject.test.tsx). Nothing in the
// app writes the old kept project (gizai-task-project) any more. A small hook runtime stands in for React (App runs as a
// function, its effects by their deps), as in AppSidebarKey.test.tsx; the sidebar, the toolbar, the pages and the api are
// fakes. Files are read from disk; node:fs comes in through a computed specifier, as in rebrand.test.ts.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));

const h = vi.hoisted(() => ({
  slots: [] as unknown[],
  slot: 0,
  effects: [] as { deps?: unknown[]; cleanup?: unknown }[],
  effect: 0,
  queued: [] as (() => void)[],
  route: { page: "tasks" } as Record<string, string>,
  listeners: new Map<string, (e: unknown) => void>(),
}));

vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = (init: unknown) => {
    const k = h.slot++;
    if (!(k in h.slots)) h.slots[k] = typeof init === "function" ? (init as () => unknown)() : init;
    return [h.slots[k], (v: unknown) => { h.slots[k] = typeof v === "function" ? (v as (x: unknown) => unknown)(h.slots[k]) : v; }];
  };
  const useRef = (current: unknown) => {
    const k = h.slot++;
    if (!(k in h.slots)) h.slots[k] = { current };
    return h.slots[k];
  };
  const useEffect = (f: () => unknown, deps?: unknown[]) => {
    const k = h.effect++;
    const was = h.effects[k];
    if (was && deps && was.deps && deps.length === was.deps.length && deps.every((d, i) => Object.is(d, was.deps![i]))) return;
    h.queued.push(() => {
      if (typeof was?.cleanup === "function") (was.cleanup as () => void)();
      h.effects[k] = { deps, cleanup: f() };
    });
  };
  return { ...R, useState, useRef, useEffect, useCallback: (f: unknown) => f, startTransition: (f: () => void) => f() };
});
vi.mock("./router", async (orig) => ({ ...(await orig<object>()), useRoute: () => h.route }));
vi.mock("./api", () => {
  const ok = (v: unknown = null) => () => Promise.resolve(v);
  return {
    appInfo: () => new Promise(() => {}), // never answers: the start's self-test paths stay out of these tests
    exitApp: ok(), selftestReport: ok(), getDoc: ok(), getTask: ok({ descriptionMd: "" }), getTeam: ok({ states: [] }), listRuns: ok([]), saveDoc: ok(),
    updateTask: ok(), listClients: ok([]), listProjects: ok([]), listTasks: ok([]), tourStill: ok(), tourSteps: ok(),
  };
});
vi.mock("./lib/appearance", () => ({ appearanceOf: () => ({}), setAppearance: () => {}, toggleDensity: () => {}, toggleTheme: () => {} }));
vi.mock("./lib/terminals", () => ({ startTerminals: () => {} }));
vi.mock("./lib/windowFocus", () => ({ watchAway: () => () => {} }));
vi.mock("./lib/drawers", () => ({ DrawerHost: function DrawerHost({ children }: { children: ReactNode }) { return children; } }));
vi.mock("./components/Sidebar", () => ({ Sidebar: function Sidebar() { return null; } }));
vi.mock("./components/SidebarToolbar", () => ({ SidebarToolbar: function SidebarToolbar() { return null; } }));
vi.mock("./components/CommandPalette", () => ({ CommandPalette: function CommandPalette() { return null; } }));
vi.mock("./components/CloseGuard", () => ({ CloseGuard: function CloseGuard() { return null; } }));
vi.mock("./pages/TasksPage", () => ({ TasksPage: function TasksPage() { return null; } }));

const doc = {
  activeElement: null,
  body: { dataset: {} as Record<string, string> },
  documentElement: { dataset: {} as Record<string, string> },
  dialog: false,
  querySelector: (sel: string) => (sel === "[role=dialog]" ? (doc.dialog ? {} : null) : null),
};
vi.stubGlobal("document", doc);
const win = {
  location: { hash: "" },
  addEventListener: (type: string, f: (e: unknown) => void) => { h.listeners.set(type, f); },
  removeEventListener: (type: string, f: unknown) => { if (h.listeners.get(type) === f) h.listeners.delete(type); },
};
vi.stubGlobal("window", win);
const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => { store.set(k, String(v)); },
  removeItem: (k: string) => { store.delete(k); },
});

const { default: App } = await import("./App");
const { href, parseHash } = await import("./router");

type Props = Record<string, unknown> & { children?: ReactNode };
let tree: ReactElement<Props>;
function render(): ReactElement<Props> {
  h.slot = 0; h.effect = 0;
  tree = (App as (p: object) => ReactElement<Props>)({});
  for (const f of h.queued.splice(0)) f();
  return tree;
}
/** A start at `hash`: the route App reads (useRoute) and the window's address, as they are in the app. */
function start(hash: string) {
  h.slots = []; h.effects = []; h.queued = []; h.listeners.clear();
  win.location.hash = hash;
  h.route = parseHash(hash) as Record<string, string>;
  return render();
}
/** The address changes (a link, Back, the page's go()): App draws again with its route. */
function goTo(hash: string) {
  win.location.hash = hash;
  h.route = parseHash(hash) as Record<string, string>;
  render();
}
function all(): ReactElement<Props>[] {
  const walk = (n: ReactNode): ReactElement<Props>[] => {
    if (!n || typeof n !== "object") return [];
    if (Array.isArray(n)) return n.flatMap(walk);
    const e = n as ReactElement<Props>;
    return [e, ...walk(e.props?.children)];
  };
  return walk(tree);
}
const named = (name: string) => all().filter((e) => typeof e.type === "function" && (e.type as { name: string }).name === name);
const request = () => named("DrawerHost")[0]!.props.request;
const page = () => named("TasksPage")[0]!;
/** N on the window, its target the page's body (no text field), App drawn again after. */
function pressN() {
  const e = { key: "n", ctrlKey: false, metaKey: false, altKey: false, shiftKey: false, target: { closest: () => null }, preventDefault: () => {} };
  h.listeners.get("keydown")!(e);
  render();
}
/** Closes the drawer, as the drawer's Close does. */
const close = () => { (named("DrawerHost")[0]!.props.setRequest as (r: null) => void)(null); render(); };
/** Each way to New task: [its name, what opens it]. */
const openers: [string, () => void][] = [
  ["the Tasks page's New task", () => { (page().props.onNewTask as (s?: string) => void)(); render(); }],
  ["the N key", pressN],
  ["the sidebar's New task", () => { (named("Sidebar")[0]!.props.onNewTask as () => void)(); render(); }],
  ["Search's New task", () => { (named("CommandPalette")[0]!.props.onNewTask as () => void)(); render(); }],
];

beforeEach(() => { store.clear(); doc.dialog = false; });

describe("App hands the address's project to the Tasks page (GA-372)", () => {
  it("#/tasks/project/<id>: the Tasks page with that project; #/tasks: without one", () => {
    start("#/tasks/project/p1");
    expect(page().props.project).toBe("p1");
    expect(page().key).toBe("tasks");
    start("#/tasks");
    expect(page().props.project).toBeUndefined();
    expect(page().key).toBe("tasks");
  });
  it("on the board and in the Inbox as well, each its own page", () => {
    start("#/board/project/p2");
    expect([page().key, page().props.project, page().props.initialView]).toEqual(["board", "p2", "board"]);
    start("#/inbox/project/p2");
    expect([page().key, page().props.project, page().props.inboxFor !== undefined]).toEqual(["inbox", "p2", true]);
    start("#/inbox");
    expect(page().props.project).toBeUndefined();
  });
  it("from a project's tasks to the sidebar's Tasks and Back again: the same page (its key), first without the project, then with it", () => {
    start("#/tasks/project/p1");
    goTo(href({ page: "tasks" }));
    expect([page().key, page().props.project]).toEqual(["tasks", undefined]);
    goTo("#/tasks/project/p1");
    expect([page().key, page().props.project]).toEqual(["tasks", "p1"]);
  });
});

describe("New task on a project's tasks picks that project (GA-372)", () => {
  it.each(openers)("%s on #/tasks/project/<id> opens the drawer with the project", (_name, open) => {
    start("#/tasks/project/p1");
    expect(request()).toBeNull();
    open();
    expect(request()).toEqual({ kind: "task", stateId: undefined, projectId: "p1" });
  });

  it.each(openers)("%s on #/tasks opens the drawer without a project, for the drawer to pick", (_name, open) => {
    start("#/tasks");
    open();
    expect(request()).toEqual({ kind: "task", stateId: undefined });
    expect(request()).not.toHaveProperty("projectId");
  });

  it("a column's + on a project's board opens the drawer with the column and the project", () => {
    start("#/board/project/p2");
    (page().props.onNewTask as (s?: string) => void)("s-todo");
    render();
    expect(request()).toEqual({ kind: "task", stateId: "s-todo", projectId: "p2" });
  });

  it("follows the address: after the sidebar's Tasks it no longer picks the project, and on another project's tasks that one", () => {
    start("#/tasks/project/p1");
    pressN();
    expect(request()).toMatchObject({ projectId: "p1" });
    close();
    goTo("#/tasks");
    pressN();
    expect(request()).not.toHaveProperty("projectId");
    close();
    goTo("#/tasks/project/p2");
    pressN();
    expect(request()).toMatchObject({ projectId: "p2" });
    close();
    // in the Inbox of one project, that project; on a project's own page the drawer isn't given one by App
    goTo("#/inbox/project/p1");
    pressN();
    expect(request()).toMatchObject({ projectId: "p1" });
    close();
    goTo("#/project/p1");
    pressN();
    expect(request()).not.toHaveProperty("projectId");
  });

  it("an old project kept by an older Hersir is never what New task picks", () => {
    store.set("gizai-task-project", "p9");
    start("#/tasks");
    pressN();
    expect(request()).toEqual({ kind: "task", stateId: undefined });
  });
});

describe("nothing writes the old kept project any more (GA-372)", () => {
  const read = (path: string): string => fs.readFileSync(new URL(`./${path}`, import.meta.url), "utf8");
  // every source file of the app, without the tests
  const files = (fs.readdirSync(new URL("./", import.meta.url), { recursive: true }) as string[])
    .map((f) => f.replace(/\\/g, "/")).filter((f) => /\.tsx?$/.test(f) && !/\.test\.tsx?$/.test(f));
  it("only the Tasks page names gizai-task-project, to clear it; the drawer keeps its own pick under gizai-new-task-project", () => {
    expect(files.length).toBeGreaterThan(150);
    expect(files.filter((f) => read(f).includes("gizai-task-project"))).toEqual(["pages/TasksPage.tsx"]);
    const tasks = read("pages/TasksPage.tsx");
    // the only use of the key there is the clearing: no setItem with it
    expect(tasks).not.toMatch(/setItem\(\s*(OLD_PROJECT_KEY|"gizai-task-project")/);
    expect(read("components/NewTaskDrawer.tsx")).toContain('"gizai-new-task-project"');
  });
});
