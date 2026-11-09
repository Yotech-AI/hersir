// GA-219 QA: Restore in words (src/lib/restore.ts), against the card's texts word for word: the confirm with its reason, time,
// computer and folder; which folder today's data goes to, as src-tauri's save_today picks it; the ready notice with the time of
// the backup; a failure; and the result after the start, with the time read from the snapshot's name.
import { describe, expect, it } from "vitest";
import { confirmText, failedText, isRestoring, readyText, restoreStep, resultText, saveFolder, snapshotMadeAt } from "./restore";
import type { BackupEntry, RestoreStatus } from "../types";

// local times, so "today" and "yesterday" don't depend on the time zone the tests run in
const NOW = new Date(2026, 9, 27, 14, 0).getTime();
const at = (day: number, hh: number, mm = 0) => new Date(2026, 9, day, hh, mm).getTime();
const mine = { reason: "daily", madeAt: at(27, 11), thisComputer: true, deviceName: "pc" } as Pick<BackupEntry, "reason" | "madeAt" | "thisComputer" | "deviceName">;
const AFTER = "After a restart, Hersir shows your data as it was then: what changed since, also until the restart, is not kept. "
  + "Keys and passwords stay as they are on this computer. Agents are paused afterwards.";
const result = (over: Partial<NonNullable<RestoreStatus["result"]>>) =>
  ({ ok: true, at: NOW, snapshot: "gizai-daily-20261027-110000-000-ce82a749", fromComputer: "work-laptop", message: "", ...over });

describe("the confirm's text", () => {
  it("this computer's backup: the card's text word for word", () => {
    expect(confirmText(mine, "/home/you/Dropbox/Hersir backups", NOW)).toBe(
      `Daily, today 11:00, from this computer. Hersir first saves today's data as a backup in /home/you/Dropbox/Hersir backups. ${AFTER}`);
  });
  it("another computer's says which", () => {
    expect(confirmText({ ...mine, reason: "manual", madeAt: at(26, 9), thisComputer: false, deviceName: "work-laptop" }, "/media/usb", NOW)).toBe(
      `By hand, yesterday 09:00, from work-laptop. Hersir first saves today's data as a backup in /media/usb. ${AFTER}`);
  });
  it("a computer Hersir couldn't name is another computer, as in the list", () => {
    for (const deviceName of ["", "  ", "this machine"]) {
      expect(confirmText({ ...mine, thisComputer: false, deviceName }, "/x", NOW)).toMatch(/^Daily, today 11:00, from another computer\. /);
    }
  });
  it.each([["daily", "Daily"], ["weekly", "Weekly"], ["manual", "By hand"], ["quit", "On quit"], ["before-restore", "Before a restore"]] as const)(
    "reason %s reads %s", (reason, words) => {
      expect(confirmText({ ...mine, reason }, "/x", NOW).startsWith(`${words}, `)).toBe(true);
    });
});

describe("the folder today's data goes to, as save_today picks it", () => {
  it("the backups folder in Settings", () => {
    expect(saveFolder({ folder: "/home/you/Dropbox/Hersir backups", folderProblem: null }, "/media/usb")).toBe("/home/you/Dropbox/Hersir backups");
  });
  it("the folder restored from when none is set", () => {
    expect(saveFolder({ folder: null, folderProblem: null }, "/media/usb")).toBe("/media/usb");
  });
  it("the folder restored from when the one in Settings can't be used now", () => {
    expect(saveFolder({ folder: "/media/gone/Hersir backups", folderProblem: "The folder isn't there: is the disk plugged in?" }, "/media/usb")).toBe("/media/usb");
  });
});

describe("the steps", () => {
  it("a restore runs while it checks, saves today's data or copies, and not otherwise", () => {
    const running = (state: RestoreStatus["state"]) => isRestoring({ state });
    expect(["checking", "saving", "staging"].every((s) => running(s as RestoreStatus["state"]))).toBe(true);
    expect(["none", "ready", "failed"].some((s) => running(s as RestoreStatus["state"]))).toBe(false);
    expect(isRestoring(null)).toBe(false);
  });
  it("show as the backend says them", () => {
    expect(restoreStep({ step: "Copying files (120 of 300)" })).toBe("Copying files (120 of 300)…");
    expect(restoreStep({ step: null })).toBe("Starting the restore…");
  });
  it("a failure says why, also when the backend doesn't", () => {
    expect(failedText("Made by a newer Hersir (backup format 2). Update Hersir first.")).toBe("The restore failed: Made by a newer Hersir (backup format 2). Update Hersir first.");
    expect(failedText(null)).toBe("The restore failed: Hersir didn't say why.");
  });
});

describe("ready", () => {
  it("names the time of the backup, not of the restore", () => {
    expect(readyText({ snapshot: { madeAt: at(26, 9) } as BackupEntry }, NOW)).toBe("Restore ready: yesterday 09:00");
  });
  it("a restore staged before this start whose snapshot can't be found now (the USB disk is out) still says ready", () => {
    expect(readyText({ snapshot: null }, NOW)).toBe("Restore ready");
  });
});

describe("the time in a snapshot's name", () => {
  it("is read as the local time db::stamp wrote", () => {
    expect(snapshotMadeAt("gizai-daily-20261027-141506-330-ce82a749")).toBe(new Date(2026, 9, 27, 14, 15, 6, 330).getTime());
  });
  it("also for a reason with a hyphen in it", () => {
    expect(snapshotMadeAt("gizai-before-restore-20261020-120000-000-ce82a749")).toBe(at(20, 12));
  });
  it.each(["", "gizai-daily", "gizai-daily-2026-1400-ce82a749", "other-daily-20261027-141506-330-ce82a749", "gizai-daily-20261027-141506-330"])("not in %j", (name) => {
    expect(snapshotMadeAt(name)).toBeNull();
  });
});

describe("the result after the start", () => {
  it("ok: the card's text word for word, with the time from the snapshot's name", () => {
    expect(resultText(result({}), NOW)).toBe(
      "Restored the backup of today 11:00 from work-laptop. Agents are paused: resume them in Settings → Agents and runs. Today's data was saved as a backup first.");
  });
  it("ok: the time of the backup, not of the restart (`at`)", () => {
    expect(resultText(result({ at: at(27, 13), snapshot: "gizai-manual-20261026-090000-000-ce82a749" }), NOW)).toContain("the backup of yesterday 09:00 from");
  });
  it("ok without a computer's name, or with a name that isn't a snapshot's, still reads", () => {
    expect(resultText(result({ fromComputer: "  " }), NOW)).toContain("today 11:00 from another computer. ");
    expect(resultText(result({ snapshot: "some-name" }), NOW)).toContain("the backup of some-name from work-laptop. ");
  });
  it("failed: the card's text word for word", () => {
    expect(resultText(result({ ok: false, message: "the disk is full" }), NOW)).toBe("The restore failed: the disk is full. Hersir started with today's data.");
  });
  it("failed: the backend's message ends in a full stop, and the text has one there", () => {
    const message = "Hersir couldn't open the restored data of gizai-daily-20261027-110000-000-ce82a749, so it started with today's data: file is not a database.";
    expect(resultText(result({ ok: false, message }), NOW)).toBe(
      "The restore failed: Hersir couldn't open the restored data of gizai-daily-20261027-110000-000-ce82a749, so it started with today's data: file is not a database. Hersir started with today's data.");
  });
});
