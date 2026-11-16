// GA-287 QA: what the @ chip's hover card reads of an item, how long it keeps it, and where the card goes
// (src/lib/itemPreview.ts; design/islands/README.md → The chip's hover card). The backend is faked: every read is counted,
// so "an item is read only when its card is about to show, and kept for 60 seconds" is checked by the calls it makes.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { Client, Doc, LiveRun, Member, Person, Project, Task } from "../types";

const api = vi.hoisted(() => ({
  itemId: vi.fn(),
  getTask: vi.fn(),
  getProject: vi.fn(),
  getClient: vi.fn(),
  getAgent: vi.fn(),
  getDoc: vi.fn(),
  listUsers: vi.fn(),
  liveRuns: vi.fn(),
}));
vi.mock("../api", () => api);

const { CARD_EDGE, CARD_GAP, CARD_WIDTH, KEEP_MS, cardPlace, forgetPreviews, keptPreview, loadPreview } = await import("./itemPreview");

const task = (o: Partial<Task> = {}) => ({ id: "t-12", identifier: "GA-12", title: "Fix the login", projectName: "Hersir", projectColor: "#4c8dff",
  stateName: "In progress", stateCategory: "in_progress", assigneeName: "Frontend Agent", assigneeKind: "agent", hold: null, ...o }) as Task;
const project = (o: Partial<Project> = {}) => ({ id: "p-1", key: "GA", name: "Giz AI", status: "active", color: "#4c8dff", clientName: "Yotech", ...o }) as Project;
const agent = (o: Partial<Member> = {}) => ({ actorId: "a-1", name: "Design Agent", kind: "agent", roleKey: "design", status: "active", ...o }) as Member;
const person = (o: Partial<Person> = {}) => ({ id: "u-1", name: "Sanne de Vries", handle: "sanne", email: "sanne@example.com", openTasks: 0, ...o }) as Person;
const client = (o: Partial<Client> = {}) => ({ id: "c-1", name: "Kade", status: "active", ...o }) as Client;
const doc = (o: Partial<Doc> = {}) => ({ id: "d-1", projectId: "p-1", title: "Release checklist", bodyMd: "", currentVersion: 1, updatedAt: 0, ...o }) as Doc;

/** Every read the fake backend got, by name. */
const reads = () => Object.fromEntries(Object.entries(api).map(([k, f]) => [k, f.mock.calls.length]).filter(([, n]) => n));

beforeEach(() => {
  forgetPreviews();
  for (const f of Object.values(api)) f.mockReset();
  api.itemId.mockImplementation(async (kind: string, key: string) => {
    if (kind === "task" && key === "GA-12") return "t-12";
    if (kind === "project" && key.toLowerCase() === "ga") return "p-1";
    throw new Error(`No ${kind} ${key}`);
  });
  api.getTask.mockImplementation(async (id: string) => (id === "t-12" ? task() : id === "t-7" ? task({ id: "t-7", identifier: "GA-7" }) : null));
  api.getProject.mockImplementation(async (id: string) => (id === "p-1" ? project() : null));
  api.getClient.mockImplementation(async (id: string) => (id === "c-1" ? client() : null));
  api.getAgent.mockImplementation(async (id: string) => (id === "a-1" ? agent() : id === "a-2" ? agent({ actorId: "a-2", status: "paused" }) : null));
  api.getDoc.mockImplementation(async (id: string) => (id === "d-1" ? doc() : id === "d-2" ? doc({ id: "d-2", projectId: null }) : null));
  api.listUsers.mockResolvedValue([person(), person({ id: "u-2", name: "Ada", email: null })]);
  api.liveRuns.mockResolvedValue([]);
});
afterEach(() => vi.useRealTimers());

describe("reading an item for its card", () => {
  it("reads nothing until a card asks for it", () => {
    expect(keptPreview("task", "GA-12")).toBeUndefined();
    expect(reads()).toEqual({});
  });

  it("a card: its id from the identifier, then the card itself", async () => {
    expect(await loadPreview("task", "GA-12")).toEqual({ kind: "task", task: task() });
    expect(api.itemId).toHaveBeenCalledWith("task", "GA-12");
    expect(api.getTask).toHaveBeenCalledWith("t-12");
    expect(reads()).toEqual({ itemId: 1, getTask: 1 });
  });

  it("a project by its key, with any case", async () => {
    expect(await loadPreview("project", "ga")).toEqual({ kind: "project", project: project() });
    expect(api.getProject).toHaveBeenCalledWith("p-1");
  });

  it("a client by its id", async () => {
    expect(await loadPreview("client", "c-1")).toEqual({ kind: "client", client: client() });
  });

  it("a person from the list of people", async () => {
    expect(await loadPreview("person", "u-2")).toEqual({ kind: "person", person: person({ id: "u-2", name: "Ada", email: null }) });
    expect(reads()).toEqual({ listUsers: 1 });
  });

  it("a doc with its project, and one without a project", async () => {
    expect(await loadPreview("doc", "d-1")).toEqual({ kind: "doc", doc: doc(), project: project() });
    expect(await loadPreview("doc", "d-2")).toEqual({ kind: "doc", doc: doc({ id: "d-2", projectId: null }), project: undefined });
    expect(api.getProject).toHaveBeenCalledTimes(1);
  });

  it("a doc whose project can't be read still shows, without a project", async () => {
    api.getProject.mockRejectedValue(new Error("gone"));
    expect(await loadPreview("doc", "d-1")).toEqual({ kind: "doc", doc: doc(), project: undefined });
  });

  it("an agent at rest: active or paused, working on nothing", async () => {
    expect(await loadPreview("agent", "a-1")).toEqual({ kind: "agent", agent: agent(), workingOn: undefined });
    expect(await loadPreview("agent", "a-2")).toEqual({ kind: "agent", agent: agent({ actorId: "a-2", status: "paused" }), workingOn: undefined });
    expect(api.getTask).not.toHaveBeenCalled();
  });

  it("an agent at work: the id of the card its live run is on", async () => {
    api.liveRuns.mockResolvedValue([{ runId: "r-9", taskId: "t-12", agentId: "a-9" }, { runId: "r-1", taskId: "t-7", agentId: "a-1" }] satisfies LiveRun[]);
    expect(await loadPreview("agent", "a-1")).toEqual({ kind: "agent", agent: agent(), workingOn: "GA-7" });
    expect(api.getTask).toHaveBeenCalledWith("t-7");
  });

  it("an agent still shows when the live runs can't be read", async () => {
    api.liveRuns.mockRejectedValue(new Error("no"));
    expect(await loadPreview("agent", "a-1")).toEqual({ kind: "agent", agent: agent(), workingOn: undefined });
  });

  it.each([
    ["task", "GA-999", "a card key that matches nothing"],
    ["project", "ZZ", "a project key that matches nothing"],
    ["client", "c-9", "a deleted client (null)"],
    ["agent", "a-9", "a deleted agent (null)"],
    ["person", "u-9", "a person not in the list"],
    ["doc", "d-9", "a deleted doc (null)"],
  ] as const)("%s %s is Not found: %s", async (kind, key) => {
    expect(await loadPreview(kind, key)).toEqual({ kind: "missing" });
  });

  it("a card whose id is found but the card is gone, or read fails, is Not found", async () => {
    api.itemId.mockResolvedValue("t-gone");
    expect(await loadPreview("task", "GA-13")).toEqual({ kind: "missing" });
    api.itemId.mockResolvedValue("");
    expect(await loadPreview("task", "GA-14")).toEqual({ kind: "missing" });
    api.itemId.mockResolvedValue("t-12");
    api.getTask.mockRejectedValue(new Error("db"));
    expect(await loadPreview("task", "GA-15")).toEqual({ kind: "missing" });
  });
});

describe("keeping an item for a minute", () => {
  it("resting on the same chip again within a minute reads nothing", async () => {
    const t0 = 1_000_000;
    await loadPreview("task", "GA-12", t0);
    await loadPreview("task", "GA-12", t0 + 1_000);
    await loadPreview("task", "GA-12", t0 + KEEP_MS - 1);
    expect(KEEP_MS).toBe(60_000);
    expect(reads()).toEqual({ itemId: 1, getTask: 1 });
  });

  it("after a minute it reads the item again", async () => {
    const t0 = 1_000_000;
    await loadPreview("task", "GA-12", t0);
    api.getTask.mockResolvedValue(task({ stateName: "Testing" }));
    const again = await loadPreview("task", "GA-12", t0 + KEEP_MS);
    expect(again).toEqual({ kind: "task", task: task({ stateName: "Testing" }) });
    expect(reads()).toEqual({ itemId: 2, getTask: 2 });
  });

  it("two chips of the same item that ask at once share one read", async () => {
    const [a, b] = await Promise.all([loadPreview("agent", "a-1", 5), loadPreview("agent", "a-1", 6)]);
    expect(a).toBe(b);
    expect(reads()).toEqual({ getAgent: 1, liveRuns: 1 });
  });

  it("keeps a Not found for the minute too", async () => {
    await loadPreview("client", "c-9", 10);
    expect(await loadPreview("client", "c-9", 20)).toEqual({ kind: "missing" });
    expect(api.getClient).toHaveBeenCalledTimes(1);
  });

  it("keeps each kind apart: a client and a person with the same id are two reads", async () => {
    api.getClient.mockResolvedValue(client({ id: "x" }));
    api.listUsers.mockResolvedValue([person({ id: "x" })]);
    expect((await loadPreview("client", "x", 1)).kind).toBe("client");
    expect((await loadPreview("person", "x", 2)).kind).toBe("person");
  });

  it("keptPreview gives the item only once it has arrived, and only for the minute", async () => {
    let arrive!: (t: Task) => void;
    api.getTask.mockReturnValue(new Promise<Task>((r) => { arrive = r; }));
    const t0 = 2_000_000;
    const p = loadPreview("task", "GA-12", t0);
    await vi.waitFor(() => expect(api.getTask).toHaveBeenCalled());
    expect(keptPreview("task", "GA-12", t0 + 10)).toBeUndefined();
    arrive(task());
    await p;
    await Promise.resolve();
    expect(keptPreview("task", "GA-12", t0 + 10)).toEqual({ kind: "task", task: task() });
    expect(keptPreview("task", "GA-12", t0 + KEEP_MS - 1)).toBeDefined();
    expect(keptPreview("task", "GA-12", t0 + KEEP_MS)).toBeUndefined();
  });
});

describe("where the card goes", () => {
  const win = { width: 1280, height: 720 };
  const card = { width: CARD_WIDTH, height: 160 };
  const chip = (left: number, top: number, height = 20) => ({ left, top, bottom: top + height });

  it("has the README's sizes: 300px wide, 6px from the chip, 8px from the window's edges", () => {
    expect([CARD_WIDTH, CARD_GAP, CARD_EDGE]).toEqual([300, 6, 8]);
  });

  it("goes below the chip, 6px under it, its left edge on the chip's", () => {
    expect(cardPlace(chip(200, 100), card, win)).toEqual({ left: 200, top: 126 });
  });

  it("goes above the chip when there's no room below and more room above", () => {
    const at = chip(200, 640);
    expect(cardPlace(at, card, win)).toEqual({ left: 200, top: 640 - 6 - 160 });
  });

  it("stays below when it just fits (its height, the gap and 8px)", () => {
    const at = chip(200, 720 - 8 - 160 - 6 - 20);
    expect(cardPlace(at, card, win).top).toBe(at.bottom + 6);
    const tight = chip(200, at.top + 1);
    expect(cardPlace(tight, card, win).top).toBe(tight.top - 6 - 160);
  });

  it("stays below when there is less room above than below, kept inside the window", () => {
    const short = { width: 1280, height: 300 };
    const at = chip(200, 120);
    const p = cardPlace(at, { width: 300, height: 200 }, short);
    expect(p.top + 200).toBeLessThanOrEqual(300 - 8);
    expect(p.top).toBeGreaterThanOrEqual(8);
  });

  it("is kept 8px inside the right and the left edges", () => {
    expect(cardPlace(chip(1250, 100), card, win).left).toBe(1280 - 8 - 300);
    expect(cardPlace(chip(2, 100), card, win).left).toBe(8);
    expect(cardPlace(chip(-40, 100), card, win).left).toBe(8);
  });

  it("in a window narrower than the card, sits 8px from the left (the card is max-width 100vw - 16px)", () => {
    expect(cardPlace(chip(100, 100), { width: 284, height: 160 }, { width: 300, height: 720 }).left).toBe(8);
  });

  it("near any edge of the window it stays fully inside it, 8px in", () => {
    const sizes = [{ width: 1280, height: 720 }, { width: 900, height: 640 }, { width: 1440, height: 900 }, { width: 420, height: 500 }];
    for (const w of sizes) {
      const c = { width: Math.min(300, w.width - 16), height: 180 };
      for (let x = -20; x <= w.width + 20; x += 37) {
        for (let y = -10; y <= w.height + 10; y += 23) {
          const p = cardPlace(chip(x, y), c, w);
          const where = `${w.width}x${w.height} chip at ${x},${y}`;
          expect(p.left, where).toBeGreaterThanOrEqual(8);
          expect(p.left + c.width, where).toBeLessThanOrEqual(w.width - 8);
          expect(p.top, where).toBeGreaterThanOrEqual(8);
          expect(p.top + c.height, where).toBeLessThanOrEqual(w.height - 8);
        }
      }
    }
  });

  it("never covers the chip while there is room for it on one side", () => {
    for (let y = 0; y <= 700; y += 10) {
      const at = chip(200, y);
      const p = cardPlace(at, card, win);
      const below = 720 - at.bottom, above = at.top;
      if (below >= 6 + 160 + 8 || above >= 6 + 160 + 8) {
        const clear = p.top >= at.bottom || p.top + 160 <= at.top;
        expect(clear, `chip at ${y}: card at ${p.top}`).toBe(true);
      }
    }
  });
});
