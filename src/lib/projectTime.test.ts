// GA-331: a project's Time tracking tab: the agent time's format, a card's time with a run working on it, the sums, and
// the contract between gizai-core's CardTime, the command, the api and the reads that reload the tab.
import { describe, expect, it } from "vitest";
import { TIME_NOTE, WORKING_TITLE, agentTimeMs, cardNumber, columnLabel, formatAgentTime, timeTotals } from "./projectTime";
import { READS } from "./tables";
import type { CardTime } from "../types";
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
const NOW = Date.UTC(2026, 10, 23, 12, 0);

const card = (o: Partial<CardTime>): CardTime => ({ taskId: "t", identifier: "KADE-1", title: "A", column: "To do", archived: false, runs: 0, timeMs: 0,
  runningSince: null, inputTokens: 0, outputTokens: 0, ...o });

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
  it("add up every row, the running runs' time included", () => {
    const rows = [
      card({ runs: 2, timeMs: 30 * MIN, inputTokens: 1_000, outputTokens: 100 }),
      card({ runs: 3, timeMs: 3 * HOUR, runningSince: NOW - 5 * MIN, inputTokens: 500, outputTokens: 50 }),
      card({ runs: 1, timeMs: 31 * HOUR + 10 * MIN, archived: true, inputTokens: 7, outputTokens: 3 }),
      card({}),
    ];
    expect(timeTotals(rows, NOW)).toEqual({ cards: 4, runs: 6, inputTokens: 1_507, outputTokens: 153, timeMs: 34 * HOUR + 45 * MIN });
    expect(formatAgentTime(timeTotals(rows, NOW).timeMs)).toBe("34h 45m");
    expect(timeTotals([], NOW)).toEqual({ cards: 0, runs: 0, inputTokens: 0, outputTokens: 0, timeMs: 0 });
  });
});

describe("the texts, word for word", () => {
  it("has the card's note and the working dot's title", () => {
    expect(TIME_NOTE).toBe("Agent time: from each run's start to its end, added up over every run on the card (each start, Continue and nudge is a run). "
      + "Your own time isn't in it. Tokens: input (cache included) and output, as on the Usage page.");
    expect(WORKING_TITLE).toBe("An agent is working on it now");
  });
});

describe("the contract (GA-331)", () => {
  const camel = (s: string) => s.replace(/_([a-z])/g, (_, c: string) => c.toUpperCase());

  it("CardTime has the same fields in gizai-core and the UI, in camelCase", () => {
    const rs = /pub struct CardTime \{([\s\S]*?)\n\}/.exec(usageRs)?.[1] ?? "";
    expect(/#\[serde\(rename_all = "camelCase"\)\]\s*pub struct CardTime/.test(usageRs)).toBe(true);
    const rsFields = [...rs.matchAll(/pub (\w+):/g)].map((m) => camel(m[1]));
    const ts = /export type CardTime = \{([\s\S]*?)\};/.exec(typesTs)?.[1] ?? "";
    const tsFields = [...ts.matchAll(/(\w+)\??:/g)].map((m) => m[1]);
    expect(rsFields).toEqual(["taskId", "identifier", "title", "column", "archived", "runs", "timeMs", "runningSince", "inputTokens", "outputTokens"]);
    expect(tsFields).toEqual(rsFields);
    expect(ts).toContain("runningSince: number | null");
  });

  it("the command is registered and reads the run time limit as the Settings do, in ms", () => {
    expect(usageRs).toContain("pub fn project_time(db: &Db, project_id: &str, max_run_ms: i64) -> Result<Vec<CardTime>>");
    expect(tauriLib).toMatch(/commands::usage_summary, commands::project_time\b/);
    const cmd = commandsRs.slice(commandsRs.indexOf("pub async fn project_time"), commandsRs.indexOf("pub async fn subscription_limits"));
    expect(cmd).toContain("pub async fn project_time(st: State<'_, AppState>, id: String) -> R<Vec<gizai_core::usage::CardTime>>");
    expect(cmd).toContain('settings::get(&st.db, "max_run_minutes").ok().flatten().unwrap_or(crate::runs::DEFAULT_MAX_RUN_MINUTES)');
    expect(cmd).toContain("60_000");
    expect(cmd).toContain("gizai_core::usage::project_time(&st.db, &id, max_run_ms)");
  });

  it("the api calls it with the project's id, and a run's start and end reload the tab", () => {
    expect(apiTs).toContain('export const projectTime = (id: string) => invoke<T.CardTime[]>("project_time", { id });');
    expect(READS.project_time).toEqual(["runs", "tasks", "workflow_states"]);
  });

  it("the browser fixtures answer it per project, and the mock keeps a running card's start as recent as the other times", () => {
    const pt = (fixtures.commands as unknown as Record<string, { $by: string; $answers: Record<string, CardTime[]> }>).project_time;
    expect(pt.$by).toBe("id");
    const answers = Object.values(pt.$answers);
    expect(answers.length).toBeGreaterThan(0);
    for (const rows of answers) for (const r of rows) expect(Object.keys(r).sort()).toEqual(Object.keys(card({})).sort());
    expect(mockTs).toContain('k === "runningSince"');
  });
});
