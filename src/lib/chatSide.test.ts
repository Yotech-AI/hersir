// GA-296 QA: the Chief Chat's side island and tool strip, its store (src/lib/chatSide.ts), against the card and
// design/islands-chat/README.md: a tab has a kind and an id; the strip's kinds in the order Terminal, Editor, Board, Card with
// their counts, the shown kind selected; a click on a kind's icon shows its last shown tab, or hides the island while it is on
// show; closing the shown tab shows the one to its right, else its left, and the last one hides the island; the strip's words
// (tooltip, title, the menu's head, the close question for Close all) word for word; Alt+1 to Alt+4 by e.code; and the tabs
// belong to each chat (another chat shows its own; New chat's go to the chat its first message makes). The terminals' list
// is a fake, as in chatTerminal.test.ts.
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { TerminalSession } from "../types";

const f = vi.hoisted(() => ({
  list: [] as TerminalSession[],
  subs: new Set<() => void>(),
}));
const changed = () => f.subs.forEach((g) => g());

vi.mock("react", async (orig) => ({ ...(await orig<typeof import("react")>()), useSyncExternalStore: (_sub: unknown, get: () => unknown) => get() }));
vi.mock("../api", () => ({}));
vi.mock("./terminals", async (orig) => ({
  ...(await orig<typeof import("./terminals")>()),
  terminalsNow: () => f.list,
  watchTerminals: (g: () => void) => { f.subs.add(g); return () => { f.subs.delete(g); }; },
}));

const session = (id: string): TerminalSession => ({
  id, name: id, folder: "/home/you/Herd/gizai", folderPath: "~/Herd/gizai", taskId: null, shell: "sh", busy: null, ended: null,
  cols: 80, rows: 24, openedAt: 1,
});
const listIs = (list: TerminalSession[]) => { f.list = list; changed(); };

let S: typeof import("./chatSide");
beforeEach(async () => {
  f.list = [];
  f.subs.clear();
  vi.resetModules();
  S = await import("./chatSide");
});

type Kind = import("./chatSide").SideKind;
const tab = (kind: Kind, id: string) => ({ kind, id });
/** A side island with these tabs ("terminal:t1", "card:GA-1"), the shown one and whether it shows. */
const sideOf = (keys: string[], current: string | null = keys[keys.length - 1] ?? null, open = true) => ({
  tabs: keys.map((k) => tab(S.kindOf(k), S.idOf(k))), current, open, last: current ? { [S.kindOf(current)]: current } : {},
});
/** Opens a terminal in the chat on show, as openChatTerminal does once it started. */
const openTerm = (id: string, chat?: string) => {
  if (!f.list.some((s) => s.id === id)) f.list = [...f.list, session(id)];
  S.openSideTab(tab("terminal", id), chat);
};
const now = () => { const { chat, side, focus } = S.useChatSide(); return { chat, keys: side.tabs.map(S.tabKey), current: side.current, open: side.open, focus }; };

describe("the kinds", () => {
  it("are Terminal, Editor, Board and Card, in the strip's order", () => {
    expect(S.SIDE_KINDS).toEqual(["terminal", "editor", "board", "card"]);
    expect(S.SIDE_KINDS.map((k) => S.KIND_NAME[k])).toEqual(["Terminal", "Editor", "Board", "Card"]);
  });

  it("a tab's key is its kind and its id, and reads back (an id with a colon in it too)", () => {
    expect(S.tabKey(tab("terminal", "7f3a"))).toBe("terminal:7f3a");
    expect([S.kindOf("editor:src/a:b.ts"), S.idOf("editor:src/a:b.ts")]).toEqual(["editor", "src/a:b.ts"]);
  });

  it("kindsOpen: one per kind open, in the order Terminal, Editor, Board, Card, with how many, whatever order they opened in", () => {
    const side = sideOf(["card:GA-1", "terminal:t1", "editor:a.ts", "card:GA-2", "terminal:t2", "card:GA-3"]);
    expect(S.kindsOpen(side)).toEqual([{ kind: "terminal", count: 2 }, { kind: "editor", count: 1 }, { kind: "card", count: 3 }]);
    expect(S.kindsOpen(S.NO_SIDE), "nothing open: no icons").toEqual([]);
  });

  it("shownKind: the shown tab's kind while the island shows, none while it is hidden", () => {
    expect(S.shownKind(sideOf(["terminal:t1", "card:GA-1"], "card:GA-1"))).toBe("card");
    expect(S.shownKind(sideOf(["terminal:t1", "card:GA-1"], "card:GA-1", false))).toBeNull();
    expect(S.shownKind(S.NO_SIDE)).toBeNull();
  });

  it("lastOfKind: the kind's last shown tab, else its newest, else none", () => {
    const side = { ...sideOf(["terminal:t1", "terminal:t2", "terminal:t3"], "terminal:t3"), last: { terminal: "terminal:t1" } };
    expect(S.lastOfKind(side, "terminal")).toBe("terminal:t1");
    expect(S.lastOfKind({ ...side, last: {} }, "terminal"), "none shown yet: the newest").toBe("terminal:t3");
    expect(S.lastOfKind({ ...side, last: { terminal: "terminal:gone" } }, "terminal"), "the last one closed: the newest").toBe("terminal:t3");
    expect(S.lastOfKind(side, "card")).toBeNull();
  });
});

describe("tabs opened, shown and closed (pure parts)", () => {
  it("a new tab goes at the end and shows; one open already stays where it is and shows", () => {
    let side = S.withTab(S.NO_SIDE, tab("terminal", "t1"));
    side = S.withTab(side, tab("card", "GA-1"));
    side = S.withTab(side, tab("terminal", "t2"));
    expect(side.tabs.map(S.tabKey)).toEqual(["terminal:t1", "card:GA-1", "terminal:t2"]);
    expect([side.current, side.open]).toEqual(["terminal:t2", true]);
    side = S.withTab({ ...side, open: false }, tab("terminal", "t1"));
    expect(side.tabs.map(S.tabKey), "no second tab").toEqual(["terminal:t1", "card:GA-1", "terminal:t2"]);
    expect([side.current, side.open, side.last.terminal]).toEqual(["terminal:t1", true, "terminal:t1"]);
  });

  it("withShown only shows a tab the chat has", () => {
    const side = sideOf(["terminal:t1"]);
    expect(S.withShown(side, "terminal:nope")).toBe(side);
  });

  it("closing the shown tab shows the one to its right, else its left; another tab closed keeps the shown one", () => {
    const side = sideOf(["terminal:t1", "card:GA-1", "terminal:t2"], "card:GA-1");
    expect(S.withoutTabs(side, new Set(["card:GA-1"]))).toMatchObject({ current: "terminal:t2", open: true });
    expect(S.withoutTabs(sideOf(["terminal:t1", "terminal:t2"], "terminal:t2"), new Set(["terminal:t2"]))).toMatchObject({ current: "terminal:t1", open: true });
    expect(S.withoutTabs(side, new Set(["terminal:t1"]))).toMatchObject({ current: "card:GA-1", open: true });
    expect(S.withoutTabs(side, new Set(["nothing:x"])), "nothing closed: the same").toBe(side);
  });

  it("closing the last tab hides the island; closing while hidden keeps it hidden", () => {
    expect(S.withoutTabs(sideOf(["terminal:t1"]), new Set(["terminal:t1"]))).toEqual({ tabs: [], current: null, open: false, last: {} });
    const hidden = S.withoutTabs(sideOf(["terminal:t1", "terminal:t2"], "terminal:t1", false), new Set(["terminal:t1"]));
    expect(hidden).toMatchObject({ current: "terminal:t2", open: false });
  });

  it("toggledKind: a click on the selected icon hides the island; another kind's shows its last tab; a kind with none does nothing", () => {
    const side = { ...sideOf(["terminal:t1", "terminal:t2", "card:GA-1"], "terminal:t1"), last: { terminal: "terminal:t1", card: "card:GA-1" } };
    expect(S.toggledKind(side, "terminal")).toMatchObject({ current: "terminal:t1", open: false });
    expect(S.toggledKind(side, "card")).toMatchObject({ current: "card:GA-1", open: true });
    const hidden = { ...side, open: false };
    expect(S.toggledKind(hidden, "terminal"), "hidden: the click shows it again on the tab it had").toMatchObject({ current: "terminal:t1", open: true });
    expect(S.toggledKind(side, "editor")).toBe(side);
    expect(S.toggledKind(side, "board")).toBe(side);
  });
});

describe("the strip's words (design: The tool strip)", () => {
  it("the tooltip and label: the kind and its count", () => {
    expect(S.stripLabel("terminal", 2)).toBe("Terminal, 2 open");
    expect(S.stripLabel("editor", 1)).toBe("Editor, 1 open");
    expect(S.stripLabel("board", 1)).toBe("Board, 1 open");
    expect(S.stripLabel("card", 3)).toBe("Card, 3 open");
  });

  it("the title has the key in brackets: Ctrl+` for Terminal, Alt+2 to Alt+4 for the others (Option on macOS)", () => {
    expect(S.stripTitle("terminal", 2, false)).toBe("Terminal, 2 open (Ctrl+`)");
    expect(S.stripTitle("editor", 1, false)).toBe("Editor, 1 open (Alt+2)");
    expect(S.stripTitle("board", 1, false)).toBe("Board, 1 open (Alt+3)");
    expect(S.stripTitle("card", 3, false)).toBe("Card, 3 open (Alt+4)");
    expect(S.stripTitle("terminal", 1, true), "Ctrl+` on macOS too, as today").toBe("Terminal, 1 open (Ctrl+`)");
    expect([S.kindKey("editor", true), S.kindKey("board", true), S.kindKey("card", true)]).toEqual(["Option+2", "Option+3", "Option+4"]);
  });

  it("the right-click menu's head: the kind and its count", () => {
    expect(S.stripMenuHead("card", 3)).toBe("Card · 3 open");
    expect(S.stripMenuHead("terminal", 2)).toBe("Terminal · 2 open");
  });

  it("Close all's question while terminals run a program, word for word as the notes give it", () => {
    expect(S.closeAllQuestion([{ name: "gizai 2", busy: "npm" }])).toBe("npm is still running in gizai 2. Close it and end npm?");
    expect(S.closeAllQuestion([{ name: "gizai 2", busy: "npm" }, { name: "gizai", busy: "sleep" }]))
      .toBe("npm is still running in gizai 2, and sleep in gizai. Close all and end them?");
    expect(S.closeAllQuestion([{ name: "a", busy: "npm" }, { name: "b", busy: "sleep" }, { name: "c", busy: "php" }]))
      .toBe("npm is still running in a, sleep in b, and php in c. Close all and end them?");
  });
});

describe("Alt+1 to Alt+4 (sideKindKey)", () => {
  const key = (code: string, more: Partial<{ ctrlKey: boolean; shiftKey: boolean; altKey: boolean; metaKey: boolean }> = {}) =>
    S.sideKindKey({ code, ctrlKey: false, shiftKey: false, altKey: true, metaKey: false, ...more });

  it("Alt with Digit1 to Digit4 is Terminal, Editor, Board and Card, by where the key is", () => {
    expect(["Digit1", "Digit2", "Digit3", "Digit4"].map((c) => key(c))).toEqual(["terminal", "editor", "board", "card"]);
  });

  it("not Digit5, not the number pad, not without Alt, not with Ctrl, Shift or Meta besides", () => {
    expect(key("Digit5")).toBeNull();
    expect(key("Digit0")).toBeNull();
    expect(key("Numpad1")).toBeNull();
    expect(key("Digit1", { altKey: false })).toBeNull();
    expect(key("Digit1", { ctrlKey: true })).toBeNull();
    expect(key("Digit2", { shiftKey: true })).toBeNull();
    expect(key("Digit3", { metaKey: true })).toBeNull();
  });
});

describe("the store: each chat's tabs while Hersir runs", () => {
  it("starts on New chat with nothing open", () => {
    expect(now()).toEqual({ chat: "new", keys: [], current: null, open: false, focus: 0 });
    expect(S.useSideShows()).toBeNull();
    expect([S.chatSideKey(undefined, false), S.chatSideKey(undefined, true), S.chatSideKey("c1", false), S.chatSideKey("c1", true)])
      .toEqual(["new", "archive", "c1", "c1"]);
  });

  it("a tab opened in chat A isn't in chat B; back in A it is there, and its terminal was never closed", () => {
    S.showChat("A");
    openTerm("t1");
    openTerm("t2");
    expect(now()).toMatchObject({ chat: "A", keys: ["terminal:t1", "terminal:t2"], current: "terminal:t2", open: true });
    S.showChat("B");
    expect(now()).toMatchObject({ chat: "B", keys: [], current: null, open: false });
    expect(S.useSideShows(), "no island and no strip in B").toBeNull();
    openTerm("t3");
    expect(now().keys).toEqual(["terminal:t3"]);
    S.showChat("A");
    expect(now()).toMatchObject({ chat: "A", keys: ["terminal:t1", "terminal:t2"], current: "terminal:t2", open: true });
    expect(f.list.map((s) => s.id), "every terminal still in Hersir's list").toEqual(["t1", "t2", "t3"]);
    expect(S.sideNow("B").tabs.map(S.tabKey)).toEqual(["terminal:t3"]);
  });

  it("each chat keeps whether its island shows and which tab it shows", () => {
    S.showChat("A");
    openTerm("t1");
    openTerm("t2");
    S.showSideTab("terminal:t1");
    S.showChat("B");
    openTerm("t3");
    S.hideSide();
    S.showChat("A");
    expect(now()).toMatchObject({ current: "terminal:t1", open: true });
    expect(S.useSideShows()).toBe("island");
    S.showChat("B");
    expect(now()).toMatchObject({ current: "terminal:t3", open: false });
    expect(S.useSideShows(), "B: the strip alone").toBe("strip");
  });

  it("a terminal that leaves Hersir's list leaves every chat's tabs; a chat left with none has no strip", () => {
    S.showChat("A");
    openTerm("t1");
    openTerm("t2");
    S.showChat("B");
    openTerm("t3");
    listIs(f.list.filter((s) => s.id !== "t2" && s.id !== "t3"));
    expect(now().keys, "B").toEqual([]);
    expect(S.useSideShows()).toBeNull();
    S.showChat("A");
    expect(now()).toMatchObject({ keys: ["terminal:t1"], current: "terminal:t1", open: true });
  });

  it("openSideTab into another chat than the one on show adds it there without moving the focus here", () => {
    S.showChat("A");
    openTerm("t1");
    S.showChat("B");
    const was = now().focus;
    openTerm("t2", "A");
    expect(now()).toMatchObject({ chat: "B", keys: [], focus: was });
    expect(S.sideNow("A").tabs.map(S.tabKey)).toEqual(["terminal:t1", "terminal:t2"]);
    expect(S.sideNow("A").current).toBe("terminal:t2");
  });

  it("the focus counts up when a tab is opened, picked or shown, not when the island hides or a chat shows", () => {
    S.showChat("A");
    let was = now().focus;
    openTerm("t1");
    expect(now().focus).toBe(was + 1);
    openTerm("t2");
    was = now().focus;
    S.showSideTab("terminal:t1");
    expect(now().focus, "picked").toBe(was + 1);
    was = now().focus;
    S.hideSide();
    expect(now().focus, "hidden").toBe(was);
    S.showSide();
    expect(now().focus, "shown").toBe(was + 1);
    was = now().focus;
    S.showChat("B");
    S.showChat("A");
    expect(now().focus, "another chat and back").toBe(was);
    S.showSideTab("terminal:nope");
    expect(now().focus, "a tab the chat hasn't").toBe(was);
  });

  it("Hide hides the island and keeps every tab and the shown one; Show shows it on the tab it had", () => {
    S.showChat("A");
    openTerm("t1");
    openTerm("t2");
    S.showSideTab("terminal:t1");
    S.hideSide();
    expect(now()).toMatchObject({ keys: ["terminal:t1", "terminal:t2"], current: "terminal:t1", open: false });
    expect(S.useSideShows()).toBe("strip");
    expect(f.list.map((s) => s.id), "nothing closed").toEqual(["t1", "t2"]);
    S.showSide();
    expect(now()).toMatchObject({ current: "terminal:t1", open: true });
    expect(S.useSideShows()).toBe("island");
  });

  it("toggleSideKind: Terminal hides the island while it shows a terminal, and shows the last one again; Editor, Board and Card do nothing yet", () => {
    S.showChat("A");
    openTerm("t1");
    openTerm("t2");
    S.showSideTab("terminal:t1");
    expect(S.toggleSideKind("terminal")).toBe(true);
    expect(now()).toMatchObject({ current: "terminal:t1", open: false });
    const was = now().focus;
    expect(S.toggleSideKind("terminal")).toBe(true);
    expect(now()).toMatchObject({ current: "terminal:t1", open: true, focus: was + 1 });
    for (const k of ["editor", "board", "card"] as const) {
      expect(S.toggleSideKind(k), k).toBe(false);
      expect(now()).toMatchObject({ current: "terminal:t1", open: true });
    }
    S.showChat("B");
    expect(S.toggleSideKind("terminal"), "a chat with nothing open").toBe(false);
    expect(S.useSideShows()).toBeNull();
  });

  it("a terminal opened while the island is hidden shows the island on it", () => {
    S.showChat("A");
    openTerm("t1");
    S.hideSide();
    openTerm("t2");
    expect(now()).toMatchObject({ current: "terminal:t2", open: true });
  });

  it("the shown terminal closed while hidden: the strip stays with the others, and the next one is the one shown", () => {
    S.showChat("A");
    openTerm("t1");
    openTerm("t2");
    S.showSideTab("terminal:t1");
    S.hideSide();
    listIs(f.list.filter((s) => s.id !== "t1"));
    expect(now()).toMatchObject({ keys: ["terminal:t2"], current: "terminal:t2", open: false });
    expect(S.useSideShows()).toBe("strip");
  });

  it("closeSideTabs closes tabs of the chat on show (Close all, Own page); the last one gone, the strip goes", () => {
    S.showChat("A");
    openTerm("t1");
    openTerm("t2");
    S.closeSideTabs(["terminal:t2"]);
    expect(now()).toMatchObject({ keys: ["terminal:t1"], current: "terminal:t1", open: true });
    S.closeSideTabs(["terminal:t1"]);
    expect(now()).toMatchObject({ keys: [], current: null, open: false });
    expect(S.useSideShows()).toBeNull();
  });

  it("New chat has its own tabs, and the chat its first message makes takes them", () => {
    S.showChat("A");
    openTerm("t1");
    S.showChat(S.NEW_CHAT);
    expect(now().keys).toEqual([]);
    openTerm("t2");
    openTerm("t3");
    S.adoptNewChat("C");
    expect(S.sideNow(S.NEW_CHAT).tabs, "New chat is empty again").toEqual([]);
    S.showChat("C");
    expect(now()).toMatchObject({ chat: "C", keys: ["terminal:t2", "terminal:t3"], current: "terminal:t3", open: true });
    S.showChat("A");
    expect(now().keys, "chat A untouched").toEqual(["terminal:t1"]);
  });

  it("adoptNewChat doesn't replace what a chat had already, and does nothing with nothing open in New chat", () => {
    S.showChat("A");
    openTerm("t1");
    S.showChat(S.NEW_CHAT);
    openTerm("t2");
    S.adoptNewChat("A");
    expect(S.sideNow("A").tabs.map(S.tabKey)).toEqual(["terminal:t1"]);
    expect(S.sideNow(S.NEW_CHAT).tabs.map(S.tabKey)).toEqual(["terminal:t2"]);
    S.closeSideTabs(["terminal:t2"]);
    S.adoptNewChat("D");
    expect(S.sideNow("D").tabs).toEqual([]);
  });

  it("the Archive has tabs of its own", () => {
    S.showChat(S.ARCHIVE);
    openTerm("t1");
    S.showChat("A");
    expect(now().keys).toEqual([]);
    S.showChat(S.ARCHIVE);
    expect(now().keys).toEqual(["terminal:t1"]);
  });
});
