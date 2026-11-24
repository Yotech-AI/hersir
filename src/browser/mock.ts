// The browser mode's fake backend (GA-185): answers Tauri's commands from src/browser/fixtures.json. A command without a
// fixture answers empty, by the type src/api.ts gives it ([] for a list, false, 0, "" or null), and says so in the
// console once. A few writes change the fixtures in memory, so a form's save shows until the page is reloaded.
import { emit } from "@tauri-apps/api/event";
import apiSource from "../api.ts?raw";
import { PICK_TITLE } from "../lib/restore";
import { keyBetween } from "../lib/sortKey";
import { screenshotPicture } from "./pictures";

/** An answer for any arguments, or answers by one argument's value. */
type Keyed = { $by: string; $answers: Record<string, unknown> };
export type Fixtures = { about?: string; writtenAt: number; commands: Record<string, unknown> };

let commands: Record<string, unknown> = {};

/** The folder Hersir makes inside a chosen one for backups (gizai-core's `backups::FOLDER_NAME`), as the fixtures name it. */
let backupsFolderName = "";

const isKeyed = (f: unknown): f is Keyed => !!f && typeof f === "object" && "$by" in f && "$answers" in f;

/** Moves every time (a field whose name ends in "At", `at`, a backup status's `next` and a card's `runningSince`) by `delta`, so
 *  "2 h ago" in the fixtures stays 2 h ago. */
function shiftTimes(v: unknown, delta: number): void {
  if (Array.isArray(v)) { v.forEach((x) => shiftTimes(x, delta)); return; }
  if (!v || typeof v !== "object") return;
  const o = v as Record<string, unknown>;
  for (const [k, x] of Object.entries(o)) {
    if (typeof x === "number" && (k === "at" || k === "next" || k === "runningSince" || k.endsWith("At")) && x > 0) o[k] = x + delta;
    else shiftTimes(x, delta);
  }
}

export function loadFixtures(f: Fixtures) {
  commands = f.commands;
  shiftTimes(commands, Date.now() - f.writtenAt);
  backupsFolderName = String((commands.backup_status as { folder?: string | null } | undefined)?.folder ?? "").split("/").pop() ?? "";
  usbSnapshots = null;
  restoreRun++;
  console.info(`Hersir in a browser: ${Object.keys(commands).length} commands answer from the fixtures (src/browser/fixtures.json).`);
}

/** What each command answers without a fixture, from its type in src/api.ts. */
const EMPTY: Record<string, unknown> = (() => {
  const out: Record<string, unknown> = {};
  for (const m of apiSource.matchAll(/invoke<(.+?)>\(\s*"([^"]+)"/g)) {
    const [, type, cmd] = m;
    out[cmd] = type.trim().endsWith("[]") ? [] : type === "boolean" ? false : type === "number" ? 0 : type === "string" ? "" : null;
  }
  return out;
})();

const told = new Set<string>();
function empty(cmd: string, why: string) {
  const v = cmd in EMPTY ? EMPTY[cmd] : null;
  if (!told.has(why)) {
    told.add(why);
    console.warn(`Hersir in a browser: no fixture for ${why}, so it answers ${JSON.stringify(v)}.`);
  }
  return structuredClone(v);
}

/** The fixture `cmd` answers with these arguments, or undefined without one. */
function fixture(cmd: string, args: Record<string, unknown>): { found: boolean; value?: unknown; why: string } {
  const f = commands[cmd];
  if (f === undefined) return { found: false, why: cmd };
  if (!isKeyed(f)) return { found: true, value: f, why: cmd };
  const key = String(args[f.$by]);
  return key in f.$answers ? { found: true, value: f.$answers[key], why: cmd } : { found: false, why: `${cmd} (${f.$by} ${key})` };
}

/** The answer object behind a keyed fixture, to change it in place. */
function stored(cmd: string, key: string): Record<string, unknown> | undefined {
  const f = commands[cmd];
  return isKeyed(f) ? (f.$answers[key] as Record<string, unknown> | undefined) : undefined;
}

const changed = (table: string) => { void emit("rows-changed", { table }); };

type Row = Record<string, unknown>;

/** An agent as get_agent and every team in get_team have it, to change it in place. */
function agents(id: unknown): Row[] {
  const teams = commands.get_team;
  const members = isKeyed(teams) ? Object.values(teams.$answers).flatMap((t) => ((t as { members?: Row[] }).members ?? [])) : [];
  return [stored("get_agent", String(id)), ...members.filter((m) => m.actorId === id)].filter((m): m is Row => !!m);
}

/** The few writes the screens need to show what they saved. */
const WRITES: Record<string, (args: Row) => unknown> = {
  update_task: ({ id, patch }) => {
    const p = patch as Row;
    for (const t of [stored("get_task", String(id)), ...((commands.list_tasks as Row[] | undefined) ?? []).filter((x) => x.id === id)]) if (t) Object.assign(t, p);
    changed("tasks");
    return null;
  },
  move_task: ({ id, stateId, sortKey }) => {
    const team = fixture("get_team", { id: null }).value as { states?: Row[] } | undefined;
    const s = team?.states?.find((x) => x.id === stateId);
    for (const t of [stored("get_task", String(id)), ...((commands.list_tasks as Row[] | undefined) ?? []).filter((x) => x.id === id)]) {
      if (t) Object.assign(t, { stateId, sortKey, ...(s ? { stateName: s.name, stateCategory: s.category } : {}) });
    }
    changed("tasks");
    return null;
  },
  // GA-306: several cards at once, each to the bottom of the column in their order; archived ones leave the list until
  // restore_tasks puts them back at the bottom of their Done column.
  move_tasks: ({ ids, stateId }) => {
    const team = fixture("get_team", { id: null }).value as { states?: Row[] } | undefined;
    const s = team?.states?.find((x) => x.id === stateId);
    return bulk(ids as string[], (t) => {
      if (archivedRows.has(String(t.id))) return `${t.identifier} is archived: restore it first`;
      Object.assign(t, { stateId, sortKey: bottomKey(String(stateId)), ...(s ? { stateName: s.name, stateCategory: s.category } : {}) });
      const page = stored("get_task", String(t.id));
      if (page) Object.assign(page, { stateId: t.stateId, sortKey: t.sortKey, stateName: t.stateName, stateCategory: t.stateCategory });
      return null;
    });
  },
  archive_tasks: ({ ids }) => bulk(ids as string[], (t) => {
    if (archivedRows.has(String(t.id))) return `${t.identifier} is already archived`;
    if (t.stateCategory !== "done") return `Only a card in Done can be archived, and ${t.identifier} is in ${t.stateName}`;
    const list = (commands.list_tasks as Row[] | undefined) ?? [];
    list.splice(list.indexOf(t), 1);
    archivedRows.set(String(t.id), t);
    return null;
  }),
  restore_tasks: ({ ids }) => bulk(ids as string[], (t) => {
    if (!archivedRows.delete(String(t.id))) return `${t.identifier} isn't archived`;
    t.sortKey = bottomKey(String(t.stateId));
    ((commands.list_tasks as Row[] | undefined) ??= []).push(t);
    return null;
  }),
  save_settings: ({ settings }) => { commands.get_settings = settings; changed("settings"); return null; },
  save_project: ({ id, input }) => {
    const i = { ...(input as Row) };
    // Test runs at once 0 is no limit, kept as null (GA-212), as the backend does
    if (i.testRunsAtOnce === 0) i.testRunsAtOnce = null;
    for (const p of [stored("get_project", String(id)), ...((commands.list_projects as Row[] | undefined) ?? []).filter((x) => x.id === id)]) if (p) Object.assign(p, i);
    changed("projects");
    return id ?? "new-project";
  },
  save_agent_cli_tools: ({ agentId, tools }) => {
    const view = stored("agent_cli_tools", String(agentId));
    const saved = { ...((view?.saved as Row | undefined) ?? {}), ...(tools as Row) };
    if (view) view.saved = saved;
    for (const a of agents(agentId)) a.cliTools = { ...((a.cliTools as Row | undefined) ?? {}), ...(tools as Row) };
    changed("actors");
    return saved;
  },
  // the browser's switch and the MCP servers' are the agent's own (its form's Tools)
  save_agent_mcp: ({ agentId, tools }) => {
    for (const a of agents(agentId)) a.tools = structuredClone(tools);
    changed("team");
    const view = fixture("agent_mcp", { agentId });
    return view.found ? structuredClone(view.value) : empty("agent_mcp", view.why);
  },
  // Settings → MCP servers → Claude Desktop (GA-340, on GA-338's contract): Connect makes a made-up key, new each time, and
  // with addToConfig "adds" the entry (only the status changes: a browser writes no file); Disconnect ends the key, and the
  // settings keep the entry, as the real ones do.
  claude_desktop_connect: ({ addToConfig }) => {
    const st = claudeDesktop();
    if (!st) return empty("claude_desktop_connect", "claude_desktop_connect");
    if (!st.shimPath) return Promise.reject("Hersir can't find its hersir-mcp program next to Hersir: reinstall Hersir");
    const configPath = (st.configPath as string | null) ?? null;
    const wroteConfig = !!addToConfig && !!configPath;
    const backupPath = wroteConfig && !st.configHasHersir ? `${configPath}.before-hersir` : null;
    Object.assign(st, { connected: true, connectedAt: Date.now(), configHasHersir: !!st.configHasHersir || wroteConfig });
    const entry = JSON.stringify({ mcpServers: { hersir: { command: st.shimPath, args: [], env: { HERSIR_MCP_KEY: madeUpKey() } } } }, null, 2);
    return { entry, wroteConfig, configPath, backupPath, configError: null, status: structuredClone(st) };
  },
  claude_desktop_disconnect: () => {
    const st = claudeDesktop();
    if (!st) return empty("claude_desktop_disconnect", "claude_desktop_disconnect");
    Object.assign(st, { connected: false, connectedAt: null });
    return structuredClone(st);
  },
  open_file: ({ id }) => { console.info(`Hersir in a browser: Open would show file ${id} in the system's image viewer.`); return null; },
  // Settings → General → Backups (GA-217): the backend's refusals, the status with what changed, and backups-changed. There
  // is no folder picker in a browser, so Browse… does nothing; Turn off does.
  save_backup_settings: ({ settings }) => {
    const st = backups();
    const s = settings as Row;
    if (!st) return empty("save_backup_settings", "save_backup_settings");
    if (!(Number.isInteger(s.keep) && Number(s.keep) >= 1 && Number(s.keep) <= 100)) return Promise.reject(`Keep must be 1 to 100 snapshots, not ${s.keep}`);
    const folder = typeof s.folder === "string" && s.folder.trim() ? s.folder.trim() : null;
    Object.assign(st, { settings: { ...s, folder }, folder: folder && `${folder}/${backupsFolderName}`, folderProblem: null });
    if (!folder) Object.assign(st, { last: null, lastError: null });
    st.next = nextBackup(st);
    return backupsChanged();
  },
  // a snapshot by hand: its steps a moment each, then it is the newest in the list
  back_up_now: () => {
    const st = backups();
    if (!st) return empty("back_up_now", "back_up_now");
    if (!st.folder) return Promise.reject("Choose a folder for backups first");
    if (st.running) return Promise.reject("A backup is running already");
    const steps = ["database", "files", "compress", "manifest", "keep"];
    st.running = { reason: "manual", startedAt: Date.now(), step: steps[0] };
    steps.slice(1).forEach((step, i) => setTimeout(() => { st.running = { ...(st.running as Row), step }; backupsChanged(); }, (i + 1) * BACKUP_STEP_MS));
    setTimeout(() => {
      const list = (commands.list_backups as Row[] | undefined) ?? [];
      const mine = list.find((e) => e.thisComputer);
      const madeAt = Date.now();
      const name = String(mine?.name ?? "gizai-manual-00000000-000000-000-device").replace(/^gizai-[a-z-]+-\d{8}-\d{6}-\d{3}-/, `gizai-manual-${stamp(madeAt)}-`);
      if (mine) list.unshift({ ...mine, name, reason: "manual", madeAt });
      Object.assign(st, { running: null, lastError: null, last: { name, madeAt, bytes: Math.round(Number(mine?.dbBytes ?? 4_000_000) / 4) } });
      st.next = nextBackup(st);
      backupsChanged();
    }, steps.length * BACKUP_STEP_MS);
    return backupsChanged();
  },
  // Restore (GA-219, contract BR): the backend's refusals, its steps a moment each with restore-changed, then ready.
  start_restore: ({ folder, name }) => {
    const st = restoreNow();
    if (!st) return empty("start_restore", "start_restore");
    if (backups()?.running) return Promise.reject("A backup is running: restore once it is done");
    if (["checking", "saving", "staging"].includes(String(st.state))) return Promise.reject("A restore is running already");
    const onDisk = !inSettings(folder);
    const snapshot = snapshotsIn(folder).find((e) => e.name === name) ?? null;
    const mine = ++restoreRun;
    const at = (i: number, f: () => void) => setTimeout(() => { if (mine === restoreRun) { f(); restoreChanged(); } }, i * BACKUP_STEP_MS);
    const files = Number(snapshot?.filesCount ?? 0);
    Object.assign(st, NO_RESTORE, { state: "checking", folder: backupsDir(folder), snapshot, step: "Checking the snapshot" });
    at(1, () => { st.step = "Checking the database"; });
    if (onDisk && snapshot?.reason === "manual") {
      at(2, () => Object.assign(st, { state: "failed", step: null, error: `The snapshot's database file has changed since the snapshot was made: ${st.folder}/snapshots/${name}.db.zst doesn't match its manifest` }));
      return restoreChanged();
    }
    at(2, () => { st.step = `Checking files (${files} of ${files})`; });
    at(3, () => { Object.assign(st, { state: "saving", step: "Saving today's data" }); savingToday(true); });
    at(4, () => { Object.assign(st, { state: "staging", step: "Copying the database", savedToday: savedToday(folder) }); savingToday(false); });
    at(5, () => { st.step = `Copying files (${files} of ${files})`; });
    at(6, () => Object.assign(st, { state: "ready", step: null }));
    return restoreChanged();
  },
  cancel_restore: () => {
    const st = restoreNow();
    if (!st) return empty("cancel_restore", "cancel_restore");
    restoreRun++;
    if (st.state === "saving") savingToday(false);
    Object.assign(st, NO_RESTORE);
    return restoreChanged();
  },
  // The real one quits and starts again, and the start swaps the restore in. Here only the status changes: a reload would
  // read the fixtures again.
  restart_to_restore: () => {
    const st = restoreNow();
    if (st?.state !== "ready") return Promise.reject("No restore is ready: restore a snapshot first");
    const { name: named = "", deviceName: from = "", madeAt = 0, reason } = (st.snapshot ?? {}) as { name?: string; deviceName?: string; madeAt?: number; reason?: string };
    // the fixtures moved its time to now, not the time in its name
    const name = named.replace(/\d{8}-\d{6}-\d{3}/, stamp(madeAt));
    const opened = !(st.folder !== backups()?.folder && reason === "quit");
    const message = opened
      ? `Hersir restored ${name} from ${from}. Today's data was saved as ${st.savedToday} first. The agents are paused: start them again when you're ready.`
      : `Hersir couldn't open the restored data of ${name}, so it started with today's data: the database disk image is malformed.`;
    Object.assign(st, NO_RESTORE, { result: { ok: opened, at: Date.now(), snapshot: name, fromComputer: from, message } });
    const settings = commands.get_settings as Row | undefined;
    if (opened && settings) { settings.agentsPaused = true; changed("settings"); }
    restoreChanged();
    return null;
  },
  dismiss_restore_result: () => {
    const st = restoreNow();
    if (!st) return empty("dismiss_restore_result", "dismiss_restore_result");
    st.result = null;
    return restoreChanged();
  },
};

/** The cards archive_tasks took off the list (until restore_tasks), by id. */
const archivedRows = new Map<string, Row>();

/** A sort key after every card's in column `stateId`. */
function bottomKey(stateId: string): string {
  const keys = ((commands.list_tasks as Row[] | undefined) ?? []).filter((t) => t.stateId === stateId).map((t) => String(t.sortKey)).sort();
  return keyBetween(keys.length ? keys[keys.length - 1] : null, null);
}

/** A bulk command's answer (GA-306): `one` on each card, which answers why it can't go, or null when it went. */
function bulk(ids: string[], one: (t: Row) => string | null) {
  const out: { done: string[]; skipped: { identifier: string; reason: string }[] } = { done: [], skipped: [] };
  for (const id of ids) {
    const t = ((commands.list_tasks as Row[] | undefined) ?? []).find((x) => x.id === id) ?? archivedRows.get(id);
    const why = t ? one(t) : `not found: task ${id}`;
    if (why) out.skipped.push({ identifier: String(t?.identifier ?? id), reason: why });
    else out.done.push(id);
  }
  changed("tasks");
  return out;
}

/** claude_desktop_status's fixture, to change it in place. */
const claudeDesktop = () => commands.claude_desktop_status as Row | undefined;

/** A key for the browser mode's Claude Desktop entry: made up, and no key to any board. */
function madeUpKey(): string {
  const bytes = crypto.getRandomValues(new Uint8Array(16));
  return `browser-demo-${[...bytes].map((b) => b.toString(16).padStart(2, "0")).join("")}`;
}

/** How long each step of a browser-mode backup takes. */
const BACKUP_STEP_MS = 900;

/** backup_status's fixture, to change it in place. */
const backups = () => commands.backup_status as Row | undefined;

/** Says the backups' status changed, and answers it. */
function backupsChanged() {
  const st = structuredClone(backups());
  void emit("backups-changed", st);
  return st;
}

/** When the next scheduled backup is due, as gizai-core's `next_due`: a day or a week after the last, now without one. */
function nextBackup(st: Row): number | null {
  const s = st.settings as Row;
  const every = s.every === "daily" ? 86_400_000 : s.every === "weekly" ? 7 * 86_400_000 : null;
  if (!st.folder || every === null) return null;
  const last = st.last as Row | null;
  return last ? Number(last.madeAt) + every : Date.now();
}

/** restore_status's fixture, to change it in place. */
const restoreNow = () => commands.restore_status as Row | undefined;

/** A status with no restore running or ready (its result stays until it is dismissed). */
const NO_RESTORE = { state: "none", folder: null, snapshot: null, step: null, error: null, savedToday: null };

/** Which browser-mode restore runs now: one that was cancelled does nothing more. */
let restoreRun = 0;

/** Says the restore's status changed, and answers it. */
function restoreChanged() {
  const st = structuredClone(restoreNow());
  void emit("restore-changed", st);
  return st;
}

/** The other folder Restore from another folder… finds here, as there is no folder picker in a browser. */
const USB_DISK = "/media/you/USB";

/** What it holds, made once: work-laptop's backups, so another computer's. Its daily one restores. Two show what can go
 *  wrong: the one made by hand is refused when it is checked, and the one made on quit can't be opened at the start. The
 *  newer Hersir's can't be restored. */
let usbSnapshots: Row[] | null = null;
function usb(): Row[] {
  if (usbSnapshots) return usbSnapshots;
  const all = (commands.list_backups as Row[] | undefined) ?? [];
  const newer = all.find((e) => !e.thisComputer);
  const mine = all.find((e) => e.thisComputer);
  if (!newer || !mine) return (usbSnapshots = structuredClone(all));
  const tag = String(newer.name).split("-").pop();
  const made = (reason: string, madeAt: number) => ({ ...mine, reason, madeAt, name: `gizai-${reason}-${stamp(madeAt)}-${tag}`, thisComputer: false, deviceId: newer.deviceId, deviceName: newer.deviceName });
  const hour = 3_600_000, first = Number(mine.madeAt);
  usbSnapshots = [made("daily", first - hour), made("manual", first - 20 * hour), made("quit", first - 30 * hour), structuredClone(newer)];
  return usbSnapshots;
}

/** Whether `folder` is the one in Settings, as chosen or as the backups folder in use. */
const inSettings = (folder: unknown) => folder == null || folder === (backups()?.settings as Row | undefined)?.folder || folder === backups()?.folder;

/** The snapshots list_backups finds in `folder`. */
const snapshotsIn = (folder: unknown): Row[] => (inSettings(folder) ? ((commands.list_backups as Row[] | undefined) ?? []) : usb());

/** The backups folder `folder` stands for. */
const backupsDir = (folder: unknown) => (inSettings(folder) ? backups()?.folder ?? null : `${folder}/${backupsFolderName}`);

/** While today's data is saved, a backup runs: the backend's before-restore snapshot, in the section's Last and next too. */
function savingToday(running: boolean) {
  const st = backups();
  if (!st) return;
  st.running = running ? { reason: "before-restore", startedAt: Date.now(), step: "database" } : null;
  backupsChanged();
}

/** The before-restore snapshot, made now: in the backups folder in Settings, or else the folder restored from. Its name. */
function savedToday(from: unknown): string {
  const list = backups()?.folder ? ((commands.list_backups as Row[] | undefined) ?? []) : snapshotsIn(from);
  const mine = ((commands.list_backups as Row[] | undefined) ?? []).find((e) => e.thisComputer);
  const madeAt = Date.now();
  const name = `gizai-before-restore-${stamp(madeAt)}-${String(mine?.name ?? "device").split("-").pop()}`;
  if (mine) list.unshift({ ...mine, name, reason: "before-restore", madeAt });
  return name;
}

/** A snapshot name's time, local: "20261027-141506-330". */
function stamp(ms: number): string {
  const d = new Date(ms), p = (n: number, w = 2) => String(n).padStart(w, "0");
  return `${d.getFullYear()}${p(d.getMonth() + 1)}${p(d.getDate())}-${p(d.getHours())}${p(d.getMinutes())}${p(d.getSeconds())}-${p(d.getMilliseconds(), 3)}`;
}

/** The answer to one command: a fixture, a write, a screenshot's picture, or empty. */
export async function answer(cmd: string, args: Row): Promise<unknown> {
  if (cmd in WRITES) return WRITES[cmd](args);
  if (cmd === "screenshot_bytes") return picture(String(args.fileId));
  if (cmd === "list_tasks") return tasks(args.filter as Row | undefined);
  if (cmd === "item_id") return itemId(String(args.kind), String(args.key));
  if (cmd === "project_time") return projectTime(args);
  // the folder in Settings: none once Backups is turned off
  if (cmd === "list_backups" && args.folder == null && backups() && !backups()!.folder) return [];
  if (cmd === "list_backups" && !inSettings(args.folder)) return structuredClone(usb());
  if (cmd === "plugin:dialog|open" && (args.options as Row | undefined)?.title === PICK_TITLE) return USB_DISK;
  const f = fixture(cmd, args);
  return f.found ? structuredClone(f.value) : empty(cmd, f.why);
}

/** list_tasks: every card in the fixtures, filtered as the backend does by project and open ones only. */
function tasks(filter: Row | undefined) {
  const all = commands.list_tasks as Row[] | undefined;
  if (!all) return empty("list_tasks", "list_tasks");
  return structuredClone(all.filter((t) => (!filter?.projectId || t.projectId === filter.projectId)
    && (!filter?.openOnly || !["done", "cancelled"].includes(String(t.stateCategory)))));
}

const DAY = 86_400_000;

/** project_time (GA-354), as gizai-core counts it, from the fixtures' runs (list_runs) of the project's cards (its fixture):
 *  the runs that started from `since` up to `until`, or all of them, each on the UTC day it started; an interrupted run at
 *  most the default run time limit, 90 minutes. In a period only the cards with a run in it, and every day of it; for all
 *  time every card, and the days from the first run to today. */
function projectTime(args: Row): unknown {
  const f = fixture("project_time", args);
  if (!f.found) return empty("project_time", f.why);
  const since = typeof args.since === "number" ? args.since : null, until = typeof args.until === "number" ? args.until : null;
  const period = since != null && until != null;
  const utcDay = (ms: number) => ms - (((ms % DAY) + DAY) % DAY);
  type Sums = { runs: number; timeMs: number; inputTokens: number; outputTokens: number; costUsdMicros: number; unknownCostRuns: number };
  const zero = (): Sums => ({ runs: 0, timeMs: 0, inputTokens: 0, outputTokens: 0, costUsdMicros: 0, unknownCostRuns: 0 });
  const byDay = new Map<number, Sums>();
  let firstRun: number | null = null;
  const cards = ((f.value as { cards?: Row[] }).cards ?? []).map((c) => {
    const card = { ...c, ...zero(), runningSince: null as number | null };
    const runs = fixture("list_runs", { taskId: c.taskId });
    for (const r of (runs.found ? runs.value : []) as Row[]) {
      const start = typeof r.startedAt === "number" ? r.startedAt : null;
      if (start == null) continue;
      firstRun = firstRun == null ? start : Math.min(firstRun, start);
      if (period && (start < since || start >= until)) continue;
      const end = typeof r.endedAt === "number" ? r.endedAt : null;
      const span = end == null ? 0 : Math.max(0, end - start);
      const input = Number(r.inputTokens ?? 0), output = Number(r.outputTokens ?? 0), cost = Number(r.costUsdMicros ?? 0);
      const add: Sums = { runs: 1, timeMs: r.error === "interrupted" ? Math.min(span, 90 * 60_000) : span, inputTokens: input, outputTokens: output,
        costUsdMicros: cost, unknownCostRuns: cost === 0 && input + output > 0 ? 1 : 0 };
      const day = byDay.get(utcDay(start)) ?? zero();
      for (const k of Object.keys(add) as (keyof Sums)[]) { card[k] += add[k]; day[k] += add[k]; }
      byDay.set(utcDay(start), day);
      if (end == null) card.runningSince = card.runningSince == null ? start : Math.min(card.runningSince, start);
    }
    return card;
  });
  const [from, to] = period ? [since, until] : firstRun == null ? [0, 0] : [utcDay(firstRun), utcDay(Date.now()) + DAY];
  const days = [];
  for (let d = from; d < to; d += DAY) days.push({ dayStart: d, ...(byDay.get(d) ?? zero()) });
  return { since: period ? since : null, until: period ? until : null, firstRun, cards: period ? cards.filter((c) => c.runs > 0) : cards, days };
}

/** item_id, as the backend: a card's id by its id or identifier, a project's by its id or key (case ignored); any other kind
 *  answers its key. One that matches nothing fails. */
function itemId(kind: string, key: string): Promise<string> | string {
  if (kind === "task") {
    const t = ((commands.list_tasks as Row[] | undefined) ?? []).find((x) => x.id === key || x.identifier === key);
    return t ? String(t.id) : Promise.reject(`No task ${key}`);
  }
  if (kind === "project") {
    const p = ((commands.list_projects as Row[] | undefined) ?? []).find((x) => x.id === key || String(x.key).toLowerCase() === key.toLowerCase());
    return p ? String(p.id) : Promise.reject(`No project with the key ${key}`);
  }
  return key;
}

/** A screenshot's bytes: a picture drawn from its size and caption (the fixtures hold no images). */
function picture(fileId: string): Promise<ArrayBuffer> {
  const sets = commands.list_screenshots;
  const all = isKeyed(sets) ? Object.values(sets.$answers).flat() as { shots: { fileId: string; name: string; caption: string; width: number; height: number }[] }[] : [];
  const shot = all.flatMap((s) => s.shots).find((s) => s.fileId === fileId);
  if (!shot) return Promise.reject("That file isn't a run's screenshot: only screenshots are shown here.");
  return screenshotPicture(shot);
}
