// GA-331, GA-354: a project's Time tracking tab, rendered to HTML on the server. useData hands over the tab's data at once,
// the clock says `state.now`, and the list draws every row (the virtualizer is a stand-in that has them all in sight).
// DataTable is the real one, wrapped so a test can see what the tab gives it (the row click, J/K and Enter, the first sort,
// the columns). GA-354: the data is a ProjectTime (cards and UTC days) for the period chosen; the totals are stat cards at
// the top, with two charts under them, and the footer has only the note.
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { CardTime, ProjectTime as ProjectTimeData, TimeDay } from "../types";
import type { Col } from "./DataTable";
import type { TimePeriod } from "./TimePeriod";

const NOW = Date.UTC(2026, 10, 23, 12, 0);
const MIN = 60_000;
const HOUR = 60 * MIN;
const DAY = 24 * HOUR;
/** Monday 23 Nov 2026, midnight UTC. */
const TODAY = Date.UTC(2026, 10, 23);

const state = vi.hoisted(() => ({
  data: null as unknown, error: null as string | null, asked: [] as unknown[][], clock: [] as number[], went: [] as unknown[],
  table: null as null | Record<string, unknown>, now: 0,
}));
vi.mock("../lib/useData", () => ({
  useData: (load: () => unknown) => {
    const got = load();
    if (got instanceof Promise) got.catch(() => {});
    return { data: state.data, error: state.error, reload: () => {}, setData: () => {} };
  },
}));
vi.mock("../api", async (orig) => ({ ...(await orig<typeof import("../api")>()),
  projectTime: (...args: unknown[]) => { state.asked.push(args); return Promise.resolve(null); } }));
vi.mock("../lib/windowFocus", async (orig) => ({ ...(await orig<typeof import("../lib/windowFocus")>()),
  useClock: (every: number) => { state.clock.push(every); return state.now; } }));
vi.mock("../router", async (orig) => ({ ...(await orig<typeof import("../router")>()), go: (r: unknown) => { state.went.push(r); } }));
vi.mock("@tanstack/react-virtual", () => ({
  useVirtualizer: ({ count }: { count: number }) => ({
    getVirtualItems: () => Array.from({ length: count }, (_, i) => ({ index: i, key: i, start: i * 34, end: (i + 1) * 34, size: 34, lane: 0 })),
    getTotalSize: () => count * 34, scrollToIndex: () => {},
  }),
}));
vi.mock("./DataTable", async (orig) => {
  const m = await orig<typeof import("./DataTable")>();
  return { ...m, DataTable: (p: Parameters<typeof m.DataTable>[0]) => { state.table = p as unknown as Record<string, unknown>; return m.DataTable(p); } };
});

const { ProjectTime } = await import("./ProjectTime");
const { formatTokenCount, unknownCostNote } = await import("../lib/usage");
/** unknownCostNote as it is in an HTML attribute. */
const noteAttr = (n: number) => unknownCostNote(n)!.replace(/'/g, "&#x27;");
const { TIME_NOTE, TIME_COST_NOTE } = await import("../lib/projectTime");

const card = (o: Partial<CardTime>): CardTime => ({ taskId: "t", identifier: "KADE-1", title: "A", column: "To do", archived: false, runs: 0, timeMs: 0,
  runningSince: null, inputTokens: 0, outputTokens: 0, costUsdMicros: 0, unknownCostRuns: 0, ...o });
const day = (dayStart: number, o: Partial<TimeDay> = {}): TimeDay => ({ dayStart, runs: 0, timeMs: 0, inputTokens: 0, outputTokens: 0,
  costUsdMicros: 0, unknownCostRuns: 0, ...o });

// out of order on purpose: KADE-9 before KADE-10, so a sort by text would be wrong. KADE-2 ran on Codex: tokens, no cost.
const ROWS: CardTime[] = [
  card({ taskId: "t9", identifier: "KADE-9", title: "Working now", column: "Testing", runs: 3, timeMs: 3 * HOUR, runningSince: NOW - 5 * MIN,
    inputTokens: 500, outputTokens: 50, costUsdMicros: 250_000 }),
  card({ taskId: "t1", identifier: "KADE-1", title: "No runs", column: "Backlog" }),
  card({ taskId: "t10", identifier: "KADE-10", title: "Export invoices", column: "In progress", runs: 2, timeMs: 30 * MIN,
    inputTokens: 12_000, outputTokens: 345, costUsdMicros: 1_500_000 }),
  card({ taskId: "t2", identifier: "KADE-2", title: "Archived one", column: "Done", archived: true, runs: 1, timeMs: 31 * HOUR + 10 * MIN,
    inputTokens: 1_200_000, outputTokens: 34_567, unknownCostRuns: 1 }),
];
// the same runs per UTC day: KADE-2's on Saturday, KADE-10's on Sunday, KADE-9's today (its running run started today)
const DAYS: TimeDay[] = [
  day(TODAY - 2 * DAY, { runs: 1, timeMs: 31 * HOUR + 10 * MIN, inputTokens: 1_200_000, outputTokens: 34_567, unknownCostRuns: 1 }),
  day(TODAY - DAY, { runs: 2, timeMs: 30 * MIN, inputTokens: 12_000, outputTokens: 345, costUsdMicros: 1_500_000 }),
  day(TODAY, { runs: 3, timeMs: 3 * HOUR, inputTokens: 500, outputTokens: 50, costUsdMicros: 250_000 }),
];
const FIRST_RUN = TODAY - 2 * DAY + HOUR;
const all = (cards: CardTime[], days: TimeDay[] = DAYS, firstRun: number | null = FIRST_RUN): ProjectTimeData =>
  ({ since: null, until: null, firstRun, cards, days });
const WEEK = { since: TODAY, until: TODAY + 7 * DAY };
const inWeek = (cards: CardTime[], days: TimeDay[]): ProjectTimeData => ({ since: WEEK.since, until: WEEK.until, firstRun: FIRST_RUN, cards, days });
const weekDays = (today: Partial<TimeDay> = {}) => Array.from({ length: 7 }, (_, i) => day(TODAY + i * DAY, i === 0 ? today : {}));
const period = (o: Partial<TimePeriod>): TimePeriod => ({ key: "all", range: null, from: "", to: "", problem: null, choose: () => {}, setDays: () => {}, ...o });

const render = (data: unknown, error: string | null = null, p?: TimePeriod) => {
  Object.assign(state, { data, error, table: null });
  return renderToStaticMarkup(<ProjectTime projectId="p-kade" period={p} />);
};
const text = (html: string) => html.replace(/<[^>]+>/g, "").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&");
/** The rows of the table's body, cell by cell, as HTML. */
const cells = (html: string) => {
  const body = html.slice(html.indexOf("<tbody>"), html.indexOf("</tbody>"));
  return body.split("<tr").slice(1).map((tr) => tr.split("<td").slice(1).map((td) => td.slice(td.indexOf(">") + 1, td.lastIndexOf("</td>"))));
};
const cols = () => state.table!.columns as Col<CardTime>[];
/** The stat cards at the top, each as its text. */
const stats = (html: string) => {
  const box = html.slice(html.indexOf('<div class="stats time-stats">'), html.indexOf('<div class="usage-charts time-charts">') >= 0
    ? html.indexOf('<div class="usage-charts time-charts">') : html.indexOf("<table"));
  return box.split('<div class="stat-card">').slice(1);
};
/** The charts, each as its HTML. */
const charts = (html: string) => {
  const i = html.indexOf('<div class="usage-charts time-charts">');
  if (i < 0) return [];
  return html.slice(i, html.indexOf('<div class="tablewrap', i) >= 0 ? html.indexOf('<div class="tablewrap', i) : undefined).split('<div class="stat-card">').slice(1);
};
/** The tips of a chart's bars. */
const tips = (chart: string) => [...chart.matchAll(/<div class="day" title="([^"]*)"/g)].map((m) => text(m[1]));

beforeEach(() => { state.asked = []; state.clock = []; state.went = []; state.now = NOW; });

describe("the Time tracking tab", () => {
  it("asks for the project's time for all time, without a period, and counts up every minute", () => {
    render(all(ROWS));
    expect(state.asked).toEqual([["p-kade", undefined, undefined]]);
    expect(state.clock).toEqual([60_000]);
  });

  it("has the columns Card, Column, Runs, Tokens, API cost and Agent time, each sortable, the numbers on the right", () => {
    const html = render(all(ROWS));
    const ths = [...html.matchAll(/<th[^>]*>([\s\S]*?)<\/th>/g)];
    expect(ths.map((m) => text(m[1]).replace(/[↑↓]/, ""))).toEqual(["Card", "Column", "Runs", "Tokens", "API cost", "Agent time"]);
    for (const m of ths) expect(m[0]).toContain("cursor:pointer");
    expect(ths.slice(2).every((m) => /class="[^"]*num/.test(m[0]))).toBe(true);
    expect(cols().every((c) => typeof c.sort === "function")).toBe(true);
  });

  it("lists the newest card first by its number, and says each card's column, runs, tokens, API cost and agent time", () => {
    const rows = cells(render(all(ROWS)));
    expect(rows.map((r) => text(r[0]))).toEqual(["KADE-10 Export invoices", "KADE-9 Working now", "KADE-2 Archived one", "KADE-1 No runs"]);
    expect(rows.map((r) => r.slice(1).map(text))).toEqual([
      ["In progress", "2", formatTokenCount(12_345), "$1.50", "30m"],
      ["Testing", "3", formatTokenCount(550), "$0.25", "3h 05m"],
      ["Archived", "1", formatTokenCount(1_234_567), "Unknown", "31h 10m"],
      ["Backlog", "—", "—", "—", "—"],
    ]);
    // the Card column: the identifier in the id style, then the title; the whole on hover
    expect(rows[0][0]).toBe('<span title="KADE-10 Export invoices"><span class="id">KADE-10</span> Export invoices</span>');
    expect(state.table!.initialSort).toEqual([{ id: "card", desc: true }]);
  });

  it("shows a faint — for a card without runs, tokens, cost or time", () => {
    const none = cells(render(all(ROWS)))[3];
    expect(none.slice(2)).toEqual(Array(4).fill('<span class="faint">—</span>').map((d, i) => (i === 3 ? `<span class="card-time">${d}</span>` : d)));
  });

  it("says the input and output tokens in full on hover", () => {
    const rows = cells(render(all(ROWS)));
    expect(rows[0][3]).toContain('title="12,000 input (cache included) · 345 output tokens"');
    expect(rows[2][3]).toContain('title="1,200,000 input (cache included) · 34,567 output tokens"');
  });

  it("says a card's API cost as the Usage page does, with why it is unknown on hover", () => {
    const rows = cells(render(all([...ROWS,
      card({ taskId: "t3", identifier: "KADE-3", title: "Both", runs: 2, inputTokens: 10, outputTokens: 1, costUsdMicros: 40_000, unknownCostRuns: 1 })])));
    expect(text(rows[2][4])).toBe("$0.04 + unknown");
    expect(rows[2][4]).toContain('class="usage-unknown"');
    expect(rows[3][4]).toBe(`<span class="usage-unknown" title="${noteAttr(1)}">Unknown</span>`);
    // a known cost has no note
    expect(rows[0][4]).toBe("<span>$1.50</span>");
  });

  it("puts a teal dot before the time of a card an agent works on now, in the list only there", () => {
    const html = render(all(ROWS));
    const rows = cells(html);
    expect(rows[1][5]).toContain('<span class="pulse" title="An agent is working on it now" aria-label="An agent is working on it now" role="img">');
    const list = html.slice(html.indexOf("<tbody>"), html.indexOf("</tbody>"));
    expect(list.match(/class="pulse"/g)).toHaveLength(1);
  });

  it("says Archived for an archived card", () => {
    expect(cells(render(all(ROWS)))[2][1]).toContain(">Archived<");
  });

  it("opens the card on a click or Enter, and moves with J and K", () => {
    render(all(ROWS));
    expect(state.table!.keyboardNav).toBe(true);
    (state.table!.onRowClick as (c: CardTime) => void)(ROWS[0]);
    expect(state.went).toEqual([{ page: "task", id: "t9" }]);
    expect((state.table!.rowId as (c: CardTime) => string)(ROWS[0])).toBe("t9");
  });

  it("sorts every column by what it shows: the number, the column (Archived for archived ones), runs, all tokens, the cost and the time with the running run", () => {
    render(all(ROWS));
    const by = (key: string) => {
      const c = cols().find((x) => x.key === key)!;
      return [...ROWS].sort((a, b) => {
        const x = c.sort!(a), y = c.sort!(b);
        return typeof x === "number" && typeof y === "number" ? x - y : String(x).localeCompare(String(y));
      }).map((r) => r.identifier);
    };
    expect(by("card")).toEqual(["KADE-1", "KADE-2", "KADE-9", "KADE-10"]);
    expect(by("column")).toEqual(["KADE-2", "KADE-1", "KADE-10", "KADE-9"]);
    expect(by("runs")).toEqual(["KADE-1", "KADE-2", "KADE-10", "KADE-9"]);
    expect(by("tokens")).toEqual(["KADE-1", "KADE-9", "KADE-10", "KADE-2"]);
    expect(by("cost")).toEqual(["KADE-1", "KADE-2", "KADE-9", "KADE-10"]);
    expect(by("time")).toEqual(["KADE-1", "KADE-10", "KADE-9", "KADE-2"]);
    // the running run's 5 minutes count in its sort: 3h 05m, not 3h
    expect(cols().find((x) => x.key === "time")!.sort!(ROWS[0])).toBe(3 * HOUR + 5 * MIN);
  });

  it("has only the note in the footer, word for word, with the sentence for the cost; the totals are at the top", () => {
    const html = render(all(ROWS));
    const foot = text(html.slice(html.indexOf('<div class="tablefoot">')));
    expect(foot).toBe(`${TIME_NOTE} ${TIME_COST_NOTE}`);
    expect(TIME_COST_NOTE).toBe("API cost: an estimate at API prices, as on the Usage page.");
    expect(foot).not.toContain("agent time ·");
    expect(foot).not.toMatch(/\d+ cards? ·/);
  });

  it("says No cards yet. for a project without cards, with no stat of runs and no charts", () => {
    const html = render(all([], [], null));
    expect(text(html)).toContain("No cards yet.");
    expect(text(html)).not.toContain("No runs in this period.");
    expect(html).not.toContain("<table");
    expect(charts(html)).toEqual([]);
    expect(text(html)).toContain("All time: no runs yet");
    expect(text(stats(html)[0])).toContain("0 runs on 0 cards");
  });

  it("shows nothing while it loads, and the error when it fails", () => {
    const loading = render(null);
    expect(text(loading)).toBe("");
    expect(loading).not.toContain("No cards yet.");
    expect(render(null, "not found: project nope")).toContain('<div class="error-banner">not found: project nope</div>');
  });
});

describe("the totals at the top (GA-354)", () => {
  it("are three stat cards, Agent time, Tokens and API cost, as the Usage page's", () => {
    const html = render(all(ROWS));
    expect(html).toContain('<div class="stats time-stats">');
    const s = stats(html);
    expect(s.map((c) => /<h4>([^<]*)<\/h4>/.exec(c)?.[1])).toEqual(["Agent time", "Tokens", "API cost"]);
    // they come before the list
    expect(html.indexOf('<div class="stats time-stats">')).toBeLessThan(html.indexOf("<table"));
  });

  it("Agent time: the total with the running run's time, and its runs on the cards that had one", () => {
    const s = stats(render(all(ROWS)));
    expect(text(s[0])).toBe("Agent time34h 45m6 runs on 3 cards");
    expect(s[0]).toContain('class="pulse"');
  });

  it("Agent time counts up with the clock while a run works, and has no dot without one", () => {
    state.now = NOW + 10 * MIN;
    expect(text(stats(render(all(ROWS)))[0])).toContain("34h 55m");
    const done = stats(render(all([ROWS[2]])));
    expect(text(done[0])).toBe("Agent time30m2 runs on 1 card");
    expect(done[0]).not.toContain("pulse");
  });

  it("Tokens: input plus output, with the split under it and in full on hover", () => {
    const s = stats(render(all(ROWS)));
    const total = 12_345 + 550 + 1_234_567;
    expect(text(s[1])).toBe(`Tokens${formatTokenCount(total)}${formatTokenCount(1_212_500)} input (incl. cache) · ${formatTokenCount(34_962)} output`);
    expect(s[1]).toContain('title="1,212,500 input (cache included) · 34,962 output tokens"');
  });

  it("API cost: the known part, its unknown part with why on hover, and that it is an estimate", () => {
    const s = stats(render(all(ROWS)));
    expect(text(s[2])).toBe("API cost$1.75+ an unknown cost for 1 runAn estimate at API prices, not a bill");
    expect(s[2]).toContain(`title="${noteAttr(1)}"`);
    expect(s[2]).toContain('class="sub usage-unknown"');
    // all known: no unknown line, no note
    const known = stats(render(all([ROWS[0], ROWS[2]])))[2];
    expect(text(known)).toBe("API cost$1.75An estimate at API prices, not a bill");
    expect(known).not.toContain("usage-unknown");
    // only a Codex run: the cost is Unknown
    expect(text(stats(render(all([ROWS[3]])))[2])).toBe("API costUnknownAn estimate at API prices, not a bill");
  });
});

describe("the charts (GA-354)", () => {
  it("are Agent time and Tokens per day under the totals, a bar per day", () => {
    const html = render(all(ROWS));
    const c = charts(html);
    expect(c.map((x) => /<h4>([^<]*)<\/h4>/.exec(x)?.[1])).toEqual(["Agent time per day", "Tokens per day"]);
    expect(html.indexOf('<div class="stats time-stats">')).toBeLessThan(html.indexOf('<div class="usage-charts time-charts">'));
    expect(html.indexOf('<div class="usage-charts time-charts">')).toBeLessThan(html.indexOf("<table"));
    expect(tips(c[0])).toHaveLength(3);
    expect(tips(c[1])).toHaveLength(3);
  });

  it("each agent time bar's tip says the day, the time (a running run's included), the runs and the cost", () => {
    const c = charts(render(all(ROWS)));
    expect(tips(c[0])).toEqual([
      "21 Nov: 31h 10m · 1 run · Unknown",
      "22 Nov: 30m · 2 runs · $1.50",
      "23 Nov: 3h 05m · 3 runs · $0.25",
    ]);
  });

  it("each tokens bar's tip says the day, the input and output tokens and the cost; input and output are stacked", () => {
    const c = charts(render(all(ROWS)));
    expect(tips(c[1])).toEqual([
      `21 Nov: ${formatTokenCount(1_200_000)} input, ${formatTokenCount(34_567)} output tokens · Unknown`,
      `22 Nov: ${formatTokenCount(12_000)} input, ${formatTokenCount(345)} output tokens · $1.50`,
      `23 Nov: ${formatTokenCount(500)} input, ${formatTokenCount(50)} output tokens · $0.25`,
    ]);
    expect(c[1]).toContain('class="seg-in"');
    expect(c[1]).toContain('class="seg-out"');
    expect(text(c[1])).toContain("Input (incl. cache)");
    expect(text(c[1])).toContain("Output");
  });

  it("a day without runs is a low empty bar", () => {
    const c = charts(render(all(ROWS, [day(TODAY - DAY), ...DAYS.slice(2)])));
    expect(c[0]).toContain('<div class="day" title="22 Nov: 0m · 0 runs · $0.00"><div class="seg-none" style="height:3%"></div></div>');
  });

  it("a period over 92 days has a bar per week, and one over two years a bar per month, the title saying so", () => {
    const days = (n: number) => Array.from({ length: n }, (_, i) => day(TODAY - (n - 1 - i) * DAY, { runs: 1, timeMs: HOUR }));
    const titles = (n: number) => charts(render(all(ROWS, days(n)))).map((x) => /<h4>([^<]*)<\/h4>/.exec(x)?.[1]);
    expect(titles(92)).toEqual(["Agent time per day", "Tokens per day"]);
    expect(titles(93)).toEqual(["Agent time per week", "Tokens per week"]);
    expect(titles(731)).toEqual(["Agent time per week", "Tokens per week"]);
    expect(titles(732)).toEqual(["Agent time per month", "Tokens per month"]);
    // a week's tip says its days
    const week = tips(charts(render(all(ROWS, days(93))))[0]);
    expect(week[week.length - 1]).toMatch(/^23 Nov 2026: /);
    expect(week[week.length - 2]).toMatch(/^16 – 22 Nov 2026: 7h 00m · 7 runs/);
    const month = tips(charts(render(all(ROWS, days(732))))[0]);
    expect(month[month.length - 1]).toMatch(/^Nov 2026: /);
  });
});

describe("the period (GA-354)", () => {
  it("asks for the period chosen, its since and until", () => {
    render(inWeek([ROWS[0]], weekDays()), null, period({ key: "week", range: WEEK }));
    expect(state.asked).toEqual([["p-kade", WEEK.since, WEEK.until]]);
  });

  it("says the period's days in words under the chips, in UTC days", () => {
    const week = render(inWeek([ROWS[0]], weekDays()), null, period({ key: "week", range: WEEK }));
    expect(text(week)).toContain("23 – 29 Nov 2026");
    expect(text(week)).toContain("UTC days");
    expect(text(render(all(ROWS)))).toContain("All time: 21 – 23 Nov 2026");
  });

  it("the totals, the charts and the list follow the period's data", () => {
    const html = render(inWeek([ROWS[0]], weekDays({ runs: 3, timeMs: 3 * HOUR, inputTokens: 500, outputTokens: 50, costUsdMicros: 250_000 })), null,
      period({ key: "week", range: WEEK }));
    expect(cells(html).map((r) => text(r[0]))).toEqual(["KADE-9 Working now"]);
    const s = stats(html);
    expect(text(s[0])).toBe("Agent time3h 05m3 runs on 1 card");
    expect(text(s[2])).toBe("API cost$0.25An estimate at API prices, not a bill");
    const c = charts(html);
    expect(tips(c[0])[0]).toBe("23 Nov: 3h 05m · 3 runs · $0.25");
    // the days after today: empty, not yet
    expect(tips(c[0]).slice(1)).toEqual(["24 Nov", "25 Nov", "26 Nov", "27 Nov", "28 Nov", "29 Nov"].map((d) => `${d}: not yet`));
    // the Runs and Agent time columns say it's the period's
    expect(cols().find((x) => x.key === "runs")!.title).toContain("in the period");
    expect(cols().find((x) => x.key === "time")!.title).toContain("in the period");
  });

  it("says No runs in this period. when no run started in it, with the totals at 0", () => {
    const html = render(inWeek([], weekDays()), null, period({ key: "week", range: WEEK }));
    expect(text(html)).toContain("No runs in this period.");
    expect(text(html)).not.toContain("No cards yet.");
    expect(html).not.toContain("<table");
    const s = stats(html);
    expect(text(s[0])).toBe("Agent time0m0 runs on 0 cards");
    expect(text(s[2])).toBe("API cost$0.00An estimate at API prices, not a bill");
    expect(charts(html)).toHaveLength(2);
  });

  it("Custom shows its From and To days, and why they aren't a period yet", () => {
    const custom = period({ key: "custom", range: WEEK, from: "2026-11-23", to: "2026-11-29" });
    const html = render(inWeek([], weekDays()), null, custom);
    expect(html).toContain('type="date"');
    expect(html).toContain('value="2026-11-23"');
    expect(html).toContain('value="2026-11-29"');
    expect(text(html)).toContain("From");
    expect(text(html)).toContain("To");
    const bad = render(inWeek([], weekDays()), null, period({ key: "custom", range: WEEK, from: "2026-11-29", to: "2026-11-23", problem: "To is before From" }));
    expect(bad).toContain('<span class="time-problem">To is before From</span>');
    expect(bad).toContain('aria-invalid="true"');
    // not on the other chips
    expect(render(all(ROWS))).not.toContain('type="date"');
  });
});
