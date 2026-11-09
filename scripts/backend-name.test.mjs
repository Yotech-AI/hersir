// GA-232: the name from the backend. Every text the Rust side shows a person or an agent says Hersir, never Gizai or
// GizAi: notifications, the tray, messages, errors and dialogs, the prompts, the role templates and the tools'
// descriptions. What a text keeps is a code name (GIZAI_RESULT, the MCP server gizai, gizai/ branches, .gizai-tmp, the
// backup and team template formats, the data folders). GA-233 renamed the app bundle and the window title to Hersir.
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const git = (...args) => spawnSync("git", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 64 * 1000 * 1000 });

const NAME = /\bGiz[Aa][Ii]\b/;
// The Gizais a backend text keeps, each with why.
const KEPT = [
  /Library\/Application Support\/Gizai/, // macOS's data folder (data folders stay)
  /\.join\("Gizai"\)/, // Windows's data folder under APPDATA
  /%APPDATA%\\Gizai/, // gizai --help names the data folders as they are
  /OLD_FOLDER_NAME: &str = "Gizai backups";/, // the backups folder an older Hersir made, still found (GA-246)
  /"Stopped because Gizai quit\."/, // what runs an older version's quit stopped say: the board check reads it as a quit
];

const rustFiles = () => git("ls-files", "src-tauri/src", "crates").stdout.split("\n")
  .filter((p) => /^(src-tauri\/src|crates\/[^/]+\/src)\/.*\.rs$/.test(p));

/** A Rust file's lines without its comments and its unit tests (`#[cfg(test)]` to the end). */
function codeLines(path) {
  let text = read(path);
  const tests = text.search(/^#\[cfg\(test\)\]\s*\nmod /m);
  if (tests >= 0) text = text.slice(0, tests);
  return text.split("\n").map((line, i) => {
    if (/^\s*(\/\/|\*|\/\*)/.test(line)) return [i + 1, ""];
    // a comment after the code, when the // is outside a string (an even number of quotes before it)
    const at = [...line.matchAll(/\s\/\/\s/g)].map((m) => m.index).find((x) => (line.slice(0, x).match(/(?<!\\)"/g) ?? []).length % 2 === 0);
    return [i + 1, at === undefined ? line : line.slice(0, at)];
  });
}

describe("the name in the backend's texts (GA-232)", () => {
  it("the Rust files it reads include the tray, the notifications, the prompts, the tools and the MCP server", () => {
    const files = rustFiles();
    for (const f of ["src-tauri/src/lib.rs", "src-tauri/src/notifications.rs", "src-tauri/src/update.rs", "src-tauri/src/chat.rs",
      "src-tauri/src/tools/mod.rs", "crates/gizai-agents/src/prompt.rs", "crates/gizai-core/src/seed.rs", "crates/gizai-core/src/board.rs",
      "crates/gizai-mcp/src/bin/gizai-mcp.rs"])
      expect(files).toContain(f);
    expect(files.length).toBeGreaterThan(100);
  });

  it("no text of the Rust side says Gizai or GizAi, except a data folder, a code name or GA-233's", () => {
    const left = [];
    for (const f of rustFiles())
      for (const [n, code] of codeLines(f))
        if (NAME.test(code) && !KEPT.some((k) => k.test(code))) left.push(`${f}:${n}: ${code.trim().slice(0, 140)}`);
    expect(left).toEqual([]);
  });

  it("each kept Gizai is still there, so this list doesn't keep one the code no longer has", () => {
    const all = rustFiles().map((f) => codeLines(f).map(([, c]) => c).join("\n")).join("\n");
    for (const k of KEPT) expect(all, String(k)).toMatch(k);
  });

  it("the role templates say Hersir and never Gizai, and still ask for the GIZAI_RESULT line", () => {
    const roles = git("ls-files", "crates/gizai-core/roles").stdout.split("\n").filter((p) => p.endsWith(".md"));
    expect(roles.length).toBeGreaterThanOrEqual(4);
    for (const r of roles) {
      const t = read(r);
      expect(t, r).not.toMatch(NAME);
      expect(t, r).toMatch(/^You are the \w+ Agent in Hersir's Software team\./);
      expect(t, r).toContain('GIZAI_RESULT: {"outcome":"<outcome>"');
    }
  });

  it("the tray says Open Hersir and Quit Hersir completely, its tooltip and the notifications Hersir, with the ids and icon names kept", () => {
    const lib = read("src-tauri/src/lib.rs");
    expect(lib).toContain('MenuItem::with_id(app, "open", "Open Hersir", true, None::<&str>)');
    expect(lib).toContain('MenuItem::with_id(app, "quit", "Quit Hersir completely", true, None::<&str>)');
    expect(lib).toContain('.tooltip("Hersir")');
    expect(lib).toContain('TrayIconBuilder::with_id("gizai")');
    // GA-248: the app id is dev.hersir.app, and the notifications use install.sh's hersir icon and desktop entry
    expect(lib).toContain('pub const APP_ID: &str = "dev.hersir.app";');
    const notes = read("src-tauri/src/notifications.rs");
    expect(notes.match(/appname\("Hersir"\)/g)).toHaveLength(2);
    expect(notes).not.toContain('appname("Gizai")');
    expect(notes).toContain('.icon("hersir")');
    expect(notes).toContain('Hint::DesktopEntry("hersir".into())');
  });

  it("with GA-233 in, the app bundle and the window title say Hersir too, and the gizai program stays", () => {
    expect(read("src-tauri/src/lib.rs")).toContain('w.set_title(&format!("Hersir ({label} data)"))');
    expect(read("crates/gizai-agents/src/update.rs")).toContain('d.ends_with("Hersir.app/Contents/MacOS")');
    expect(read("crates/gizai-agents/src/update.rs")).toContain('pub const REPO: &str = "https://github.com/Yotech-AI/hersir.git";');
    expect(read("crates/gizai-agents/src/update.rs")).toContain("Hersir asks GitHub for new releases with it.");
  });

  it("the code names stay: GIZAI_RESULT, the MCP server gizai, gizai/ branches, .gizai-tmp and the backup and team template formats", () => {
    expect(read("crates/gizai-agents/src/outcome.rs")).toContain('.find(|l| l.starts_with("GIZAI_RESULT:"))');
    expect(read("crates/gizai-core/src/mcp_servers.rs")).toContain('pub const TAKEN: [&str; 2] = ["gizai", "chrome-devtools"];');
    expect(read("crates/gizai-agents/src/worktree.rs")).toContain('pub const TEMP_DIR: &str = ".gizai-tmp";');
    expect(read("crates/gizai-core/src/backups.rs")).toContain('pub const MARKER: &str = "gizai-backups.json";');
    expect(read("crates/gizai-core/src/backups.rs")).toContain('pub const FOLDER_NAME: &str = "Hersir backups";');
    expect(read("crates/gizai-core/src/backups.rs")).toContain('pub const OLD_FOLDER_NAME: &str = "Gizai backups";');
    const templates = read("crates/gizai-core/src/templates.rs");
    expect(templates).toContain('pub const FORMAT: &str = "gizai-team";');
    expect(templates).toContain('pub const FILE_SUFFIX: &str = ".gizai-team.json";');
  });
});
