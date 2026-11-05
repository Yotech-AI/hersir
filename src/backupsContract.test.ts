// GA-217 QA against GA-216 as merged: what Settings → General → Backups and the browser mode count on from the backend, read
// as text (Vite's ?raw), so a change on one side without the other fails here. BackupSettings.test.tsx and mock.ts stand in for
// the backend with copies of its refusals, steps and reasons: this keeps those copies true.
import { describe, expect, it, vi } from "vitest";
import { EVERY, KEEP_MAX, KEEP_MIN, reasonText, stepText } from "./lib/backups";
import type { BackupEntry, BackupStatus } from "./types";
import fixturesJson from "./browser/fixtures.json";
import libRs from "../src-tauri/src/lib.rs?raw";
import commandsRs from "../src-tauri/src/commands.rs?raw";
import appBackupsRs from "../src-tauri/src/backups.rs?raw";
import coreBackupsRs from "../crates/gizai-core/src/backups.rs?raw";
import typesTs from "./types.ts?raw";
import mockTs from "./browser/mock.ts?raw";
import fakeTs from "./components/BackupSettings.test.tsx?raw";

const invoked: [string, unknown][] = [];
const listened: string[] = [];
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => { invoked.push([cmd, args]); return Promise.resolve(null); } }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (name: string) => { listened.push(name); return Promise.resolve(() => {}); } }));

const api = await import("./api");
const fixtures = (fixturesJson as unknown as { commands: { backup_status: BackupStatus; list_backups: BackupEntry[] } }).commands;

/** The fields of a Rust struct, camelCased as serde's rename_all = "camelCase" sends them. */
function serdeFields(name: string): string[] {
  const body = new RegExp(`#\\[serde\\(rename_all = "camelCase"\\)\\]\\s*pub struct ${name} \\{([\\s\\S]*?)\\n\\}`).exec(coreBackupsRs);
  if (!body) throw new Error(`no camelCase struct ${name}`);
  return [...body[1].matchAll(/^\s*pub (\w+):/gm)].map((m) => m[1].replace(/_(\w)/g, (_, c: string) => c.toUpperCase())).sort();
}

/** What's between the braces of an object type in src/types.ts. */
function typeBody(name: string): string {
  const body = new RegExp(`export type ${name} = \\{([\\s\\S]*?)\\};\\n`).exec(typesTs);
  if (!body) throw new Error(`no object type ${name}`);
  return body[1].replace(/\/\/.*$/gm, "");
}

/** The fields of an object type's body, not those of the objects inside it. */
const tsFields = (body: string) => [...body.replace(/\{[^}]*\}/g, "{}").matchAll(/(?:^|;)\s*(\w+):/g)].map((m) => m[1]).sort();

/** The strings a Rust enum's as_str gives, in its order. */
const asStr = (name: string) => [...coreBackupsRs.matchAll(new RegExp(`${name}::\\w+ => "([a-z-]+)"`, "g"))].map((m) => m[1]);

describe("the commands the section calls are GA-216's", () => {
  it("are registered with Tauri, and the UI sends what they take", async () => {
    invoked.length = 0;
    const settings = { folder: null, every: "daily", onQuit: false, keep: 10 } as const;
    await api.backupStatus();
    await api.saveBackupSettings(settings);
    await api.listBackups(null);
    await api.backUpNow();
    expect(invoked).toEqual([["backup_status", undefined], ["save_backup_settings", { settings }], ["list_backups", { folder: null }], ["back_up_now", undefined]]);
    const handler = /generate_handler!\[([\s\S]*?)\]\)/.exec(libRs)?.[1] ?? "";
    for (const [cmd] of invoked) expect(handler, cmd).toMatch(new RegExp(`commands::${cmd}\\b`));
    // Without the app state Tauri passes itself
    const args = (cmd: string) => new RegExp(`pub async fn ${cmd}\\(([^)]*)\\)`).exec(commandsRs)?.[1].replace(/^st: State<'_, AppState>,?/, "").trim();
    expect(args("backup_status")).toBe("");
    expect(args("save_backup_settings")).toBe("settings: gizai_core::backups::BackupSettings");
    expect(args("list_backups")).toBe("folder: Option<String>");
    expect(args("back_up_now")).toBe("");
  });
  it("answer with the status, and list_backups with the snapshots", () => {
    const answers = (cmd: string) => new RegExp(`pub async fn ${cmd}\\([^)]*\\) -> R<(.+)> \\{`).exec(commandsRs)?.[1];
    for (const cmd of ["backup_status", "save_backup_settings", "back_up_now"]) expect(answers(cmd), cmd).toBe("gizai_core::backups::BackupStatus");
    expect(answers("list_backups")).toBe("Vec<gizai_core::backups::BackupEntry>");
  });
  it("say what changed in backups-changed, with the status", async () => {
    listened.length = 0;
    await api.onBackupsChanged(() => {});
    expect(listened).toEqual(["backups-changed"]);
    expect(libRs).toContain('runs::Note::BackupsChanged(status) => app.emit("backups-changed", status)');
  });
});

describe("the types in src/types.ts have the backend's fields", () => {
  it.each(["BackupSettings", "BackupEntry", "BackupStatus"])("%s", (name) => {
    expect(tsFields(typeBody(name))).toEqual(serdeFields(name));
  });
  it.each([["running", "Running"], ["last", "Last"], ["lastError", "LastError"]])("BackupStatus's %s", (field, name) => {
    const inner = new RegExp(`\\b${field}: \\{([^}]*)\\}`).exec(typeBody("BackupStatus"));
    expect(tsFields(inner?.[1] ?? "")).toEqual(serdeFields(name));
  });
  it("and so do the browser's fixtures: written again when a field is added", () => {
    expect(Object.keys(fixtures.backup_status).sort()).toEqual(serdeFields("BackupStatus"));
    expect(Object.keys(fixtures.backup_status.settings).sort()).toEqual(serdeFields("BackupSettings"));
    for (const e of fixtures.list_backups) expect(Object.keys(e).sort(), e.name).toEqual(serdeFields("BackupEntry"));
  });
});

describe("the section has words for each of the backend's values", () => {
  it("each reason a snapshot can have, as src/types.ts lists them", () => {
    const reasons = asStr("Reason");
    expect(reasons).toEqual(["daily", "weekly", "manual", "quit", "before-restore"]);
    expect(/export type BackupReason = (.+);/.exec(typesTs)?.[1].split(" | ").map((r) => JSON.parse(r))).toEqual(reasons);
    // an unknown one shows as it is, so one with words reads differently
    for (const r of reasons) expect(reasonText(r), r).not.toBe(r);
  });
  it("each step of a snapshot, and the browser mode takes the same steps in the same order", () => {
    const steps = asStr("Step");
    expect(steps).toEqual(["database", "files", "compress", "manifest", "keep"]);
    for (const s of steps) expect(stepText(s), s).not.toBe(`${s}…`);
    const inMock = /const steps = \[([^\]]*)\]/.exec(mockTs)?.[1] ?? "";
    expect(inMock.split(",").map((s) => JSON.parse(s.trim()))).toEqual(steps);
  });
  it("each choice of When", () => {
    const every = /pub enum Every \{([\s\S]*?)\n\}/.exec(coreBackupsRs)?.[1] ?? "";
    expect(coreBackupsRs).toMatch(/#\[serde\(rename_all = "lowercase"\)\]\s*pub enum Every/);
    const variants = [...every.matchAll(/^\s*([A-Z]\w*),$/gm)].map((m) => m[1].toLowerCase());
    expect(variants).toEqual(["daily", "weekly", "manual"]);
    expect(EVERY.map(([v]) => v)).toEqual(variants);
  });
});

describe("the fakes' copies of the backend's texts are true", () => {
  it("Keep's range and its refusal", () => {
    expect(coreBackupsRs).toContain(`pub const KEEP_MAX: i64 = ${KEEP_MAX};`);
    expect(coreBackupsRs).toContain(`if !(${KEEP_MIN}..=KEEP_MAX).contains(&s.keep) {`);
    const said = /format!\("(Keep must be 1 to )\{KEEP_MAX\}( snapshots, not )\{\}"/.exec(coreBackupsRs);
    const copy = `${said?.[1]}${KEEP_MAX}${said?.[2]}\${s.keep}`;
    expect(copy).toBe("Keep must be 1 to 100 snapshots, not ${s.keep}");
    expect(mockTs).toContain(copy);
    expect(fakeTs).toContain(copy);
  });
  it.each(["Choose a folder for backups first", "A backup is running already"])("Back up now's refusal: %s", (text) => {
    expect(appBackupsRs).toContain(`"${text}"`);
    expect(mockTs).toContain(`"${text}"`);
    expect(fakeTs).toContain(`"${text}"`);
  });
  it("a folder Hersir can't write in", () => {
    expect(coreBackupsRs).toContain('format!("Hersir can\'t write in {}: {e}"');
    expect(fakeTs).toContain("`Hersir can't write in ${folder}: ");
  });
  it("the snapshot a newer Hersir made, in the fixtures", () => {
    expect(coreBackupsRs).toContain('format!("A newer Hersir made it ({v}): update Hersir to restore it")');
    const newer = fixtures.list_backups.find((e) => !e.restorable)!;
    expect(newer.problem).toBe(`A newer Hersir made it (${newer.appVersion}): update Hersir to restore it`);
  });
  it("the backups folder's name, kept through the rename (GA-232)", () => {
    const name = /pub const FOLDER_NAME: &str = "([^"]+)";/.exec(coreBackupsRs)?.[1] ?? "";
    expect(name).not.toBe("");
    expect(fixtures.backup_status.folder).toBe(`${fixtures.backup_status.settings.folder}/${name}`);
    expect(fakeTs).toContain(`\${folder}/${name}`);
  });
});
