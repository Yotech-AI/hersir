// The Card kind of the Chat's side island (GA-297; design/islands-chat → Card): a card for reading and checking quickly,
// beside the chat. Its tab's id is the card's id (GA-259, upper case), so a chip, an answer's button and the cards a
// Chief-started chat is about all find the one tab. The parts here are pure, apart from the once-per-chat list.
// - What opens by itself: a chat the Chief started about cards (ChatThread.tasks) opens them as Card tabs the first time
//   it opens, the first one shown; Hersir keeps the chat's id in localStorage (gizai.chat.sideOpened, the newest 200), so
//   once you close them or hide the island, opening the chat again leaves it as you left it, also after a restart.
// localStorage is read inside try/catch: the tests run in Node, which has none.
import type { Comment, Run } from "../types";
import { cardTabId, openSideTabs } from "./chatSide";

export { cardTabId };

// ---------- what the tab shows ----------

/** The description folds after this many lines, a comment after COMMENT_LINES; the first CRITERIA_SHOWN criteria, the
 *  latest COMMENTS_SHOWN comments and RUNS_SHOWN runs show until Show all. */
export const DESCRIPTION_LINES = 4;
export const COMMENT_LINES = 3;
export const CRITERIA_SHOWN = 3;
export const COMMENTS_SHOWN = 2;
export const RUNS_SHOWN = 3;
/** The hold's reason shows its end, this many characters, as Properties does, then Read in comments. */
export const REASON_CHARS = 60;

/** Read in comments: the latest comment whose text holds the hold's reason (an agent's summary is posted as one), as on
 *  the card's page, or undefined (no reason, or no comment has it: the waiting-for-OK hold's reason is in none). */
export function reasonComment<C extends Pick<Comment, "bodyMd">>(reason: string | null | undefined, comments: readonly C[] | null | undefined): C | undefined {
  const r = reason?.trim();
  return r ? [...(comments ?? [])].reverse().find((c) => c.bodyMd.includes(r)) : undefined;
}

export const showAll = (n: number) => `Show all ${n}`;

/** One acceptance criterion: whether it is checked, and its text (Markdown, on one line). */
export type Criterion = { done: boolean; text: string };
const ITEM = /^\s*(?:[-*+]|\d+[.)])\s+\[([ xX])\]\s?(.*)$/;
const MORE = /^(?:\s{2,}|\t)\S/;
/** The criteria of a card's acceptance text: its checklist items (- [ ] and - [x]), a line indented under one going with
 *  it. Null when it has none (plain text shows as the description does). */
export function criteriaOf(md: string | null | undefined): Criterion[] | null {
  const items: Criterion[] = [];
  let last: Criterion | null = null;
  for (const line of (md ?? "").split(/\r?\n/)) {
    const m = ITEM.exec(line);
    if (m) { last = { done: m[1] !== " ", text: m[2]!.trim() }; items.push(last); continue; }
    if (last && MORE.test(line)) last.text = `${last.text} ${line.trim()}`;
    else last = null;
  }
  return items.length ? items : null;
}
/** "2 of 6": the checked criteria of all. */
export const criteriaCount = (items: Criterion[]) => `${items.filter((c) => c.done).length} of ${items.length}`;

/** The runs newest first (as list_runs gives them): a run's number counts from the oldest, Run 1. */
export function runNumber(runs: Pick<Run, "id">[], id: string): number {
  const i = runs.findIndex((r) => r.id === id);
  return i < 0 ? 0 : runs.length - i;
}
/** QA's verdict on a card in Review: its newest run that ended QA passed or QA failed, or null. */
export const qaVerdict = <R extends Pick<Run, "outcome">>(runs: R[]): R | null =>
  runs.find((r) => r.outcome === "qa_pass" || r.outcome === "qa_fail") ?? null;
/** "Run 2 · 1 h ago". */
export const runLine = (n: number, ago: string) => `Run ${n} · ${ago}`;

/** Run or Continue in the foot, where the card's page allows it: neither on a card in Review or Done, archived, or while
 *  an agent works on it; Continue when its last run can go on (it clears a hold); Run when it isn't held and an agent can
 *  run it (RunPanel's rule). */
export function footRun(c: { category: string; held: boolean; archived: boolean; live: boolean; continuable: boolean; canRun: boolean }): "run" | "continue" | null {
  if (c.archived || c.live || c.category === "review" || c.category === "done") return null;
  if (c.continuable) return "continue";
  return !c.held && c.canRun ? "run" : null;
}

// ---------- once per chat ----------

export const SIDE_OPENED_KEY = "gizai.chat.sideOpened";
export const SIDE_OPENED_MAX = 200;

function readKey(key: string): string | null { try { return localStorage.getItem(key); } catch { return null; } }
function writeKey(key: string, value: string) { try { localStorage.setItem(key, value); } catch { /* private mode */ } }

/** The chats whose cards opened once, oldest first; nothing kept, or something unreadable, is none. */
export function openedChats(kept: string | null): string[] {
  try {
    const list: unknown = JSON.parse(kept ?? "[]");
    return Array.isArray(list) ? list.filter((x): x is string => typeof x === "string") : [];
  } catch { return []; }
}
/** The list with the chat at its end, the newest `max` kept. */
export function withOpenedChat(list: string[], id: string, max = SIDE_OPENED_MAX): string[] {
  return [...list.filter((x) => x !== id), id].slice(-max);
}

/** Opening a chat: the first time a chat the Chief started about cards opens, its cards open as Card tabs, the first one
 *  shown. Whether they opened. */
export function openChatCardsOnce(chatId: string, cards: string[] | null | undefined): boolean {
  const ids = [...new Set((cards ?? []).map(cardTabId).filter(Boolean))];
  if (!ids.length) return false;
  const kept = openedChats(readKey(SIDE_OPENED_KEY));
  if (kept.includes(chatId)) return false;
  writeKey(SIDE_OPENED_KEY, JSON.stringify(withOpenedChat(kept, chatId)));
  openSideTabs(ids.map((id) => ({ kind: "card", id })), chatId);
  return true;
}
