// GA-291 QA: the sidebar closes to a small toolbar at the top left (design/islands/README.md → A sidebar that closes).
// Close sidebar sits at the top right of the sidebar island, on New task's row; the closed sidebar's toolbar has Show
// sidebar, New task, Search and the Inbox with its count, each doing what the sidebar's own button does, each with the
// notes' tooltip (its title); whether it is closed is kept on this computer (gizai.sidebar.closed). As in
// SidebarFold.test.tsx the sidebar and the toolbar are rendered to HTML on the server with their data handed in, and a
// click is tried on the element tree (components called as functions); React's state keeps its values in `slots` by call
// order, so emptying `slots` is a restart, which reads localStorage again (a Map here).
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { Member, Project } from "../types";

const slots: unknown[] = [];
let slot = 0;
const layout: (() => unknown)[] = [];
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = ((init?: unknown) => {
    const k = slot++;
    if (!(k in slots)) slots[k] = typeof init === "function" ? (init as () => unknown)() : init;
    return [slots[k], (v: unknown) => { slots[k] = typeof v === "function" ? (v as (x: unknown) => unknown)(slots[k]) : v; }];
  }) as unknown as typeof R.useState;
  const useEffect = (() => {}) as typeof R.useEffect;
  const useLayoutEffect = ((f: () => unknown) => { layout.push(f); }) as typeof R.useLayoutEffect;
  const useRef = ((current: unknown) => ({ current })) as typeof R.useRef;
  const useCallback = (<T,>(f: T) => f) as typeof R.useCallback;
  const useSyncExternalStore = ((_subscribe: unknown, snapshot: () => unknown) => snapshot()) as typeof R.useSyncExternalStore;
  const hooks = { useState, useEffect, useLayoutEffect, useRef, useCallback, useSyncExternalStore };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});

const state = vi.hoisted(() => ({ data: {} as Record<string, unknown>, open: vi.fn(), go: vi.fn() }));
vi.mock("../api", async (orig) => {
  // Each list the sidebar and the toolbar load names itself, so the replaced useData hands back its fixture at once.
  const as = (key: string) => () => { const p = { key, catch: () => p }; return p; };
  return { ...(await orig<object>()), listProjects: as("projects"), listTasks: as("tasks"), getTeam: as("team"), memoryNotes: as("notes"), listChatThreads: as("threads") };
});
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => { key: string }) => ({ data: state.data[fetch().key] ?? null, error: null, reload: () => {}, setData: () => {} }),
}));
vi.mock("../lib/team", async (orig) => ({ ...(await orig<object>()), useCurrentTeam: () => ["t1", () => {}] }));
vi.mock("../lib/useLiveRuns", () => ({ useLiveRuns: () => [] }));
vi.mock("../lib/drawers", async (orig) => ({ ...(await orig<object>()), useDrawer: () => state.open }));
vi.mock("../router", async (orig) => ({ ...(await orig<object>()), go: state.go }));
vi.mock("./chat/useChat", () => ({ useChatLive: () => [] }));
vi.mock("./UpdateNotice", () => ({ UpdateNotice: () => null }));

const { Sidebar } = await import("./Sidebar");
const { SidebarToolbar } = await import("./SidebarToolbar");
const { CLOSED_KEY, useSidebarClosed } = await import("../lib/sidebar");

const store = new Map<string, string>();
vi.stubGlobal("localStorage", {
  getItem: (k: string) => store.get(k) ?? null,
  setItem: (k: string, v: string) => { store.set(k, String(v)); },
  removeItem: (k: string) => { store.delete(k); },
});

const you: Member = { actorId: "u1", name: "Jeffrey", kind: "user", roleKey: "owner", handle: "jeffrey", status: "active", isLead: false, allowedTools: [], chatEnabled: false };
const project: Project = {
  id: "kade", name: "Kade portal", color: "var(--c-yellow)", status: "active", number: "1", key: "KADE", defaultBranch: "main", openTasks: 0, doneTasks: 0,
  updatedAt: 1, aiCostUsdMicros: 0, aiUnknownCostRuns: 0, worktreeCopy: [], worktreeInstall: false,
  screenshots: { on: true, sizes: [{ name: "Desktop", width: 1280, height: 800 }], bothThemes: false, fullPage: false }, previewCommand: null, previewStartSecs: 120,
};
/** Cards and chats: `cards` cards in Review for you, `held` held ones, `chats` chats of the Chief that wait for you, and
 *  some that don't need you (done, in progress, someone else's review, a chat already answered). */
function data(cards: number, held: number, chats: number) {
  const card = (stateCategory: string, more: Record<string, unknown> = {}) => ({ id: `c${Math.random()}`, stateCategory, assigneeId: null, hold: null, ...more });
  state.data = {
    projects: [project],
    team: { id: "t1", name: "Software", members: [you], states: [], labels: [] },
    notes: [],
    tasks: [
      ...Array.from({ length: cards }, () => card("review", { assigneeId: "u1" })),
      ...Array.from({ length: held }, () => card("started", { hold: "question" })),
      card("done", { hold: "question" }), card("started"), card("review", { assigneeId: "someone-else" }),
    ],
    threads: [
      ...Array.from({ length: chats }, (_, i) => ({ id: `t${i}`, kind: "question", waiting: true, updatedAt: i })),
      { id: "answered", kind: "question", waiting: false, updatedAt: 99 }, { id: "mine", kind: null, waiting: false, updatedAt: 98 },
    ],
  };
}

const sideProps = (more: Record<string, unknown> = {}) => ({ route: { page: "tasks" } as const, youId: "u1", onSearch: vi.fn(), onNewTask: vi.fn(), onClose: vi.fn(), ...more });
const toolProps = (more: Record<string, unknown> = {}) => ({ youId: "u1", onShow: vi.fn(), onSearch: vi.fn(), onNewTask: vi.fn(), onWidth: vi.fn(), ...more });
const sideHtml = (props = sideProps()) => { slot = 0; return renderToStaticMarkup(<Sidebar {...props} />); };
const toolHtml = (props = toolProps()) => { slot = 0; layout.length = 0; return renderToStaticMarkup(<SidebarToolbar {...props} />); };

type Props = Record<string, unknown> & { children?: ReactNode };
/** The elements of a component's tree (components called as functions, lucide's icons left as they are). */
function tree(root: ReactNode): ReactElement<Props>[] {
  slot = 0;
  const all: ReactElement<Props>[] = [];
  const walk = (n: ReactNode): void => {
    if (!n || typeof n !== "object") return;
    if (Array.isArray(n)) { n.forEach(walk); return; }
    const el = n as ReactElement<Props>;
    if (typeof el.type === "function" && !(el.type as { displayName?: string }).displayName?.startsWith("Lucide")
      && !String((el.type as { name?: string }).name).match(/^[A-Z][a-zA-Z]*Icon$/) && el.type.length <= 1 && !("render" in (el.type as object))) {
      try { walk((el.type as (p: unknown) => ReactNode)(el.props)); return; } catch { /* an icon or a forwardRef: kept as it is */ }
    }
    all.push(el);
    walk(el.props?.children);
  };
  walk(root);
  return all;
}
const byLabel = (els: ReactElement<Props>[], label: string) => {
  const found = els.filter((el) => el.props["aria-label"] === label);
  expect(found, `one element named ${label}`).toHaveLength(1);
  return found[0]!.props;
};
/** The toolbar's buttons and link in order: what a screen reader names them, their tooltip, and their tag. */
function toolbarItems(page: string) {
  const inner = /<div class="side-tools" role="group" aria-label="Sidebar actions">([\s\S]*)<\/div>$/.exec(page)?.[1] ?? "";
  return [...inner.matchAll(/<(button|a)\b([^>]*)>/g)].map((m) => ({
    tag: m[1], label: /aria-label="([^"]*)"/.exec(m[2] ?? "")?.[1], title: /title="([^"]*)"/.exec(m[2] ?? "")?.[1], attrs: m[2] ?? "",
  }));
}
const unescape = (s: string | undefined) => (s ?? "").replace(/&quot;/g, '"').replace(/&amp;/g, "&");

beforeEach(() => {
  slots.length = 0; store.clear(); state.open.mockReset(); state.go.mockReset();
  vi.unstubAllGlobals();
  vi.stubGlobal("localStorage", {
    getItem: (k: string) => store.get(k) ?? null,
    setItem: (k: string, v: string) => { store.set(k, String(v)); },
    removeItem: (k: string) => { store.delete(k); },
  });
  data(2, 1, 1);
});

describe("Close sidebar, in the sidebar island (GA-291)", () => {
  it("sits at the top of the sidebar, on New task's row, after New task, which keeps its N key", () => {
    const page = sideHtml();
    const main = page.slice(page.indexOf('<div class="side-main">'));
    expect(main).toMatch(/^<div class="side-main"><div class="side-top"><button class="nav-item">.*?<span>New task<\/span><span class="meta"><span class="kbd">N<\/span><\/span><\/button><button type="button" class="tool" data-side-toggle="true" aria-label="Close sidebar"/);
  });
  it("is a tool button with panel-left-close, its tooltip naming it and Ctrl+\\", () => {
    const page = sideHtml();
    const button = /<button type="button" class="tool" data-side-toggle="true" ([^>]*)>(<svg[^>]*>)/.exec(page);
    expect(button).not.toBeNull();
    expect(unescape(button![1])).toBe('aria-label="Close sidebar" aria-expanded="true" aria-controls="sidebar" title="Close sidebar (Ctrl+\\)"');
    expect(button![2]).toMatch(/class="lucide lucide-panel-left-close icon"/);
  });
  it("controls the sidebar island, whose id is sidebar", () => {
    expect(sideHtml()).toMatch(/^<aside class="side" id="sidebar" aria-label="Navigation">/);
  });
  it("says Cmd+\\ on macOS", () => {
    vi.stubGlobal("navigator", { userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15" });
    expect(unescape(sideHtml())).toContain('title="Close sidebar (Cmd+\\)"');
  });
  it("closes the sidebar when clicked (onClose), and New task still opens the drawer", () => {
    const props = sideProps();
    const els = tree(<Sidebar {...props} />);
    (byLabel(els, "Close sidebar").onClick as () => void)();
    expect(props.onClose).toHaveBeenCalledTimes(1);
    expect(props.onNewTask).not.toHaveBeenCalled();
    const newTask = els.find((el) => el.type === "button" && el.props.className === "nav-item" && JSON.stringify(el.props.children).includes("New task"));
    (newTask!.props.onClick as () => void)();
    expect(props.onNewTask).toHaveBeenCalledTimes(1);
    expect(props.onClose).toHaveBeenCalledTimes(1);
  });
});

describe("the closed sidebar's toolbar (GA-291)", () => {
  it("is one group, Sidebar actions, with Show sidebar, New task, Search and Inbox, in that order", () => {
    const items = toolbarItems(toolHtml());
    expect(items.map((i) => i.label)).toEqual(["Show sidebar", "New task", "Search", "Inbox, 4 need you"]);
    expect(items.map((i) => i.tag)).toEqual(["button", "button", "button", "a"]);
    expect(items.slice(0, 3).every((i) => /type="button"/.test(i.attrs) && /class="tool"/.test(i.attrs))).toBe(true);
    expect(items[3]!.attrs).toContain('class="tool"');
  });
  it("has the notes' tooltips: each name with its key, the Inbox with its count", () => {
    expect(toolbarItems(toolHtml()).map((i) => unescape(i.title))).toEqual(["Show sidebar (Ctrl+\\)", "New task (N)", "Search (Ctrl K)", "Inbox (4 need you)"]);
  });
  it("says Cmd on macOS, as the sidebar's own kbd does", () => {
    vi.stubGlobal("navigator", { userAgent: "Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) AppleWebKit/605.1.15" });
    expect(toolbarItems(toolHtml()).map((i) => unescape(i.title))).toEqual(["Show sidebar (Cmd+\\)", "New task (N)", "Search (Cmd K)", "Inbox (4 need you)"]);
  });
  it("draws the notes' icons: panel-left-open, square-pen, search and inbox", () => {
    const icons = [...toolHtml().matchAll(/class="lucide lucide-([a-z-]+) icon"/g)].map((m) => m[1]);
    expect(icons).toEqual(["panel-left-open", "square-pen", "search", "inbox"]);
  });
  it("Show sidebar says the sidebar is closed and is the toggle the focus moves to", () => {
    const show = toolbarItems(toolHtml())[0]!;
    expect(show.attrs).toContain('aria-expanded="false"');
    expect(show.attrs).toContain('data-side-toggle="true"');
    expect(toolbarItems(toolHtml()).filter((i) => i.attrs.includes("data-side-toggle"))).toHaveLength(1);
  });
  it("each button does what the sidebar's does: Show sidebar opens it, New task opens the drawer, Search the palette", () => {
    const props = toolProps();
    const els = tree(<SidebarToolbar {...props} />);
    (byLabel(els, "Show sidebar").onClick as () => void)();
    expect(props.onShow).toHaveBeenCalledTimes(1);
    (byLabel(els, "New task").onClick as () => void)();
    expect(props.onNewTask).toHaveBeenCalledTimes(1);
    (byLabel(els, "Search").onClick as () => void)();
    expect(props.onSearch).toHaveBeenCalledTimes(1);
    expect(props.onShow).toHaveBeenCalledTimes(1);
    expect(props.onNewTask).toHaveBeenCalledTimes(1);
  });
  it("its Inbox goes where the sidebar's Inbox row goes", () => {
    const sideInbox = /<a class="nav-item" href="([^"]*)"><svg[^>]*lucide-inbox[^>]*>.*?<span>Inbox<\/span>/.exec(sideHtml())?.[1];
    expect(sideInbox).toBe("#/inbox");
    expect(/href="([^"]*)"/.exec(toolbarItems(toolHtml())[3]!.attrs)?.[1]).toBe(sideInbox);
  });
  it("shows the same Inbox count as the sidebar, on the same cards and chats", () => {
    for (const [cards, held, chats] of [[2, 1, 1], [0, 0, 1], [5, 3, 0], [12, 0, 0]] as const) {
      data(cards, held, chats);
      const sideCount = /<span>Inbox<\/span><span class="meta"><span class="count alert">(\d+)<\/span>/.exec(sideHtml())?.[1];
      const toolCount = /<span class="count" aria-hidden="true">(\d+)<\/span>/.exec(toolHtml())?.[1];
      expect(sideCount, `the sidebar's count for ${cards}, ${held}, ${chats}`).toBe(String(cards + held + chats));
      expect(toolCount, `the toolbar's count for ${cards}, ${held}, ${chats}`).toBe(sideCount);
    }
  });
  it("the count is the pill after the icon, hidden from screen readers (the Inbox's name says it)", () => {
    const page = toolHtml();
    expect(page).toMatch(/<svg[^>]*lucide-inbox[^>]*>.*?<\/svg><span class="count" aria-hidden="true">4<\/span><\/a>/);
  });
  it("with nothing waiting the pill goes, and the tooltip is just Inbox, as the sidebar has no count then", () => {
    data(0, 0, 0);
    const page = toolHtml();
    expect(page).not.toContain('class="count"');
    expect(toolbarItems(page)[3]).toMatchObject({ label: "Inbox", title: "Inbox" });
    expect(sideHtml()).not.toMatch(/<span>Inbox<\/span><span class="meta">/);
  });
  it("tells App its width, rounded up, so the page's top bar starts after it", () => {
    const props = toolProps();
    const els = tree(<SidebarToolbar {...props} />);
    const group = els.find((el) => el.props.role === "group")!;
    (group.props as unknown as { ref: { current: unknown } }).ref.current = { getBoundingClientRect: () => ({ width: 132.4 }) };
    for (const f of layout.splice(0)) f();
    expect(props.onWidth).toHaveBeenCalledWith(133);
  });
});

describe("closed or open, kept on this computer (GA-291)", () => {
  /** A start of the app: useSidebarClosed as App calls it, its state read from localStorage again after a restart. */
  const start = () => { slot = 0; return useSidebarClosed(); };
  const restart = () => { slots.length = 0; return start(); };

  it("is open on a computer that never closed it, and keeps nothing", () => {
    expect(restart()[0]).toBe(false);
    expect(store.has(CLOSED_KEY)).toBe(false);
  });
  it("closing keeps '1' at once, and after a restart it is still closed", () => {
    restart()[1](true);
    expect(store.get(CLOSED_KEY)).toBe("1");
    expect(start()[0]).toBe(true);
    expect(restart()[0]).toBe(true);
  });
  it("Show sidebar (or Ctrl+\\ again) after a restart opens it, keeps nothing, and the next restart is open", () => {
    store.set(CLOSED_KEY, "1");
    const [closed, set] = restart();
    expect(closed).toBe(true);
    set(false);
    expect(store.has(CLOSED_KEY)).toBe(false);
    expect(start()[0]).toBe(false);
    expect(restart()[0]).toBe(false);
  });
  it("toggles as Ctrl+\\ does (a function of the last value), keeping each step", () => {
    const toggle = restart()[1];
    toggle((was) => !was);
    expect(store.get(CLOSED_KEY)).toBe("1");
    toggle((was) => !was);
    expect(store.has(CLOSED_KEY)).toBe(false);
    toggle((was) => !was);
    expect(restart()[0]).toBe(true);
  });
  it("leaves the folded sections as they are", () => {
    store.set("gizai.sidebar.folded", "agents,memory");
    restart()[1](true);
    restart()[1](false);
    expect(store.get("gizai.sidebar.folded")).toBe("agents,memory");
  });
  it("opens, and still closes for this start, when localStorage can't be used (private mode)", () => {
    vi.stubGlobal("localStorage", { getItem: () => { throw new Error("denied"); }, setItem: () => { throw new Error("denied"); }, removeItem: () => { throw new Error("denied"); } });
    const [closed, set] = restart();
    expect(closed).toBe(false);
    expect(() => set(true)).not.toThrow();
    expect(start()[0]).toBe(true);
  });
});
