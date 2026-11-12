// GA-278 QA: the Chat page's terminal panel, its store (src/lib/chatTerminal.ts), against the card's acceptance criteria:
// openChatTerminal as GA-279 needs it, word for word (a new terminal, or with reuse the newest of the panel's terminals in
// the folder that waits for a command); another pick adds a tab; why a terminal couldn't start, as a sentence; a terminal
// closed anywhere leaves the tabs, the next tab shows and closing the last one hides the panel; Hide keeps the terminals
// (the rail); and the width: 45% the first time, kept once for every chat (gizai.chat.panel), at least 360px of panel and
// 400px of conversation, and when the chat list has room beside the panel. The terminals' list is a fake here (TF0's store
// is tested in terminals.test.ts); its pure parts (nextAfterClose, sentence) are the real ones.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TerminalSession } from "../types";
import source from "./chatTerminal.ts?raw";

const f = vi.hoisted(() => ({
  list: [] as TerminalSession[],
  subs: new Set<() => void>(),
  opens: [] as [string, number, number, string | null][],
  /** What terminalOpen answers next: a terminal, or why it couldn't start. */
  next: [] as (TerminalSession | Error | string)[],
}));
const changed = () => f.subs.forEach((g) => g());

vi.mock("react", async (orig) => ({ ...(await orig<typeof import("react")>()), useSyncExternalStore: (_sub: unknown, get: () => unknown) => get() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: () => Promise.resolve() }));
vi.mock("../api", () => ({
  terminalOpen: (folder: string, cols: number, rows: number, name: string | null) => {
    f.opens.push([folder, cols, rows, name]);
    const n = f.next.shift();
    if (n === undefined) return Promise.reject(new Error("no terminal ready in the test"));
    return n instanceof Error || typeof n === "string" ? Promise.reject(n) : Promise.resolve(n);
  },
}));
vi.mock("./terminals", async (orig) => ({
  ...(await orig<typeof import("./terminals")>()),
  terminalsNow: () => f.list,
  watchTerminals: (g: () => void) => { f.subs.add(g); return () => { f.subs.delete(g); }; },
  noteOpened: (s: TerminalSession) => { if (!f.list.some((x) => x.id === s.id)) { f.list = [...f.list, s]; changed(); } },
}));

const HERD = "/home/you/Herd";
const session = (id: string, folder = `${HERD}/doq-studio`, more: Partial<TerminalSession> = {}): TerminalSession => ({
  id, name: id, folder, folderPath: folder.replace("/home/you", "~"), taskId: null, shell: "sh", busy: null, ended: null,
  cols: 80, rows: 24, openedAt: 1, ...more,
});
/** The terminals' list changes (terminals-changed): a terminal closed, started elsewhere, busy or ended. */
const listIs = (list: TerminalSession[]) => { f.list = list; changed(); };
const update = (id: string, more: Partial<TerminalSession>) => listIs(f.list.map((s) => (s.id === id ? { ...s, ...more } : s)));

let C: typeof import("./chatTerminal");
const store = new Map<string, string>();
beforeEach(async () => {
  f.list = [];
  f.subs.clear();
  f.opens = [];
  f.next = [];
  store.clear();
  vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => { store.set(k, v); }, removeItem: (k: string) => { store.delete(k); } });
  // a fresh store each time: the chat's terminals are kept in the module while Hersir runs
  vi.resetModules();
  C = await import("./chatTerminal");
});
afterEach(() => { vi.unstubAllGlobals(); });

/** Opens a terminal in the panel the way Open terminal does, with terminalOpen answering `s`. */
async function open(s: TerminalSession, reuse = false) {
  f.next.push(s);
  return C.openChatTerminal(s.folder, reuse);
}

describe("openChatTerminal, as GA-279 needs it", () => {
  it("is exported word for word, with its comment", () => {
    expect(source).toContain([
      "/** Opens a terminal in the folder (an EditorFolder id) in the Chat page's terminal panel, shows the panel on it and gives it the focus.",
      " *  reuse: the newest of the panel's terminals in that folder that waits for a command (not busy, not ended) shows instead of a new one.",
      " *  Resolves with why it couldn't start, as a sentence, or null. */",
      "export function openChatTerminal(folder: string, reuse: boolean): Promise<string | null> {",
    ].join("\n"));
  });

  it("starts a terminal in the folder and shows the panel on it, with the focus", async () => {
    const before = C.useChatTerminals();
    expect(before).toEqual({ ids: [], current: null, open: false, focus: 0 });
    expect(C.useChatTerminalShows(), "nothing on the right without terminals").toBeNull();
    expect(await open(session("t1"))).toBeNull();
    expect(f.opens, "the folder, 0 × 0 (the panel fits it at once) and the folder's own name").toEqual([[`${HERD}/doq-studio`, 0, 0, null]]);
    const t = C.useChatTerminals();
    expect([t.ids, t.current, t.open]).toEqual([["t1"], "t1", true]);
    expect(t.focus, "the terminal takes the focus").toBeGreaterThan(before.focus);
    expect(C.useChatTerminalShows()).toBe("panel");
    expect(f.list.map((s) => s.id), "in the terminals' list at once (the sidebar lists it)").toEqual(["t1"]);
  });

  it("adds a tab with another pick, oldest first, and shows the new one", async () => {
    await open(session("t1"));
    await open(session("t2", `${HERD}/gizai`));
    expect(C.useChatTerminals().ids).toEqual(["t1", "t2"]);
    expect(C.useChatTerminals().current).toBe("t2");
  });

  it("with reuse false always starts a new one, also while one in the folder waits for a command", async () => {
    await open(session("t1"));
    expect(await open(session("t2"), false)).toBeNull();
    expect(f.opens).toHaveLength(2);
    expect(C.useChatTerminals().ids).toEqual(["t1", "t2"]);
  });

  it("with reuse shows the panel's terminal in that folder while it waits for a command: no new terminal", async () => {
    await open(session("t1"));
    await open(session("t2", `${HERD}/gizai`));
    C.hideChatPanel();
    const focus = C.useChatTerminals().focus;
    expect(await C.openChatTerminal(`${HERD}/doq-studio`, true)).toBeNull();
    expect(f.opens, "nothing started").toHaveLength(2);
    const t = C.useChatTerminals();
    expect([t.ids, t.current, t.open]).toEqual([["t1", "t2"], "t1", true]);
    expect(t.focus, "it takes the focus").toBe(focus + 1);
  });

  it("with reuse picks the newest of the panel's waiting terminals in that folder", async () => {
    await open(session("t1"));
    await open(session("t2"));
    await open(session("t3", `${HERD}/gizai`));
    expect(await C.openChatTerminal(`${HERD}/doq-studio`, true)).toBeNull();
    expect(C.useChatTerminals().current).toBe("t2");
    // the newest runs a program: the one before it, still waiting, shows
    update("t2", { busy: "php" });
    await C.openChatTerminal(`${HERD}/gizai`, true);
    expect(await C.openChatTerminal(`${HERD}/doq-studio`, true)).toBeNull();
    expect(C.useChatTerminals().current).toBe("t1");
    expect(f.opens).toHaveLength(3);
  });

  it("with reuse starts a new one while a program runs in it", async () => {
    await open(session("t1"));
    update("t1", { busy: "sleep" });
    expect(await open(session("t2"), true)).toBeNull();
    expect(f.opens).toHaveLength(2);
    expect(C.useChatTerminals().ids).toEqual(["t1", "t2"]);
    expect(C.useChatTerminals().current).toBe("t2");
  });

  it("with reuse starts a new one after it ended", async () => {
    await open(session("t1"));
    update("t1", { ended: { code: 0, at: 2, why: null } });
    expect(C.useChatTerminals().ids, "an ended terminal stays as a tab (ended) until it is closed").toEqual(["t1"]);
    await open(session("t2"), true);
    expect(f.opens).toHaveLength(2);
    expect(C.useChatTerminals().ids).toEqual(["t1", "t2"]);
  });

  it("with reuse doesn't take a terminal in that folder that wasn't opened in the chat (the Terminal page's, the Editor's)", async () => {
    listIs([session("elsewhere")]);
    await open(session("t1"), true);
    expect(f.opens, "a new one").toHaveLength(1);
    expect(C.useChatTerminals().ids).toEqual(["t1"]);
  });

  it("with reuse starts a new one in another folder", async () => {
    await open(session("t1"));
    await open(session("t2", `${HERD}/gizai`), true);
    expect(f.opens).toHaveLength(2);
    expect(C.useChatTerminals().current).toBe("t2");
  });

  it("resolves with why it couldn't start, as a sentence, and leaves the panel as it was", async () => {
    f.next.push(new Error("QA Agent is working on GA-118: a terminal there waits until its run ends"));
    expect(await C.openChatTerminal(`${HERD}/x`, false)).toBe("QA Agent is working on GA-118: a terminal there waits until its run ends.");
    f.next.push("Error: Couldn't start zsh in ~/Herd/x: /usr/bin/zsh wasn't found.");
    expect(await C.openChatTerminal(`${HERD}/x`, true), "Tauri's error text, already a sentence").toBe("Couldn't start zsh in ~/Herd/x: /usr/bin/zsh wasn't found.");
    expect(C.useChatTerminals()).toEqual({ ids: [], current: null, open: false, focus: 0 });
    expect(C.useChatTerminalShows()).toBeNull();
  });
});

describe("the tabs: the chat's terminals while Hersir runs", () => {
  it("only lists terminals opened in the chat, not the others in the list", async () => {
    listIs([session("page", `${HERD}/gizai`)]);
    await open(session("t1"));
    listIs([...f.list, session("editor", `${HERD}/gizai`)]);
    expect(C.useChatTerminals().ids).toEqual(["t1"]);
  });

  it("a tab picked shows, with the focus; a terminal not in the chat can't be picked", async () => {
    await open(session("t1"));
    await open(session("t2"));
    const focus = C.useChatTerminals().focus;
    C.pickChatTerminal("t1");
    expect([C.useChatTerminals().current, C.useChatTerminals().focus]).toEqual(["t1", focus + 1]);
    listIs([...f.list, session("other")]);
    C.pickChatTerminal("other");
    expect(C.useChatTerminals().current).toBe("t1");
  });

  it("closing the shown tab shows the next one, else the one before it", async () => {
    await open(session("t1"));
    await open(session("t2"));
    await open(session("t3"));
    C.pickChatTerminal("t2");
    listIs(f.list.filter((s) => s.id !== "t2"));
    expect([C.useChatTerminals().ids, C.useChatTerminals().current, C.useChatTerminals().open]).toEqual([["t1", "t3"], "t3", true]);
    listIs(f.list.filter((s) => s.id !== "t3"));
    expect([C.useChatTerminals().ids, C.useChatTerminals().current]).toEqual([["t1"], "t1"]);
  });

  it("closing another tab keeps the shown one", async () => {
    await open(session("t1"));
    await open(session("t2"));
    listIs(f.list.filter((s) => s.id !== "t1"));
    expect([C.useChatTerminals().ids, C.useChatTerminals().current]).toEqual([["t2"], "t2"]);
  });

  it("closing the last one hides the panel, and nothing (no rail) is left", async () => {
    await open(session("t1"));
    listIs([]);
    expect(C.useChatTerminals()).toMatchObject({ ids: [], current: null, open: false });
    expect(C.useChatTerminalShows()).toBeNull();
    C.showChatPanel();
    expect(C.useChatTerminals().open, "nothing to show").toBe(false);
  });

  it("all go when Hersir's terminals end (the list empties), also while the panel is hidden", async () => {
    await open(session("t1"));
    await open(session("t2"));
    C.hideChatPanel();
    listIs([]);
    expect(C.useChatTerminals()).toMatchObject({ ids: [], current: null, open: false });
    expect(C.useChatTerminalShows()).toBeNull();
  });

  it("a terminal closed and its id never coming back: a new one later is a new tab", async () => {
    await open(session("t1"));
    listIs([]);
    await open(session("t2"));
    expect(C.useChatTerminals().ids).toEqual(["t2"]);
    expect(C.useChatTerminals().open).toBe(true);
  });
});

describe("Hide and show: the shells keep running, the rail says how many", () => {
  it("Hide hides the panel, keeps the terminals and the shown one; the rail shows", async () => {
    await open(session("t1"));
    await open(session("t2"));
    C.pickChatTerminal("t1");
    C.hideChatPanel();
    expect(C.useChatTerminals()).toMatchObject({ ids: ["t1", "t2"], current: "t1", open: false });
    expect(C.useChatTerminalShows()).toBe("rail");
    expect(f.list.map((s) => s.id), "nothing closed").toEqual(["t1", "t2"]);
  });

  it("the rail (and Ctrl+`) shows the panel on the terminal it had, with the focus", async () => {
    await open(session("t1"));
    await open(session("t2"));
    C.pickChatTerminal("t1");
    C.hideChatPanel();
    const focus = C.useChatTerminals().focus;
    C.showChatPanel();
    expect(C.useChatTerminals()).toMatchObject({ current: "t1", open: true, focus: focus + 1 });
    expect(C.useChatTerminalShows()).toBe("panel");
  });

  it("a terminal opened while the panel is hidden shows the panel on it", async () => {
    await open(session("t1"));
    C.hideChatPanel();
    await open(session("t2"));
    expect(C.useChatTerminals()).toMatchObject({ current: "t2", open: true });
  });

  it("the shown terminal closed while hidden: the rail stays with the others and shows the next one", async () => {
    await open(session("t1"));
    await open(session("t2"));
    C.pickChatTerminal("t1");
    C.hideChatPanel();
    listIs(f.list.filter((s) => s.id !== "t1"));
    expect(C.useChatTerminals()).toMatchObject({ ids: ["t2"], current: "t2", open: false });
    expect(C.useChatTerminalShows()).toBe("rail");
  });
});

describe("the width: a share of the space right of the chat list (gizai.chat.panel)", () => {
  it("is 45% the first time; ← and → move it 5%; at least 360px of panel and 400px of conversation", () => {
    expect([C.CHAT_PANEL_KEY, C.CHAT_FIRST_SHARE, C.CHAT_SHARE_STEP, C.CHAT_PANEL_MIN, C.CONVERSATION_MIN]).toEqual(["gizai.chat.panel", 45, 5, 360, 400]);
    expect(C.loadChatShare(), "nothing kept yet").toBe(45);
  });

  it("is a key of its own, not the Editor panel's", async () => {
    const { PANEL_KEY } = await import("./terminalPanel");
    expect(C.CHAT_PANEL_KEY).not.toBe(PANEL_KEY);
    C.keepChatShare(60);
    expect([...store.keys()]).toEqual(["gizai.chat.panel"]);
  });

  it("keeps a share to a tenth of a percent, and reads it back the next time (a restart, any chat)", () => {
    C.keepChatShare(52.3456);
    expect(store.get("gizai.chat.panel")).toBe("52.3");
    expect(C.loadChatShare()).toBe(52.3);
  });

  it("reads anything it can't read as 45%, and stays between 0 and 100", () => {
    expect(C.chatShareOf(null)).toBe(45);
    expect(C.chatShareOf("33")).toBe(33);
    expect(C.chatShareOf("140")).toBe(100);
    expect(C.chatShareOf("-3")).toBe(0);
    for (const bad of ["", "abc", "\"40\"", "[40]", "{}", "null", "NaN", "Infinity"]) expect(C.chatShareOf(bad), `kept ${JSON.stringify(bad)}`).toBe(45);
  });

  it("works without localStorage (read in try/catch): nothing is kept, and the panel opens at 45%", () => {
    vi.stubGlobal("localStorage", { getItem: () => { throw new Error("denied"); }, setItem: () => { throw new Error("denied"); } });
    expect(() => C.keepChatShare(50)).not.toThrow();
    expect(C.loadChatShare()).toBe(45);
    vi.stubGlobal("localStorage", undefined);
    expect(C.loadChatShare()).toBe(45);
  });

  it("a drag stays within the limits: 360px of panel at least, 400px of conversation at least", () => {
    // a panel of 500px beside a conversation of 700px: 1200px to share
    expect(C.clampChatPanel(600, 500, 700)).toBe(600);
    expect(C.clampChatPanel(1100, 500, 700), "the conversation keeps 400px").toBe(800);
    expect(C.clampChatPanel(100, 500, 700), "the panel keeps 360px").toBe(360);
    expect(C.clampChatPanel(360, 500, 700)).toBe(360);
    // no room for both: the panel keeps its least width
    expect(C.clampChatPanel(500, 360, 300)).toBe(360);
  });

  it("is a share of the space right of the chat list", () => {
    expect(C.chatShareFor(540, 1200)).toBe(45);
    expect(C.chatShareFor(1200, 1200)).toBe(100);
    expect(C.chatShareFor(0, 0), "nothing measured: 45%").toBe(45);
    expect(C.chatShareFor(10, NaN)).toBe(45);
  });

  it("the chat list is as wide as app.css draws it: 260px, 220px up to 1100px, 6px more per pixel of text size", () => {
    expect(C.chatListWidth(false, 0)).toBe(260);
    expect(C.chatListWidth(true, 0)).toBe(220);
    expect(C.chatListWidth(false, 3)).toBe(278);
    expect(C.chatListWidth(true, 3)).toBe(238);
  });

  it("the chat list has room beside the panel while the rest holds 360px of panel and 400px of conversation", () => {
    expect(C.listFits(260 + 760, 260)).toBe(true);
    expect(C.listFits(260 + 759, 260)).toBe(false);
    // a 1280px window's chat (the sidebar aside) has room; a narrow one doesn't
    expect(C.listFits(1280 - 240, 260)).toBe(true);
    expect(C.listFits(900 - 240, 220)).toBe(false);
  });
});
