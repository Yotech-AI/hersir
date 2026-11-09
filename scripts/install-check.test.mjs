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
