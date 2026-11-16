// GA-278 QA, updated for GA-296 (the side island): the Chat page's terminals (src/lib/chatTerminal.ts) against the cards'
// acceptance criteria: openChatTerminal as GA-279 needs it (a new terminal, or with reuse the newest of the chat's terminals
// in the folder that waits for a command), now as a Terminal tab in the side island of the chat it was asked in
// (src/lib/chatSide.ts, tested on its own in chatSide.test.ts); another pick adds a tab; why a terminal couldn't start, as a
// sentence; and the width: 45% the first time, kept once for every chat (gizai.chat.panel), at least 360px of island and
// 400px of conversation, and when the chat list has room beside the island and the tool strip. The terminals' list is a
// fake here (TF0's store is tested in terminals.test.ts); its pure parts (nextAfterClose, sentence) are the real ones.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { TerminalSession } from "../types";
import source from "./chatTerminal.ts?raw";

const f = vi.hoisted(() => ({
  list: [] as TerminalSession[],
  subs: new Set<() => void>(),
  opens: [] as [string, number, number, string | null][],
  /** What terminalOpen answers next: a terminal, or why it couldn't start. */
  next: [] as (TerminalSession | Error | string)[],
  /** terminalOpen waits for this before it answers, when set (a terminal still starting). */
  gate: null as Promise<void> | null,
}));
const changed = () => f.subs.forEach((g) => g());

vi.mock("react", async (orig) => ({ ...(await orig<typeof import("react")>()), useSyncExternalStore: (_sub: unknown, get: () => unknown) => get() }));
vi.mock("@tauri-apps/plugin-opener", () => ({ openUrl: () => Promise.resolve() }));
vi.mock("../api", () => ({
  terminalOpen: async (folder: string, cols: number, rows: number, name: string | null) => {
    f.opens.push([folder, cols, rows, name]);
    const n = f.next.shift();
    if (f.gate) await f.gate;
    if (n === undefined) throw new Error("no terminal ready in the test");
    if (n instanceof Error || typeof n === "string") throw n;
    return n;
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
let S: typeof import("./chatSide");
const store = new Map<string, string>();
beforeEach(async () => {
  f.list = [];
  f.subs.clear();
  f.opens = [];
  f.next = [];
  f.gate = null;
  store.clear();
  vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => { store.set(k, v); }, removeItem: (k: string) => { store.delete(k); } });
  // a fresh store each time: the chats' tabs are kept in the module while Hersir runs
  vi.resetModules();
  C = await import("./chatTerminal");
  S = await import("./chatSide");
  S.showChat("c1");
});
afterEach(() => { vi.unstubAllGlobals(); });

/** Opens a terminal in the chat on show the way New terminal does, with terminalOpen answering `s`. */
async function open(s: TerminalSession, reuse = false) {
  f.next.push(s);
  return C.openChatTerminal(s.folder, reuse);
}
/** The chat on show's side island: its terminals' ids, the one shown, whether it shows. */
const side = () => { const { side: d } = S.useChatSide(); return { ids: d.tabs.map((t) => t.id), current: d.current ? S.idOf(d.current) : null, open: d.open }; };
const focus = () => S.useChatSide().focus;

describe("openChatTerminal, as GA-279 needs it", () => {
  it("is exported with the same signature, and its comment says it goes to the chat's side island", () => {
    expect(source).toContain("export function openChatTerminal(folder: string, reuse: boolean): Promise<string | null> {");
    expect(source).toContain(" *  reuse: the newest of the chat's terminals in that folder that waits for a command (not busy, not ended) shows instead of a new one.");
    expect(source).toContain(" *  Resolves with why it couldn't start, as a sentence, or null. */");
  });

  it("starts a terminal in the folder and shows the island on its tab, with the focus", async () => {
    expect(S.useChatSide().side).toEqual({ tabs: [], current: null, open: false, last: {} });
    expect(S.useSideShows(), "nothing on the right without tabs").toBeNull();
    const before = focus();
    expect(await open(session("t1"))).toBeNull();
    expect(f.opens, "the folder, 0 × 0 (the island fits it at once) and the folder's own name").toEqual([[`${HERD}/doq-studio`, 0, 0, null]]);
    expect(side()).toEqual({ ids: ["t1"], current: "t1", open: true });
    expect(S.useChatSide().side.tabs, "a Terminal tab").toEqual([{ kind: "terminal", id: "t1" }]);
    expect(focus(), "the terminal takes the focus").toBeGreaterThan(before);
    expect(S.useSideShows()).toBe("island");
    expect(f.list.map((s) => s.id), "in the terminals' list at once (the sidebar lists it)").toEqual(["t1"]);
    expect(C.chatTerminalIds()).toEqual(["t1"]);
  });

  it("adds a tab with another pick, in the order they opened, and shows the new one", async () => {
    await open(session("t1"));
    await open(session("t2", `${HERD}/gizai`));
    expect(side()).toEqual({ ids: ["t1", "t2"], current: "t2", open: true });
  });

  it("with reuse false always starts a new one, also while one in the folder waits for a command", async () => {
    await open(session("t1"));
    expect(await open(session("t2"), false)).toBeNull();
    expect(f.opens).toHaveLength(2);
    expect(side().ids).toEqual(["t1", "t2"]);
  });

  it("with reuse shows the chat's terminal in that folder while it waits for a command: no new terminal", async () => {
    await open(session("t1"));
    await open(session("t2", `${HERD}/gizai`));
    S.hideSide();
    const was = focus();
    expect(await C.openChatTerminal(`${HERD}/doq-studio`, true)).toBeNull();
    expect(f.opens, "nothing started").toHaveLength(2);
    expect(side()).toEqual({ ids: ["t1", "t2"], current: "t1", open: true });
    expect(focus(), "it takes the focus").toBe(was + 1);
  });

  it("with reuse picks the newest of the chat's waiting terminals in that folder", async () => {
    await open(session("t1"));
    await open(session("t2"));
    await open(session("t3", `${HERD}/gizai`));
    expect(await C.openChatTerminal(`${HERD}/doq-studio`, true)).toBeNull();
    expect(side().current).toBe("t2");
    // the newest runs a program: the one before it, still waiting, shows
    update("t2", { busy: "php" });
    await C.openChatTerminal(`${HERD}/gizai`, true);
    expect(await C.openChatTerminal(`${HERD}/doq-studio`, true)).toBeNull();
    expect(side().current).toBe("t1");
    expect(f.opens).toHaveLength(3);
  });

  it("with reuse starts a new one while a program runs in it", async () => {
    await open(session("t1"));
    update("t1", { busy: "sleep" });
    expect(await open(session("t2"), true)).toBeNull();
    expect(f.opens).toHaveLength(2);
    expect(side()).toEqual({ ids: ["t1", "t2"], current: "t2", open: true });
  });

  it("with reuse starts a new one after it ended", async () => {
    await open(session("t1"));
    update("t1", { ended: { code: 0, at: 2, why: null } });
    expect(side().ids, "an ended terminal stays as a tab (ended) until it is closed").toEqual(["t1"]);
    await open(session("t2"), true);
    expect(f.opens).toHaveLength(2);
    expect(side().ids).toEqual(["t1", "t2"]);
  });

  it("with reuse doesn't take a terminal in that folder that wasn't opened in the chat (the Terminal page's, the Editor's)", async () => {
    listIs([session("elsewhere")]);
    await open(session("t1"), true);
    expect(f.opens, "a new one").toHaveLength(1);
    expect(side().ids).toEqual(["t1"]);
  });

  it("with reuse doesn't take another chat's waiting terminal in that folder: this chat gets its own", async () => {
    await open(session("t1"));
    S.showChat("c2");
    expect(await open(session("t2"), true)).toBeNull();
    expect(f.opens, "a new one in chat 2").toHaveLength(2);
    expect(side()).toEqual({ ids: ["t2"], current: "t2", open: true });
    S.showChat("c1");
    expect(side(), "chat 1 keeps its own").toEqual({ ids: ["t1"], current: "t1", open: true });
    expect(await C.openChatTerminal(`${HERD}/doq-studio`, true)).toBeNull();
    expect(f.opens, "chat 1's own waits: no new one").toHaveLength(2);
  });

  it("with reuse starts a new one in another folder", async () => {
    await open(session("t1"));
    await open(session("t2", `${HERD}/gizai`), true);
    expect(f.opens).toHaveLength(2);
    expect(side().current).toBe("t2");
  });

  it("goes to the chat it was asked in, also when another chat shows by the time it has started, without taking the focus there", async () => {
    let go!: () => void;
    f.gate = new Promise<void>((r) => { go = r; });
    f.next.push(session("t1"));
    const started = C.openChatTerminal(`${HERD}/doq-studio`, false);
    S.showChat("c2");
    const was = focus();
    go();
    expect(await started).toBeNull();
    expect(side(), "nothing in chat 2").toEqual({ ids: [], current: null, open: false });
    expect(S.useSideShows()).toBeNull();
    expect(focus(), "chat 2's focus stays where it is").toBe(was);
    expect(C.chatTerminalIds("c1")).toEqual(["t1"]);
    S.showChat("c1");
    expect(side()).toEqual({ ids: ["t1"], current: "t1", open: true });
  });

  it("resolves with why it couldn't start, as a sentence, and leaves the side island as it was", async () => {
    f.next.push(new Error("QA Agent is working on GA-118: a terminal there waits until its run ends"));
    expect(await C.openChatTerminal(`${HERD}/x`, false)).toBe("QA Agent is working on GA-118: a terminal there waits until its run ends.");
    f.next.push("Error: Couldn't start zsh in ~/Herd/x: /usr/bin/zsh wasn't found.");
    expect(await C.openChatTerminal(`${HERD}/x`, true), "Tauri's error text, already a sentence").toBe("Couldn't start zsh in ~/Herd/x: /usr/bin/zsh wasn't found.");
    expect(S.useChatSide().side).toEqual({ tabs: [], current: null, open: false, last: {} });
    expect(S.useSideShows()).toBeNull();
    // with a terminal open, a failed start changes nothing either
    await open(session("t1"));
    S.hideSide();
    f.next.push(new Error("no shell"));
    expect(await C.openChatTerminal(`${HERD}/x`, false)).toBe("no shell.");
    expect(side()).toEqual({ ids: ["t1"], current: "t1", open: false });
  });
});

describe("a chat's terminals while Hersir runs", () => {
  it("are only the ones opened in the chat, not the others in the list", async () => {
    listIs([session("page", `${HERD}/gizai`)]);
    await open(session("t1"));
    listIs([...f.list, session("editor", `${HERD}/gizai`)]);
    expect(C.chatTerminalIds()).toEqual(["t1"]);
  });

  it("closing the shown tab shows the next one, else the one before it", async () => {
    await open(session("t1"));
    await open(session("t2"));
    await open(session("t3"));
    S.showSideTab("terminal:t2");
    listIs(f.list.filter((s) => s.id !== "t2"));
    expect(side()).toEqual({ ids: ["t1", "t3"], current: "t3", open: true });
    listIs(f.list.filter((s) => s.id !== "t3"));
    expect(side()).toEqual({ ids: ["t1"], current: "t1", open: true });
  });

  it("closing the last one hides the island, and the strip goes", async () => {
    await open(session("t1"));
    listIs([]);
    expect(side()).toEqual({ ids: [], current: null, open: false });
    expect(S.useSideShows()).toBeNull();
    S.showSide();
    expect(side().open, "nothing to show").toBe(false);
  });

  it("a terminal closed and its id never coming back: a new one later is a new tab", async () => {
    await open(session("t1"));
    listIs([]);
    await open(session("t2"));
    expect(side()).toEqual({ ids: ["t2"], current: "t2", open: true });
  });
});

describe("the width: a share of the room right of the chat list (gizai.chat.panel)", () => {
  it("is 45% the first time; ← and → move it 5%; at least 360px of island and 400px of conversation", () => {
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

  it("works without localStorage (read in try/catch): nothing is kept, and the island opens at 45%", () => {
    vi.stubGlobal("localStorage", { getItem: () => { throw new Error("denied"); }, setItem: () => { throw new Error("denied"); } });
    expect(() => C.keepChatShare(50)).not.toThrow();
    expect(C.loadChatShare()).toBe(45);
    vi.stubGlobal("localStorage", undefined);
    expect(C.loadChatShare()).toBe(45);
  });

  it("a drag stays within the limits: 360px of island at least, 400px of conversation at least", () => {
    // an island of 500px beside a conversation of 700px: 1200px to share
    expect(C.clampChatPanel(600, 500, 700)).toBe(600);
    expect(C.clampChatPanel(1100, 500, 700), "the conversation keeps 400px").toBe(800);
    expect(C.clampChatPanel(100, 500, 700), "the island keeps 360px").toBe(360);
    expect(C.clampChatPanel(360, 500, 700)).toBe(360);
    // no room for both: the island keeps its least width
    expect(C.clampChatPanel(500, 360, 300)).toBe(360);
  });

  it("is a share of the room right of the chat list", () => {
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

  it("the gaps between islands are 6px and the strip's buttons 28px (app.css's island-gap and size-tool)", async () => {
    expect([C.ISLAND_GAP, C.STRIP_WIDTH]).toEqual([6, 28]);
    const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
    const tokens: string = fs.readFileSync(new URL("../styles/tokens.css", import.meta.url), "utf8");
    expect(tokens).toMatch(/--island-gap: 6px;/);
    expect(tokens).toMatch(/--size-tool: 28px;/);
  });

  it("the chat list has room beside the island and the strip while the rest holds 360px of island and 400px of conversation", () => {
    // the list, three gaps, the strip, then 760px for the island and the conversation
    expect(C.listFits(260 + 3 * 6 + 28 + 760, 260)).toBe(true);
    expect(C.listFits(260 + 3 * 6 + 28 + 759, 260)).toBe(false);
    expect(C.listFits(260 + 3 * 6 + 28 + 760, 260, true), "the island is the default").toBe(true);
    // a 1440px window's chat (sidebar 248, gaps) has room; a narrow one doesn't
    expect(C.listFits(1440 - 248 - 3 * 6, 260)).toBe(true);
    expect(C.listFits(900 - 240, 220)).toBe(false);
  });

  it("with the strip alone (the island hidden) the chat list has room while the conversation keeps 400px", () => {
    // the list, two gaps, the strip, then 400px for the conversation
    expect(C.listFits(260 + 2 * 6 + 28 + 400, 260, false)).toBe(true);
    expect(C.listFits(260 + 2 * 6 + 28 + 399, 260, false)).toBe(false);
    // 900 × 600 with the sidebar closed: the chat list has room beside the conversation and the strip
    expect(C.listFits(900 - 2 * 6, 220, false)).toBe(true);
    expect(C.listFits(900 - 2 * 6, 220, true), "but not beside the island too").toBe(false);
  });
});
