// What a Chief answer asks you to run (GA-279), for Open in terminal under it in the chat. Pure, so it can be tested in Node.
// - Its commands, in order: each line of a fenced code block in a shell's language (SHELL_LANGUAGES), without a leading
//   `$ ` or `> ` prompt, leaving out empty lines and lines that start with #; and each inline code span right after the
//   word run, type, paste or execute (any case), with at most this, these, the command, the commands or a colon in between
//   ("Run `php artisan migrate`", "run this: `npm run build`"). A code span after other words ("QA ran `npm test`", "the
//   file `src/api.ts`"), a code block in another language and plain text aren't commands.
// - The folders it names: each path, in code or in the text, that starts with ~/ or / at the start of a word, in the
//   order named, without a trailing / . , : or ;. A URL's path (http://127.0.0.1:8081/page.html) and a path after other
//   characters (<APP_URL>/embed/v1/loader.js) don't count. A path is in the Editor folder of kind project or root whose
//   path (with ~) or id (absolute) it is, or that it lies inside, the longest one; a card's worktree and any other path
//   aren't.
import type { EditorFolder } from "../types";

/** A fenced code block in one of these languages holds commands, one on each line. */
export const SHELL_LANGUAGES: readonly string[] = ["sh", "bash", "zsh", "shell", "console", "terminal", "fish", "powershell", "pwsh", "ps1", "cmd"];

/** Open in terminal shows at most this many folders under a message. */
export const MOST_FOLDERS = 3;

type Block = { text: string } | { lang: string; lines: string[] };

/** The message's fenced code blocks (``` or ~~~, also indented in a list; one that isn't closed runs to the end) and the
 *  text around them, in order. */
function blocks(md: string): Block[] {
  const lines = md.replace(/\r\n?/g, "\n").split("\n");
  const out: Block[] = [];
  let text: string[] = [];
  for (let i = 0; i < lines.length; i++) {
    const line = lines[i]!;
    const open = /^[ \t]*(`{3,}|~{3,})(.*)$/.exec(line);
    // a backtick fence's info string has no backtick: ```npm ci``` is inline code
    if (!open || (open[1]![0] === "`" && open[2]!.includes("`"))) { text.push(line); continue; }
    const fence = open[1]!;
    const body: string[] = [];
    let j = i + 1;
    for (; j < lines.length; j++) {
      const close = /^[ \t]*(`{3,}|~{3,})[ \t]*$/.exec(lines[j]!);
      if (close && close[1]![0] === fence[0] && close[1]!.length >= fence.length) break;
      body.push(lines[j]!);
    }
    if (text.length) out.push({ text: text.join("\n") });
    text = [];
    out.push({ lang: (open[2]!.trim().split(/\s+/)[0] ?? "").toLowerCase(), lines: body });
    i = j;
  }
  if (text.length) out.push({ text: text.join("\n") });
  return out;
}

/** Whether the character at `at` has a backslash before it that escapes it (an odd number of them). */
function escaped(text: string, at: number): boolean {
  let k = at;
  while (k > 0 && text[k - 1] === "\\") k--;
  return (at - k) % 2 === 1;
}

/** The inline code spans in Markdown text, as CommonMark finds them: a run of backticks up to the next run of as many; a run
 *  without one is text, and a backslash before a run takes its first backtick out of it. `from` and `to` are where the
 *  span starts and ends in `text`, its backticks included. */
function codeSpans(text: string): { from: number; to: number; code: string }[] {
  const runs = [...text.matchAll(/`+/g)].map((m) => ({ at: m.index ?? 0, n: m[0].length }));
  const out: { from: number; to: number; code: string }[] = [];
  for (let i = 0; i < runs.length; i++) {
    let { at, n } = runs[i]!;
    if (escaped(text, at)) { at += 1; n -= 1; }
    if (n === 0) continue;
    const j = runs.findIndex((r, k) => k > i && r.n === n);
    if (j < 0) continue;
    const close = runs[j]!;
    let code = text.slice(at + n, close.at).replace(/\n/g, " ");
    if (code.startsWith(" ") && code.endsWith(" ") && code.trim()) code = code.slice(1, -1);
    out.push({ from: at, to: close.at + n, code });
    i = j;
  }
  return out;
}

/** The text before a code span ends in run, type, paste or execute (a word of its own, any case), with at most this, these,
 *  the command, the commands or a colon after it, and the emphasis around them (**Run:**). The spaces between are bounded,
 *  so a long run of them can't make it slow. */
const ASKS = /(?:^|[^\p{L}\p{N}_])(?:run|type|paste|execute)[*_]{0,3}(?:\s{1,12}[*_]{0,3}(?:this|these|the\s{1,12}commands?)[*_]{0,3})?\s{0,12}:?[*_]{0,3}\s{0,12}$/iu;

/** The commands a message (Markdown) asks you to run, in order (above). */
export function runCommands(md: string): string[] {
  // both kinds are code: without a backtick or a ~~~ fence there is none
  if (!md.includes("`") && !md.includes("~~~")) return [];
  const out: string[] = [];
  for (const b of blocks(md)) {
    if ("lines" in b) {
      if (!SHELL_LANGUAGES.includes(b.lang)) continue;
      for (const raw of b.lines) {
        const line = raw.trim().replace(/^[$>](?:\s+|$)/, "");
        if (line && !line.startsWith("#")) out.push(line);
      }
      continue;
    }
    let after = 0;
    for (const s of codeSpans(b.text)) {
      const ask = ASKS.exec(b.text.slice(after, s.from));
      // a blank line between is another paragraph
      if (ask && !/\n[ \t]*\n/.test(ask[0]) && s.code.trim()) out.push(s.code.trim());
      after = s.to;
    }
  }
  return out;
}

/** A path at the start of a word (after a space, a backtick, a bracket, a quote or emphasis), up to a space or one of them. */
const PATH = /(^|[\s`(["'“‘{*])(~?\/[^\s`'"()<>[\]{}|*]*)/g;

/** The paths a message (Markdown) names, in the order named, each once (above). */
export function namedPaths(md: string): string[] {
  const out: string[] = [];
  for (const m of md.matchAll(PATH)) {
    const p = m[2]!.replace(/[/.,:;?!]+$/, "");
    if ((p.startsWith("/") || p.startsWith("~/")) && !out.includes(p)) out.push(p);
  }
  return out;
}

/** A path with / between its parts once, without . and with .. taken back: ~/Herd/a/../b/ is ~/Herd/b. */
function normal(path: string): string {
  const [head = "", ...rest] = path.replace(/\\/g, "/").split("/");
  const parts: string[] = [];
  for (const s of rest) {
    if (s === "" || s === ".") continue;
    if (s === "..") parts.pop();
    else parts.push(s);
  }
  return [head, ...parts].join("/");
}

/** The Editor folder a path is: the project's or the one in the Editor folders whose path (with ~) or id (absolute) it is,
 *  or that it lies inside, the longest one. A card's worktree isn't one; null for any other path. */
export function folderOf(path: string, folders: readonly EditorFolder[]): EditorFolder | null {
  const p = normal(path);
  let best: EditorFolder | null = null;
  let length = 0;
  for (const f of folders) {
    if (f.kind !== "project" && f.kind !== "root") continue;
    for (const at of [f.path, f.id]) {
      const base = normal(at);
      if (base && base.length > length && (p === base || p.startsWith(`${base}/`))) { best = f; length = base.length; }
    }
  }
  return best;
}

/** The folders a message names that Open in terminal opens a terminal in: each once, in the order named, at most
 *  MOST_FOLDERS. */
export function terminalFolders(md: string, folders: readonly EditorFolder[]): EditorFolder[] {
  const out: EditorFolder[] = [];
  for (const p of namedPaths(md)) {
    const f = folderOf(p, folders);
    if (f && !out.some((x) => x.id === f.id)) out.push(f);
    if (out.length === MOST_FOLDERS) break;
  }
  return out;
}
