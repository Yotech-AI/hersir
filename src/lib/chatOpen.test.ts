// GA-298 QA: what a Chief answer opens beside the chat, read from what the answer already holds (src/lib/chatOpen.ts;
// design/islands-chat → What opens, and when): the cards its gizai:task chips link (in order, each once, at most three as
// buttons, then "and 2 more"), whether its tool calls made or moved cards (Hersir's create_task or move_task, ended without
// an error), each file and line it names (path:line, path:line:col, "path, line 4") and where such a file is looked for (an
// absolute path in the Editor folder it is in, else the chat project's folder first, then the others; never a card's
// worktree), the Open in Editor button's words, and the chat's project (the cards it is about, else the cards its answers link).
import { describe, expect, it } from "vitest";
import type { EditorFolder } from "../types";
import {
  andMore, CARDS_SHOWN, chatProjectOf, fileLabel, fileLines, FILES_SHOWN, foldersToLook, inFolder, isAbsolute, linkedCards, madeOrMoved,
  openCardLabel, projectOfCard,
} from "./chatOpen";

const HOME = "/home/you";
const folder = (id: string, more: Partial<EditorFolder> = {}): EditorFolder =>
  ({ id, path: id.replace(HOME, "~"), name: id.split("/").pop()!, kind: "root", projectId: null, taskId: null, readOnly: false, readOnlyWhy: null, ...more });
const HERSIR = folder(`${HOME}/Herd/gizai`, { kind: "project", projectId: "p-ga" });
const KADE = folder(`${HOME}/Herd/kade-portal`, { kind: "project", projectId: "p-kade" });
const SHOP = folder(`${HOME}/Herd/shop`);
const CARD = folder(`${HOME}/.local/share/gizai/worktrees/GA/GA-279`, { kind: "card", taskId: "t279", name: "GA-279" });

describe("the cards an answer links (Open GA-12)", () => {
  it("are its gizai:task chips, in order, each once, in upper case", () => {
    const md = "I made [GA-12 - The board tab](gizai:task/GA-12) and moved [ga-3](gizai:task/ga-3); [GA-12](gizai:task/GA-12) again.";
    expect(linkedCards(md)).toEqual(["GA-12", "GA-3"]);
  });
  it("leave out other kinds of chips and a card id written as plain text", () => {
    expect(linkedCards("In [Kade](gizai:project/KADE) for [Marloes](gizai:person/marloes): GA-12 and `GA-13`.")).toEqual([]);
    expect(linkedCards("")).toEqual([]);
  });
  it("show three as buttons, then 'and N more' for the rest", () => {
    expect([CARDS_SHOWN, openCardLabel("GA-259"), andMore(2)]).toEqual([3, "Open GA-259", "and 2 more"]);
  });
});

describe("Open board: the answer made or moved cards", () => {
  const tool = (name: string, tool: Record<string, unknown> | null) => ({ toolName: name, tool });
  it("after Hersir's create_task or move_task that ended without an error", () => {
    expect(madeOrMoved([tool("mcp__gizai__create_task", { result: "{}", isError: false })])).toBe(true);
    expect(madeOrMoved([tool("mcp__gizai__list_tasks", { result: "[]" }), tool("mcp__gizai__move_task", { result: "{}" })])).toBe(true);
  });
  it("not after one that failed or hasn't ended, another tool, or another server's tool of that name", () => {
    expect(madeOrMoved([tool("mcp__gizai__move_task", { result: "no such column", isError: true })])).toBe(false);
    expect(madeOrMoved([tool("mcp__gizai__create_task", { input: {} })]), "still running: no result yet").toBe(false);
    expect(madeOrMoved([tool("mcp__gizai__update_task", { result: "{}" }), tool("mcp__gizai__comment_task", { result: "{}" })])).toBe(false);
    expect(madeOrMoved([tool("move_task", { result: "{}" }), tool("mcp__other__create_task", { result: "{}" })])).toBe(false);
    expect(madeOrMoved([])).toBe(false);
  });
});

describe("the files and lines an answer names (Open in Editor)", () => {
  it("read path:line, path:line:column and 'path, line 4' (also 'at line', 'on line'), in order, each once", () => {
    const md = [
      "The tab is in `src/components/chat/ChatBoard.tsx:42`, the store in src/lib/chatSide.ts:120:7.",
      "Set it in src-tauri/tauri.conf.json, line 4; see src/api.ts at line 300 and README.md on line 9.",
      "Again: `src/components/chat/ChatBoard.tsx:42`.",
    ].join("\n\n");
    expect(fileLines(md)).toEqual([
      { path: "src/components/chat/ChatBoard.tsx", line: 42 },
      { path: "src/lib/chatSide.ts", line: 120 },
      { path: "src-tauri/tauri.conf.json", line: 4 },
      { path: "src/api.ts", line: 300 },
      { path: "README.md", line: 9 },
    ]);
  });
  it("keep ~/ and / of an absolute path, drop ./ and keep the same file at another line apart", () => {
    expect(fileLines("`~/Herd/shop/app/Cart.php:12`, /home/you/Herd/gizai/src/api.ts:3, ./src/a.ts:1 and src/a.ts:2")).toEqual([
      { path: "~/Herd/shop/app/Cart.php", line: 12 },
      { path: "/home/you/Herd/gizai/src/api.ts", line: 3 },
      { path: "src/a.ts", line: 1 },
      { path: "src/a.ts", line: 2 },
    ]);
  });
  it("take a name with an extension, a dot file, or a path with a folder; not a bare word, a number, a time or a version", () => {
    expect(fileLines(".env:2 and src/Makefile:3")).toEqual([{ path: ".env", line: 2 }, { path: "src/Makefile", line: 3 }]);
    expect(fileLines("Makefile:3, at 10:30, version 0.17.0:2, step 2: done, 127.0.0.1:8081")).toEqual([]);
  });
  it("leave out a URL's path and port, a path with .., line 0 and a path:line inside a longer word", () => {
    expect(fileLines("Open http://127.0.0.1:8081/page.html:3 or https://github.com/Yotech-AI/hersir/blob/main/src/api.ts:12")).toEqual([]);
    expect(fileLines("../secrets/key.ts:1, src/a.ts:0, src/a.ts:12abc, src/a.ts:12/b")).toEqual([]);
  });
  it("show at most three, as the file's name and line (tauri.conf.json:4)", () => {
    expect(FILES_SHOWN).toBe(3);
    expect([fileLabel("src-tauri/tauri.conf.json", 4), fileLabel("README.md", 1), fileLabel("~/Herd/shop/app/Cart.php", 12)])
      .toEqual(["tauri.conf.json:4", "README.md:1", "Cart.php:12"]);
  });
});

describe("where a named file is looked for", () => {
  const FOLDERS = [SHOP, CARD, KADE, HERSIR];
  it("an absolute path (/ or ~/) is in the Editor folder it lies in, by its path with ~ or its id, the longest", () => {
    expect([isAbsolute("~/Herd/a.ts"), isAbsolute("/x/a.ts"), isAbsolute("src/a.ts"), isAbsolute("~a.ts")]).toEqual([true, true, false, false]);
    expect(inFolder("~/Herd/gizai/src/api.ts", FOLDERS)).toEqual({ folder: HERSIR, path: "src/api.ts" });
    expect(inFolder(`${HOME}/Herd/shop/app/Cart.php`, FOLDERS)).toEqual({ folder: SHOP, path: "app/Cart.php" });
    const inner = folder(`${HOME}/Herd/gizai/crates/core`);
    expect(inFolder("~/Herd/gizai/crates/core/src/lib.rs", [...FOLDERS, inner])).toEqual({ folder: inner, path: "src/lib.rs" });
  });
  it("is none for the folder itself, a card's worktree, a folder of the same start, or a path outside them all", () => {
    expect(inFolder("~/Herd/gizai", FOLDERS)).toBeNull();
    expect(inFolder("~/.local/share/gizai/worktrees/GA/GA-279/src/api.ts", FOLDERS)).toBeNull();
    expect(inFolder("~/Herd/gizai-old/src/api.ts", FOLDERS)).toBeNull();
    expect(inFolder("/etc/hosts", FOLDERS)).toBeNull();
  });
  it("a path from a folder: the chat project's folder first, then the other projects', then the Editor's other folders; never a card", () => {
    expect(foldersToLook(FOLDERS, "p-ga").map((f) => f.name)).toEqual(["gizai", "kade-portal", "shop"]);
    expect(foldersToLook(FOLDERS, "p-kade").map((f) => f.name)).toEqual(["kade-portal", "gizai", "shop"]);
    expect(foldersToLook(FOLDERS, null).map((f) => f.name), "no chat project: the projects in their order").toEqual(["kade-portal", "gizai", "shop"]);
  });
});

describe("the chat's project", () => {
  const P = [{ id: "p-ga", key: "GA", name: "Hersir" }, { id: "p-kade", key: "KADE", name: "Kade portal" }, { id: "p-ab", key: "AB-CD", name: "Dashed" }];
  it("a card's is the project whose key its id starts with, in any case", () => {
    expect(projectOfCard("GA-259", P)?.name).toBe("Hersir");
    expect(projectOfCard("kade-1", P)?.name).toBe("Kade portal");
    expect(projectOfCard("AB-CD-12", P)?.name).toBe("Dashed");
    expect([projectOfCard("XYZ-1", P), projectOfCard("GA", P), projectOfCard("-1", P)]).toEqual([null, null, null]);
  });
  it("is the first card's the chat is about that has one, else the first linked card's, else none (the team's board)", () => {
    expect(chatProjectOf({ about: ["KADE-1", "GA-2"], named: ["GA-3"] }, P)?.name).toBe("Kade portal");
    expect(chatProjectOf({ about: ["XYZ-1", "GA-2"], named: ["KADE-3"] }, P)?.name).toBe("Hersir");
    expect(chatProjectOf({ about: [], named: ["XYZ-9", "KADE-3", "GA-1"] }, P)?.name).toBe("Kade portal");
    expect(chatProjectOf({ about: [], named: [] }, P)).toBeNull();
    expect(chatProjectOf({ about: ["XYZ-1"], named: [] }, P)).toBeNull();
  });
});
