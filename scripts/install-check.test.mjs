// Tests for install.sh --check's tray icon line (GA-248): it warns that libayatana-appindicator is missing only when
// ldconfig's list hasn't got it. install.sh runs under set -o pipefail, so a grep that stops at the first match (grep -q)
// left ldconfig writing into a closed pipe: ldconfig died of SIGPIPE, the pipeline counted as failed, and --check said
// the library was missing although it was there. A list of thousands of lines (a desktop's is about 4000) after the
// match makes that happen every time. A stand-in ldconfig prints the list; install.sh runs with HOME in a scratch folder.
// With main's install.sh from before GA-248 (grep -q), the first test fails.
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const linux = process.platform === "linux";
const scratch = realpathSync(mkdtempSync(join(tmpdir(), "install-check-")));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));

const TRAY = "Tray icon: libayatana-appindicator is missing";
const entry = (lib) => `\t${lib} (libc6,x86-64) => /usr/lib/${lib}`;
// ldconfig -p's list: its first line, then `libs` (where they are in it) among `filler` other libraries.
function list(libs, filler, at) {
  const others = Array.from({ length: filler }, (_, i) => entry(`libfiller${i}-with-a-longer-name.so.${i % 7}`));
  others.splice(at === "first" ? 0 : others.length, 0, ...libs.map(entry));
  return `${filler + libs.length} libs found in cache \`/etc/ld.so.cache'\n${others.join("\n")}\n`;
}

let n = 0;
// install.sh --check with a stand-in ldconfig that prints `text` for -p.
function check(text) {
  const dir = join(scratch, `run-${++n}`);
  const bin = join(dir, "bin");
  mkdirSync(join(dir, "home"), { recursive: true });
  mkdirSync(bin);
  writeFileSync(join(dir, "ldconfig.txt"), text);
  // awk writes the list a buffer at a time, as ldconfig does (GNU cat may grow the pipe and write it all in one go)
  writeFileSync(join(bin, "ldconfig"), `#!/bin/sh\n[ "$1" = -p ] && exec awk '{ print }' "${join(dir, "ldconfig.txt")}"\nexit 0\n`);
  chmodSync(join(bin, "ldconfig"), 0o755);
  const env = { HOME: join(dir, "home"), USER: "hersir", PATH: `${bin}:${process.env.PATH}` };
  const r = spawnSync("bash", [join(ROOT, "install.sh"), "--check"], { cwd: dir, env, encoding: "utf8" });
  return `${r.stdout}${r.stderr}`;
}

describe.runIf(linux)("install.sh --check: the tray icon library", () => {
  it("gives no tray warning when ldconfig lists libayatana-appindicator3.so.1 first in a list thousands of lines long", () => {
    const out = check(list(["libayatana-appindicator3.so.1", "libayatana-appindicator3.so"], 5000, "first"));
    expect(out).toContain("Checking what Hersir needs");
    expect(out).not.toContain(TRAY);
  });

  it("gives no tray warning when the library is the last of thousands of lines", () => {
    expect(check(list(["libayatana-appindicator3.so.1"], 5000, "last"))).not.toContain(TRAY);
  });

  it("gives no tray warning for the older libappindicator3.so.1 either", () => {
    expect(check(list(["libappindicator3.so.1"], 5000, "first"))).not.toContain(TRAY);
  });

  it("still warns when the list hasn't got the library, short or thousands of lines long", () => {
    expect(check(list([], 5000, "first"))).toContain(TRAY);
    expect(check(list(["libayatana-indicator3.so.7"], 3, "first"))).toContain(TRAY);
  });
});

// GA-252: install.sh --check --skip-build, as the one line runs it before it installs a release's ready-made build, asks
// only for what Hersir needs to run: git, ssh-keygen (with which Hersir checks a release's signature) and on Linux
// WebKitGTK 4.1's library. Not Rust, Node.js, npm, a C compiler or pkg-config, which a build needs and --check alone
// still names. It runs with a PATH of only the programs given here, so none of those is there.
const RUN_LIBS = list(["libwebkit2gtk-4.1.so.0", "libayatana-appindicator3.so.1"], 50, "first");
function checkWith(programs, args) {
  const dir = join(scratch, `run-${++n}`);
  const bin = join(dir, "bin");
  mkdirSync(join(dir, "home"), { recursive: true });
  mkdirSync(bin);
  for (const name of ["uname", "grep", "sed", "cat", "readlink", ...programs]) {
    const found = spawnSync("/bin/sh", ["-c", `command -v ${name}`], { encoding: "utf8" }).stdout.trim();
    if (!found.startsWith("/")) throw new Error(`${name} isn't installed as a program`);
    writeFileSync(join(bin, name), `#!/bin/sh\nexec '${found}' "$@"\n`);
    chmodSync(join(bin, name), 0o755);
  }
  writeFileSync(join(dir, "ldconfig.txt"), RUN_LIBS);
  const awk = spawnSync("/bin/sh", ["-c", "command -v awk"], { encoding: "utf8" }).stdout.trim();
  writeFileSync(join(bin, "ldconfig"), `#!/bin/sh\n[ "$1" = -p ] && exec '${awk}' '{ print }' "${join(dir, "ldconfig.txt")}"\nexit 0\n`);
  chmodSync(join(bin, "ldconfig"), 0o755);
  const bash = spawnSync("/bin/sh", ["-c", "command -v bash"], { encoding: "utf8" }).stdout.trim();
  const env = { HOME: join(dir, "home"), USER: "hersir", PATH: bin };
  const r = spawnSync(bash, [join(ROOT, "install.sh"), ...args], { cwd: dir, env, encoding: "utf8" });
  return { code: r.status, out: `${r.stdout}${r.stderr}` };
}
const BUILD_NEEDS = /Rust|Node\.js|npm|pkg-config|compiler|cargo|Command Line Tools|build-essential|base-devel|-devel?\b/;

describe.runIf(linux)("install.sh --check --skip-build: what a ready-made build needs to run (GA-252)", () => {
  it("with git, ssh-keygen and WebKitGTK 4.1, and no Rust, Node.js or compiler, has everything Hersir needs to run", () => {
    const r = checkWith(["git", "ssh-keygen"], ["--check", "--skip-build"]);
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain("Everything Hersir needs to run is here.");
    expect(r.out).not.toMatch(BUILD_NEEDS);
    expect(r.out).not.toContain("Missing system packages");
    // the note about Claude Code stays
    expect(r.out).toContain("Claude Code: not found.");
  });

  it("names git and ssh-keygen when they are missing, with the line that installs what Hersir needs to run", () => {
    const r = checkWith([], ["--check", "--skip-build"]);
    expect(r.code).toBe(1);
    expect(r.out).toContain("Missing system packages: git ssh-keygen \nInstall them with:\n  ");
    const line = r.out.split("Install them with:\n  ")[1].split("\n")[0];
    expect(line).toMatch(/git/);
    expect(line).toMatch(/openssh|OpenSSH/);
    expect(r.out).not.toMatch(BUILD_NEEDS);
    expect(r.out).not.toContain("Everything Hersir needs");
  });

  it("without --skip-build, names what a build needs, as before", () => {
    const r = checkWith(["git", "ssh-keygen"], ["--check"]);
    expect(r.code).toBe(1);
    expect(r.out).toContain("Missing system packages: pkg-config cc");
    expect(r.out).toContain("Rust is missing");
    expect(r.out).toContain("Node.js 20 or newer is missing");
    expect(r.out).toContain("npm is missing");
    expect(r.out).not.toContain("Everything Hersir needs");
  });
});
