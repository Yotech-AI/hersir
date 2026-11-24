// A project's Time tracking tab (GA-331, GA-354): how long the agents worked on each card in a period, the sums, and the bars
// of its charts.
import type { CardTime, TimeDay } from "../types";

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

export type TimeTotals = { cards: number; runs: number; inputTokens: number; outputTokens: number; timeMs: number; costUsdMicros: number;
  unknownCostRuns: number };

/** The sums of all rows at `now`, the running runs' time included; `cards`: the rows with a run that started. */
export function timeTotals(rows: readonly CardTime[], now: number): TimeTotals {
  const t: TimeTotals = { cards: 0, runs: 0, inputTokens: 0, outputTokens: 0, timeMs: 0, costUsdMicros: 0, unknownCostRuns: 0 };
  for (const c of rows) {
    if (c.runs > 0) t.cards += 1;
    t.runs += c.runs;
    t.inputTokens += c.inputTokens;
    t.outputTokens += c.outputTokens;
    t.timeMs += agentTimeMs(c, now);
    t.costUsdMicros += c.costUsdMicros;
    t.unknownCostRuns += c.unknownCostRuns;
  }
  return t;
}

// ---- GA-354: the period filter, and the bars of the charts. Days are UTC, as on the Usage page. ----

const DAY = 86_400_000;

/** The sentence after TIME_NOTE in the tab's footer. */
export const TIME_COST_NOTE = "API cost: an estimate at API prices, as on the Usage page.";

/** "<n> runs on <n> cards", under the Agent time total. */
export const runsOnCards = (t: Pick<TimeTotals, "runs" | "cards">) =>
  `${t.runs.toLocaleString("en-GB")} run${t.runs === 1 ? "" : "s"} on ${t.cards.toLocaleString("en-GB")} card${t.cards === 1 ? "" : "s"}`;

export type TimePeriodKey = "all" | "year" | "month" | "week" | "custom";

/** The chips, in their order. */
export const TIME_PERIODS: readonly { key: TimePeriodKey; label: string }[] = [
  { key: "all", label: "All" }, { key: "year", label: "This year" }, { key: "month", label: "This month" }, { key: "week", label: "This week" },
  { key: "custom", label: "Custom" },
];

/** A period: the runs that started from `since` up to (not including) `until`, both UTC midnights. */
export type TimeRange = { since: number; until: number };

/** The longest custom period, in days: gizai-core's MAX_TIME_DAYS (ten years). */
export const MAX_RANGE_DAYS = 3_660;

/** Midnight UTC of the day `ms` falls on. */
export const utcDay = (ms: number) => ms - (((ms % DAY) + DAY) % DAY);

/** The Monday (midnight UTC) of the week `ms` falls in. */
export function utcMonday(ms: number): number {
  const day = utcDay(ms);
  return day - ((new Date(day).getUTCDay() + 6) % 7) * DAY;
}

/** The 1st (midnight UTC) of the month `ms` falls in. */
export function utcMonthStart(ms: number): number {
  const d = new Date(ms);
  return Date.UTC(d.getUTCFullYear(), d.getUTCMonth(), 1);
}

/** The whole UTC year, month or week (Monday to Sunday) that `now` falls in; null for All. */
export function periodRange(key: Exclude<TimePeriodKey, "custom">, now: number): TimeRange | null {
  const d = new Date(now);
  const y = d.getUTCFullYear(), m = d.getUTCMonth();
  switch (key) {
    case "year": return { since: Date.UTC(y, 0, 1), until: Date.UTC(y + 1, 0, 1) };
    case "month": return { since: Date.UTC(y, m, 1), until: Date.UTC(y, m + 1, 1) };
    case "week": { const since = utcMonday(now); return { since, until: since + 7 * DAY }; }
    default: return null;
  }
}

/** A date input's value for the UTC day `ms` falls on: "2026-11-24". */
export function dayInput(ms: number): string {
  return new Date(utcDay(ms)).toISOString().slice(0, 10);
}

/** Midnight UTC of a date input's value ("2026-11-24"), or null when it isn't a date. */
export function parseDayInput(s: string): number | null {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(s.trim());
  if (!m) return null;
  const ms = Date.UTC(Number(m[1]), Number(m[2]) - 1, Number(m[3]));
  return Number.isFinite(ms) && dayInput(ms) === s.trim() ? ms : null;
}

/** A custom period from its From and To days (both included), or why it can't be one: not both set, To before From, or
 *  longer than ten years. */
export function customRange(from: string, to: string): { range: TimeRange } | { problem: string } {
  const a = parseDayInput(from), b = parseDayInput(to);
  if (a == null || b == null) return { problem: "Pick both days" };
  if (b < a) return { problem: "To is before From" };
  if ((b - a) / DAY + 1 > MAX_RANGE_DAYS) return { problem: "At most ten years" };
  return { range: { since: a, until: b + DAY } };
}

const fmt = (o: Intl.DateTimeFormatOptions) => new Intl.DateTimeFormat("en-GB", { ...o, timeZone: "UTC" });
const DAY_MONTH = fmt({ day: "numeric", month: "short" });
const DAY_MONTH_YEAR = fmt({ day: "numeric", month: "short", year: "numeric" });
const MONTH_YEAR = fmt({ month: "short", year: "numeric" });
const year = (ms: number) => new Date(ms).getUTCFullYear();

/** The days of a period in words, from `since` up to (not including) `until`: "24 Nov 2026", "23 – 29 Nov 2026",
 *  "1 Jan – 31 Dec 2026", "29 Dec 2025 – 4 Jan 2026". */
export function rangeWords(since: number, until: number): string {
  const first = utcDay(since), last = utcDay(until - 1);
  if (last <= first) return DAY_MONTH_YEAR.format(first);
  const a = new Date(first), b = new Date(last);
  if (a.getUTCFullYear() !== b.getUTCFullYear()) return `${DAY_MONTH_YEAR.format(first)} – ${DAY_MONTH_YEAR.format(last)}`;
  if (a.getUTCMonth() === b.getUTCMonth()) return `${a.getUTCDate()} – ${DAY_MONTH_YEAR.format(last)}`;
  return `${DAY_MONTH.format(first)} – ${DAY_MONTH_YEAR.format(last)}`;
}

/** Under the chips: the period's days in words; for All, since the day of the first run, or that there is none yet. */
export function periodWords(range: TimeRange | null, firstRun: number | null, now: number): string {
  if (range) return rangeWords(range.since, range.until);
  return firstRun == null ? "All time: no runs yet" : `All time: ${rangeWords(firstRun, utcDay(Math.max(now, firstRun)) + DAY)}`;
}

/** What a bar of the charts covers: a day for up to 92 days, a week (from Monday) up to two years, else a month. */
export type BarUnit = "day" | "week" | "month";
export const barUnit = (days: number): BarUnit => (days <= 92 ? "day" : days <= 731 ? "week" : "month");

/** One bar: the days from `start` up to (not including) `end` added up, with `future` for a bar that starts after today. */
export type TimeBar = { start: number; end: number; runs: number; timeMs: number; inputTokens: number; outputTokens: number;
  costUsdMicros: number; unknownCostRuns: number; future: boolean };

/** The bars of the charts at `now`: the days grouped by `barUnit(days.length)`, oldest first, the time of the runs working now
 *  (`running`, each card's `runningSince`) in the bar they started in. */
export function timeBars(days: readonly TimeDay[], running: readonly number[], now: number): { unit: BarUnit; bars: TimeBar[] } {
  const unit = barUnit(days.length);
  const startOf = (ms: number) => (unit === "day" ? utcDay(ms) : unit === "week" ? utcMonday(ms) : utcMonthStart(ms));
  const endOf = (start: number) => {
    if (unit === "day") return start + DAY;
    if (unit === "week") return start + 7 * DAY;
    const d = new Date(start);
    return Date.UTC(d.getUTCFullYear(), d.getUTCMonth() + 1, 1);
  };
  const bars: TimeBar[] = [];
  for (const d of days) {
    const start = startOf(d.dayStart);
    let b = bars[bars.length - 1];
    if (!b || b.start !== start) {
      b = { start, end: endOf(start), runs: 0, timeMs: 0, inputTokens: 0, outputTokens: 0, costUsdMicros: 0, unknownCostRuns: 0,
        future: start > now };
      bars.push(b);
    }
    b.runs += d.runs;
    b.timeMs += d.timeMs;
    b.inputTokens += d.inputTokens;
    b.outputTokens += d.outputTokens;
    b.costUsdMicros += d.costUsdMicros;
    b.unknownCostRuns += d.unknownCostRuns;
  }
  for (const since of running) {
    const b = bars.find((x) => since >= x.start && since < x.end);
    if (b) b.timeMs += Math.max(0, now - since);
  }
  return { unit, bars };
}

/** A bar's name: "24 Nov" for a day, "Week of 23 Nov" for a week, "Nov 2026" for a month; with the year when the chart
 *  (`spansYears`) isn't in one year. */
export function barLabel(b: Pick<TimeBar, "start">, unit: BarUnit, years: boolean): string {
  if (unit === "month") return MONTH_YEAR.format(b.start);
  const day = (years ? DAY_MONTH_YEAR : DAY_MONTH).format(b.start);
  return unit === "week" ? `Week of ${day}` : day;
}

/** Under a chart: its first, middle and last bar's day, or the one or two it has. */
export function axisBars<T>(bars: readonly T[]): T[] {
  if (bars.length <= 2) return [...bars];
  return [bars[0], bars[Math.floor(bars.length / 2)], bars[bars.length - 1]];
}

/** On the axis: "24 Nov", "Nov 2026" for months; with the year when the chart isn't in one year. */
export function axisLabel(start: number, unit: BarUnit, years: boolean): string {
  return unit === "month" ? MONTH_YEAR.format(start) : (years ? DAY_MONTH_YEAR : DAY_MONTH).format(start);
}

/** Whether the bars span more than one calendar year. */
export const spansYears = (bars: readonly Pick<TimeBar, "start" | "end">[]) =>
  bars.length > 0 && year(bars[0].start) !== year(bars[bars.length - 1].end - 1);
