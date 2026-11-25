// GA-360 QA: the Hersir Cloud contract (research 5.4), which GA-359's Rust side builds from the same words. src/types.ts
// holds its types word for word, and src/api.ts sends each command by its name with the argument names Tauri passes on
// (s, lastGroup, code, name, id and lost) and listens to `cloud-changed`. The browser mode's stand-in (src/browser/mockCloud.ts)
// answers each of those commands, so the screens can be clicked through there too.
import { describe, expect, it, vi } from "vitest";
import typesTs from "./types.ts?raw";
import apiTs from "./api.ts?raw";
import mockCloudTs from "./browser/mockCloud.ts?raw";

const invoked: [string, unknown][] = [];
const listened: string[] = [];
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => { invoked.push([cmd, args]); return Promise.resolve(null); } }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (name: string) => { listened.push(name); return Promise.resolve(() => {}); } }));

const api = await import("./api");

// The card's contract, as it is written there
const CONTRACT = `
type CloudPlan = { name: "personal" | "none"; status: "active" | "past_due" | "lapsed" | "none";
  validUntil: number | null; readOnlyUntil: number | null; quotaBytes: number; usedBytes: number };
type CloudStep = "copying" | "chunking" | "uploading" | "registering" | "downloading" | "checking";
type CloudStatus = {
  signedIn: boolean; email: string | null; signingIn: boolean;
  plan: CloudPlan | null;
  key: "missing" | "here" | "needed";
  recoveryMadeAt: number | null;
  backup: { on: boolean; every: "daily" | "manual"; onQuit: boolean;
            running: { step: CloudStep; done: number; total: number } | null;
            last: { at: number; name: string; sentBytes: number } | null; next: number | null;
            lastError: { at: number; message: string } | null };
};
type CloudBackup = { name: string; madeAt: number; device: { id: string; name: string; this: boolean };
  reason: "daily" | "manual" | "quit" | "before-restore"; bytes: number; restorable: boolean; problem: string | null };
type CloudComputer = { id: string; name: string; platform: string; appVersion: string; lastSeenAt: number | null;
  online: boolean; this: boolean; chief: boolean };`;

/** A type as words, without comments, `export` and the spacing. */
const squash = (s: string) => s.replace(/\/\/.*$/gm, "").replace(/^export /gm, "").replace(/\s+/g, " ").replace(/\s*([{};:|,()])\s*/g, "$1").trim();
const typeText = (src: string, name: string) => {
  const at = src.search(new RegExp(`^(export )?type ${name} =`, "m"));
  if (at < 0) throw new Error(`no type ${name}`);
  // up to the `;` that ends it: the one after the last closing brace at depth 0, or the first for a union
  let depth = 0;
  for (let i = at; i < src.length; i++) {
    const c = src[i];
    if (c === "{") depth++;
    else if (c === "}") depth--;
    else if (c === ";" && depth === 0) return squash(src.slice(at, i + 1));
  }
  throw new Error(`type ${name} doesn't end`);
};

describe("the types (src/types.ts) are the card's contract, word for word", () => {
  for (const name of ["CloudPlan", "CloudStep", "CloudStatus", "CloudBackup", "CloudComputer"])
    it(name, () => { expect(typeText(typesTs, name)).toBe(typeText(CONTRACT, name)); });
});

describe("the commands (src/api.ts)", () => {
  const cases: [string, () => Promise<unknown>, string, unknown][] = [
    ["cloudStatus", () => api.cloudStatus(), "cloud_status", undefined],
    ["cloudSignIn", () => api.cloudSignIn(), "cloud_sign_in", undefined],
    ["cloudCancelSignIn", () => api.cloudCancelSignIn(), "cloud_cancel_sign_in", undefined],
    ["cloudSignOut", () => api.cloudSignOut(), "cloud_sign_out", undefined],
    ["cloudMakeKey", () => api.cloudMakeKey(), "cloud_make_key", undefined],
    ["cloudConfirmRecovery", () => api.cloudConfirmRecovery("Q8YC"), "cloud_confirm_recovery", { lastGroup: "Q8YC" }],
    ["cloudEnterRecovery", () => api.cloudEnterRecovery("HC-7K2Q-M4VB-9TXD-2HWN-PL6R-Q8YC"), "cloud_enter_recovery", { code: "HC-7K2Q-M4VB-9TXD-2HWN-PL6R-Q8YC" }],
    ["cloudNewRecovery", () => api.cloudNewRecovery(), "cloud_new_recovery", undefined],
    ["cloudSaveSettings", () => api.cloudSaveSettings({ on: true, every: "manual", onQuit: false }), "cloud_save_settings", { s: { on: true, every: "manual", onQuit: false } }],
    ["cloudBackUpNow", () => api.cloudBackUpNow(), "cloud_back_up_now", undefined],
    ["cloudListBackups", () => api.cloudListBackups(), "cloud_list_backups", undefined],
    ["cloudStartRestore", () => api.cloudStartRestore("hc-daily-1"), "cloud_start_restore", { name: "hc-daily-1" }],
    ["cloudComputers", () => api.cloudComputers(), "cloud_computers", undefined],
    ["cloudRemoveComputer", () => api.cloudRemoveComputer("laptop", true), "cloud_remove_computer", { id: "laptop", lost: true }],
    ["cloudOpenAccount", () => api.cloudOpenAccount(), "cloud_open_account", undefined],
  ];
  for (const [fn, call, cmd, args] of cases) {
    it(`${fn} sends ${cmd}${args ? ` with ${Object.keys(args).join(", ")}` : ""}`, async () => {
      invoked.length = 0;
      await call();
      expect(invoked).toEqual([[cmd, args]]);
    });
  }

  it("onCloudChanged listens to cloud-changed", async () => {
    listened.length = 0;
    await api.onCloudChanged(() => {});
    expect(listened).toEqual(["cloud-changed"]);
  });

  it("are all of the cloud commands api.ts sends, and the browser mode's stand-in answers each", () => {
    const sent = [...new Set([...apiTs.matchAll(/invoke<[^>]*>\("(cloud_\w+)"/g)].map((m) => m[1]!))].sort();
    expect(sent).toEqual(cases.map(([, , cmd]) => cmd).sort());
    const answered = [...mockCloudTs.matchAll(/case "(cloud_\w+)":/g)].map((m) => m[1]!).sort();
    expect(answered).toEqual(sent);
  });
});
