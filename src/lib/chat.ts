// What the Chat page shows for tool calls and how it groups messages. Pure, so it is tested.
import { href, type Page } from "../router";
import type { ChatMessage, ChatNoteMeta } from "../types";

const PREFIX = "mcp__gizai__";

/** GA-351: the small line over a message of yours that came from Claude Desktop (its ask_chief), sent or queued. */
export const FROM_CLAUDE_DESKTOP = "From Claude Desktop";

/** GA-351: whether a message's meta says it came from Claude Desktop: `{from: "claude-desktop"}` (`gizai_core::chat::desktop_meta`). */
export const fromClaudeDesktop = (meta?: ChatNoteMeta | null): boolean => meta?.from === "claude-desktop";

/** "mcp__gizai__create_task" → "create_task"; other tools keep their name. */
export function toolName(raw: string): string {
  return raw.startsWith(PREFIX) ? raw.slice(PREFIX.length) : raw;
}

/** Each Hersir tool's line: what it did ("Changed task") and its base form ("change task"), for a call that was refused
 *  ("Couldn't change task DU-37") or kept for your OK ("Waiting for your OK: change task DU-37"). */
const VERBS: Record<string, [done: string, base: string]> = {
  get_overview: ["Looked at the overview", "look at the overview"], read_inbox: ["Read the inbox", "read the inbox"],
  check_board: ["Checked the board", "check the board"], start_chat: ["Started a chat", "start a chat"],
  list_clients: ["Listed clients", "list clients"], get_client: ["Opened client", "open client"],
  list_projects: ["Listed projects", "list projects"], get_project: ["Opened project", "open project"],
  list_tasks: ["Listed tasks", "list tasks"], get_task: ["Opened task", "open task"],
  list_agents: ["Listed agents", "list agents"], get_agent: ["Opened agent", "open agent"],
  list_docs: ["Listed docs", "list docs"], read_doc: ["Read doc", "read doc"], list_people: ["Listed people", "list people"],
  get_workflow: ["Read the workflow", "read the workflow"],
  create_client: ["Added client", "add client"], update_client: ["Changed client", "change client"],
  save_contact: ["Saved contact for", "save contact for"],
  create_project: ["Created project", "create project"], update_project: ["Changed project", "change project"],
  create_task: ["Created task", "create task"], update_task: ["Changed task", "change task"], move_task: ["Moved task", "move task"],
  archive_tasks: ["Archived tasks", "archive tasks"], restore_tasks: ["Restored tasks", "restore tasks"],
  comment_on_task: ["Commented on", "comment on"],
  create_agent: ["Added agent", "add agent"], update_agent: ["Changed agent", "change agent"],
  set_agent_status: ["Changed agent status", "change agent status"], add_routing_rule: ["Added a routing rule", "add a routing rule"],
  add_column: ["Added a column", "add a column"], set_column: ["Changed column", "change column"],
  save_label: ["Saved label", "save label"],
  start_agent_run: ["Started an agent on", "start an agent on"], continue_agent_run: ["Continued the agent on", "continue the agent on"],
  stop_agent_run: ["Stopped the agent on", "stop the agent on"],
  create_doc: ["Created doc", "create doc"], write_doc: ["Wrote doc", "write doc"],
  attach_file: ["Attached a file to", "attach a file to"], add_person: ["Added person", "add person"],
  update_checkout: ["Updated the checkout", "update the checkout"], merge_pull_request: ["Merged the pull request of", "merge the pull request of"],
  memory_list: ["Listed notes", "list notes"], memory_search: ["Searched notes for", "search notes for"], memory_read: ["Read note", "read note"],
  memory_write: ["Saved note", "save note"], memory_append: ["Added to note", "add to note"], memory_move: ["Moved note", "move note"],
};
/** Claude Code's own (read-only) tools in chat. */
const BUILTIN: Record<string, [done: string, base: string]> = {
  Read: ["Read a file", "read a file"], Glob: ["Looked for files", "look for files"], Grep: ["Searched the code", "search the code"],
};

/** A call's state: running, done, refused (`error`), or kept under the answer for your OK (`waiting`, GA-102). */
export type ToolCard = { verb: string; label?: string; href?: string; state: "running" | "ok" | "error" | "waiting"; detail?: string };

function parse(text: unknown): Record<string, unknown> | null {
  if (typeof text !== "string") return null;
  try { const v = JSON.parse(text); return v && typeof v === "object" ? v : null; } catch { return null; }
}

/** What a call was about, from its input: a task id, a project, a note, a file name. `whole`: a path names a memory note,
 *  shown whole ("Chief/Notes"), not only its last part as a file's. */
function target(input: unknown, whole = false): string | undefined {
  if (!input || typeof input !== "object") return undefined;
  const i = input as Record<string, unknown>;
  for (const k of ["task", "project", "client", "agent", "doc", "name", "title", "note"]) if (typeof i[k] === "string" && i[k]) return i[k] as string;
  for (const k of ["file_path", "path"]) if (typeof i[k] === "string") return whole ? i[k] as string : (i[k] as string).split("/").pop();
  for (const k of ["column", "folder"]) if (typeof i[k] === "string" && i[k]) return i[k] as string;
  if (typeof i.pattern === "string") return i.pattern;
  if (typeof i.query === "string") return i.query;
  return undefined;
}

/** The answer to a call Hersir kept for your OK instead of running it (`not_run` in the app's tools): "… wasn't run: this chat
 *  used …, a tool from outside Hersir. Hersir keeps the call for the user to approve …" (GA-261), or "this answer used …" as
 *  GA-102 said until 9 Nov. One it couldn't keep is refused. */
const KEPT = /wasn't run: this (?:chat|answer) used .*?Hersir keeps the call for the user to approve/s;

/** "mcp__gizai__memory_append" → "memory append": a Hersir tool without a line of its own still reads as words. */
const words = (name: string) => name.replace(/_/g, " ");

export function toolCard(m: ChatMessage): ToolCard {
  const raw = m.toolName ?? "";
  const name = toolName(raw);
  const own = raw.startsWith(PREFIX);
  // Another server's tool keeps its name: it has no base form to say what failed.
  const [done, base] = (own ? VERBS[name] : BUILTIN[raw]) ?? (own ? [words(name).replace(/^./, (c) => c.toUpperCase()), words(name)] : [raw, null]);
  const t = (m.tool ?? {}) as Record<string, unknown>;
  const fromInput = target(t.input, own && name.startsWith("memory_"));
  if (t.result === undefined) return { verb: done, label: fromInput, state: "running" };
  if (t.isError) {
    const why = String(t.result);
    if (own && base && KEPT.test(why)) return { verb: `Waiting for your OK: ${base}`, label: fromInput, state: "waiting" };
    return { verb: base ? `Couldn't ${base}` : done, label: fromInput, state: "error", detail: why };
  }
  const r = parse(t.result);
  const link = r?.link as { page?: string; id?: string; label?: string } | undefined;
  if (link?.page && link.id) return { verb: done, label: link.label ?? fromInput, href: href({ page: link.page as Page, id: link.id }), state: "ok" };
  if (typeof r?.count === "number") return { verb: done, label: `${r.count} ${r.count === 1 ? "item" : "items"}`, state: "ok" };
  return { verb: done, label: fromInput, state: "ok" };
}

export type Group = { side: "user" | "agent" | "system"; key: string; at: number; author?: string | null; items: ChatMessage[] };

const QUIET_MS = 5 * 60_000;

/** Consecutive messages of one speaker share a header; the Team Lead's text and tool calls are one speaker. */
export function groupMessages(msgs: ChatMessage[]): Group[] {
  const out: Group[] = [];
  for (const m of msgs) {
    const side: Group["side"] = m.role === "user" ? "user" : m.role === "system" ? "system" : "agent";
    const last = out[out.length - 1];
    const lastAt = last?.items[last.items.length - 1].createdAt ?? 0;
    if (last && last.side === side && side !== "system" && m.createdAt - lastAt <= QUIET_MS) last.items.push(m);
    else out.push({ side, key: m.id, at: m.createdAt, author: m.authorName, items: [m] });
  }
  return out;
}

/** Ways to start, on an empty chat. */
export const SUGGESTIONS: { label: string; text: string }[] = [
  { label: "What needs my attention?", text: "What needs my attention today?" },
  { label: "Add a client", text: "Add a client: " },
  { label: "Plan a project", text: "Plan a new project for " },
  { label: "Set up my team", text: "Set up my software team: a Frontend, a Backend and a QA agent, on their usual columns." },
];

/** The text being written, and the last change to it that it holds (ChatStatus.seq). */
export type LiveDraft = { text: string; seq: number };
/** A change to the text being written: words added, or a new block (the draft starts again). */
export type DraftChange = { kind: "delta"; text: string; seq: number } | { kind: "block"; seq: number };

/** Applies a change once: one the draft already holds (it came from a snapshot taken after it) is skipped. */
export function applyDraft(d: LiveDraft, c: DraftChange): LiveDraft {
  if (c.seq <= d.seq) return d;
  return c.kind === "delta" ? { text: d.text + c.text, seq: c.seq } : { text: "", seq: c.seq };
}

/** A snapshot of the text being written (chat_live) joined with the changes heard after it, also those that arrived while
 *  it was asked for: no words go missing, and none come twice. */
export function withSnapshot(snap: LiveDraft, heard: DraftChange[]): LiveDraft {
  return heard.filter((c) => c.seq > snap.seq).sort((a, b) => a.seq - b.seq).reduce(applyDraft, snap);
}

/** Chat → Recent shows this many chats, the newest activity first; the Archive (#/chats) lists them all. */
export const RECENT_MAX = 30;

/** Runs of spaces and line breaks as one space, trimmed. */
const oneLine = (s: string) => s.split(/\s+/).filter(Boolean).join(" ");

/** The Archive's search as a pattern: the words as typed, case ignored, every character literal. Null for an empty one. */
function searchPattern(query: string, flags: string): RegExp | null {
  const q = oneLine(query);
  return q ? new RegExp(q.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), flags) : null;
}

/** Markdown without its marks, for a line of preview: links and images keep their text; heading, quote and list marks, bold
 *  and code marks go. */
function plain(md: string): string {
  return md.replace(/!?\[([^\]\n]*)\]\([^)\s]*\)/g, "$1").replace(/^[ \t]*(?:#{1,6}|>|[-*+]|\d+[.)])[ \t]+/gm, "").replace(/\*\*|__|`/g, "");
}

/** How far a snippet's cut moves to fall between two words; in a longer word it cuts inside. */
const WORD_CUT = 20;

/** A search result's piece of a message: one line of at most about `max` characters around the first match of `query`, cut
 *  on words (a very long word, like a link, is cut inside), with … where text was left out, and without Markdown's marks
 *  unless the match is in them (a link's address). Without a match, the message's start. */
export function snippet(body: string, query: string, max = 140): string {
  const re = searchPattern(query, "i");
  const bare = oneLine(plain(body));
  const text = !re || re.test(bare) || !re.test(oneLine(body)) ? bare : oneLine(body);
  if (text.length <= max) return text;
  const m = re?.exec(text);
  const at = m?.index ?? 0;
  const end = at + (m?.[0].length ?? 0);
  // A third of the room before the match, so it reads in context; near the end, more of what comes before.
  let from = Math.max(0, Math.min(at - Math.floor(Math.max(0, max - (end - at)) / 3), text.length - max));
  let to = from + max;
  const space = text.indexOf(" ", from);
  if (from > 0 && space >= 0 && space < at && space - from <= WORD_CUT) from = space + 1;
  const last = text.lastIndexOf(" ", to);
  if (to < text.length && last >= end && to - last <= WORD_CUT) to = last;
  return `${from > 0 ? "…" : ""}${text.slice(from, to)}${to < text.length ? "…" : ""}`;
}

/** `text` in parts, the matches of `query` (case ignored) marked, to highlight them. */
export function highlight(text: string, query: string): { text: string; hit: boolean }[] {
  const re = searchPattern(query, "gi");
  const parts: { text: string; hit: boolean }[] = [];
  let at = 0;
  for (const m of re ? text.matchAll(re) : []) {
    const i = m.index ?? 0;
    if (i > at) parts.push({ text: text.slice(at, i), hit: false });
    parts.push({ text: m[0], hit: true });
    at = i + m[0].length;
  }
  if (at < text.length || parts.length === 0) parts.push({ text: text.slice(at), hit: false });
  return parts;
}

/** Who wrote a message the Archive found: you, or the Team Lead (by its name). */
export function hitAuthor(m: Pick<ChatMessage, "role" | "authorName">): string {
  return m.role === "user" ? "You" : m.authorName || "Chief";
}

/** The coding CLI a chat runs on: its own pick while that is still in Settings, else the Team Lead's Runs on. */
export function chatRunsOn(chatCli: string | null | undefined, leadCli: string | null | undefined, clis: { id: string }[] | null): string {
  if (chatCli && (!clis || clis.some((c) => c.id === chatCli))) return chatCli;
  return leadCli || "claude_code";
}
