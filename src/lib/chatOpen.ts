// What a Chief answer opens beside the chat (GA-298; design/islands-chat → What opens, and when), and the chat's project.
// Pure, so it can be tested in Node. The buttons only read what an answer already holds:
// - Open GA-12: each card the answer links (its gizai:task chips), in order, each once; the first CARDS_SHOWN as buttons,
//   then "and 2 more" with the rest in a menu.
// - Open board: when its tool calls made or moved cards (a create_task or move_task of Hersir's that ended without an
//   error).
// - Open in Editor: each file and line it names (src/api.ts:12, src/api.ts:12:5, or "src-tauri/tauri.conf.json, line 4"),
//   in order, each once, that is a file in the chat project's folder or in one of the Editor's folders: an absolute path
//   (/ or ~/) in one of them as it is, a path from a folder looked for in the chat project's folder first, then the
//   others. At most FILES_SHOWN. A URL's path and a number like 0.17.0 aren't files.
// - Open in terminal: GA-279's (src/lib/runCommands.ts), the row's last.
// The chat's project: the project of the cards the chat is about (ChatThread.tasks; the first one's), else of the cards its
// answers link, else none (the team's board). A card's project is the one whose key its id starts with (GA-259: GA).
import type { EditorFolder, Project } from "../types";
import { ITEM_LINK_SOURCE, parseItemUrl } from "./itemLinks";
import { normalPath } from "./runCommands";

/** The row shows this many cards as buttons, then "and N more"; and this many files. */
export const CARDS_SHOWN = 3;
export const FILES_SHOWN = 3;
export const openCardLabel = (id: string) => `Open ${id}`;
export const andMore = (n: number) => `and ${n} more`;

// ---------- the cards ----------

/** The cards a text (Markdown) links with gizai:task chips, in order, each once, in upper case. */
export function linkedCards(md: string): string[] {
  const out: string[] = [];
  if (!md.includes("gizai:task/")) return out;
  for (const m of md.matchAll(new RegExp(ITEM_LINK_SOURCE, "g"))) {
    if (m[2] !== "task") continue;
    const id = parseItemUrl(`gizai:task/${m[3]}`)?.key.trim().toUpperCase();
    if (id && !out.includes(id)) out.push(id);
  }
  return out;
}

const PREFIX = "mcp__gizai__";
const CARD_TOOLS = new Set(["create_task", "move_task"]);
/** Whether an answer's tool calls made or moved cards: one of Hersir's create_task or move_task that ended without an error. */
export function madeOrMoved(tools: readonly { toolName?: string | null; tool?: unknown }[]): boolean {
  return tools.some((m) => {
    const raw = m.toolName ?? "";
    if (!raw.startsWith(PREFIX) || !CARD_TOOLS.has(raw.slice(PREFIX.length))) return false;
    const t = (m.tool ?? {}) as { result?: unknown; isError?: boolean };
    return t.result !== undefined && !t.isError;
  });
}

// ---------- the files ----------

/** A file and line an answer names: the path as written (~/ and / kept, ./ dropped) and the line, from 1. */
export type FileLine = { path: string; line: number };

/** A URL, left out before files are looked for (its path isn't one). */
const URL_RE = /\b[a-z][a-z0-9+.-]*:\/\/[^\s<>()[\]`"']*/gi;
/** A path at the start of a word (after a space, a backtick, a bracket, a quote or emphasis), then :line (and :column), or
 *  ", line 4" / " at line 4" / " on line 4"; not followed by more of a word. */
const FILE_AT = /(^|[\s`(["'“‘*_>])((?:~?\/)?(?:[\w.@+-]+\/)*[\w.@+-]+)(?::(\d{1,7})(?::\d{1,5})?|,?[ \t]+(?:at[ \t]+|on[ \t]+)?line[ \t]+(\d{1,7}))(?![\w/])/gi;
/** A file's name has an extension (tauri.conf.json, .env), or the path has a folder in it (src/Makefile). */
const looksLikeFile = (p: string) => (p.includes("/") || /[^.]\.[A-Za-z][A-Za-z0-9]{0,9}$/.test(p) || /^\.[A-Za-z]\w*$/.test(p)) && !/^[\d.]+$/.test(p);

/** The files and lines a text (Markdown) names, in order, each once (above). */
export function fileLines(md: string): FileLine[] {
  const out: FileLine[] = [];
  if (!/:\d|line\s+\d/i.test(md)) return out;
  const text = md.replace(URL_RE, " ");
  for (const m of text.matchAll(FILE_AT)) {
    let path = m[2]!.replace(/^(?:\.\/)+/, "").replace(/[.]+$/, "");
    const line = Number(m[3] ?? m[4]);
    if (!path || !looksLikeFile(path) || !(line >= 1)) continue;
    if (path.split("/").includes("..")) continue;
    path = path.replace(/\/+$/, "");
    if (!out.some((f) => f.path === path && f.line === line)) out.push({ path, line });
  }
  return out;
}

export const isAbsolute = (path: string) => path.startsWith("/") || path.startsWith("~/");

/** An absolute path (/ or ~/) in one of the Editor's folders of kind project or root (by its path with ~ or its id), the
 *  longest: that folder and the file's path in it. Null for the folder itself, a card's worktree or any other path. */
export function inFolder(path: string, folders: readonly EditorFolder[]): { folder: EditorFolder; path: string } | null {
  const p = normalPath(path);
  let best: { folder: EditorFolder; path: string } | null = null;
  let length = 0;
  for (const f of folders) {
    if (f.kind !== "project" && f.kind !== "root") continue;
    for (const at of [f.path, f.id]) {
      const base = normalPath(at);
      if (base && base.length > length && p.startsWith(`${base}/`)) { best = { folder: f, path: p.slice(base.length + 1) }; length = base.length; }
    }
  }
  return best;
}

/** The folders a path from a folder (src/api.ts) is looked for in, in order: the chat project's own folder first, then the
 *  other projects' folders, then the Editor's other folders (kind root). Card worktrees aren't. */
export function foldersToLook(folders: readonly EditorFolder[], projectId: string | null): EditorFolder[] {
  const rank = (f: EditorFolder) => (f.kind === "project" && projectId && f.projectId === projectId ? 0 : f.kind === "project" ? 1 : 2);
  return folders.filter((f) => f.kind === "project" || f.kind === "root").map((f, i) => ({ f, i, r: rank(f) }))
    .sort((a, b) => a.r - b.r || a.i - b.i).map((x) => x.f);
}

/** The button's words after "Open in Editor": the file's name and the line (tauri.conf.json:4). */
export const fileLabel = (path: string, line: number) => `${path.slice(path.lastIndexOf("/") + 1)}:${line}`;

// ---------- the chat's project ----------

/** The project whose key a card's id starts with (GA-259: the project with key GA), or null. */
export function projectOfCard<P extends Pick<Project, "key">>(identifier: string, projects: readonly P[]): P | null {
  const at = identifier.lastIndexOf("-");
  if (at <= 0) return null;
  const key = identifier.slice(0, at).toUpperCase();
  return projects.find((p) => p.key.toUpperCase() === key) ?? null;
}

/** The chat's project: of the first card the chat is about that has one, else of the first card its answers link that has
 *  one, else null (the team's board). */
export function chatProjectOf<P extends Pick<Project, "key">>(cards: { about: readonly string[]; named: readonly string[] }, projects: readonly P[]): P | null {
  for (const list of [cards.about, cards.named]) {
    for (const id of list) {
      const p = projectOfCard(id, projects);
      if (p) return p;
    }
  }
  return null;
}
