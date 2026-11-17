// GA-291 QA: App closes and opens the sidebar (design/islands/README.md → A sidebar that closes). Ctrl+\ (Cmd+\ on macOS)
// toggles it from any page, also with the focus in a text field, the chat's text box, the Markdown editor or the Editor's
// code, but not from a terminal (xterm), where it stays the shell's; Ctrl+Shift+\ and Ctrl+Alt+\ do nothing. Closed, the
// sidebar goes and the toolbar (SidebarToolbar) shows in its place, its width handed to the page's top bar as
// --side-tools; Close sidebar and Show sidebar do what the key does, and the focus moves to the other one when it was in
// what goes. Kept on this computer (gizai.sidebar.closed). A small hook runtime stands in for React (App runs as a
// function, its effects by their deps), as in AppStart.test.tsx; the sidebar, the toolbar, the pages and the api are fakes.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import type { Route } from "./router";

const h = vi.hoisted(() => ({
  slots: [] as unknown[],
  slot: 0,
  effects: [] as { deps?: unknown[]; cleanup?: unknown }[],
  effect: 0,
  queued: [] as (() => void)[],
  route: { page: "tasks" } as { page: string; id?: string },
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
  return {
    ...R,
    useState, useRef, useEffect,
    useCallback: (f: unknown) => f,
    startTransition: (f: () => void) => f(),
  };
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

// ---- the window, the document and localStorage, as much as App's key handler and focus move use ----
type Fake = { closest: (sel: string) => unknown; focus: () => void };
/** An element inside `inside` (".xterm", "input", ".cm-editor", ".side" …): closest() answers for selectors naming it. */
function el(...inside: string[]): Fake {
  const self: Fake = {
    closest: (sel: string) => (sel.split(",").map((s) => s.trim()).some((s) => inside.includes(s)) ? self : null),
    focus: vi.fn(),
  };
  return self;
}
const toggle = el("[data-side-toggle]");
const doc = {
  activeElement: el() as Fake | null,
  onb: false,
  dialog: false,
  body: { dataset: {} as Record<string, string> },
  documentElement: { dataset: {} as Record<string, string> },
  querySelector: (sel: string) => (sel === ".onb" ? (doc.onb ? {} : null) : sel === "[data-side-toggle]" ? toggle : sel === "[role=dialog]" ? (doc.dialog ? {} : null) : null),
};
vi.stubGlobal("document", doc);
vi.stubGlobal("window", {
  location: { hash: "" },
  addEventListener: (type: string, f: (e: unknown) => void) => { h.listeners.set(type, f); },
  removeEventListener: (type: string, f: unknown) => { if (h.listeners.get(type) === f) h.listeners.delete(type); },
});
const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => { store.set(k, String(v)); },
  removeItem: (k: string) => { store.delete(k); },
});

const { default: App } = await import("./App");

type Props = Record<string, unknown> & { children?: ReactNode; className?: string; style?: Record<string, string> };
let tree: ReactElement<Props>;
/** App's render: the hooks in their slots, then the effects whose deps changed (the key handler among them). */
function render(): ReactElement<Props> {
  h.slot = 0; h.effect = 0;
  tree = (App as (p: object) => ReactElement<Props>)({});
  for (const f of h.queued.splice(0)) f();
  return tree;
}
/** A fresh start: nothing in React's slots, localStorage as it is. */
function start(route: Route | { page: string; id?: string } = { page: "tasks" }) {
  h.slots = []; h.effects = []; h.queued = []; h.listeners.clear(); h.route = route as { page: string };
  return render();
}
/** Every element of the tree (the fakes left as elements). */
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
const app = () => all().find((e) => e.type === "div" && /^app\b/.test(String(e.props.className)))!;
const closed = () => {
  const side = named("Sidebar").length, tools = named("SidebarToolbar").length;
  expect(side + tools, "the sidebar or its toolbar, never both").toBe(1);
  expect(app().props.className === "app side-closed").toBe(tools === 1);
  return tools === 1;
};

type KeyInit = Partial<{ key: string; code: string; ctrlKey: boolean; metaKey: boolean; shiftKey: boolean; altKey: boolean; target: Fake; defaultPrevented: boolean }>;
/** A keydown on the window, as the WebView sends it; App drawn again after. Whether App took it (preventDefault). */
function press(init: KeyInit): boolean {
  let prevented = false;
  const e = { key: "\\", code: "Backslash", ctrlKey: false, metaKey: false, shiftKey: false, altKey: false, defaultPrevented: false, target: el(), ...init, preventDefault: () => { prevented = true; } };
  const on = h.listeners.get("keydown");
  expect(on, "App listens for keys on the window").toBeTypeOf("function");
  on!(e);
  render();
  return prevented;
}
const ctrlBackslash = (more: KeyInit = {}) => press({ ctrlKey: true, ...more });

beforeEach(() => {
  store.clear(); doc.onb = false; doc.dialog = false; doc.activeElement = el(); (toggle.focus as ReturnType<typeof vi.fn>).mockClear();
});

describe("Ctrl+\\ closes and opens the sidebar (GA-291)", () => {
  it("starts open on a computer that never closed it", () => {
    start();
    expect(closed()).toBe(false);
    expect(app().props.style).toBeUndefined();
  });
  it("closes it, the toolbar in its place, and opens it again", () => {
    start();
    expect(ctrlBackslash()).toBe(true);
    expect(closed()).toBe(true);
    expect(store.get("gizai.sidebar.closed")).toBe("1");
    expect(ctrlBackslash()).toBe(true);
    expect(closed()).toBe(false);
    expect(store.has("gizai.sidebar.closed")).toBe(false);
  });
  it("is Cmd+\\ on macOS", () => {
    start();
    expect(press({ metaKey: true })).toBe(true);
    expect(closed()).toBe(true);
    press({ metaKey: true });
    expect(closed()).toBe(false);
  });
  it("is read by the key's place on other layouts (Backslash typing #), or by the \\ it types", () => {
    start();
    ctrlBackslash({ key: "#" });
    expect(closed()).toBe(true);
    ctrlBackslash({ code: "IntlBackslash" });
    expect(closed()).toBe(false);
  });
  const pages: { page: string; id?: string }[] = [
    { page: "chat" }, { page: "chat", id: "c1" }, { page: "chats" }, { page: "tasks" }, { page: "board" }, { page: "inbox" }, { page: "task", id: "t1" },
    { page: "doc", id: "d1" }, { page: "agent", id: "a1" }, { page: "editor" }, { page: "terminal" }, { page: "terminal", id: "s1" }, { page: "memory" },
    { page: "team" }, { page: "usage" }, { page: "settings" }, { page: "clients" }, { page: "client", id: "k1" }, { page: "users" }, { page: "projects" },
    { page: "project", id: "p1" },
  ];
  it.each(pages)("works the same on $page $id", (route) => {
    start(route);
    ctrlBackslash();
    expect(closed()).toBe(true);
    // the page stays in the main column, which now has the width (app.css: .app.side-closed)
    expect(all().some((e) => e.type === "main" && e.props.className === "main")).toBe(true);
    ctrlBackslash();
    expect(closed()).toBe(false);
  });
  const fields: [string, Fake][] = [
    ["a text field", el("input")], ["the chat's text box", el("textarea")], ["the Markdown editor", el(".cm-editor", "[contenteditable]")],
    ["the Editor's code", el(".cm-editor", "[contenteditable]")], ["a select", el("select")],
  ];
  it.each(fields)("works with the focus in %s, and types nothing there", (_, target) => {
    start();
    expect(ctrlBackslash({ target })).toBe(true);
    expect(closed()).toBe(true);
    expect(ctrlBackslash({ target })).toBe(true);
    expect(closed()).toBe(false);
  });
  it("in a terminal stays the shell's: the sidebar stays as it is, open or closed, and the key goes on to xterm", () => {
    start();
    const term = el(".xterm", "textarea");
    expect(ctrlBackslash({ target: term })).toBe(false);
    expect(closed()).toBe(false);
    expect(press({ metaKey: true, target: term })).toBe(false);
    expect(closed()).toBe(false);
    ctrlBackslash();
    expect(closed()).toBe(true);
    expect(ctrlBackslash({ target: term })).toBe(false);
    expect(closed()).toBe(true);
    expect(store.get("gizai.sidebar.closed")).toBe("1");
  });
  it("Ctrl+Shift+\\ and Ctrl+Alt+\\ (and with Cmd) do nothing", () => {
    start();
    for (const more of [{ ctrlKey: true, shiftKey: true, key: "|" }, { ctrlKey: true, altKey: true }, { metaKey: true, shiftKey: true }, { metaKey: true, altKey: true }]) {
      expect(press(more), JSON.stringify(more)).toBe(false);
      expect(closed()).toBe(false);
    }
    expect(store.size).toBe(0);
  });
  it("\\ alone does nothing (it types a backslash)", () => {
    start();
    expect(press({ target: el("input") })).toBe(false);
    expect(press({})).toBe(false);
    expect(closed()).toBe(false);
  });
  it("leaves a key an editor already handled", () => {
    start();
    expect(ctrlBackslash({ defaultPrevented: true })).toBe(false);
    expect(closed()).toBe(false);
  });
  it("Ctrl+K still opens Search with the sidebar closed", () => {
    start();
    ctrlBackslash();
    expect(press({ ctrlKey: true, key: "k", code: "KeyK" })).toBe(true);
    expect(named("CommandPalette")[0]!.props.open).toBe(true);
    expect(closed()).toBe(true);
  });
});

describe("Close sidebar and Show sidebar (GA-291)", () => {
  it("Close sidebar closes it and Show sidebar opens it, as the key does, kept each time", () => {
    start();
    (named("Sidebar")[0]!.props.onClose as () => void)();
    render();
    expect(closed()).toBe(true);
    expect(store.get("gizai.sidebar.closed")).toBe("1");
    (named("SidebarToolbar")[0]!.props.onShow as () => void)();
    render();
    expect(closed()).toBe(false);
    expect(store.has("gizai.sidebar.closed")).toBe(false);
  });
  it("the toolbar's New task and Search do what the sidebar's do", () => {
    start();
    const side = named("Sidebar")[0]!.props;
    ctrlBackslash();
    const tools = named("SidebarToolbar")[0]!.props;
    expect(tools.youId).toBe(side.youId);
    (tools.onSearch as () => void)();
    render();
    expect(named("CommandPalette")[0]!.props.open).toBe(true);
    expect(closed()).toBe(true);
    // New task: the same drawer as the sidebar's New task (and N)
    expect(named("DrawerHost")[0]!.props.request).toBeNull();
    (tools.onNewTask as () => void)();
    render();
    const fromToolbar = named("DrawerHost")[0]!.props.request;
    expect(fromToolbar).toEqual({ kind: "task", stateId: undefined });
    ctrlBackslash();
    (named("Sidebar")[0]!.props.onNewTask as () => void)();
    render();
    expect(named("DrawerHost")[0]!.props.request).toEqual(fromToolbar);
  });
  it("the page's top bar starts after the toolbar: its width goes to --side-tools while closed, and goes when open", () => {
    start();
    ctrlBackslash();
    (named("SidebarToolbar")[0]!.props.onWidth as (px: number) => void)(130);
    render();
    expect(app().props.style).toEqual({ "--side-tools": "130px" });
    ctrlBackslash();
    expect(app().props.style).toBeUndefined();
  });
  it("moves the focus to Show sidebar when it was in the sidebar, and to Close sidebar when it was in the toolbar", () => {
    start();
    doc.activeElement = el(".side");
    (named("Sidebar")[0]!.props.onClose as () => void)();
    render();
    expect(toggle.focus).toHaveBeenCalledTimes(1);
    doc.activeElement = el(".side-tools");
    ctrlBackslash();
    expect(closed()).toBe(false);
    expect(toggle.focus).toHaveBeenCalledTimes(2);
  });
  it("leaves the focus where it was when it was in the page", () => {
    start();
    doc.activeElement = el("textarea");
    ctrlBackslash();
    ctrlBackslash();
    expect(toggle.focus).not.toHaveBeenCalled();
  });
});

describe("kept on this computer (GA-291)", () => {
  it("a start after closing it starts closed, with the toolbar; Show sidebar opens it and the next start is open", () => {
    start();
    ctrlBackslash();
    start();
    expect(closed()).toBe(true);
    (named("SidebarToolbar")[0]!.props.onShow as () => void)();
    render();
    expect(closed()).toBe(false);
    start();
    expect(closed()).toBe(false);
  });
});
