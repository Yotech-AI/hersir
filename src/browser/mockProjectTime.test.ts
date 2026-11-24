// GA-354 QA: the Time tracking tab in the browser preview. src/browser/mock.ts answers project_time from the fixture's cards
// and the fixtures' runs (list_runs), as gizai-core counts it: a run counts in a period when it started in it, on the UTC
// day it started. All: every card and the days from the first run's day to today. A period: only the cards with a run that
// started in it, and every day of it.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { ProjectTime, Run } from "../types";

vi.mock("@tauri-apps/api/event", () => ({ emit: async () => {} }));
vi.mock("./pictures", () => ({ screenshotPicture: async () => new ArrayBuffer(0) }));

const fixtures = (await import("./fixtures.json")).default as unknown as import("./mock").Fixtures;
const { answer, loadFixtures } = await import("./mock");

const DAY = 86_400_000;
const utcDay = (ms: number) => ms - (((ms % DAY) + DAY) % DAY);
const projectIds = Object.keys((fixtures as unknown as { commands: { project_time: { $answers: Record<string, unknown> } } }).commands.project_time.$answers);

const time = async (id: string, since?: number, until?: number) => (await answer("project_time", { id, since, until })) as ProjectTime;
const runsOf = async (taskId: string) => ((await answer("list_runs", { taskId })) as Run[]).filter((r) => r.startedAt != null);
const sum = <T>(xs: T[], f: (x: T) => number) => xs.reduce((n, x) => n + f(x), 0);

beforeEach(() => {
  vi.spyOn(console, "info").mockImplementation(() => {});
  vi.spyOn(console, "warn").mockImplementation(() => {});
  loadFixtures(structuredClone(fixtures));
});
afterEach(() => { vi.restoreAllMocks(); });

describe("project_time in the browser preview (GA-354)", () => {
  it("has fixtures for at least one project with runs", async () => {
    expect(projectIds.length).toBeGreaterThan(0);
    const withRuns = await Promise.all(projectIds.map(async (id) => (await time(id)).firstRun != null));
    expect(withRuns).toContain(true);
  });

  it("All: every card, no period, and the days from the first run's UTC day to today, adding up to the cards", async () => {
    for (const id of projectIds) {
      const t = await time(id);
      expect([t.since, t.until]).toEqual([null, null]);
      if (t.firstRun == null) { expect(t.days).toEqual([]); continue; }
      expect(t.days[0].dayStart).toBe(utcDay(t.firstRun));
      expect(t.days[t.days.length - 1].dayStart).toBe(utcDay(Date.now()));
      expect(t.days.every((d, i) => i === 0 || d.dayStart - t.days[i - 1].dayStart === DAY)).toBe(true);
      expect(sum(t.days, (d) => d.runs)).toBe(sum(t.cards, (c) => c.runs));
      expect(sum(t.days, (d) => d.inputTokens + d.outputTokens)).toBe(sum(t.cards, (c) => c.inputTokens + c.outputTokens));
      expect(sum(t.days, (d) => d.costUsdMicros)).toBe(sum(t.cards, (c) => c.costUsdMicros));
    }
  });

  it("a period: only the cards with a run that started in it, the run started 1 ms before it left out, and every day of it", async () => {
    const id = (await Promise.all(projectIds.map(async (p) => ((await time(p)).firstRun != null ? p : null)))).find(Boolean)!;
    const all = await time(id);
    const card = all.cards.find((c) => c.runs > 0)!;
    const runs = await runsOf(card.taskId);
    const first = Math.min(...runs.map((r) => r.startedAt!));
    // the period starts 1 ms after the card's first run, for two days
    const t = await time(id, first + 1, first + 1 + 2 * DAY);
    expect([t.since, t.until]).toEqual([first + 1, first + 1 + 2 * DAY]);
    expect(t.days).toHaveLength(2);
    const inIt = runs.filter((r) => r.startedAt! > first && r.startedAt! < first + 1 + 2 * DAY);
    const got = t.cards.find((c) => c.taskId === card.taskId);
    if (inIt.length === 0) expect(got).toBeUndefined();
    else expect(got?.runs).toBe(inIt.length);
    expect(t.cards.every((c) => c.runs > 0)).toBe(true);
    // from its own start the run is in it
    const from = await time(id, first, first + DAY);
    expect(from.cards.find((c) => c.taskId === card.taskId)?.runs).toBeGreaterThanOrEqual(1);
  });

  it("a period without runs has no cards and every day at 0", async () => {
    const since = Date.UTC(2000, 0, 1);
    for (const id of projectIds) {
      const t = await time(id, since, since + 7 * DAY);
      expect(t.cards).toEqual([]);
      expect(t.days.map((d) => d.dayStart)).toEqual(Array.from({ length: 7 }, (_, i) => since + i * DAY));
      expect(t.days.every((d) => d.runs === 0 && d.timeMs === 0 && d.costUsdMicros === 0)).toBe(true);
    }
  });
});
