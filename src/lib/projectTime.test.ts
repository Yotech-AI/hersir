// GA-331, GA-354: a project's Time tracking tab: the agent time's format, a card's time with a run working on it, the sums,
// the period filter (All, This year, This month, This week, Custom; UTC days), the bars of the charts (a day, a week or a
// month each), and the contract between gizai-core's CardTime, TimeDay and ProjectTime, the command, the api, the reads
// that reload the tab and the browser fixtures.
import { describe, expect, it } from "vitest";
import {
  MAX_RANGE_DAYS, TIME_COST_NOTE, TIME_NOTE, TIME_PERIODS, WORKING_TITLE, agentTimeMs, axisBars, axisLabel, barLabel, barUnit, cardNumber, columnLabel,
  customRange, dayInput, formatAgentTime, parseDayInput, periodRange, periodWords, rangeWords, runsOnCards, spansYears, timeBars, timeTotals, utcDay,
  utcMonday, utcMonthStart,
} from "./projectTime";
import { READS } from "./tables";
import type { CardTime, ProjectTime, TimeDay } from "../types";
import usageRs from "../../crates/gizai-core/src/usage.rs?raw";
import commandsRs from "../../src-tauri/src/commands.rs?raw";
import tauriLib from "../../src-tauri/src/lib.rs?raw";
import apiTs from "../api.ts?raw";
import typesTs from "../types.ts?raw";
import mockTs from "../browser/mock.ts?raw";
import fixtures from "../browser/fixtures.json";

const S = 1_000;
const MIN = 60 * S;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
const NOW = Date.UTC(2026, 10, 23, 12, 0);
/** Monday 23 Nov 2026, midnight UTC. */
const TODAY = Date.UTC(2026, 10, 23);

const card = (o: Partial<CardTime>): CardTime => ({ taskId: "t", identifier: "KADE-1", title: "A", column: "To do", archived: false, runs: 0, timeMs: 0,
  runningSince: null, inputTokens: 0, outputTokens: 0, costUsdMicros: 0, unknownCostRuns: 0, ...o });
const day = (dayStart: number, o: Partial<TimeDay> = {}): TimeDay => ({ dayStart, runs: 0, timeMs: 0, inputTokens: 0, outputTokens: 0,
  costUsdMicros: 0, unknownCostRuns: 0, ...o });
/** `n` days up to and including `last`, each with one run of an hour, 10 tokens in and 1 out, and 1,000 micros. */
const days = (n: number, last = TODAY) => Array.from({ length: n }, (_, i) => day(last - (n - 1 - i) * DAY,
  { runs: 1, timeMs: HOUR, inputTokens: 10, outputTokens: 1, costUsdMicros: 1_000 }));

describe("the agent time's format", () => {
  it("is — for 0, <1m under a minute, minutes under an hour, else hours and two-digit minutes, with no days", () => {
    expect(formatAgentTime(0)).toBe("—");
    expect(formatAgentTime(30 * S)).toBe("<1m");
    expect(formatAgentTime(1)).toBe("<1m");
    expect(formatAgentTime(MIN - 1)).toBe("<1m");
    expect(formatAgentTime(MIN)).toBe("1m");
    expect(formatAgentTime(42 * MIN)).toBe("42m");
    expect(formatAgentTime(42 * MIN + 59 * S)).toBe("42m");
    expect(formatAgentTime(HOUR - 1)).toBe("59m");
    expect(formatAgentTime(HOUR)).toBe("1h 00m");
    expect(formatAgentTime(3 * HOUR + 5 * MIN)).toBe("3h 05m");
    expect(formatAgentTime(24 * HOUR)).toBe("24h 00m");
    expect(formatAgentTime(31 * HOUR + 10 * MIN)).toBe("31h 10m");
    expect(formatAgentTime(250 * HOUR + 59 * MIN + 59 * S)).toBe("250h 59m");
  });

  it("is — for a time below 0 or none at all", () => {
    expect(formatAgentTime(-5 * MIN)).toBe("—");
    expect(formatAgentTime(Number.NaN)).toBe("—");
  });
});

describe("a card's agent time", () => {
  it("is its ended runs' time, plus the time of the run working on it now", () => {
    expect(agentTimeMs(card({ timeMs: 30 * MIN }), NOW)).toBe(30 * MIN);
    expect(agentTimeMs(card({ timeMs: 30 * MIN, runningSince: NOW - 12 * MIN }), NOW)).toBe(42 * MIN);
    // a minute later it counted up
    expect(agentTimeMs(card({ timeMs: 30 * MIN, runningSince: NOW - 12 * MIN }), NOW + MIN)).toBe(43 * MIN);
    // a start just ahead of this computer's clock adds nothing, never less
    expect(agentTimeMs(card({ timeMs: 30 * MIN, runningSince: NOW + 5 * S }), NOW)).toBe(30 * MIN);
  });
});

describe("the card's number and column", () => {
  it("takes the number from the identifier, so KADE-10 is newer than KADE-9", () => {
    expect(cardNumber("KADE-10")).toBe(10);
    expect(cardNumber("GA-331")).toBe(331);
    expect(cardNumber("KADE-9")).toBeLessThan(cardNumber("KADE-10"));
    expect(cardNumber("no number")).toBe(0);
  });

  it("says the column's name, or Archived for an archived card", () => {
    expect(columnLabel(card({ column: "Testing" }))).toBe("Testing");
    expect(columnLabel(card({ column: "Done", archived: true }))).toBe("Archived");
  });
});


describe("the sums", () => {
  it("add up every row, the running runs' time and the API cost included; cards are the ones with a run", () => {
    const rows = [
      card({ runs: 2, timeMs: 30 * MIN, inputTokens: 1_000, outputTokens: 100, costUsdMicros: 20_000 }),
      card({ runs: 3, timeMs: 3 * HOUR, runningSince: NOW - 5 * MIN, inputTokens: 500, outputTokens: 50, costUsdMicros: 5_000 }),
      card({ runs: 1, timeMs: 31 * HOUR + 10 * MIN, archived: true, inputTokens: 7, outputTokens: 3, unknownCostRuns: 1 }),
      card({}),
    ];
    expect(timeTotals(rows, NOW)).toEqual({ cards: 3, runs: 6, inputTokens: 1_507, outputTokens: 153, timeMs: 34 * HOUR + 45 * MIN,
      costUsdMicros: 25_000, unknownCostRuns: 1 });
    expect(formatAgentTime(timeTotals(rows, NOW).timeMs)).toBe("34h 45m");
    // a minute later the running run counted up
    expect(timeTotals(rows, NOW + MIN).timeMs).toBe(34 * HOUR + 46 * MIN);
    expect(timeTotals([], NOW)).toEqual({ cards: 0, runs: 0, inputTokens: 0, outputTokens: 0, timeMs: 0, costUsdMicros: 0, unknownCostRuns: 0 });
  });

  it("say '<n> runs on <n> cards' under the Agent time, one run on one card", () => {
    expect(runsOnCards({ runs: 6, cards: 3 })).toBe("6 runs on 3 cards");
    expect(runsOnCards({ runs: 1, cards: 1 })).toBe("1 run on 1 card");
    expect(runsOnCards({ runs: 0, cards: 0 })).toBe("0 runs on 0 cards");
    expect(runsOnCards({ runs: 1_234, cards: 2 })).toBe("1,234 runs on 2 cards");
  });
});

describe("the texts, word for word", () => {
  it("has the card's note, the cost's sentence and the working dot's title", () => {
    expect(TIME_NOTE).toBe("Agent time: from each run's start to its end, added up over every run on the card (each start, Continue and nudge is a run). "
      + "Your own time isn't in it. Tokens: input (cache included) and output, as on the Usage page.");
    expect(TIME_COST_NOTE).toBe("API cost: an estimate at API prices, as on the Usage page.");
    expect(WORKING_TITLE).toBe("An agent is working on it now");
  });
});

describe("the period's chips (GA-354)", () => {
  it("are All, This year, This month, This week and Custom, in that order", () => {
    expect(TIME_PERIODS.map((p) => p.label)).toEqual(["All", "This year", "This month", "This week", "Custom"]);
    expect(TIME_PERIODS.map((p) => p.key)).toEqual(["all", "year", "month", "week", "custom"]);
  });

  it("All has no period", () => {
    expect(periodRange("all", NOW)).toBeNull();
  });

  it("This year, This month and This week are the whole UTC year, month and week, until the day after their last", () => {
    expect(periodRange("year", NOW)).toEqual({ since: Date.UTC(2026, 0, 1), until: Date.UTC(2027, 0, 1) });
    expect(periodRange("month", NOW)).toEqual({ since: Date.UTC(2026, 10, 1), until: Date.UTC(2026, 11, 1) });
    expect(periodRange("week", NOW)).toEqual({ since: TODAY, until: TODAY + 7 * DAY });
    // December's month ends with the year, February has its 28 days
    expect(periodRange("month", Date.UTC(2026, 11, 31, 23, 59))).toEqual({ since: Date.UTC(2026, 11, 1), until: Date.UTC(2027, 0, 1) });
    expect(periodRange("month", Date.UTC(2026, 1, 14))).toEqual({ since: Date.UTC(2026, 1, 1), until: Date.UTC(2026, 2, 1) });
  });

  it("This week runs Monday to Sunday in UTC, also over New Year", () => {
    // Sunday 29 Nov, its last ms: still the week of Monday the 23rd
    expect(periodRange("week", TODAY + 7 * DAY - 1)).toEqual({ since: TODAY, until: TODAY + 7 * DAY });
    // Monday 30 Nov at midnight: the next week
    expect(periodRange("week", TODAY + 7 * DAY)).toEqual({ since: TODAY + 7 * DAY, until: TODAY + 14 * DAY });
    // Friday 1 Jan 2027: the week from Monday 28 Dec 2026
    expect(periodRange("week", Date.UTC(2027, 0, 1, 9))).toEqual({ since: Date.UTC(2026, 11, 28), until: Date.UTC(2027, 0, 4) });
    // UTC, whatever the clock's time zone: 23:30 UTC on Sunday is still Sunday
    expect(utcMonday(Date.UTC(2026, 10, 29, 23, 30))).toBe(TODAY);
    expect(utcMonday(TODAY)).toBe(TODAY);
    expect(utcMonthStart(NOW)).toBe(Date.UTC(2026, 10, 1));
    expect(utcDay(NOW)).toBe(TODAY);
    expect(utcDay(-1)).toBe(-DAY);
  });

  it("Custom: From and To, both days included, as UTC midnights; until is the day after To", () => {
    expect(customRange("2026-11-01", "2026-11-10")).toEqual({ range: { since: Date.UTC(2026, 10, 1), until: Date.UTC(2026, 10, 11) } });
    // one day
    expect(customRange("2026-11-24", "2026-11-24")).toEqual({ range: { since: Date.UTC(2026, 10, 24), until: Date.UTC(2026, 10, 25) } });
  });

  it("Custom applies only when both days are set, To isn't before From, and it is at most ten years (gizai-core's limit)", () => {
    expect(customRange("", "")).toEqual({ problem: "Pick both days" });
    expect(customRange("2026-11-01", "")).toEqual({ problem: "Pick both days" });
    expect(customRange("", "2026-11-01")).toEqual({ problem: "Pick both days" });
    expect(customRange("2026-11-10", "2026-11-01")).toEqual({ problem: "To is before From" });
    const from = Date.UTC(2020, 0, 1);
    expect(customRange(dayInput(from), dayInput(from + (MAX_RANGE_DAYS - 1) * DAY))).toEqual({ range: { since: from, until: from + MAX_RANGE_DAYS * DAY } });
    expect(customRange(dayInput(from), dayInput(from + MAX_RANGE_DAYS * DAY))).toEqual({ problem: "At most ten years" });
    expect(MAX_RANGE_DAYS).toBe(3_660);
    expect(usageRs).toContain("pub const MAX_TIME_DAYS: i64 = 3_660;");
  });

  it("reads a date input's day, and no other text", () => {
    expect(parseDayInput("2026-11-24")).toBe(Date.UTC(2026, 10, 24));
    expect(parseDayInput(" 2026-11-24 ")).toBe(Date.UTC(2026, 10, 24));
    expect(parseDayInput("2026-02-30")).toBeNull();
    expect(parseDayInput("2026-13-01")).toBeNull();
    expect(parseDayInput("24-11-2026")).toBeNull();
    expect(parseDayInput("")).toBeNull();
    expect(dayInput(Date.UTC(2026, 10, 24, 23, 59))).toBe("2026-11-24");
    expect(dayInput(TODAY + 7 * DAY - 1)).toBe("2026-11-29");
  });

  it("says the period's days in words, as the Usage page does", () => {
    expect(rangeWords(TODAY, TODAY + DAY)).toBe("23 Nov 2026");
    expect(rangeWords(TODAY, TODAY + 7 * DAY)).toBe("23 – 29 Nov 2026");
    expect(rangeWords(Date.UTC(2026, 10, 30), Date.UTC(2026, 11, 3))).toBe("30 Nov – 2 Dec 2026");
    expect(rangeWords(Date.UTC(2026, 0, 1), Date.UTC(2027, 0, 1))).toBe("1 Jan – 31 Dec 2026");
    expect(rangeWords(Date.UTC(2025, 11, 29), Date.UTC(2026, 0, 5))).toBe("29 Dec 2025 – 4 Jan 2026");
  });

  it("says All's days from the first run's day to today, or that there is no run yet", () => {
    expect(periodWords(null, null, NOW)).toBe("All time: no runs yet");
    expect(periodWords(null, TODAY - 2 * DAY + HOUR, NOW)).toBe("All time: 21 – 23 Nov 2026");
    expect(periodWords(null, NOW - HOUR, NOW)).toBe("All time: 23 Nov 2026");
    expect(periodWords({ since: TODAY, until: TODAY + 7 * DAY }, TODAY - 400 * DAY, NOW)).toBe("23 – 29 Nov 2026");
  });
});

describe("the bars of the charts (GA-354)", () => {
  it("are a bar per day up to 92 days, per week up to two years, else per month", () => {
    expect(barUnit(1)).toBe("day");
    expect(barUnit(92)).toBe("day");
    expect(barUnit(93)).toBe("week");
    expect(barUnit(365)).toBe("week");
    expect(barUnit(731)).toBe("week");
    expect(barUnit(732)).toBe("month");
    expect(barUnit(3_660)).toBe("month");
  });

  it("a day's bar has that day's sums", () => {
    const { unit, bars } = timeBars([day(TODAY - DAY, { runs: 2, timeMs: 30 * MIN, inputTokens: 100, outputTokens: 10, costUsdMicros: 500, unknownCostRuns: 1 }),
      day(TODAY)], [], NOW);
    expect(unit).toBe("day");
    expect(bars).toEqual([
      { start: TODAY - DAY, end: TODAY, runs: 2, timeMs: 30 * MIN, inputTokens: 100, outputTokens: 10, costUsdMicros: 500, unknownCostRuns: 1, future: false },
      { start: TODAY, end: TODAY + DAY, runs: 0, timeMs: 0, inputTokens: 0, outputTokens: 0, costUsdMicros: 0, unknownCostRuns: 0, future: false },
    ]);
  });

  it("a run working now counts up in the bar of the day it started, and only there", () => {
    const d = [day(TODAY - DAY, { timeMs: HOUR }), day(TODAY, { timeMs: HOUR })];
    const { bars } = timeBars(d, [NOW - 5 * MIN], NOW);
    expect(bars.map((b) => b.timeMs)).toEqual([HOUR, HOUR + 5 * MIN]);
    expect(timeBars(d, [NOW - 5 * MIN], NOW + MIN).bars.map((b) => b.timeMs)).toEqual([HOUR, HOUR + 6 * MIN]);
    // one that started before the first day isn't on the chart
    expect(timeBars(d, [TODAY - 3 * DAY], NOW).bars.map((b) => b.timeMs)).toEqual([HOUR, HOUR]);
  });

  it("the days after today are 'future' (This week, This month and This year are whole)", () => {
    const week = Array.from({ length: 7 }, (_, i) => day(TODAY + i * DAY));
    expect(timeBars(week, [], NOW).bars.map((b) => b.future)).toEqual([false, true, true, true, true, true, true]);
  });

  it("weeks start on Monday, and a period that starts midweek has a short first week", () => {
    // 93 days up to Monday 23 Nov: Sunday 23 Aug alone, 13 whole weeks, then Monday 23 Nov alone
    const { unit, bars } = timeBars(days(93), [], NOW);
    expect(unit).toBe("week");
    expect(bars).toHaveLength(15);
    expect([bars[0].start, bars[0].end, bars[0].runs]).toEqual([Date.UTC(2026, 7, 23), Date.UTC(2026, 7, 24), 1]);
    expect([bars[1].start, bars[1].end, bars[1].runs, bars[1].timeMs]).toEqual([Date.UTC(2026, 7, 24), Date.UTC(2026, 7, 31), 7, 7 * HOUR]);
    expect(bars.slice(1, 14).every((b) => new Date(b.start).getUTCDay() === 1 && b.end - b.start === 7 * DAY)).toBe(true);
    expect([bars[14].start, bars[14].end, bars[14].runs]).toEqual([TODAY, TODAY + DAY, 1]);
    // every day is in one bar, once
    expect(bars.reduce((n, b) => n + b.runs, 0)).toBe(93);
    expect(bars.reduce((n, b) => n + b.costUsdMicros, 0)).toBe(93_000);
  });

  it("months start on the 1st, over more than two years", () => {
    const { unit, bars } = timeBars(days(732), [], NOW);
    expect(unit).toBe("month");
    // 22 Nov 2024 to 23 Nov 2026: Nov 2024 (from the 22nd) to Nov 2026 (to the 23rd), 25 months
    expect(bars).toHaveLength(25);
    expect([bars[0].start, bars[0].runs]).toEqual([Date.UTC(2024, 10, 22), 9]);
    expect([bars[1].start, bars[1].end, bars[1].runs]).toEqual([Date.UTC(2024, 11, 1), Date.UTC(2025, 0, 1), 31]);
    expect([bars[3].start, bars[3].runs]).toEqual([Date.UTC(2025, 1, 1), 28]);
    expect([bars[24].start, bars[24].end, bars[24].runs]).toEqual([Date.UTC(2026, 10, 1), TODAY + DAY, 23]);
    expect(bars.reduce((n, b) => n + b.runs, 0)).toBe(732);
  });

  it("no days, no bars", () => {
    expect(timeBars([], [NOW - MIN], NOW)).toEqual({ unit: "day", bars: [] });
  });

  it("name a bar in its tip: the day, the week's days, or the month; with the year when the chart spans years", () => {
    expect(barLabel({ start: TODAY, end: TODAY + DAY }, "day", false)).toBe("23 Nov");
    expect(barLabel({ start: TODAY, end: TODAY + DAY }, "day", true)).toBe("23 Nov 2026");
    expect(barLabel({ start: TODAY - 7 * DAY, end: TODAY }, "week", false)).toBe("16 – 22 Nov 2026");
    expect(barLabel({ start: Date.UTC(2026, 10, 1), end: Date.UTC(2026, 11, 1) }, "month", false)).toBe("Nov 2026");
    expect(axisLabel(TODAY, "day", false)).toBe("23 Nov");
    expect(axisLabel(TODAY, "week", true)).toBe("23 Nov 2026");
    expect(axisLabel(Date.UTC(2026, 10, 1), "month", false)).toBe("Nov 2026");
    expect(spansYears([{ start: TODAY - DAY, end: TODAY }, { start: TODAY, end: TODAY + DAY }])).toBe(false);
    expect(spansYears([{ start: Date.UTC(2025, 11, 31), end: Date.UTC(2026, 0, 1) }, { start: Date.UTC(2026, 0, 1), end: Date.UTC(2026, 0, 2) }])).toBe(true);
    // a whole year ends at the next year's midnight: still one year
    expect(spansYears([{ start: Date.UTC(2026, 0, 1), end: Date.UTC(2027, 0, 1) }])).toBe(false);
    expect(spansYears([])).toBe(false);
  });

  it("the axis has the first, middle and last bar, or the one or two there are", () => {
    expect(axisBars([])).toEqual([]);
    expect(axisBars([1])).toEqual([1]);
    expect(axisBars([1, 2])).toEqual([1, 2]);
    expect(axisBars([1, 2, 3, 4, 5])).toEqual([1, 3, 5]);
    expect(axisBars([1, 2, 3, 4])).toEqual([1, 3, 4]);
  });
});

describe("the contract (GA-331, GA-354)", () => {
  const camel = (s: string) => s.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());
  const rsFields = (name: string) => {
    expect(new RegExp(`#\\[serde\\(rename_all = "camelCase"\\)\\]\\s*pub struct ${name}\\b`).test(usageRs)).toBe(true);
    const rs = new RegExp(`pub struct ${name} \\{([\\s\\S]*?)\\n\\}`).exec(usageRs)?.[1] ?? "";
    return [...rs.matchAll(/pub (\w+):/g)].map((m) => camel(m[1]));
  };
  const tsType = (name: string) => new RegExp(`export type ${name} = \\{([\\s\\S]*?)\\};`).exec(typesTs)?.[1] ?? "";
  const tsFields = (name: string) => [...tsType(name).matchAll(/(\w+)\??:/g)].map((m) => m[1]);

  it("CardTime has the same fields in gizai-core and the UI, in camelCase, with the API cost (GA-354)", () => {
    expect(rsFields("CardTime")).toEqual(["taskId", "identifier", "title", "column", "archived", "runs", "timeMs", "runningSince", "inputTokens",
      "outputTokens", "costUsdMicros", "unknownCostRuns"]);
    expect(tsFields("CardTime")).toEqual(rsFields("CardTime"));
    expect(tsType("CardTime")).toContain("runningSince: number | null");
  });

  it("TimeDay and ProjectTime have the same fields in gizai-core and the UI, in camelCase", () => {
    expect(rsFields("TimeDay")).toEqual(["dayStart", "runs", "timeMs", "inputTokens", "outputTokens", "costUsdMicros", "unknownCostRuns"]);
    expect(tsFields("TimeDay")).toEqual(rsFields("TimeDay"));
    expect(rsFields("ProjectTime")).toEqual(["since", "until", "firstRun", "cards", "days"]);
    expect(tsFields("ProjectTime")).toEqual(rsFields("ProjectTime"));
    expect(tsType("ProjectTime")).toContain("since: number | null; until: number | null; firstRun: number | null; cards: CardTime[]; days: TimeDay[]");
  });

  it("the command takes an optional period, reads the run time limit as the Settings do, in ms, and answers a ProjectTime", () => {
    // the Chief's tools keep the all-time list (GA-332)
    expect(usageRs).toContain("pub fn project_time(db: &Db, project_id: &str, max_run_ms: i64) -> Result<Vec<CardTime>>");
    expect(usageRs).toContain("pub fn project_time_period(db: &Db, project_id: &str, max_run_ms: i64, since: Option<i64>, until: Option<i64>) -> Result<ProjectTime>");
    expect(tauriLib).toMatch(/commands::usage_summary, commands::project_time\b/);
    const cmd = commandsRs.slice(commandsRs.indexOf("pub async fn project_time"), commandsRs.indexOf("pub async fn subscription_limits"));
    expect(cmd).toContain("pub async fn project_time(st: State<'_, AppState>, id: String, since: Option<i64>, until: Option<i64>) -> R<gizai_core::usage::ProjectTime>");
    expect(cmd).toContain('settings::get(&st.db, "max_run_minutes").ok().flatten().unwrap_or(crate::runs::DEFAULT_MAX_RUN_MINUTES)');
    expect(cmd).toContain("60_000");
    expect(cmd).toContain("gizai_core::usage::project_time_period(&st.db, &id, max_run_ms, since, until)");
  });

  it("the api calls it with the project's id and the period, and a run's start and end reload the tab", () => {
    expect(apiTs).toContain('export const projectTime = (id: string, since?: number, until?: number) => invoke<T.ProjectTime>("project_time", { id, since, until });');
    expect(READS.project_time).toEqual(["runs", "tasks", "workflow_states"]);
  });

  it("the browser fixtures answer a ProjectTime per project, and the mock keeps a running card's start as recent as the other times", () => {
    const pt = (fixtures.commands as unknown as Record<string, { $by: string; $answers: Record<string, ProjectTime> }>).project_time;
    expect(pt.$by).toBe("id");
    const answers = Object.values(pt.$answers);
    expect(answers.length).toBeGreaterThan(0);
    for (const a of answers) {
      expect(Object.keys(a).sort()).toEqual(["cards", "days", "firstRun", "since", "until"]);
      expect([a.since, a.until]).toEqual([null, null]);
      for (const r of a.cards) expect(Object.keys(r).sort()).toEqual(Object.keys(card({})).sort());
      for (const d of a.days) expect(Object.keys(d).sort()).toEqual(Object.keys(day(0)).sort());
    }
    expect(mockTs).toContain('k === "runningSince"');
  });
});
