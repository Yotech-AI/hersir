import { describe, expect, it } from "vitest";
import { canStop, checkLine, isRunning, notice, packageHint, packageText, stepText } from "./update";
import type { Release, UpdateJob, UpdateStatus, UpdateStep } from "../types";

const release: Release = { version: "0.1.6", tag: "v0.1.6", url: "https://github.com/Yotech-AI/hersir/releases/tag/v0.1.6", notes: "- Fixes" };
const status = (more: Partial<UpdateStatus> = {}): UpdateStatus => ({ current: "0.1.5", autoCheck: true, checking: false, repo: "https://github.com/Yotech-AI/hersir.git", ...more });
const job = (step: UpdateStep, more: Partial<UpdateJob> = {}): UpdateJob =>
  ({ version: "0.1.6", step, startedAt: 1, log: "/data/update/update.log", unchanged: true, ...more });
const NOW = 1_800_000_000_000;

describe("the notice above Company", () => {
  it("is not there when nothing newer is out", () => {
    expect(notice(status())).toBeNull();
    expect(notice(status({ latest: { ...release, version: "0.1.5", tag: "v0.1.5" }, checkedAt: NOW }))).toBeNull();
  });
  it("offers Update to <version> for a newer release", () => {
    expect(notice(status({ latest: release, available: release }))).toEqual({ kind: "offer", version: "0.1.6", text: "Update to 0.1.6", canInstall: true });
  });
  it("still names the release on a Gizai that can't update itself, without installing", () => {
    const n = notice(status({ available: release, cannotInstall: "This Gizai runs from /repo/target/release/gizai, not from an install" }));
    expect(n).toEqual({ kind: "offer", version: "0.1.6", text: "Update to 0.1.6", canInstall: false });
  });
  it("says where a running update is", () => {
    expect(notice(status({ available: release, job: job("source") }))).toEqual({ kind: "working", version: "0.1.6", text: "Getting 0.1.6…" });
    expect(notice(status({ available: release, job: job("build") }))?.text).toBe("Building 0.1.6…");
    expect(notice(status({ available: release, job: job("backup") }))?.text).toBe("Backing up your data…");
    expect(notice(status({ available: release, job: job("install") }))?.text).toBe("Installing 0.1.6…");
  });
  it("offers the restart once it is installed, or when a newer version was installed meanwhile", () => {
    expect(notice(status({ available: release, installed: "0.1.6", job: job("installed", { unchanged: false }) })))
      .toEqual({ kind: "restart", version: "0.1.6", text: "Restart to use 0.1.6" });
    expect(notice(status({ installed: "0.1.7" }))).toEqual({ kind: "restart", version: "0.1.7", text: "Restart to use 0.1.7" });
  });
  it("says a failed update failed, and offers a newer release than the one that failed", () => {
    expect(notice(status({ available: release, job: job("failed", { problem: "./install.sh --build-only failed (exit code 101)" }) })))
      .toEqual({ kind: "failed", version: "0.1.6", text: "Update to 0.1.6 failed" });
    const newer = { ...release, version: "0.1.7", tag: "v0.1.7" };
    expect(notice(status({ available: newer, job: job("failed") }))?.kind).toBe("offer");
  });
  it("offers the update again after it was stopped", () => {
    expect(notice(status({ available: release, job: job("stopped") }))).toEqual({ kind: "offer", version: "0.1.6", text: "Update to 0.1.6", canInstall: true });
  });
});

describe("an update's steps", () => {
  it("runs while it gets the source, builds, backs up and installs", () => {
    for (const s of ["source", "build", "backup", "install"] as const) expect(isRunning(job(s))).toBe(true);
    for (const s of ["installed", "failed", "stopped"] as const) expect(isRunning(job(s))).toBe(false);
  });
  it("can be stopped only while it gets the source or builds", () => {
    expect(canStop(job("source"))).toBe(true);
    expect(canStop(job("build"))).toBe(true);
    for (const s of ["backup", "install", "installed", "failed", "stopped"] as const) expect(canStop(job(s))).toBe(false);
  });
  it("says each step in a few words", () => {
    expect(stepText(job("installed"))).toBe("Installed 0.1.6");
    expect(stepText(job("failed"))).toBe("Update to 0.1.6 failed");
    expect(stepText(job("stopped"))).toBe("Update to 0.1.6 stopped");
  });
});

describe("Settings → Updates: what the last check found", () => {
  it("says no check ran yet, or that one runs now", () => {
    expect(checkLine(status(), NOW)).toEqual({ mark: "skipped", text: "Not checked yet." });
    expect(checkLine(status({ checking: true, checkedAt: NOW - 1000 }), NOW)).toEqual({ mark: "skipped", text: "Asking GitHub for the latest release…" });
  });
  it("says this is the latest version, with when it checked", () => {
    expect(checkLine(status({ checkedAt: NOW - 3 * 3600_000, latest: { ...release, version: "0.1.5" } }), NOW))
      .toEqual({ mark: "ok", text: "This is the latest version (checked 3h ago)." });
  });
  it("says a newer version is out, or installed and waiting for a restart", () => {
    expect(checkLine(status({ checkedAt: NOW, latest: release, available: release }), NOW)).toEqual({ mark: "new", text: "Version 0.1.6 is out (checked just now)." });
    expect(checkLine(status({ checkedAt: NOW, latest: release, available: release, installed: "0.1.6" }), NOW))
      .toEqual({ mark: "new", text: "Version 0.1.6 is installed. Restart Hersir to use it." });
  });
  it("says there is no release yet", () => {
    expect(checkLine(status({ checkedAt: NOW }), NOW)).toEqual({ mark: "skipped", text: "There is no release on GitHub yet (checked just now)." });
  });
  it("says why the last check didn't work, with what to do", () => {
    expect(checkLine(status({ checkedAt: NOW - 120_000, available: release, problem: { what: "Can't reach GitHub", fix: "Check your internet connection, then try again." } }), NOW))
      .toEqual({ mark: "failed", text: "The last check didn't work (checked 2m ago): Can't reach GitHub. Check your internet connection, then try again." });
    expect(checkLine(status({ checkedAt: NOW, problem: { what: "GitHub answered the release check with HTTP 500" } }), NOW).text)
      .toBe("The last check didn't work (checked just now): GitHub answered the release check with HTTP 500");
  });
});

describe("GA-253: signed releases and ready-made builds", () => {
  it("doesn't offer a release that isn't signed yet, and says so in Settings → Updates", () => {
    expect(notice(status({ checkedAt: NOW, latest: release, unsigned: release }))).toBeNull();
    expect(checkLine(status({ checkedAt: NOW, latest: release, unsigned: release }), NOW))
      .toEqual({ mark: "skipped", text: "v0.1.6 is out, but not signed yet." });
  });
  it("says a newer version installed meanwhile, or a failed check, before an unsigned release", () => {
    expect(checkLine(status({ checkedAt: NOW, latest: release, unsigned: release, installed: "0.1.6" }), NOW).text)
      .toBe("Version 0.1.6 is installed. Restart Hersir to use it.");
    expect(checkLine(status({ checkedAt: NOW, unsigned: release, problem: { what: "Can't reach GitHub" } }), NOW).mark).toBe("failed");
  });
  it("shows the download as a step that runs, can be stopped and says Downloading", () => {
    expect(isRunning(job("download"))).toBe(true);
    expect(canStop(job("download"))).toBe(true);
    expect(stepText(job("download"))).toBe("Downloading 0.1.6…");
    expect(notice(status({ available: release, job: job("download") }))).toEqual({ kind: "working", version: "0.1.6", text: "Downloading 0.1.6…" });
  });
});

// GA-200: a Hersir from the AUR, Homebrew or Scoop doesn't update itself. The texts are the card's, word for word; the
// cannotInstall texts are GA-199's contract (the backend sends them, the notice only shows them as its tooltip).
describe("GA-200: a Hersir from a package manager", () => {
  const CANNOT = {
    aur: "Hersir came from the AUR, so it doesn't update itself. Update it with yay -Syu, then restart Hersir.",
    homebrew: "Hersir came from Homebrew, so it doesn't update itself. Update it with brew update && brew upgrade hersir, then restart Hersir.",
    scoop: "Hersir came from Scoop, so it doesn't update itself. Update it with scoop update; scoop update hersir, then restart Hersir.",
    nix: "Hersir came from a package, so it doesn't update itself. Update it with the package manager that installed it, then restart Hersir.",
  };
  const COMMAND = { aur: "yay -Syu", homebrew: "brew update && brew upgrade hersir", scoop: "scoop update; scoop update hersir", nix: null };
  const pkg = (p: keyof typeof CANNOT, more: Partial<UpdateStatus> = {}) =>
    status({ package: p, updateCommand: COMMAND[p], installTo: null, cannotInstall: CANNOT[p], ...more });

  it("the notice says Hersir <version> is out instead of Update to <version>, and doesn't install", () => {
    for (const p of ["aur", "homebrew", "scoop", "nix"] as const) {
      expect(notice(pkg(p, { latest: release, available: release }))).toEqual({ kind: "offer", version: "0.1.6", text: "Hersir 0.1.6 is out", canInstall: false });
    }
  });
  it("the notice isn't there with nothing newer, or with a release that isn't signed yet", () => {
    expect(notice(pkg("aur", { latest: { ...release, version: "0.1.5", tag: "v0.1.5" }, checkedAt: NOW }))).toBeNull();
    expect(notice(pkg("aur", { latest: release, unsigned: release, checkedAt: NOW }))).toBeNull();
  });
  it("without a package (null as the backend sends it, or not there) the notice offers Update to <version> as before", () => {
    expect(notice(status({ available: release, package: null, updateCommand: null })))
      .toEqual({ kind: "offer", version: "0.1.6", text: "Update to 0.1.6", canInstall: true });
    expect(notice(status({ available: release, package: null, updateCommand: null, installTo: null, cannotInstall: "This Gizai runs from /repo/target/release/gizai, not from an install" })))
      .toEqual({ kind: "offer", version: "0.1.6", text: "Update to 0.1.6", canInstall: false });
    expect(notice(status({ available: release, package: "" }))?.text).toBe("Update to 0.1.6");
  });
  it("Settings → Updates' section text names the package, and another word is a package", () => {
    expect(packageText("aur")).toBe("Hersir came from the AUR, so yay updates it, not Hersir itself. Hersir still asks GitHub for new releases and tells you when one is out.");
    expect(packageText("homebrew")).toBe("Hersir came from Homebrew, so brew updates it, not Hersir itself. Hersir still asks GitHub for new releases and tells you when one is out.");
    expect(packageText("scoop")).toBe("Hersir came from Scoop, so scoop updates it, not Hersir itself. Hersir still asks GitHub for new releases and tells you when one is out.");
    for (const other of ["nix", "flatpak", "deb"]) {
      expect(packageText(other)).toBe("Hersir came from a package, so its package manager updates it, not Hersir itself. Hersir still asks GitHub for new releases and tells you when one is out.");
    }
  });
  it("the hint under the command says how soon the package follows a release, and there's none for another word", () => {
    expect(packageHint("aur")).toBe("The AUR package follows each release, usually on the same day.");
    expect(packageHint("homebrew")).toBe("The formula in yotech-ai/tap follows each release, usually on the same day.");
    expect(packageHint("scoop")).toBe("The Scoop bucket follows each release, usually on the same day.");
    expect(packageHint("nix")).toBeNull();
    expect(packageHint("flatpak")).toBeNull();
  });
});
