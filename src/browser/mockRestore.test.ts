// GA-219 QA: Restore in the browser mode. src/browser/fixtures.json has restore_status (state none, written by
// crates/gizai-core/examples/fixtures.rs), and the fake backend in mock.ts answers as GA-218's commands do: the steps a moment
// each with `restore-changed`, ready, refusals with the backend's reasons, Cancel restore, Restart now's result and Dismiss.
// Restore from another folder… finds a made-up USB disk, with a backup that restores, one refused when it is checked and one
// that fails at the start.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { PICK_TITLE, snapshotMadeAt } from "../lib/restore";
import type { BackupEntry, BackupStatus, RestoreStatus, Settings } from "../types";

const emitted = vi.hoisted(() => [] as [string, unknown][]);
vi.mock("@tauri-apps/api/event", () => ({ emit: async (name: string, payload: unknown) => { emitted.push([name, structuredClone(payload)]); } }));
vi.mock("./pictures", () => ({ screenshotPicture: async () => new ArrayBuffer(0) }));

const fixtures = (await import("./fixtures.json")).default as unknown as import("./mock").Fixtures;
const { answer, loadFixtures } = await import("./mock");

const STEP_MS = 900;
const USB = "/media/you/USB";
const status = async () => (await answer("restore_status", {})) as RestoreStatus;
const inSettings = async () => (await answer("list_backups", { folder: null })) as BackupEntry[];
const onUsb = async () => (await answer("list_backups", { folder: USB })) as BackupEntry[];
const backup = async () => (await answer("backup_status", {})) as BackupStatus;
const events = () => emitted.filter(([n]) => n === "restore-changed").map(([, p]) => p as RestoreStatus);
const start = (folder: string, name: string) => answer("start_restore", { folder, name }) as Promise<RestoreStatus>;
/** Starts a restore of the backup with this reason on the USB disk, and goes through its steps. */
async function usbReason(reason: string) {
  const e = (await onUsb()).find((x) => x.reason === reason && x.restorable)!;
  await start(USB, e.name);
  await vi.advanceTimersByTimeAsync(10 * STEP_MS);
  return e;
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 9, 27, 14, 0));
  vi.spyOn(console, "info").mockImplementation(() => {});
  vi.spyOn(console, "warn").mockImplementation(() => {});
  emitted.length = 0;
  loadFixtures(structuredClone(fixtures));
});
afterEach(() => { vi.useRealTimers(); vi.restoreAllMocks(); });

describe("the fixtures (fixtures.rs → fixtures.json)", () => {
  it("restore_status: none runs, none is ready and none was just done", async () => {
    expect(await status()).toEqual({ state: "none", folder: null, snapshot: null, step: null, error: null, savedToday: null, result: null });
  });
});

describe("Restore from another folder… in the browser", () => {
  it("the folder picker finds the USB disk for Restore, and answers nothing for other pickers", async () => {
    expect(await answer("plugin:dialog|open", { options: { title: PICK_TITLE, directory: true, multiple: false } })).toBe(USB);
    expect(await answer("plugin:dialog|open", { options: { title: "Choose a folder for backups", directory: true, multiple: false } })).not.toBe(USB);
  });

  it("the disk holds another computer's backups: daily, by hand and on quit restorable, the newer Hersir's not", async () => {
    const l = await onUsb();
    expect(l.map((e) => [e.reason, e.restorable])).toEqual([["daily", true], ["manual", true], ["quit", true], ["daily", false]]);
    expect(l.every((e) => !e.thisComputer && e.deviceName === "work-laptop")).toBe(true);
    expect(l[3]!.problem).toMatch(/^A newer Hersir made it/);
    expect(new Set(l.map((e) => e.name)).size).toBe(l.length);
    for (const e of l) expect(e.name).toMatch(new RegExp(`^gizai-${e.reason}-\\d{8}-\\d{6}-\\d{3}-[0-9a-f]+$`));
  });

  it("the list in Settings stays what it was", async () => {
    await onUsb();
    expect(await inSettings()).toHaveLength(3);
  });

  it("the disk is still there after the backups folder is turned off", async () => {
    const b = await backup();
    await answer("save_backup_settings", { settings: { ...b.settings, folder: null } });
    expect(await inSettings()).toEqual([]);
    expect(await onUsb()).toHaveLength(4);
  });
});

describe("start_restore in the browser", () => {
  it("takes the backend's steps one by one with restore-changed, then is ready with no step", async () => {
    const e = (await inSettings()).find((x) => x.restorable)!;
    const first = await start("/home/you/Dropbox", e.name);
    expect(first).toMatchObject({ state: "checking", step: "Checking the snapshot" });
    await vi.advanceTimersByTimeAsync(7 * STEP_MS);
    const n = e.filesCount;
    expect(events().map((s) => [s.state, s.step])).toEqual([
      ["checking", "Checking the snapshot"], ["checking", "Checking the database"], ["checking", `Checking files (${n} of ${n})`],
      ["saving", "Saving today's data"], ["staging", "Copying the database"], ["staging", `Copying files (${n} of ${n})`], ["ready", null],
    ]);
    expect(await status()).toEqual(events().at(-1));
    expect(events().at(-1)).toMatchObject({ folder: "/home/you/Dropbox/Hersir backups", snapshot: { name: e.name }, error: null, result: null });
  });

  it("while today's data is saved a before-restore backup runs, and then is the newest row with savedToday's name", async () => {
    const before = await inSettings();
    const e = before.find((x) => x.restorable)!;
    await start("/home/you/Dropbox", e.name);
    await vi.advanceTimersByTimeAsync(3 * STEP_MS);
    expect((await status()).state).toBe("saving");
    expect((await backup()).running).toMatchObject({ reason: "before-restore" });
    await vi.advanceTimersByTimeAsync(STEP_MS);
    expect((await backup()).running).toBeNull();
    const after = await inSettings();
    expect(after).toHaveLength(before.length + 1);
    expect(after[0]).toMatchObject({ reason: "before-restore", thisComputer: true });
    expect((await status()).savedToday).toBe(after[0]!.name);
  });

  it("another computer's from the disk: its backups folder, and today's data goes to the folder in Settings", async () => {
    const before = await inSettings();
    const e = await usbReason("daily");
    expect(await status()).toMatchObject({ state: "ready", folder: `${USB}/Hersir backups`, snapshot: { name: e.name, deviceName: "work-laptop" } });
    expect(await inSettings()).toHaveLength(before.length + 1);
    expect(await onUsb()).toHaveLength(4);
  });

  it("with no folder in Settings, today's data goes to the folder restored from", async () => {
    const b = await backup();
    await answer("save_backup_settings", { settings: { ...b.settings, folder: null } });
    await usbReason("daily");
    expect((await status()).state).toBe("ready");
    const l = await onUsb();
    expect(l).toHaveLength(5);
    expect(l[0]).toMatchObject({ reason: "before-restore" });
    expect((await status()).savedToday).toBe(l[0]!.name);
  });

  it("a backup refused when it is checked fails with the reason, and today's data isn't saved", async () => {
    const before = await inSettings();
    await usbReason("manual");
    const s = await status();
    expect(s.state).toBe("failed");
    expect(s.step).toBeNull();
    expect(s.error).toMatch(/^The snapshot's database file has changed since the snapshot was made: \/media\/you\/USB\/Hersir backups\/snapshots\/.+\.db\.zst doesn't match its manifest$/);
    expect(await inSettings()).toHaveLength(before.length);
    expect((await backup()).running).toBeNull();
  });

  it("is refused while a backup runs, with the backend's reason, and nothing starts", async () => {
    await answer("back_up_now", {});
    const e = (await inSettings()).find((x) => x.restorable)!;
    await expect(start("/home/you/Dropbox", e.name)).rejects.toBe("A backup is running: restore once it is done");
    expect((await status()).state).toBe("none");
    expect(events()).toEqual([]);
  });

  it("is refused while another restore runs, with the backend's reason", async () => {
    const e = (await inSettings()).find((x) => x.restorable)!;
    await start("/home/you/Dropbox", e.name);
    await expect(start("/home/you/Dropbox", e.name)).rejects.toBe("A restore is running already");
  });

  it("a restore that is ready goes: a new one takes its place", async () => {
    await usbReason("daily");
    const e = (await inSettings()).find((x) => x.reason === "daily" && x.restorable)!;
    const s = await start("/home/you/Dropbox", e.name);
    expect(s).toMatchObject({ state: "checking", savedToday: null });
    await vi.advanceTimersByTimeAsync(10 * STEP_MS);
    expect(await status()).toMatchObject({ state: "ready", snapshot: { name: e.name } });
  });
});

describe("cancel_restore in the browser", () => {
  it("of one that runs: none, no more steps, and a backup that was saving stops", async () => {
    const e = (await inSettings()).find((x) => x.restorable)!;
    await start("/home/you/Dropbox", e.name);
    await vi.advanceTimersByTimeAsync(3 * STEP_MS);
    expect((await backup()).running).not.toBeNull();
    expect(await answer("cancel_restore", {})).toMatchObject({ state: "none", step: null, snapshot: null });
    const said = events().length;
    await vi.advanceTimersByTimeAsync(10 * STEP_MS);
    expect(events()).toHaveLength(said);
    expect((await status()).state).toBe("none");
    expect((await backup()).running).toBeNull();
  });

  it("of one that is ready, or failed: none, and it says so", async () => {
    await usbReason("daily");
    await answer("cancel_restore", {});
    expect(events().at(-1)).toMatchObject({ state: "none", folder: null, snapshot: null, savedToday: null });
    await usbReason("manual");
    expect((await status()).state).toBe("failed");
    await answer("cancel_restore", {});
    expect(await status()).toMatchObject({ state: "none", error: null });
  });
});

describe("restart_to_restore and the result in the browser", () => {
  it("is refused with the backend's reason while no restore is ready", async () => {
    await expect(answer("restart_to_restore", {})).rejects.toBe("No restore is ready: restore a snapshot first");
  });

  it("ok: the result says which backup and computer, the backend's message, agents paused, and the restore is gone", async () => {
    const e = await usbReason("daily");
    expect(await answer("restart_to_restore", {})).toBeNull();
    const s = await status();
    expect(s).toMatchObject({ state: "none", folder: null, snapshot: null, savedToday: null });
    expect(s.result).toMatchObject({ ok: true, at: Date.now(), fromComputer: "work-laptop" });
    expect(s.result!.message).toMatch(/^Hersir restored gizai-daily-.+ from work-laptop\. Today's data was saved as gizai-before-restore-.+ first\. The agents are paused: start them again when you're ready\.$/);
    // the time in its name is the one its row shows, so the result says the same time as the list
    expect(snapshotMadeAt(s.result!.snapshot)).toBe(e.madeAt);
    expect(((await answer("get_settings", {})) as Settings).agentsPaused).toBe(true);
    expect(events().at(-1)).toEqual(s);
  });

  it("the backup that can't be opened at the start: a failed result, and agents keep going", async () => {
    await usbReason("quit");
    await answer("restart_to_restore", {});
    const s = await status();
    expect(s.result).toMatchObject({ ok: false, fromComputer: "work-laptop" });
    expect(s.result!.message).toMatch(/^Hersir couldn't open the restored data of gizai-quit-.+, so it started with today's data: the database disk image is malformed\.$/);
    expect(((await answer("get_settings", {})) as Settings).agentsPaused).toBe(false);
  });

  it("the result stays until dismiss_restore_result", async () => {
    await usbReason("daily");
    await answer("restart_to_restore", {});
    expect((await status()).result).not.toBeNull();
    await vi.advanceTimersByTimeAsync(10 * STEP_MS);
    expect((await status()).result).not.toBeNull();
    expect(await answer("dismiss_restore_result", {})).toMatchObject({ state: "none", result: null });
    expect(events().at(-1)!.result).toBeNull();
  });

  it("a new restore keeps a result that isn't dismissed", async () => {
    await usbReason("daily");
    await answer("restart_to_restore", {});
    const { result } = await status();
    await usbReason("daily");
    expect(await status()).toMatchObject({ state: "ready", result });
  });

  it("reading the fixtures again forgets it all, also a restore that was running", async () => {
    await usbReason("daily");
    await answer("restart_to_restore", {});
    const e = (await inSettings()).find((x) => x.restorable)!;
    await start("/home/you/Dropbox", e.name);
    loadFixtures(structuredClone(fixtures));
    const said = events().length;
    await vi.advanceTimersByTimeAsync(10 * STEP_MS);
    expect(events()).toHaveLength(said);
    expect((await status()).state).toBe("none");
    expect((await status()).result).toBeNull();
  });
});
