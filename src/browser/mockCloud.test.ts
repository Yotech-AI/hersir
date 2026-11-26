// GA-370 QA: the browser mode's Hersir Cloud (mockCloud.ts) makes a new recovery code as the app now does: Make a new
// recovery code shows HC-3MWT-9QXE-7RKD-B2NA-6PLF-X7KD and changes nothing until its last group, X7KD, is typed back
// (cloud_confirm_recovery). Then it replaces the old code, and Cloud Backup's settings stay. A wrong group changes
// nothing; signing out before that drops it, and the old code keeps working. Each test loads the mock afresh, signed in
// on a computer with the key (?cloud=signed-in).
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { CloudStatus } from "../types";

vi.mock("@tauri-apps/api/event", () => ({ emit: async () => {} }));

const CODE = "HC-7K2Q-M4XD-9TRP-B6WN-3HFJ-Q8YC";
const NEW_CODE = "HC-3MWT-9QXE-7RKD-B2NA-6PLF-X7KD";
const WRONG_GROUP = "That isn't the code's last group. Look at the code above again.";
const WRONG_CODE = "That recovery code doesn't open this account's backups.";

type Ask = (cmd: string, args?: Record<string, unknown>) => Promise<unknown>;
async function load(): Promise<Ask> {
  vi.resetModules();
  const { cloudAnswer } = await import("./mockCloud");
  const hooks = { status: () => undefined, changed: () => undefined };
  return async (cmd, args = {}) => cloudAnswer(cmd, args, hooks);
}
const status = async (ask: Ask) => (await ask("cloud_status")) as CloudStatus;
/** Signs out and in again: the computer then needs the account's code. */
async function signInAgain(ask: Ask) {
  await ask("cloud_sign_out");
  const signingIn = ask("cloud_sign_in");
  await vi.advanceTimersByTimeAsync(3000);
  expect(((await signingIn) as CloudStatus).key).toBe("needed");
}

beforeEach(() => {
  vi.useFakeTimers();
  vi.setSystemTime(new Date(2026, 10, 26, 14, 0));
  vi.stubGlobal("location", { search: "?cloud=signed-in" });
});
afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals(); });

describe("the browser mode's new recovery code", () => {
  it("changes nothing until its last group is typed back; then it replaces the old code and the settings stay", async () => {
    const ask = await load();
    const before = await status(ask);
    expect(before.key).toBe("here");
    expect(await ask("cloud_new_recovery")).toEqual({ recoveryCode: NEW_CODE });
    expect(await status(ask)).toEqual(before);
    await expect(ask("cloud_confirm_recovery", { lastGroup: "Q8YC" })).rejects.toBe(WRONG_GROUP);
    expect(await status(ask)).toEqual(before);

    vi.setSystemTime(new Date(2026, 10, 26, 14, 5));
    const after = (await ask("cloud_confirm_recovery", { lastGroup: " x7kd " })) as CloudStatus;
    expect(after.recoveryMadeAt).toBe(new Date(2026, 10, 26, 14, 5).getTime());
    expect(after.backup).toEqual(before.backup);
    expect(after.key).toBe("here");
    await expect(ask("cloud_confirm_recovery", { lastGroup: "X7KD" })).rejects.toBe("Make a recovery code first.");

    await signInAgain(ask);
    await expect(ask("cloud_enter_recovery", { code: CODE })).rejects.toBe(WRONG_CODE);
    expect(((await ask("cloud_enter_recovery", { code: NEW_CODE })) as CloudStatus).key).toBe("here");
  });

  it("signing out before its last group is typed back drops it, and the old code keeps working", async () => {
    const ask = await load();
    await ask("cloud_new_recovery");
    await signInAgain(ask);
    await expect(ask("cloud_confirm_recovery", { lastGroup: "X7KD" })).rejects.toBe("Make a recovery code first.");
    await expect(ask("cloud_enter_recovery", { code: NEW_CODE })).rejects.toBe(WRONG_CODE);
    expect(((await ask("cloud_enter_recovery", { code: CODE })) as CloudStatus).key).toBe("here");
  });
});
