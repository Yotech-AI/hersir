// GA-219 QA against GA-218 as merged: what Restore in Settings → General → Backups, the sidebar and the browser mode count on
// from the backend, read as text (Vite's ?raw), so a change on one side without the other fails here. BackupRestore.test.tsx and
// mock.ts stand in for the backend with copies of its refusals, steps and messages: this keeps those copies true. Also: the
// sidebar's place for the notice, the docs' links, and that the texts say Hersir.
import { describe, expect, it, vi } from "vitest";
import { isRestoring, snapshotMadeAt } from "./lib/restore";
import type { BackupEntry, RestoreStatus } from "./types";
import fixturesJson from "./browser/fixtures.json";
import libRs from "../src-tauri/src/lib.rs?raw";
import commandsRs from "../src-tauri/src/commands.rs?raw";
import appRestoreRs from "../src-tauri/src/restore.rs?raw";
import coreRestoreRs from "../crates/gizai-core/src/restore.rs?raw";
import coreBackupsRs from "../crates/gizai-core/src/backups.rs?raw";
import fixturesRs from "../crates/gizai-core/examples/fixtures.rs?raw";
import typesTs from "./types.ts?raw";
import mockTs from "./browser/mock.ts?raw";
import fakeTs from "./components/BackupRestore.test.tsx?raw";
import restoreTs from "./lib/restore.ts?raw";
import noticeTsx from "./components/RestoreNotice.tsx?raw";
import sectionTsx from "./components/BackupSettings.tsx?raw";
import sidebarTsx from "./components/Sidebar.tsx?raw";
import installMd from "../docs/install.md?raw";
import settingsMd from "../docs/settings.md?raw";
import paidMd from "../docs/PAID-FEATURES.md?raw";
import formatMd from "../docs/backup-format.md?raw";

const invoked: [string, unknown][] = [];
const listened: string[] = [];
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => { invoked.push([cmd, args]); return Promise.resolve(null); } }));
vi.mock("@tauri-apps/api/event", () => ({ listen: (name: string) => { listened.push(name); return Promise.resolve(() => {}); } }));

const api = await import("./api");
const fixtures = (fixturesJson as unknown as { commands: { restore_status: RestoreStatus; list_backups: BackupEntry[] } }).commands;

/** The fields of a Rust struct in gizai-core's restore.rs, camelCased as serde's rename_all = "camelCase" sends them. */
function serdeFields(name: string): string[] {
  const body = new RegExp(`#\\[serde\\(rename_all = "camelCase"\\)\\]\\s*pub struct ${name} \\{([\\s\\S]*?)\\n\\}`).exec(coreRestoreRs);
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

/** A Markdown heading's anchor, as GitHub makes it. */
const anchors = (md: string) => [...md.matchAll(/^#+ (.+)$/gm)].map((m) => m[1].toLowerCase().replace(/[^\w\s-]/g, "").trim().replace(/\s+/g, "-"));

/** A doc's part under a heading, up to the next heading of that level or higher. */
function section(md: string, heading: string): string {
  const level = /^#+/.exec(heading)![0].length;
  const start = md.indexOf(`\n${heading}\n`);
  if (start < 0) throw new Error(`no ${heading}`);
  const rest = md.slice(start + 1 + heading.length);
  const next = new RegExp(`^#{1,${level}} `, "m").exec(rest);
  return next ? rest.slice(0, next.index) : rest;
}

/** Text in a source file without its comments. */
const noComments = (src: string) => src.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");

describe("the commands Restore calls are GA-218's", () => {
  it("are registered with Tauri, and the UI sends what they take", async () => {
    invoked.length = 0;
    await api.startRestore("/media/you/USB", "gizai-daily-x");
    await api.restoreStatus();
    await api.cancelRestore();
    await api.restartToRestore();
    await api.dismissRestoreResult();
    await api.listBackups("/media/you/USB");
    expect(invoked).toEqual([["start_restore", { folder: "/media/you/USB", name: "gizai-daily-x" }], ["restore_status", undefined], ["cancel_restore", undefined],
      ["restart_to_restore", undefined], ["dismiss_restore_result", undefined], ["list_backups", { folder: "/media/you/USB" }]]);
    const handler = /generate_handler!\[([\s\S]*?)\]\)/.exec(libRs)?.[1] ?? "";
    for (const [cmd] of invoked) expect(handler, cmd).toMatch(new RegExp(`commands::${cmd}\\b`));
    expect(new RegExp("pub async fn start_restore\\(st: State<'_, AppState>, folder: String, name: String\\)").test(commandsRs)).toBe(true);
  });
  it("answer with the status; restart_to_restore with nothing", () => {
    const answers = (cmd: string) => new RegExp(`pub async fn ${cmd}\\([^)]*\\) -> R<(.+)> \\{`).exec(commandsRs)?.[1];
    for (const cmd of ["start_restore", "restore_status", "cancel_restore", "dismiss_restore_result"]) expect(answers(cmd), cmd).toBe("gizai_core::restore::RestoreStatus");
    expect(answers("restart_to_restore")).toBe("()");
  });
  it("say what changed in restore-changed, with the status", async () => {
    listened.length = 0;
    await api.onRestoreChanged(() => {});
    expect(listened).toEqual(["restore-changed"]);
    expect(libRs).toContain('runs::Note::RestoreChanged(status) => app.emit("restore-changed", status)');
  });
});

describe("the types in src/types.ts have the backend's fields", () => {
  it("RestoreStatus", () => {
    expect(tsFields(typeBody("RestoreStatus"))).toEqual(serdeFields("RestoreStatus"));
  });
  it("its result", () => {
    const inner = /\bresult: \{([^}]*)\}/.exec(typeBody("RestoreStatus"));
    expect(tsFields(inner?.[1] ?? "")).toEqual(serdeFields("RestoreResult"));
  });
  it("and each state it can have, as the backend says them; the three that run are those the UI calls running", () => {
    const states = /state: "none" \| (.+?);/.exec(typesTs)?.[0];
    expect(states).toBe('state: "none" | "checking" | "saving" | "staging" | "ready" | "failed";');
    expect(coreRestoreRs).toContain('/// "none", "checking", "saving", "staging", "ready" or "failed".');
    expect(appRestoreRs).toContain('matches!(self.state, "checking" | "saving" | "staging")');
    for (const state of ["checking", "saving", "staging"]) expect(isRestoring({ state } as RestoreStatus), state).toBe(true);
  });
  it("and so do the browser's fixtures, with no restore going on", () => {
    expect(Object.keys(fixtures.restore_status).sort()).toEqual(serdeFields("RestoreStatus"));
    expect(fixtures.restore_status.state).toBe("none");
    expect(fixturesRs).toContain('c.insert("restore_status".into(), val(restore::RestoreStatus { state: "none".into(), ..Default::default() }));');
  });
});

describe("the fakes' copies of the backend's texts are true", () => {
  it.each(["A backup is running: restore once it is done", "A restore is running already", "No restore is ready: restore a snapshot first"])("the refusal: %s", (text) => {
    expect(appRestoreRs).toContain(`"${text}"`);
    expect(mockTs).toContain(`"${text}"`);
    expect(fakeTs).toContain(`"${text}"`);
  });
  it("the steps, in the backend's words and order", () => {
    const said = ["Checking the snapshot", "Checking the database", "Copying the database"];
    for (const s of said) {
      expect(coreRestoreRs).toContain(`"${s}".into()`);
      expect(mockTs).toContain(`"${s}"`);
    }
    expect(coreRestoreRs).toContain('format!("Checking files ({n} of {of})")');
    expect(mockTs).toContain("`Checking files (${files} of ${files})`");
    expect(coreRestoreRs).toContain('format!("Copying files ({n} of {of})")');
    expect(mockTs).toContain("`Copying files (${files} of ${files})`");
    expect(appRestoreRs).toContain('pub const SAVING_STEP: &str = "Saving today\'s data";');
    expect(mockTs).toContain('"Saving today\'s data"');
    // in that order: checks, today's data, staging
    const order = [appRestoreRs.indexOf("core::check("), appRestoreRs.indexOf('set_state(st, "saving"'), appRestoreRs.indexOf('set_state(st, "staging", Some(Step::CopyDatabase.text()))')];
    expect(order).toEqual([...order].sort((a, b) => a - b));
    expect(order[0]).toBeGreaterThan(-1);
  });
  it("the failure a browser-mode backup shows when it is checked", () => {
    expect(coreRestoreRs).toContain('"The snapshot\'s database file has changed since the snapshot was made: {} doesn\'t match its manifest"');
    expect(mockTs).toContain("`The snapshot's database file has changed since the snapshot was made: ${st.folder}/snapshots/${name}.db.zst doesn't match its manifest`");
  });
  it("the reasons BackupRestore.test.tsx shows for a snapshot it can't restore", () => {
    expect(coreRestoreRs).toContain('format!("Made by a newer Hersir (backup format {}). Update Hersir first."');
    expect(coreRestoreRs).toContain('format!("Made by a newer Hersir (database version {}). Update Hersir first."');
    expect(coreRestoreRs).toContain('format!("{what} missing from {}"');
    expect(coreRestoreRs).toContain('"1 file of the snapshot is"');
    expect(coreRestoreRs).toContain('"{missing} files of the snapshot are"');
    expect(coreRestoreRs).toContain('format!("The snapshot\'s database file is missing: {}"');
    expect(appRestoreRs).toContain("Hersir can't save today's data first: {}. Choose a backups folder in Settings → General → Backups.");
    for (const problem of ["Its manifest can't be read", "Its database file is missing", "1 file is missing"]) expect(coreBackupsRs, problem).toContain(`"${problem}"`);
    expect(coreBackupsRs).toContain('format!("{files_missing} files are missing")');
  });
  it("the results a browser-mode restart gives", () => {
    const words = (s: string) => s.replace(/\\\n\s*/g, "");
    expect(words(coreRestoreRs)).toContain('"Hersir restored {} from {}. Today\'s data was saved as {} first. The agents are paused: start them again when you\'re ready."');
    expect(mockTs).toContain("`Hersir restored ${name} from ${from}. Today's data was saved as ${st.savedToday} first. The agents are paused: start them again when you're ready.`");
    expect(coreRestoreRs).toContain('"Hersir couldn\'t open the restored data of {}, so it started with today\'s data: {}.{kept}"');
    expect(mockTs).toContain("`Hersir couldn't open the restored data of ${name}, so it started with today's data: ");
  });
});

describe("the time the result reads from a snapshot's name", () => {
  it("the name is as the backend writes it, with the local time", () => {
    expect(coreBackupsRs).toContain('format!("gizai-{}-{}-{tag}", reason.as_str(), db::stamp(made_at))');
    expect(formatMd).toContain("gizai-<reason>-");
  });
  it.each(["daily", "weekly", "manual", "quit", "before-restore"])("is read for reason %s", (reason) => {
    expect(snapshotMadeAt(`gizai-${reason}-20261027-141506-330-ce82a749`)).toBe(new Date(2026, 9, 27, 14, 15, 6, 330).getTime());
  });
  it("the fixtures' names can be read too", () => {
    expect(fixtures.list_backups.length).toBeGreaterThan(0);
    for (const e of fixtures.list_backups) expect(snapshotMadeAt(e.name), e.name).not.toBeNull();
  });
});

describe("the sidebar", () => {
  it("has the restore's notice where the update's goes: above Company", () => {
    expect(sidebarTsx).toMatch(/<UpdateNotice \/>\s*<RestoreNotice \/>\s*<div className="nav-section">\s*<div className="nav-label">Company<\/div>/);
  });
});

describe("the texts say Hersir", () => {
  it.each([["src/lib/restore.ts", restoreTs], ["src/components/RestoreNotice.tsx", noticeTsx], ["src/components/BackupSettings.tsx", sectionTsx]])("%s, its comments aside", (_, src) => {
    expect(noComments(src)).not.toMatch(/\bGiz[Aa][Ii]\b/);
  });
  it("the docs' Restore parts, the backups folder's name too (GA-246)", () => {
    const parts = [section(settingsMd, "### Restore"), section(installMd, "### Restore a backup"), section(paidMd, "### Phase 0: local snapshots (free, in the app): 2–3 days")];
    for (const part of parts) expect(part).not.toMatch(/\bGiz[Aa][Ii]\b/);
  });
});

describe("the docs describe the restore as built", () => {
  it("install.md → Your data: how to restore, also on a new computer", () => {
    const part = section(installMd, "### Restore a backup");
    expect(part).toContain("Settings → General → Backups lists the backups in your folder, each with Restore.");
    expect(part).toContain("On a new computer, install Hersir");
    expect(part).toContain("Settings → General → Backups → Restore from another folder…");
    // under Your data
    expect(installMd.lastIndexOf("\n## ", installMd.indexOf("### Restore a backup"))).toBe(installMd.indexOf("\n## Your data"));
  });
  it("settings.md → Backups: Restore, with its steps; and no longer that it can't restore", () => {
    const part = section(settingsMd, "### Restore");
    for (const words of ["Restore from another folder…", "Cancel changes nothing.", "Restart now", "Cancel restore", "until Dismiss", "Agents are paused: resume them in Settings → Agents and runs."]) expect(part, words).toContain(words);
    expect(settingsMd).not.toContain("it can't restore one yet");
    expect(settingsMd.lastIndexOf("\n## ", settingsMd.indexOf("### Restore"))).toBe(settingsMd.indexOf("\n## Backups"));
  });
  it("PAID-FEATURES.md: Phase 0 as built, on the backups folder and its format; a line each in Phases 1 and 2", () => {
    const phase0 = section(paidMd, "### Phase 0: local snapshots (free, in the app): 2–3 days");
    expect(phase0).toContain("Built (GA-216 to GA-219).");
    expect(phase0).toContain("[docs/backup-format.md](backup-format.md)");
    expect(phase0).toContain("Restore from another folder…");
    expect(section(paidMd, "### Phase 1: Hersir Cloud account and encrypted backup (Personal): 10–15 days")).toContain("Hersir Cloud's snapshots use the format of [docs/backup-format.md](backup-format.md)");
    expect(section(paidMd, "### Phase 2: sync between your own devices (Personal): 10–20 days")).toContain("A new device's first sync starts from a snapshot in the format of [docs/backup-format.md](backup-format.md)");
  });
  it("their links go somewhere", () => {
    expect(anchors(settingsMd)).toContain("restore");
    expect(anchors(formatMd)).toContain("restore");
    expect(anchors(installMd)).toContain("in-one-line");
    expect(installMd).toContain("[Restore](settings.md#restore)");
    expect(settingsMd).toContain("(backup-format.md#restore)");
  });
});
