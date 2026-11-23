// GA-345: what Hersir's view in Claude Desktop's chat gets from its tools (src-tauri/src/tools/apps.rs), word for word as
// the card's contract has it, so GA-349's card and Chief views build on these types.
//
// - `show {view, id?}` (the model's): `structuredContent` {view, id, data}.
// - `view_read {view, id?}` (the view's alone): {view, id, seq, data}; `seq` is board_changes' counter, read before the data.
// - `board_changes {since}` (the view's alone): {seq, cards, board, reload}, in `structuredContent` and as JSON text.
// - The view writes with the model's own tools, through the host: move_task, comment_on_task, start_agent_run,
//   stop_agent_run, ask_chief, chief_reply.
//
// No result passes 150,000 characters; a text cut for that ends with CUT.

/** How every text cut to keep a result small ends. */
export const CUT = "…(cut: read the rest in Hersir)";

export type ViewName = "board" | "card" | "chief";

/** A card's hold, or null. */
export type Hold = "needs_decision" | "blocked" | "stalled";

/** A label, with its colour as the board shows it (null: none). */
export type ViewLabel = { name: string; color: string | null };

/** A card on the board. `running`: an agent works on it now. `needs_you`: it is in the Inbox (on hold, or waiting for you
 *  in Review or Deploy). `project`: the project's name. */
export type BoardCard = {
  identifier: string; title: string; project: string | null; assignee: string | null; labels: ViewLabel[]; priority: number;
  hold: Hold | null; running: boolean; needs_you: boolean;
};

/** A column, in board order: its `kind` (waiting, work, testing, review, deploy, done, backlog; cancelled), how many cards
 *  it has (`total`) and the cards sent, in board order: at most 50 (20 in Backlog, Done and Cancelled), fewer on a big board. */
export type BoardColumn = { name: string; kind: string; total: number; cards: BoardCard[] };

/** The board of a project (`project`), or (null) every project on the first team's board. */
export type BoardData = { team: string; project: { key: string; name: string } | null; columns: BoardColumn[] };

/** A card's comment: the last 20, oldest first. `at`: UTC, like 2026-11-23T14:05:09Z. */
export type CardComment = { author: string; at: string; body_md: string };

/** A run on the card: the last 5, newest first. */
export type CardRun = {
  agent: string; status: string; outcome: string | null; started_at: string | null; ended_at: string | null; summary: string | null;
};

/** One card, as its page shows it. `live`: the run working on it now, with the last 200 lines of its output (when more
 *  were left out, a line CUT follows them), or null. */
export type CardData = {
  identifier: string; title: string; project: string | null; column: string; assignee: string | null; labels: ViewLabel[];
  priority: number; hold: Hold | null; hold_reason: string | null; description_md: string; acceptance_md: string | null;
  pr_url: string | null; comments: CardComment[]; runs: CardRun[]; live: { agent: string; output: string } | null;
};

/** A message in a chat with the Chief, as chief_reply gives it: yours, the Chief's or Hersir's note. */
export type ChiefMessage = { id: string; from: "you" | "Chief" | "Hersir"; time: string; text: string };

/** A chat with the Chief, as chief_reply gives it; `thread` null before ask_chief made the chat From Claude Desktop.
 *  `draft`: what the Chief writes now, while it works. */
export type ChiefData = {
  thread: string | null; status: "working" | "waiting for you" | "answered"; messages: ChiefMessage[]; draft: string | null;
};

export type ViewData = { board: BoardData; card: CardData; chief: ChiefData };

/** show's and view_read's structuredContent. `id`: the project's key, the card's identifier or the chat's thread id; null
 *  for every project, or before the chat From Claude Desktop is made. `seq`: view_read's only. */
export type ViewResult<V extends ViewName = ViewName> = { view: V; id: string | null; seq?: number; data: ViewData[V] };

/** board_changes' answer: `seq` the counter now (ask with it next), `cards` the identifiers of the cards that changed since,
 *  `board` true when columns, labels, agents, people, projects or teams changed, `reload` true when too much changed (or
 *  since is from before a restore): read the view again. */
export type Changes = { seq: number; cards: string[]; board: boolean; reload: boolean };

/** A view's result in a tool's structuredContent; null when it isn't one. */
export function viewOf(structured: unknown): ViewResult | null {
  const v = structured as Partial<ViewResult> | null | undefined;
  if (!v || typeof v !== "object" || (v.view !== "board" && v.view !== "card" && v.view !== "chief") || !v.data || typeof v.data !== "object") return null;
  return { view: v.view, id: typeof v.id === "string" ? v.id : null, seq: typeof v.seq === "number" ? v.seq : undefined, data: v.data } as ViewResult;
}

/** board_changes' answer in a tool's structuredContent; null when it isn't one. */
export function changesOf(structured: unknown): Changes | null {
  const c = structured as Partial<Changes> | null | undefined;
  if (!c || typeof c.seq !== "number" || !Array.isArray(c.cards)) return null;
  return { seq: c.seq, cards: c.cards, board: !!c.board, reload: !!c.reload };
}
