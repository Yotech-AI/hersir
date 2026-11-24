// A project's Time tracking tab (GA-331): how long the agents worked on each card, over all of its runs, and the sums.
import type { CardTime } from "../types";

const MINUTE = 60_000;
const HOUR = 60 * MINUTE;

/** The note under the tab's list, word for word. */
export const TIME_NOTE = "Agent time: from each run's start to its end, added up over every run on the card (each start, Continue and nudge is a run). Your own time isn't in it. Tokens: input (cache included) and output, as on the Usage page.";

/** The title of a working card's teal dot. */
export const WORKING_TITLE = "An agent is working on it now";

/** "—" for none, "<1m" under a minute, "42m" under an hour, else hours and two-digit minutes: "3h 05m", "31h 10m" (no days). */
export function formatAgentTime(ms: number): string {
  if (!(ms > 0)) return "—";
  if (ms < MINUTE) return "<1m";
  if (ms < HOUR) return `${Math.floor(ms / MINUTE)}m`;
  const h = Math.floor(ms / HOUR);
  const m = Math.floor((ms - h * HOUR) / MINUTE);
  return `${h}h ${String(m).padStart(2, "0")}m`;
}

/** A card's agent time at `now`: its ended runs' time, plus the time of the run working on it now. */
export function agentTimeMs(c: Pick<CardTime, "timeMs" | "runningSince">, now: number): number {
  return c.timeMs + (c.runningSince != null ? Math.max(0, now - c.runningSince) : 0);
}

/** A card's number, from its identifier ("GA-331" → 331): the newest card has the highest. */
export function cardNumber(identifier: string): number {
  const m = /(\d+)\s*$/.exec(identifier);
  return m ? Number(m[1]) : 0;
}

/** What the column shows: the board column's name, or Archived for an archived card. */
export const columnLabel = (c: Pick<CardTime, "column" | "archived">) => (c.archived ? "Archived" : c.column);

export type TimeTotals = { cards: number; runs: number; inputTokens: number; outputTokens: number; timeMs: number };

/** The sums of all rows at `now`, the running runs' time included. */
export function timeTotals(rows: readonly CardTime[], now: number): TimeTotals {
  const t: TimeTotals = { cards: rows.length, runs: 0, inputTokens: 0, outputTokens: 0, timeMs: 0 };
  for (const c of rows) {
    t.runs += c.runs;
    t.inputTokens += c.inputTokens;
    t.outputTokens += c.outputTokens;
    t.timeMs += agentTimeMs(c, now);
  }
  return t;
}
