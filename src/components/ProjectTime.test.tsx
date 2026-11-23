// GA-331: a project's Time tracking tab, rendered to HTML on the server. useData hands over the rows at once, the clock
// says NOW, and the list draws every row (the virtualizer is a stand-in that has them all in sight). DataTable is the
// real one, wrapped so a test can see what the tab gives it (the row click, J/K and Enter, the first sort, the columns).
import { beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { CardTime } from "../types";
import type { Col } from "./DataTable";

const NOW = Date.UTC(2026, 10, 23, 12, 0);
const MIN = 60_000;
const HOUR = 60 * MIN;

const state = vi.hoisted(() => ({
  data: null as unknown, error: null as string | null, asked: [] as string[], clock: [] as number[], went: [] as unknown[],
  table: null as null | Record<string, unknown>,
}));
vi.mock("../lib/useData", () => ({
  useData: (load: () => unknown) => {
    const got = load();
    if (got instanceof Promise) got.catch(() => {});
    return { data: state.data, error: state.error, reload: () => {}, setData: () => {} };
  },
}));
vi.mock("../api", async (orig) => ({ ...(await orig<typeof import("../api")>()),
  projectTime: (id: string) => { state.asked.push(id); return Promise.resolve([]); } }));
vi.mock("../lib/windowFocus", async (orig) => ({ ...(await orig<typeof import("../lib/windowFocus")>()),
  useClock: (every: number) => { state.clock.push(every); return NOW; } }));
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
const { formatTokenCount } = await import("../lib/usage");

const card = (o: Partial<CardTime>): CardTime => ({ taskId: "t", identifier: "KADE-1", title: "A", column: "To do", archived: false, runs: 0, timeMs: 0,
  runningSince: null, inputTokens: 0, outputTokens: 0, ...o });

// out of order on purpose: KADE-9 before KADE-10, so a sort by text would be wrong
const ROWS: CardTime[] = [
  card({ taskId: "t9", identifier: "KADE-9", title: "Working now", column: "Testing", runs: 3, timeMs: 3 * HOUR, runningSince: NOW - 5 * MIN,
    inputTokens: 500, outputTokens: 50 }),
  card({ taskId: "t1", identifier: "KADE-1", title: "No runs", column: "Backlog" }),
  card({ taskId: "t10", identifier: "KADE-10", title: "Export invoices", column: "In progress", runs: 2, timeMs: 30 * MIN,
    inputTokens: 12_000, outputTokens: 345 }),
  card({ taskId: "t2", identifier: "KADE-2", title: "Archived one", column: "Done", archived: true, runs: 1, timeMs: 31 * HOUR + 10 * MIN,
    inputTokens: 1_200_000, outputTokens: 34_567 }),
];

const render = (data: unknown, error: string | null = null) => {
  Object.assign(state, { data, error, table: null });
  return renderToStaticMarkup(<ProjectTime projectId="p-kade" />);
};
const text = (html: string) => html.replace(/<[^>]+>/g, "").replace(/&#x27;/g, "'").replace(/&quot;/g, '"').replace(/&amp;/g, "&");
/** The rows of the table's body, cell by cell, as HTML. */
const cells = (html: string) => {
  const body = html.slice(html.indexOf("<tbody>"), html.indexOf("</tbody>"));
  return body.split("<tr").slice(1).map((tr) => tr.split("<td").slice(1).map((td) => td.slice(td.indexOf(">") + 1, td.lastIndexOf("</td>"))));
};
const cols = () => state.table!.columns as Col<CardTime>[];

beforeEach(() => { state.asked = []; state.clock = []; state.went = []; });

describe("the Time tracking tab", () => {
  it("asks for the project's cards and counts up every minute", () => {
    render(ROWS);
    expect(state.asked).toEqual(["p-kade"]);
    expect(state.clock).toEqual([60_000]);
  });

  it("has the columns Card, Column, Runs, Tokens and Agent time, each sortable, the numbers on the right", () => {
    const html = render(ROWS);
    const ths = [...html.matchAll(/<th[^>]*>([\s\S]*?)<\/th>/g)];
    expect(ths.map((m) => text(m[1]).replace(/[↑↓]/, ""))).toEqual(["Card", "Column", "Runs", "Tokens", "Agent time"]);
    for (const m of ths) expect(m[0]).toContain("cursor:pointer");
    expect(ths.slice(2).every((m) => /class="[^"]*num/.test(m[0]))).toBe(true);
    expect(cols().every((c) => typeof c.sort === "function")).toBe(true);
  });

  it("lists the newest card first by its number, and says each card's column, runs, tokens and agent time", () => {
    const rows = cells(render(ROWS));
    expect(rows.map((r) => text(r[0]))).toEqual(["KADE-10 Export invoices", "KADE-9 Working now", "KADE-2 Archived one", "KADE-1 No runs"]);
    expect(rows.map((r) => r.slice(1).map(text))).toEqual([
      ["In progress", "2", formatTokenCount(12_345), "30m"],
      ["Testing", "3", formatTokenCount(550), "3h 05m"],
      ["Archived", "1", formatTokenCount(1_234_567), "31h 10m"],
      ["Backlog", "—", "—", "—"],
    ]);
    // the Card column: the identifier in the id style, then the title; the whole on hover
    expect(rows[0][0]).toBe('<span title="KADE-10 Export invoices"><span class="id">KADE-10</span> Export invoices</span>');
    expect(state.table!.initialSort).toEqual([{ id: "card", desc: true }]);
  });

  it("shows a faint — for a card without runs, tokens or time", () => {
    const none = cells(render(ROWS))[3];
    expect(none.slice(2)).toEqual(Array(3).fill('<span class="faint">—</span>').map((d, i) => (i === 2 ? `<span class="card-time">${d}</span>` : d)));
  });

  it("says the input and output tokens in full on hover", () => {
    const rows = cells(render(ROWS));
    expect(rows[0][3]).toContain('title="12,000 input (cache included) · 345 output tokens"');
    expect(rows[2][3]).toContain('title="1,200,000 input (cache included) · 34,567 output tokens"');
  });

  it("puts a teal dot before the time of a card an agent works on now, only there", () => {
    const html = render(ROWS);
    const rows = cells(html);
    expect(rows[1][4]).toContain('<span class="pulse" title="An agent is working on it now" aria-label="An agent is working on it now" role="img">');
    expect(html.match(/class="pulse"/g)).toHaveLength(1);
  });

  it("says Archived for an archived card", () => {
    expect(cells(render(ROWS))[2][1]).toContain(">Archived<");
  });

  it("opens the card on a click or Enter, and moves with J and K", () => {
    render(ROWS);
    expect(state.table!.keyboardNav).toBe(true);
    (state.table!.onRowClick as (c: CardTime) => void)(ROWS[0]);
    expect(state.went).toEqual([{ page: "task", id: "t9" }]);
    expect((state.table!.rowId as (c: CardTime) => string)(ROWS[0])).toBe("t9");
  });

  it("sorts every column by what it shows: the number, the column (Archived for archived ones), runs, all tokens and the time with the running run", () => {
    render(ROWS);
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
    expect(by("time")).toEqual(["KADE-1", "KADE-10", "KADE-9", "KADE-2"]);
    // the running run's 5 minutes count in its sort: 3h 05m, not 3h
    expect(cols().find((x) => x.key === "time")!.sort!(ROWS[0])).toBe(3 * HOUR + 5 * MIN);
  });

  it("has the sums of all rows in the footer, the running time included, and the note word for word", () => {
    const html = render(ROWS);
    const foot = html.slice(html.indexOf('<div class="tablefoot">'));
    const total = 12_345 + 550 + 1_234_567;
    expect(text(foot)).toContain(`4 cards · 6 runs · ${formatTokenCount(total)} tokens · 34h 45m agent time`);
    expect(foot).toContain('title="1,212,500 input (cache included) · 34,962 output tokens"');
    expect(text(foot)).toContain("Agent time: from each run's start to its end, added up over every run on the card (each start, Continue and nudge is a run). "
      + "Your own time isn't in it. Tokens: input (cache included) and output, as on the Usage page.");
  });

  it("says one card and one run, not 1 cards", () => {
    const html = render([card({ runs: 1, timeMs: 42 * MIN, inputTokens: 10, outputTokens: 2 })]);
    expect(text(html.slice(html.indexOf('<div class="tablefoot">')))).toContain("1 card · 1 run · 12 tokens · 42m agent time");
  });

  it("says No cards yet. for a project without cards, with no footer", () => {
    const html = render([]);
    expect(text(html)).toContain("No cards yet.");
    expect(html).not.toContain("tablefoot");
    expect(html).not.toContain("<table");
  });

  it("shows nothing while it loads, and the error when it fails", () => {
    const loading = render(null);
    expect(text(loading)).toBe("");
    expect(loading).not.toContain("No cards yet.");
    expect(render(null, "not found: project nope")).toContain('<div class="error-banner">not found: project nope</div>');
  });
});
