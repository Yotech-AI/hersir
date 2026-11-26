// GA-21 (and GA-27): the warning under Settings → Runs at once when the active agents' cards at once add up to more,
// which agents count for it, and the warning by Quit Gizai completely. GA-202: the hint under Memory for agent runs, and
// what Save settings refuses there before it asks the backend. GA-277: the same for Memory per run.
import { describe, expect, it } from "vitest";
import type { Member, Team, WorkflowState } from "../types";
import { cardAgents, CONTROLLERS_AFTER, controllersNote, quitWarning, RUNS_MEMORY_PER_RUN_HINT, runsAtOnceWarning, runsCpuError, runsCpuHint, runsMemoryError, runsMemoryHint,
  runsMemoryPerRunError } from "./settings";

const member = (actorId: string, name: string, more: Partial<Member> = {}): Member => ({
  actorId, name, kind: "agent", roleKey: "backend", handle: name.toLowerCase().replace(/ /g, "-"), status: "active", isLead: false,
  allowedTools: [], chatEnabled: false, ...more,
});
const column = (id: string, agentIds: string[]): WorkflowState => ({ id, name: id, category: "ready", sortKey: id, agentIds });
const team = (id: string, members: Member[], states: WorkflowState[]): Team => ({ id, name: id, members, states, labels: [] });

describe("the agents Runs at once is shared by", () => {
  it("are the active agents on at least one column, each once, with their cards at once", () => {
    const teams = [
      team("t1", [
        member("be", "Backend Agent", { maxRuns: 3 }),
        member("qa", "QA Agent"),
        member("lead", "Team Lead", { roleKey: "lead", isLead: true, chatEnabled: true, maxRuns: 2 }),
        member("off", "Paused Agent", { status: "paused", maxRuns: 4 }),
        member("you", "Jeffrey", { kind: "person" }),
        member("fe", "Frontend Agent", { maxRuns: 2 }),
      ], [column("todo", ["be", "fe", "off", "you"]), column("testing", ["qa", "be"])]),
      team("t2", [member("be", "Backend Agent", { maxRuns: 3 }), member("ops", "DevOps Agent", { maxRuns: 0 })], [column("deploy", ["be", "ops"])]),
    ];
    expect(cardAgents(teams)).toEqual([
      { name: "Backend Agent", cardsAtOnce: 3 },
      { name: "QA Agent", cardsAtOnce: 1 },
      { name: "Frontend Agent", cardsAtOnce: 2 },
      { name: "DevOps Agent", cardsAtOnce: 1 },
    ]);
  });
  it("are none without teams or columns", () => {
    expect(cardAgents([])).toEqual([]);
    expect(cardAgents([team("t", [member("be", "Backend Agent")], [column("todo", [])])])).toEqual([]);
  });
});

describe("the warning under Runs at once", () => {
  const agents = [{ name: "Backend Agent", cardsAtOnce: 3 }, { name: "QA Agent", cardsAtOnce: 1 }, { name: "Frontend Agent", cardsAtOnce: 2 }];
  it("says in plain words what is wrong and what to change when Runs at once is lower than the agents' cards at once added up", () => {
    expect(runsAtOnceWarning(4, agents)).toBe(
      "Your active agents can work on 6 cards at once together (3 for Backend Agent, 1 for QA Agent, 2 for Frontend Agent), but Runs at once is 4, "
      + "so some of them can't use all their slots. Set Runs at once to 6, or lower an agent's Cards at once on the Team page.");
  });
  it("goes away when the numbers fit", () => {
    expect(runsAtOnceWarning(6, agents)).toBeNull();
    expect(runsAtOnceWarning(7, agents)).toBeNull();
    expect(runsAtOnceWarning(20, agents)).toBeNull();
    expect(runsAtOnceWarning(1, [])).toBeNull();
    expect(runsAtOnceWarning(1, [{ name: "QA Agent", cardsAtOnce: 1 }])).toBeNull();
  });
  it("follows the field as it is edited", () => {
    // what the field holds on each keystroke: Number(e.target.value)
    const seen = [5, 1, 0, Number(""), 6, 3].map((n) => runsAtOnceWarning(n, agents));
    expect(seen[0]).toContain("but Runs at once is 5,");
    expect(seen[1]).toContain("but Runs at once is 1,");
    expect(seen[2]).toBeNull();
    expect(seen[3]).toBeNull();
    expect(seen[4]).toBeNull();
    expect(seen[5]).toContain("but Runs at once is 3,");
  });
  it("stays quiet while the field holds no whole number from 1 up", () => {
    for (const n of [0, -2, Number.NaN, 2.5]) expect(runsAtOnceWarning(n, agents)).toBeNull();
  });
  it("says to lower the agents' cards at once when they add up to more than Runs at once can be", () => {
    const many = [{ name: "Backend Agent", cardsAtOnce: 10 }, { name: "Frontend Agent", cardsAtOnce: 10 }, { name: "QA Agent", cardsAtOnce: 5 }];
    const w = runsAtOnceWarning(20, many);
    expect(w).toContain("can work on 25 cards at once together");
    expect(w).toContain("Runs at once goes up to 20, so lower some agents' Cards at once on the Team page.");
    expect(w).not.toContain("Set Runs at once to 25");
  });
});

describe("the warning by Quit Hersir completely", () => {
  const base = "Quitting stops all agents (running cards are stopped), heartbeats and notifications until you start Hersir again. Closing the window only hides Hersir.";
  it("says what quitting stops, and that closing the window only hides Hersir", () => {
    expect(quitWarning(0, 0)).toBe(base);
  });
  it("says how many runs and chat answers it stops when some are live", () => {
    expect(quitWarning(1, 0)).toBe(`${base} Now 1 run will be stopped.`);
    expect(quitWarning(3, 0)).toBe(`${base} Now 3 runs will be stopped.`);
    expect(quitWarning(0, 1)).toBe(`${base} Now 1 chat answer will be stopped.`);
    expect(quitWarning(2, 2)).toBe(`${base} Now 2 runs and 2 chat answers will be stopped.`);
  });
});

describe("the hint under Memory for agent runs", () => {
  it("says it is for all agent runs together and how much memory this computer has", () => {
    expect(runsMemoryHint({ computerMemoryGb: 64, runsMemoryEnforced: true })).toBe("All agent runs together. This computer has 64 GB.");
    expect(runsMemoryHint({ computerMemoryGb: 16, runsMemoryEnforced: true })).toBe("All agent runs together. This computer has 16 GB.");
  });
  it("says that Hersir only waits for free memory where the system doesn't enforce the limit", () => {
    expect(runsMemoryHint({ computerMemoryGb: 16, runsMemoryEnforced: false }))
      .toBe("All agent runs together. This computer has 16 GB. On this system Hersir only waits for free memory before it starts a run.");
  });
  it("leaves out the computer's memory when the system doesn't say it (0) or the backend doesn't send it", () => {
    expect(runsMemoryHint({ computerMemoryGb: 0, runsMemoryEnforced: true })).toBe("All agent runs together.");
    expect(runsMemoryHint({})).toBe("All agent runs together. On this system Hersir only waits for free memory before it starts a run.");
  });
});

describe("what Save settings refuses in Memory for agent runs", () => {
  it("is anything but a whole number of GB from 2 up", () => {
    for (const gb of [2, 3, 32, 64]) expect(runsMemoryError(gb)).toBeNull();
    // what the field holds: Number(e.target.value), so an emptied field is 0
    for (const gb of [0, 1, -4, 2.5, Number(""), Number.NaN]) expect(runsMemoryError(gb)).toBe("Memory for agent runs is a whole number of GB, 2 or more.");
  });
  it("leaves the computer's memory as the most to the backend, and says nothing when the backend didn't send the field", () => {
    expect(runsMemoryError(10_000)).toBeNull();
    expect(runsMemoryError(undefined)).toBeNull();
  });
});

// GA-277: the hint under Memory per run, word for word, and what Save settings refuses there before it asks the backend.
describe("Memory per run (GA-277)", () => {
  it("has the card's hint, word for word", () => {
    expect(RUNS_MEMORY_PER_RUN_HINT).toBe("One run at most: its agent and everything it starts. Linux ends the biggest process of a run that reaches it, and the run goes on. Half of Memory for agent runs until you set it. Runs that start after you save get it.");
  });
  it("refuses anything but a whole number of GB from 2 up", () => {
    for (const gb of [2, 3, 16, 32]) expect(runsMemoryPerRunError(gb)).toBeNull();
    for (const gb of [0, 1, -4, 2.5, Number(""), Number.NaN]) expect(runsMemoryPerRunError(gb)).toBe("Memory per run is a whole number of GB, 2 or more.");
  });
  it("leaves Memory for agent runs as the most to the backend, and says nothing when the backend didn't send the field", () => {
    expect(runsMemoryPerRunError(10_000)).toBeNull();
    expect(runsMemoryPerRunError(undefined)).toBeNull();
  });
});

// GA-369 QA: the hint under CPU for agents (all agents' work together at low priority, the computer's cores, and whether
// the system holds the cap: Linux's slice and Windows's job at once, macOS none), what Save settings refuses there, and
// the note above the one command that turns the missing cpu and io controllers on.
describe("CPU for agents (GA-369)", () => {
  it("says the cap holds at once on Linux and Windows, and that macOS has none", () => {
    expect(runsCpuHint({ computerCores: 16, runsCpuCap: "slice" })).toBe("All agents' work together, at low priority. This computer has 16 cores. A change holds at once, for runs at work too.");
    expect(runsCpuHint({ computerCores: 8, runsCpuCap: "job" })).toBe("All agents' work together, at low priority. This computer has 8 cores. A change holds at once, for runs at work too.");
    expect(runsCpuHint({ computerCores: 10, runsCpuCap: "macos" })).toBe("All agents' work together, at low priority. This computer has 10 cores. macOS has no CPU cap: agents only run at low priority.");
  });
  it("says why the cap doesn't hold on a Linux without a systemd user session or without the cpu controller", () => {
    expect(runsCpuHint({ computerCores: 16, runsCpuCap: "no_slice" })).toBe("All agents' work together, at low priority. This computer has 16 cores. Without a systemd user session Hersir can't cap the CPU: agents only run at low priority.");
    expect(runsCpuHint({ computerCores: 16, runsCpuCap: "no_cpu" })).toBe("All agents' work together, at low priority. This computer has 16 cores. The cap doesn't hold yet: your user's systemd doesn't hand Hersir the cpu controller. Agents only run at low priority.");
  });
  it("leaves out what the backend didn't send", () => {
    expect(runsCpuHint({})).toBe("All agents' work together, at low priority.");
    expect(runsCpuHint({ computerCores: 0, runsCpuCap: "something new" })).toBe("All agents' work together, at low priority.");
  });
  it("refuses anything but a whole number of cores from 1 up, and leaves the computer's cores as the most to the backend", () => {
    for (const n of [1, 2, 12, 16, 10_000]) expect(runsCpuError(n)).toBeNull();
    for (const n of [0, -1, 2.5, Number(""), Number.NaN]) expect(runsCpuError(n)).toBe("CPU for agents is a whole number of cores, 1 or more.");
    expect(runsCpuError(undefined)).toBeNull();
  });
  it("names what the agent runs' slice lacks above the command, and nothing when nothing is missing", () => {
    expect(controllersNote(["io"])).toBe("Agents get no disk priority: your user's systemd doesn't hand Hersir the io controller. Run this once to turn it on:");
    expect(controllersNote(["cpu"])).toBe("Agents get no CPU weight or CPU cap: your user's systemd doesn't hand Hersir the cpu controller. Run this once to turn it on:");
    expect(controllersNote(["cpu", "io"])).toBe("Agents get no CPU weight, CPU cap or disk priority: your user's systemd doesn't hand Hersir the cpu and io controllers. Run this once to turn them on:");
    expect(controllersNote(["io", "cpu"])).toBe(controllersNote(["cpu", "io"]));
    expect(controllersNote([])).toBeNull();
    expect(controllersNote(undefined)).toBeNull();
    expect(CONTROLLERS_AFTER).toBe("It asks for your password (sudo) and adds one Delegate= line for your user's systemd. It holds once you log out and back in.");
  });
});
