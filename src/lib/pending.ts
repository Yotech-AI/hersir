// GA-102: what the Chat page shows for the Team Lead's calls that wait for your approval, and (GA-312) for the cards its
// answers put on the waiting-for-OK hold. Pure, so it is tested.
import type { ChatMessage, DiffLine, PendingAction } from "../types";
import { toolName, type Group } from "./chat";
import { waitsForYourOk } from "./holds";

/** Of the Chief's groups of messages: the last one of each answer (its run), and the one each message is in, by key. */
function answerGroups(groups: Group[]): { lastOfRun: Map<string, string>; groupOf: Map<string, string> } {
  const lastOfRun = new Map<string, string>(), groupOf = new Map<string, string>();
  for (const g of groups) {
    if (g.side !== "agent") continue;
    for (const m of g.items) {
      groupOf.set(m.id, g.key);
      if (m.runId) lastOfRun.set(m.runId, g.key);
    }
  }
  return { lastOfRun, groupOf };
}

/** Where each waiting call shows: under the last group of messages of the answer that tried it (its run), by that group's
 *  key. A call whose answer has no message in the chat (yet) shows at the end, under "". Each list keeps the calls' order. */
export function placePending(groups: Group[], actions: PendingAction[]): Map<string, PendingAction[]> {
  const { lastOfRun } = answerGroups(groups);
  const out = new Map<string, PendingAction[]>();
  for (const a of actions) {
    const key = (a.runId && lastOfRun.get(a.runId)) || "";
    out.set(key, [...(out.get(key) ?? []), a]);
  }
  return out;
}

/** GA-312: a card a Chief answer put on the waiting-for-OK hold (GA-101): its id and identifier from the tool's result,
 *  the message that says so and that message's answer (`runId`). `holdAt`: when the call put it on that hold (the result's
 *  `task.hold_at`; results from before GA-312 don't say); `at`: when the call started (its message's `createdAt`). */
export type HeldCard = { id: string; identifier: string; messageId: string; runId?: string | null; holdAt?: number | null; at: number };

/** For a result without `hold_at`: how long before its message, and how long after it, a hold set by the call can be. The
 *  message is saved as the call starts, about when the call runs, and a call in a row of them waits for those before it. */
const BEFORE_CALL = 60_000, AFTER_CALL = 10 * 60_000;

/** `t` (the card now) is still on the waiting-for-OK hold the answer put on it, not on another (a question, blocked) or on
 *  one another chat, or a later answer, put on after its hold came off. A card without a hold time can't say: it counts. */
export function onItsHold(c: HeldCard, t: { hold?: string | null; holdKind?: string | null; holdAt?: number | null }): boolean {
  if (!waitsForYourOk(t)) return false;
  if (typeof t.holdAt !== "number") return true;
  if (typeof c.holdAt === "number") return t.holdAt === c.holdAt;
  return t.holdAt >= c.at - BEFORE_CALL && t.holdAt <= c.at + AFTER_CALL;
}

/** The tools whose result says when they put the card on that hold (`on_hold`, `task_done` in the app's tools). */
const HOLDING = new Set(["create_task", "move_task", "update_task"]);

function json(text: unknown): Record<string, unknown> | null {
  if (typeof text !== "string") return null;
  try { const v = JSON.parse(text); return v && typeof v === "object" ? v : null; } catch { return null; }
}

/** The cards the Chief's answers in a chat put on the waiting-for-OK hold, from their create_task, move_task and
 *  update_task results with `on_hold`, in the order they did. A card held again later counts once, for the answer that held
 *  it last. Whether a card is still on that hold is the card's to say (`onItsHold`). */
export function heldCards(messages: ChatMessage[]): HeldCard[] {
  const byId = new Map<string, HeldCard>();
  for (const m of messages) {
    if (m.role !== "tool" || !m.toolName?.startsWith("mcp__gizai__") || !HOLDING.has(toolName(m.toolName))) continue;
    const t = (m.tool ?? {}) as Record<string, unknown>;
    if (t.isError) continue;
    const r = json(t.result);
    const task = r?.task as { id?: unknown; identifier?: unknown; hold_at?: unknown } | undefined;
    if (typeof r?.on_hold !== "string" || typeof task?.id !== "string") continue;
    byId.delete(task.id);
    byId.set(task.id, { id: task.id, identifier: typeof task.identifier === "string" ? task.identifier : task.id, messageId: m.id, runId: m.runId,
      holdAt: typeof task.hold_at === "number" ? task.hold_at : null, at: m.createdAt });
  }
  return [...byId.values()];
}

/** Where each held card shows: under the last group of messages of the answer that held it, as its calls (`placePending`);
 *  without a run, under the group its message is in; a message that isn't shown (yet), at the end, under "". */
export function placeHeld(groups: Group[], cards: HeldCard[]): Map<string, HeldCard[]> {
  const { lastOfRun, groupOf } = answerGroups(groups);
  const out = new Map<string, HeldCard[]>();
  for (const c of cards) {
    const key = (c.runId && lastOfRun.get(c.runId)) || groupOf.get(c.messageId) || "";
    out.set(key, [...(out.get(key) ?? []), c]);
  }
  return out;
}

/** A diff as it reads: lines, and in place of a long run of unchanged lines, how many were left out. */
export type DiffPart = DiffLine | { kind: "fold"; count: number };

/** Folds each run of unchanged lines longer than `keep` lines on each side of a change (and at the start or end) to the
 *  lines next to the changes and a count of the rest. A diff without changes keeps its first `keep` lines. */
export function foldDiff(lines: DiffLine[], keep = 2): DiffPart[] {
  const out: DiffPart[] = [];
  let i = 0;
  while (i < lines.length) {
    if (lines[i].kind !== "same") { out.push(lines[i]); i++; continue; }
    let j = i;
    while (j < lines.length && lines[j].kind === "same") j++;
    const run = lines.slice(i, j);
    const before = i === 0 ? 0 : keep; // lines kept after the change above
    const after = j === lines.length ? 0 : keep; // lines kept before the change below
    if (run.length > before + after + 1) {
      out.push(...run.slice(0, before));
      out.push({ kind: "fold", count: run.length - before - after });
      out.push(...run.slice(run.length - after));
    } else {
      out.push(...run);
    }
    i = j;
  }
  // No change at all: the start of the text.
  if (!lines.some((l) => l.kind !== "same") && lines.length > keep + 1) return [...lines.slice(0, keep), { kind: "fold", count: lines.length - keep }];
  return out;
}

/** A tool from outside Gizai in words: "search on the otus server", "web search", "fetching a web page". */
export function outsideLabel(tool: string): string {
  if (tool === "WebSearch") return "web search";
  if (tool === "WebFetch") return "fetching a web page";
  const m = /^mcp__(.+?)__(.+)$/.exec(tool);
  if (m) return `${m[2].replace(/_/g, " ")} on the ${m[1]} server`;
  return tool;
}
