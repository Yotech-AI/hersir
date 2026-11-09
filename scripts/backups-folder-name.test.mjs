// GA-246 QA: the backups folder Hersir makes is "Hersir backups". The docs, the comment in types.ts and the browser
// demo say so; the old name is left only where an older Hersir's folder is still found: the code, its tests and
// backup-format.md's note. The old name is put together here, so this file doesn't show up in a search for it.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const git = (...args) => spawnSync("git", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 64 * 1000 * 1000 });

const NEW = "Hersir backups";
const OLD = ["Gizai", "backups"].join(" ");
const TEST_FILE = /(^|\/)tests?\/|\.test\.(ts|tsx|mjs)$/;

describe("the backups folder's name (GA-246)", () => {
  it("the old name is only in backups.rs, tests and backup-format.md's note", () => {
    const hits = git("grep", "-n", "-F", OLD).stdout.split("\n").filter(Boolean);
    const files = [...new Set(hits.map((h) => h.split(":")[0]))];
    expect(files.filter((f) => !TEST_FILE.test(f)).sort()).toEqual(["crates/gizai-core/src/backups.rs", "docs/backup-format.md"]);
    // in the code: the constant, and backups_dir's doc comment
    const code = read("crates/gizai-core/src/backups.rs").split("\n").filter((l) => l.includes(OLD));
    expect(code.filter((l) => !/^\s*\/\/\//.test(l))).toEqual([`pub const OLD_FOLDER_NAME: &str = "${OLD}";`]);
    // in backup-format.md: the one note, which says it is still found and used, and that Hersir backups wins
    const note = read("docs/backup-format.md").split("\n").filter((l) => l.includes(OLD));
    expect(note).toHaveLength(1);
    expect(note[0].startsWith(`- An older Hersir named that folder \`${OLD}\`. It is still found and used`), note[0]).toBe(true);
    expect(note[0]).toContain("holds `gizai-backups.json`");
    expect(note[0]).toContain("nothing is renamed or moved");
    expect(note[0]).toContain("When both are there, `Hersir backups` is used.");
  });

  it("docs/settings.md names Hersir backups for the folder Hersir makes and the folder to restore from", () => {
    const md = read("docs/settings.md");
    expect(md).toContain("Inside it Hersir makes a folder named `Hersir backups` and backs up there, unless the folder you chose is such a folder already (it holds `gizai-backups.json`).");
    expect(md).toContain("Choose the `Hersir backups` folder or the folder it is in.");
  });

  it("docs/backup-format.md: a chosen folder gets Hersir backups, the tree starts with it, and a folder that holds the marker is used whatever its name", () => {
    const md = read("docs/backup-format.md");
    expect(md).toContain("- Any other folder gets a backups folder inside it, named `Hersir backups`. The first snapshot makes it.");
    expect(md).toContain("- A folder that holds `gizai-backups.json` is a backups folder, and Hersir uses it as it is, whatever its name.");
    expect(md).toMatch(/```\nHersir backups\/\n {2}gizai-backups\.json +\{"format":"gizai-backups","format_version":1\}\n/);
  });

  it("docs/install.md's new computer chooses the Hersir backups folder", () => {
    expect(read("docs/install.md")).toContain("choose the `Hersir backups` folder or the folder it is in");
  });

  it("the browser demo's folder and BackupStatus's comment say Hersir backups, as fixtures.rs writes it from FOLDER_NAME", () => {
    const fixtures = JSON.parse(read("src/browser/fixtures.json"));
    const st = fixtures.commands.backup_status;
    expect(st.folder).toBe(`${st.settings.folder}/${NEW}`);
    expect(read("crates/gizai-core/examples/fixtures.rs")).toContain("folder: Some(format!(\"{chosen}/{}\", backups::FOLDER_NAME))");
    expect(read("src/types.ts")).toContain("folder: string | null;           // the backups folder in use (…/Hersir backups), null when none is set");
  });

  it("the formats and the marker keep their gizai names", () => {
    const rs = read("crates/gizai-core/src/backups.rs");
    expect(rs).toContain('pub const MARKER: &str = "gizai-backups.json";');
    expect(rs).toContain('pub const MARKER_JSON: &str = r#"{"format":"gizai-backups","format_version":1}"#;');
    expect(rs).toContain('const README: &str = "Hersir backups\n');
  });
});
