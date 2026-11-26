// Settings: the warning under "Runs at once" (was GA-27), the hint under "Memory for agent runs" (GA-202) and "Memory per
// run" (GA-277), "CPU for agents" and what the agent runs' slice lacks (GA-369), and the warning by Quit Gizai completely
// (GA-21).
import type { Settings, Team } from "../types";

/** An agent that takes cards, with its cards at once. */
export type CardAgent = { name: string; cardsAtOnce: number };

/** The agents "Runs at once" is shared by: the active agents on at least one column (Team → Workflow), each once. The
 *  columns decide when an agent works, so one on no column, like the Team Lead who answers in chat, takes no cards. */
export function cardAgents(teams: Team[]): CardAgent[] {
  const seen = new Set<string>();
  const out: CardAgent[] = [];
  for (const team of teams) {
    const onColumns = new Set(team.states.flatMap((s) => s.agentIds ?? []));
    for (const m of team.members) {
      if (m.kind !== "agent" || m.status !== "active" || !onColumns.has(m.actorId) || seen.has(m.actorId)) continue;
      seen.add(m.actorId);
      out.push({ name: m.name, cardsAtOnce: Math.max(1, m.maxRuns ?? 1) });
    }
  }
  return out;
}

/** The warning under "Runs at once" when it is lower than the agents' cards at once added up: some agents then can't
 *  use all their slots. Null when the numbers fit, or while the field holds no number from 1 up. */
export function runsAtOnceWarning(runsAtOnce: number, agents: CardAgent[]): string | null {
  const total = agents.reduce((n, a) => n + a.cardsAtOnce, 0);
  if (!Number.isInteger(runsAtOnce) || runsAtOnce < 1 || runsAtOnce >= total) return null;
  const each = agents.map((a) => `${a.cardsAtOnce} for ${a.name}`).join(", ");
  const fix = total <= 20
    ? `Set Runs at once to ${total}, or lower an agent's Cards at once on the Team page.`
    : "Runs at once goes up to 20, so lower some agents' Cards at once on the Team page.";
  return `Your active agents can work on ${total} cards at once together (${each}), but Runs at once is ${runsAtOnce}, so some of them can't use all their slots. ${fix}`;
}

/** The hint under "Memory for agent runs": all agent runs together, and how much memory the computer has. Where the
 *  system doesn't enforce the limit (macOS, Windows, Linux without a systemd user session) it says that Gizai only waits
 *  for free memory before it starts a run. */
export function runsMemoryHint(s: Pick<Settings, "computerMemoryGb" | "runsMemoryEnforced">): string {
  const has = s.computerMemoryGb ? ` This computer has ${s.computerMemoryGb} GB.` : "";
  const only = s.runsMemoryEnforced ? "" : " On this system Hersir only waits for free memory before it starts a run.";
  return `All agent runs together.${has}${only}`;
}

/** Why "Memory for agent runs" can't be saved as it is, or null: a whole number of GB, 2 or more (the backend checks
 *  the computer's memory as the most). Null too when the backend didn't send the field. */
export function runsMemoryError(gb: number | undefined): string | null {
  if (gb === undefined || (Number.isInteger(gb) && gb >= 2)) return null;
  return "Memory for agent runs is a whole number of GB, 2 or more.";
}

/** The hint under "Memory per run" (GA-277). */
export const RUNS_MEMORY_PER_RUN_HINT = "One run at most: its agent and everything it starts. Linux ends the biggest process of a run that reaches it, and the run goes on. Half of Memory for agent runs until you set it. Runs that start after you save get it.";

/** Why "Memory per run" can't be saved as it is, or null: a whole number of GB, 2 or more. The backend checks Memory
 *  for agent runs as the most, as only it knows whether the value changed: one left as it was may stay above a Memory
 *  for agent runs lowered later. Null too when the backend didn't send the field. */
export function runsMemoryPerRunError(gb: number | undefined): string | null {
  if (gb === undefined || (Number.isInteger(gb) && gb >= 2)) return null;
  return "Memory per run is a whole number of GB, 2 or more.";
}

/** The hint under "CPU for agents" (GA-369): all agents' work together at low priority, the computer's cores, and
 *  whether the system holds the cap: at once on Linux (the agent runs' slice) and Windows (the agents' job); macOS has
 *  none, nor Linux without a systemd user session or without the cpu controller (the command below the field turns
 *  that on). */
export function runsCpuHint(s: Pick<Settings, "computerCores" | "runsCpuCap">): string {
  const has = s.computerCores ? ` This computer has ${s.computerCores} cores.` : "";
  const how: Record<string, string> = {
    slice: " A change holds at once, for runs at work too.",
    job: " A change holds at once, for runs at work too.",
    macos: " macOS has no CPU cap: agents only run at low priority.",
    no_slice: " Without a systemd user session Hersir can't cap the CPU: agents only run at low priority.",
    no_cpu: " The cap doesn't hold yet: your user's systemd doesn't hand Hersir the cpu controller. Agents only run at low priority.",
  };
  return `All agents' work together, at low priority.${has}${how[s.runsCpuCap ?? ""] ?? ""}`;
}

/** Why "CPU for agents" can't be saved as it is, or null: a whole number of cores, 1 or more (the backend checks the
 *  computer's cores as the most). Null too when the backend didn't send the field. */
export function runsCpuError(cores: number | undefined): string | null {
  if (cores === undefined || (Number.isInteger(cores) && cores >= 1)) return null;
  return "CPU for agents is a whole number of cores, 1 or more.";
}

/** What the agent runs' slice lacks (GA-369), for the note above the command that turns it on; null when nothing is
 *  missing. */
export function controllersNote(missing: string[] | undefined): string | null {
  const cpu = missing?.includes("cpu") ?? false, io = missing?.includes("io") ?? false;
  if (cpu && io) return "Agents get no CPU weight, CPU cap or disk priority: your user's systemd doesn't hand Hersir the cpu and io controllers. Run this once to turn them on:";
  if (cpu) return "Agents get no CPU weight or CPU cap: your user's systemd doesn't hand Hersir the cpu controller. Run this once to turn it on:";
  if (io) return "Agents get no disk priority: your user's systemd doesn't hand Hersir the io controller. Run this once to turn it on:";
  return null;
}

/** What to do after the command that turns the controllers on (GA-369). */
export const CONTROLLERS_AFTER = "It asks for your password (sudo) and adds one Delegate= line for your user's systemd. It holds once you log out and back in.";

/** The warning by Quit Gizai completely: what stops, and how many runs and chat answers it stops when some are at work. */
export function quitWarning(runs: number, answers: number): string {
  const text = "Quitting stops all agents (running cards are stopped), heartbeats and notifications until you start Hersir again. Closing the window only hides Hersir.";
  const live = [runs > 0 && `${runs} ${runs === 1 ? "run" : "runs"}`, answers > 0 && `${answers} chat ${answers === 1 ? "answer" : "answers"}`].filter(Boolean);
  if (live.length === 0) return text;
  return `${text} Now ${live.join(" and ")} will be stopped.`;
}
