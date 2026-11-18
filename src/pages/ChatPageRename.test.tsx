// GA-309 QA: Rename on a chat on the left of the Chat page, under Pinned and Recent. Each row has a Rename button (a pencil,
// "Rename" as its label and tooltip) beside its pin button, outside the chat's link, so pressing it doesn't open the chat; a
// double-click on the row and F2 on its link start the rename too. The title becomes an input with the chat's name (all
// selected, at most 80 characters, "Chat's name"); Enter or leaving it saves, Escape cancels, and an empty, blank or
// unchanged name saves nothing. The row (and the top bar) shows the new name at once; a rename the backend refuses shows
// the old name again and the toast "Couldn't rename the chat: …".
// As in SidebarClose.test.tsx, components are called as functions to try clicks and keys on their element tree, and
// rendered to HTML on the server to read them. React's state and refs keep their values in `slots`, by call order, per
// `space` (the page, a row, an input): emptying a space is a fresh mount.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { ChatStatus, ChatThread, Member } from "../types";
import type { Pins } from "../components/chat/usePins";

const h = vi.hoisted(() => ({
  spaces: {} as Record<string, unknown[]>,
  space: "page",
  slot: 0,
  effects: [] as (() => unknown)[],
  answers: {} as Record<string, (...a: unknown[]) => unknown>,
  calls: [] as [string, unknown[]][],
  go: [] as unknown[],
}));
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const cell = (init: () => unknown) => {
    const slots = (h.spaces[h.space] ??= []);
    const k = h.slot++;
    if (!(k in slots)) slots[k] = init();
    return [slots, k] as const;
  };
  const useState = ((init?: unknown) => {
    const [slots, k] = cell(() => (typeof init === "function" ? (init as () => unknown)() : init));
    return [slots[k], (v: unknown) => { slots[k] = typeof v === "function" ? (v as (x: unknown) => unknown)(slots[k]) : v; }];
  }) as unknown as typeof R.useState;
  const useRef = ((current: unknown) => { const [slots, k] = cell(() => ({ current })); return slots[k]; }) as typeof R.useRef;
  const useEffect = ((f: () => unknown) => { h.effects.push(f); }) as typeof R.useEffect;
  const useMemo = (<T,>(f: () => T) => f()) as typeof R.useMemo;
  const useCallback = (<T,>(f: T) => f) as typeof R.useCallback;
  const hooks = { useState, useRef, useEffect, useMemo, useCallback };
  return { ...R, default: { ...R, ...hooks }, ...hooks };
});
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) {
    out[k] = typeof v !== "function" ? v : (...a: unknown[]) => { h.calls.push([k, a]); return h.answers[k] ? h.answers[k](...a) : new Promise(() => {}); };
  }
  return out;
});
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    let data: unknown = null;
    try { const v = fetch(); if (!(v instanceof Promise)) data = v; } catch { /* no data */ }
    return { data, error: null, reload: () => {}, setData: () => {} };
  },
}));
vi.mock("../lib/drawers", async (orig) => ({ ...(await orig<object>()), useDrawer: () => () => {} }));
vi.mock("../router", async (orig) => ({ ...(await orig<object>()), go: (to: unknown) => { h.go.push(to); } }));
let live: ChatStatus[] = [];
vi.mock("../components/chat/useChat", () => ({ useChatLive: () => live }));
let pinned: ChatThread[] = [];
vi.mock("../components/chat/usePins", () => ({
  usePins: (): Pins => ({ list: pinned, has: (id) => pinned.some((t) => t.id === id), toggle: () => {}, error: "" }),
}));
vi.mock("../components/chat/ChatThread", () => ({
  ChatThread: ({ threadId, thread }: { threadId: string | null; thread?: ChatThread }) =>
    <section data-conversation={threadId ?? "new"}>{thread ? `title=${thread.title}` : "no thread"}</section>,
}));
vi.mock("../components/chat/ChatSide", () => ({
  ChatSide: () => null,
  OpenButton: () => <button type="button" className="btn sm chat-open">Open</button>,
  useChatSideLayout: () => ({ chatRef: null, className: "chat-layout" }),
}));

const { ChatPage } = await import("./ChatPage");
const { ChatNameInput, RenameButton } = await import("../components/chat/RenameChat");

const HOUR = 3_600_000;
const now = Date.now();
const lead = { actorId: "lead", name: "Team Lead", kind: "agent", roleKey: "lead", handle: "lead", status: "active", isLead: false, allowedTools: [],
  chatEnabled: true } as Member;
const chat = (n: number, more: Partial<ChatThread> = {}): ChatThread => ({
  id: `c${n}`, agentId: "lead", title: `Chat ${n}`, createdAt: now - n * HOUR, updatedAt: now - n * HOUR, costUsdMicros: 0, inputTokens: 0, outputTokens: 0, ...more,
});
const atOnce = <T,>(v: T) => ({ then: (f: (x: T) => unknown) => f(v) });
const settle = () => new Promise((r) => setTimeout(r, 0));

let threads: ChatThread[] = [];

type El = ReactElement<Record<string, unknown> & { children?: ReactNode }>;
const isEl = (n: unknown): n is El => !!n && typeof n === "object" && "props" in (n as object);
/** Every element in a tree (components not called), depth first. */
function all(n: ReactNode): El[] {
  if (Array.isArray(n)) return n.flatMap(all);
  if (!isEl(n)) return [];
  return [n, ...all(n.props.children as ReactNode)];
}
const named = (n: ReactNode, name: string) => all(n).filter((e) => typeof e.type === "function" && (e.type as { name: string }).name === name);

/** The page, called as a function in its own space. */
function page(id?: string): ReactNode {
  h.space = "page";
  h.slot = 0;
  return ChatPage({ id }) as ReactNode;
}
/** A row's ThreadRow element on the page (its type and props: the chat, pins, renames). */
function rowEl(id: string, pageId?: string) {
  const el = named(page(pageId), "ThreadRow").find((e) => (e.props.t as ChatThread).id === id);
  if (!el) throw new Error(`no row for ${id}`);
  return el;
}
/** ThreadRow called as a function in the row's space: its div.th-row. */
function row(id: string, pageId?: string): El {
  const el = rowEl(id, pageId);
  h.space = `row:${id}`;
  h.slot = 0;
  h.effects = [];
  return (el.type as (p: unknown) => El)(el.props);
}
const kids = (r: El) => ([] as ReactNode[]).concat(r.props.children).filter(isEl) as El[];
const html = (n: ReactNode) => { h.space = "html"; h.slot = 0; h.spaces.html = []; return renderToStaticMarkup(<>{n}</>); };
/** The page as HTML (its state kept: the page's space is read again from its start). */
const pageHtml = (id?: string) => { h.space = "page"; h.slot = 0; return renderToStaticMarkup(<ChatPage id={id} />); };

const ev = (more: Record<string, unknown> = {}) => ({
  preventDefault: vi.fn(), stopPropagation: vi.fn(), nativeEvent: { isComposing: false }, detail: 1, currentTarget: { select: vi.fn() }, ...more,
});

beforeEach(() => {
  h.spaces = {};
  h.effects = [];
  h.calls = [];
  h.go = [];
  for (const k of Object.keys(h.answers)) delete h.answers[k];
  live = [];
  pinned = [];
  threads = Array.from({ length: 6 }, (_, i) => chat(i));
  h.answers.chatAgent = () => atOnce(lead);
  h.answers.listChatThreads = () => threads;
});

describe("the Rename button on the left", () => {
  const left = (s: string) => s.slice(s.indexOf('class="chat-threads-list"'), s.indexOf('class="chat-threads-foot"'));
  const rows = (s: string) => left(s).split('<div class="th-row').slice(1);

  it("is on every chat under Recent and under Pinned, a pencil with Rename as its label and tooltip", () => {
    pinned = [chat(3), chat(40, { title: "An old plan" })];
    const r = rows(pageHtml());
    expect(r).toHaveLength(7); // 2 pinned, 5 in Recent
    for (const x of r) {
      expect(x).toContain('<button type="button" class="btn ghost sm icon-only rename-btn" aria-label="Rename" title="Rename">');
      expect(x).toContain("lucide-pencil");
    }
  });

  it("sits beside the pin button, outside the chat's link; New chat and Archive have none", () => {
    pinned = [chat(3)];
    const s = pageHtml();
    for (const x of rows(s)) {
      const link = x.slice(x.indexOf("<a "), x.indexOf("</a>") + 4);
      expect(link).not.toContain("rename-btn");
      expect(x.indexOf("</a>")).toBeLessThan(x.indexOf("rename-btn"));
      // the pin button and Rename are both after the link, next to each other
      expect(x).toMatch(/<\/a><button[^>]*pin-btn[\s\S]*?<\/button><button[^>]*rename-btn/);
    }
    expect(left(s).match(/rename-btn/g)).toHaveLength(6);
    expect(s.slice(s.indexOf('class="chat-threads-foot"'))).not.toContain("rename-btn");
  });

  it("starts the rename when pressed, without opening the chat", () => {
    const r = row("c2");
    const [a, , rename] = kids(r);
    expect(a.type).toBe("a");
    expect(a.props.href).toBe("#/chat/c2");
    expect((rename.type as { name: string }).name).toBe("RenameButton");
    const button = (rename.type as (p: unknown) => El)(rename.props);
    expect(button.type).toBe("button"); // a button: Enter and Space press it too
    expect(button.props.type).toBe("button");
    const e = ev();
    (button.props.onClick as (e: unknown) => void)(e);
    expect(e.preventDefault).toHaveBeenCalled();
    expect(e.stopPropagation).toHaveBeenCalled();
    expect(h.go).toEqual([]);
    const again = row("c2");
    expect(again.props.className).toBe("th-row renaming");
    const [head, , gone] = kids(again);
    expect(head.type).toBe("div"); // no link while renaming: a click in the input can't open the chat
    expect(head.props.href).toBeUndefined();
    expect(gone).toBeUndefined(); // Rename hides while the input shows
    expect(named(again, "ChatNameInput")).toHaveLength(1);
  });

  it("is a RenameButton that calls onRename once per press", () => {
    const onRename = vi.fn();
    const b = RenameButton({ onRename }) as El;
    (b.props.onClick as (e: unknown) => void)(ev());
    expect(onRename).toHaveBeenCalledTimes(1);
  });

  it("works the same on a pinned chat", () => {
    pinned = [chat(4)];
    const [, pin, rename] = kids(row("c4"));
    expect(pin.props.pinned).toBe(true);
    ((rename.type as (p: unknown) => El)(rename.props).props.onClick as (e: unknown) => void)(ev());
    expect(row("c4").props.className).toBe("th-row pinned renaming");
  });
});

describe("a double-click and F2 on a row", () => {
  it("a double-click starts the rename; its first click is an ordinary click on the link", () => {
    const [a] = kids(row("c1"));
    const first = ev({ detail: 1 });
    (a.props.onMouseDown as (e: unknown) => void)(first);
    expect(first.preventDefault).not.toHaveBeenCalled(); // it opens the chat, as before
    const second = ev({ detail: 2 });
    (a.props.onMouseDown as (e: unknown) => void)(second);
    expect(second.preventDefault).toHaveBeenCalled(); // no word of the title gets selected
    const dbl = ev();
    (a.props.onDoubleClick as (e: unknown) => void)(dbl);
    expect(dbl.preventDefault).toHaveBeenCalled();
    expect(row("c1").props.className).toBe("th-row renaming");
  });

  it("F2 on the focused link starts the rename; other keys do what they did", () => {
    const [a] = kids(row("c1"));
    const enter = ev({ key: "Enter" });
    (a.props.onKeyDown as (e: unknown) => void)(enter);
    expect(enter.preventDefault).not.toHaveBeenCalled(); // Enter still opens the chat
    expect(row("c1").props.className).toBe("th-row");
    const f2 = ev({ key: "F2" });
    (kids(row("c1"))[0].props.onKeyDown as (e: unknown) => void)(f2);
    expect(f2.preventDefault).toHaveBeenCalled();
    expect(row("c1").props.className).toBe("th-row renaming");
  });

  it("F2 on the pin or Rename button isn't the link's: they have no F2", () => {
    const [, pin, rename] = kids(row("c1"));
    const pinButton = (pin.type as (p: unknown) => El)(pin.props);
    const renameButton = (rename.type as (p: unknown) => El)(rename.props);
    expect(pinButton.props.onKeyDown).toBeUndefined();
    expect(renameButton.props.onKeyDown).toBeUndefined();
  });
});

describe("the name's input", () => {
  const input = (name: string, onEnd = vi.fn()) => { h.space = "input"; h.slot = 0; return { el: ChatNameInput({ name, onEnd }) as El, onEnd }; };
  const fresh = () => { h.spaces.input = []; };

  it("holds the chat's current name, all selected, focused, at most 80 characters, called Chat's name", () => {
    fresh();
    const { el } = input("Chat 2");
    expect(el.type).toBe("input");
    expect(el.props.value).toBe("Chat 2");
    expect(el.props.maxLength).toBe(80);
    expect(el.props["aria-label"]).toBe("Chat's name");
    expect(el.props.autoFocus).toBe(true);
    const select = vi.fn();
    (el.props.onFocus as (e: unknown) => void)({ currentTarget: { select } });
    expect(select).toHaveBeenCalled();
    const s = html(el);
    expect(s).toMatch(/^<input class="input name-in"/);
    for (const a of ['value="Chat 2"', 'maxLength="80"', 'aria-label="Chat&#x27;s name"', 'autofocus=""']) expect(s).toContain(a);
  });

  it("shows on the row with the name the row shows, in place of the title", () => {
    const [a] = kids(row("c2"));
    (a.props.onDoubleClick as (e: unknown) => void)(ev());
    const r = row("c2");
    const [inp] = named(r, "ChatNameInput");
    expect(inp.props.name).toBe("Chat 2");
    const s = html(r);
    expect(s).toContain('value="Chat 2"');
    expect(s).not.toContain('<span class="ellipsis">Chat 2</span>');
    expect(s).toContain('<span class="when">2h ago</span>');
  });

  it("Enter keeps the typed name; Escape keeps nothing", () => {
    fresh();
    let { el, onEnd } = input("Chat 2");
    (el.props.onChange as (e: unknown) => void)({ target: { value: "Hersir sidebar" } });
    ({ el } = input("Chat 2", onEnd));
    expect(el.props.value).toBe("Hersir sidebar");
    const enter = ev({ key: "Enter" });
    (el.props.onKeyDown as (e: unknown) => void)(enter);
    expect(enter.preventDefault).toHaveBeenCalled();
    expect(onEnd).toHaveBeenCalledWith("Hersir sidebar", true);

    fresh();
    ({ el, onEnd } = input("Chat 2"));
    (el.props.onChange as (e: unknown) => void)({ target: { value: "Something" } });
    ({ el } = input("Chat 2", onEnd));
    (el.props.onKeyDown as (e: unknown) => void)(ev({ key: "Escape" }));
    expect(onEnd).toHaveBeenCalledWith(null, true);
  });

  it("leaving it keeps the name; it ends once (the blur after Enter or Escape does nothing)", () => {
    fresh();
    let { el, onEnd } = input("Chat 2");
    (el.props.onChange as (e: unknown) => void)({ target: { value: "Kept by a click outside" } });
    ({ el } = input("Chat 2", onEnd));
    (el.props.onBlur as () => void)();
    expect(onEnd).toHaveBeenCalledWith("Kept by a click outside", false);
    (el.props.onKeyDown as (e: unknown) => void)(ev({ key: "Enter" }));
    expect(onEnd).toHaveBeenCalledTimes(1);

    fresh();
    ({ el, onEnd } = input("Chat 2"));
    (el.props.onKeyDown as (e: unknown) => void)(ev({ key: "Escape" }));
    (el.props.onBlur as () => void)();
    expect(onEnd.mock.calls).toEqual([[null, true]]);
  });

  it("doesn't end on Enter while a word is being composed (an input method)", () => {
    fresh();
    const { el, onEnd } = input("Chat 2");
    (el.props.onKeyDown as (e: unknown) => void)(ev({ key: "Enter", nativeEvent: { isComposing: true } }));
    expect(onEnd).not.toHaveBeenCalled();
  });

  it("keeps clicks, double-clicks and keys to itself", () => {
    fresh();
    const { el } = input("Chat 2");
    for (const k of ["onClick", "onDoubleClick"]) {
      const e = ev();
      (el.props[k] as (e: unknown) => void)(e);
      expect(e.stopPropagation, k).toHaveBeenCalled();
    }
    const f2 = ev({ key: "F2" });
    (el.props.onKeyDown as (e: unknown) => void)(f2);
    expect(f2.stopPropagation).toHaveBeenCalled();
  });
});

describe("saving from the row", () => {
  /** Starts the rename on a row, types `name` and ends with `how`. */
  function renameRow(id: string, name: string, how: "Enter" | "Escape" | "blur") {
    const [a] = kids(row(id));
    (a.props.onDoubleClick as (e: unknown) => void)(ev());
    const [inp] = named(row(id), "ChatNameInput");
    const onEnd = inp.props.onEnd as (name: string | null, byKey: boolean) => void;
    if (how === "Escape") onEnd(null, true);
    else onEnd(name, how === "Enter");
  }
  const renames = () => h.calls.filter(([k]) => k === "renameChat");
  const rowTitle = (id: string) => html(row(id)).match(/<span class="ellipsis">([^<]*)<\/span>/)?.[1];

  it("Enter saves the name (on one line, trimmed) and the row shows it at once", () => {
    h.answers.renameChat = () => new Promise(() => {}); // the backend hasn't answered yet
    renameRow("c2", "  Hersir   sidebar ", "Enter");
    expect(renames()).toEqual([["renameChat", ["c2", "Hersir sidebar"]]]);
    expect(row("c2").props.className).toBe("th-row");
    expect(rowTitle("c2")).toBe("Hersir sidebar");
    // the other rows keep theirs
    expect(rowTitle("c1")).toBe("Chat 1");
  });

  it("the focus goes back to the chat's link after Enter or Escape, and stays where it went after a click outside", () => {
    h.answers.renameChat = () => new Promise(() => {});
    for (const [how, back] of [["Enter", true], ["Escape", true], ["blur", false]] as const) {
      delete h.spaces["row:c3"];
      renameRow("c3", "Another name", how);
      const r = row("c3");
      const [a] = kids(r);
      const focus = vi.fn();
      (a.props.ref as { current: unknown }).current = { focus };
      for (const f of h.effects) f();
      expect(focus.mock.calls.length, how).toBe(back ? 1 : 0);
    }
  });

  it("leaving the input saves too", () => {
    h.answers.renameChat = () => new Promise(() => {});
    renameRow("c2", "By a click outside", "blur");
    expect(renames()).toEqual([["renameChat", ["c2", "By a click outside"]]]);
    expect(rowTitle("c2")).toBe("By a click outside");
  });

  it("Escape saves nothing and the old name stays", () => {
    renameRow("c2", "Not this", "Escape");
    expect(renames()).toEqual([]);
    expect(row("c2").props.className).toBe("th-row");
    expect(rowTitle("c2")).toBe("Chat 2");
  });

  it("an empty name, only spaces or the same name: nothing is saved, the old name stays and no error shows", () => {
    for (const name of ["", "   ", " \n ", "Chat 2", "  Chat   2  "]) {
      delete h.spaces["row:c2"];
      renameRow("c2", name, "Enter");
      expect(renames(), JSON.stringify(name)).toEqual([]);
      expect(rowTitle("c2")).toBe("Chat 2");
      expect(row("c2").props.className).toBe("th-row");
    }
    expect(pageHtml()).not.toContain('class="toast"');
  });

  it("the new name shows in the top bar's crumbs at once when that chat is open", () => {
    h.answers.renameChat = () => new Promise(() => {});
    renameRow("c2", "Hersir sidebar", "Enter");
    const s = pageHtml("c2");
    const crumbs = s.slice(s.indexOf('class="crumbs"'), s.indexOf('class="actions"'));
    expect(crumbs).toContain('<span class="ellipsis">Hersir sidebar</span>');
  });

  it("after the lists reload with the new name, that one counts; the chat keeps its time and place", () => {
    h.answers.renameChat = () => new Promise(() => {});
    renameRow("c2", "Hersir sidebar", "Enter");
    // the notice chat_threads: Recent read again, with the new title and the same updated_at
    threads = threads.map((t) => (t.id === "c2" ? { ...t, title: "Hersir sidebar" } : t));
    const s = pageHtml();
    const order = [...s.matchAll(/href="#\/chat\/(c\d+)"/g)].map((m) => m[1]);
    expect(order).toEqual(["c0", "c1", "c2", "c3", "c4", "c5"]);
    expect(rowTitle("c2")).toBe("Hersir sidebar");
    expect(html(row("c2"))).toContain('<span class="when">2h ago</span>');
    // a later rename from elsewhere (another window) shows too: the lists' title counts once it changed
    threads = threads.map((t) => (t.id === "c2" ? { ...t, title: "From elsewhere" } : t));
    expect(rowTitle("c2")).toBe("From elsewhere");
  });

  it("a pinned chat shows its new name under Pinned and stays there", () => {
    h.answers.renameChat = () => new Promise(() => {});
    pinned = [chat(5), chat(2)];
    renameRow("c2", "Pinned, renamed", "Enter");
    const s = pageHtml();
    const list = s.slice(s.indexOf(">Pinned<"), s.indexOf(">Recent<"));
    expect([...list.matchAll(/href="#\/chat\/(c\d+)"/g)].map((m) => m[1])).toEqual(["c5", "c2"]);
    expect(list).toContain('<span class="ellipsis">Pinned, renamed</span>');
  });

  it("a rename the backend refuses shows the old name again and the toast Couldn't rename the chat: …", async () => {
    h.answers.renameChat = () => Promise.reject("not found: chat c2");
    renameRow("c2", "Too late", "Enter");
    expect(rowTitle("c2")).toBe("Too late"); // at once, before the answer
    await settle();
    expect(rowTitle("c2")).toBe("Chat 2");
    const s = pageHtml();
    expect(s).toContain('<div class="toast" role="status">Couldn&#x27;t rename the chat: not found: chat c2</div>');
  });

  it("a refusal that comes as an Error says its message, without Error:", async () => {
    h.answers.renameChat = () => Promise.reject(new Error("A chat's name is at most 80 characters"));
    renameRow("c2", "x".repeat(80), "Enter");
    await settle();
    expect(pageHtml()).toContain("Couldn&#x27;t rename the chat: A chat&#x27;s name is at most 80 characters</div>");
    expect(rowTitle("c2")).toBe("Chat 2");
  });

  it("the toast goes after 5 seconds", async () => {
    h.answers.renameChat = () => Promise.reject("not found: chat c2");
    renameRow("c2", "Too late", "Enter");
    await settle();
    vi.useFakeTimers();
    try {
      h.effects = [];
      page();
      const cleanups = h.effects.map((f) => f()).filter((c): c is () => void => typeof c === "function");
      expect(pageHtml()).toContain('class="toast"');
      vi.advanceTimersByTime(4999);
      expect(pageHtml()).toContain('class="toast"');
      vi.advanceTimersByTime(1);
      expect(pageHtml()).not.toContain('class="toast"');
      cleanups.forEach((c) => c());
    } finally {
      vi.useRealTimers();
    }
  });

  it("an older refusal doesn't undo a newer name", async () => {
    let refuse: (e: unknown) => void = () => {};
    h.answers.renameChat = (_id, title) => (title === "First" ? new Promise((_, no) => { refuse = no; }) : new Promise(() => {}));
    renameRow("c2", "First", "Enter");
    delete h.spaces["row:c2"];
    renameRow("c2", "Second", "Enter");
    expect(rowTitle("c2")).toBe("Second");
    refuse("not found: chat c2");
    await settle();
    expect(rowTitle("c2")).toBe("Second");
  });

  it("renaming a chat while the Chief answers in it leaves the answer alone", () => {
    h.answers.renameChat = () => new Promise(() => {});
    live = [{ threadId: "c2", runId: "r1", draft: "Let me look", seq: 3 }];
    renameRow("c2", "Database choice", "Enter");
    const s = html(row("c2"));
    expect(s).toContain('class="pulse"');
    expect(s).toContain('<span class="ellipsis">Database choice</span>');
    // nothing but the rename went to the backend: no stop, no new message
    expect(h.calls.map(([k]) => k).filter((k) => !["chatAgent", "listChatThreads", "getChatThread", "pinnedChats"].includes(k))).toEqual(["renameChat"]);
  });
});
