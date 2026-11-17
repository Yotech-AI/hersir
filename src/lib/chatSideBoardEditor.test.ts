// GA-298 QA: what the Board and Editor tabs keep in the side island's store (src/lib/chatSide.ts) and how Own page of an
// Editor tab hands its file and line to the Editor page (src/lib/editor.ts): an Editor tab's id holds its folder and path;
// a chat has one Board tab; the chat's cards (those it is about, those its answers link) per chat, each once in upper case,
// the ones it is about first; whether any chat still has a file open; and the Editor page opening at that file and line once.
// The terminals' list is a fake, as in chatSide.test.ts.
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("react", async (orig) => ({ ...(await orig<typeof import("react")>()), useSyncExternalStore: (_sub: unknown, get: () => unknown) => get() }));
vi.mock("../api", () => ({}));
vi.mock("./terminals", async (orig) => ({ ...(await orig<typeof import("./terminals")>()), terminalsNow: () => [], watchTerminals: () => () => {} }));

const store = new Map<string, string>();
vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => { store.set(k, String(v)); }, removeItem: (k: string) => { store.delete(k); } });

let S: typeof import("./chatSide");
let E: typeof import("./editor");
beforeEach(async () => {
  store.clear();
  vi.resetModules();
  S = await import("./chatSide");
  E = await import("./editor");
  S.showChat("c1");
});
const side = (key?: string) => { const s = S.sideNow(key); return { tabs: s.tabs.map(S.tabKey), current: s.current, open: s.open }; };

describe("the Editor tab's id", () => {
  it("holds the file's folder (an Editor folder's id) and its path in it, and gives them back", () => {
    const id = S.editorTabId("/home/you/Herd/gizai", "src-tauri/tauri.conf.json");
    expect(id).toBe("/home/you/Herd/gizai//src-tauri/tauri.conf.json");
    expect(S.editorTabOf(id)).toEqual({ folder: "/home/you/Herd/gizai", path: "src-tauri/tauri.conf.json" });
    expect(S.editorTabOf(S.editorTabId("/home/you/Herd/gizai", "a//b.ts")), "the first // splits").toEqual({ folder: "/home/you/Herd/gizai", path: "a//b.ts" });
    expect(S.editorTabOf("README.md")).toEqual({ folder: "", path: "README.md" });
  });
  it("is still open somewhere while any chat has its tab, so its unsaved text stays", () => {
    const key = S.tabKey({ kind: "editor", id: S.editorTabId("/f", "a.ts") });
    expect(S.tabOpenAnywhere(key)).toBe(false);
    S.openSideTab({ kind: "editor", id: S.editorTabId("/f", "a.ts") });
    S.showChat("c2");
    expect(S.tabOpenAnywhere(key), "chat 1 has it while chat 2 shows").toBe(true);
    S.closeSideTabs([key], "c1");
    expect(S.tabOpenAnywhere(key)).toBe(false);
  });
});

describe("the Board tab", () => {
  it("is one per chat: Open board opens it, shown, and again only shows it", () => {
    S.openCardTab("GA-1");
    S.openBoardTab();
    expect(side()).toEqual({ tabs: ["card:GA-1", "board:board"], current: "board:board", open: true });
    S.showSideTab("card:GA-1");
    S.openBoardTab();
    expect(side()).toEqual({ tabs: ["card:GA-1", "board:board"], current: "board:board", open: true });
    expect(S.kindsOpen(S.sideNow()).map((k) => [k.kind, k.count])).toEqual([["board", 1], ["card", 1]]);
  });
  it("belongs to the chat it was opened in", () => {
    S.openBoardTab();
    S.showChat("c2");
    expect(side().tabs).toEqual([]);
    expect(side("c1").tabs).toEqual(["board:board"]);
  });
});

describe("the chat's cards", () => {
  it("are kept per chat: those it is about and those its answers link, each once, in upper case", () => {
    S.setChatCards("c1", "about", ["ga-259", "GA-283", "GA-259"]);
    S.setChatCards("c1", "named", ["KADE-1", " ga-283 "]);
    S.setChatCards("c2", "named", ["GFW-2"]);
    expect(S.chatCardsNow("c1")).toEqual({ about: ["GA-259", "GA-283"], named: ["KADE-1", "GA-283"] });
    expect(S.chatCardsNow(), "the chat on show by default").toEqual(S.chatCardsNow("c1"));
    expect(S.chatCardsNow("c2")).toEqual({ about: [], named: ["GFW-2"] });
    expect(S.chatCardsNow("c3")).toEqual({ about: [], named: [] });
  });
  it("are every card once for the board's marks, those the chat is about first", () => {
    expect(S.allChatCards({ about: ["GA-259", "GA-283"], named: ["KADE-1", "GA-283"] })).toEqual(["GA-259", "GA-283", "KADE-1"]);
  });
  it("stay as they were when set again the same (in another case), and can be emptied", () => {
    S.setChatCards("c1", "named", ["GA-1"]);
    const before = S.chatCardsNow("c1");
    S.setChatCards("c1", "named", ["ga-1"]);
    expect(S.chatCardsNow("c1"), "the same cards: the same object, so nothing draws again").toBe(before);
    S.setChatCards("c1", "named", []);
    expect(S.chatCardsNow("c1")).toEqual({ about: [], named: [] });
  });
});

describe("Own page of an Editor tab: the Editor page at that file and line", () => {
  it("withOpenTab adds the file as the folder's last tab, open, or opens it where it is", () => {
    expect(E.withOpenTab({ paths: ["a.ts", "b.ts"], active: "a.ts" }, "c.ts")).toEqual({ paths: ["a.ts", "b.ts", "c.ts"], active: "c.ts" });
    expect(E.withOpenTab({ paths: ["a.ts", "b.ts"], active: "a.ts" }, "b.ts")).toEqual({ paths: ["a.ts", "b.ts"], active: "b.ts" });
  });
  it("keeps the file among the folder's tabs as the open one, and the page on that folder takes the line once", () => {
    E.keepTabs("/f", { paths: ["README.md"], active: "README.md" });
    E.editorPageAt("/f", "src/api.ts", 12);
    expect(E.loadTabs("/f")).toEqual({ paths: ["README.md", "src/api.ts"], active: "src/api.ts" });
    expect(E.takePageAt("/other"), "another folder's page doesn't take it").toBeNull();
    expect(E.takePageAt("/f")).toEqual({ path: "src/api.ts", line: 12, column: 1 });
    expect(E.takePageAt("/f"), "once").toBeNull();
  });
  it("without a line, opens the file and leaves the page where the file was", () => {
    E.editorPageAt("/g", "a.ts", null);
    expect(E.loadTabs("/g")).toEqual({ paths: ["a.ts"], active: "a.ts" });
    expect(E.takePageAt("/g")).toBeNull();
  });
});
