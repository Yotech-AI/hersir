// Tests for Release builds (GA-251): .github/workflows/release-builds.yml, which attaches ready-made builds to every
// release. The steps that decide something are taken from the workflow as it is and run here with bash, as GitHub runs
// a bash step (-eo pipefail), against stand-ins for gh, sudo, apt and the build: which commit or tag it builds, the
// archive's name, the system packages, the archive and its check (unpacked into a copy of the source, installed with
// the real install.sh --skip-build into a scratch folder), SHA256SUMS and the upload. The rest (when it runs, who may
// write, the contract's names, the docs) is read from the files. That the real builds work only CI shows, and the
// Windows steps (PowerShell) only run there.
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
  cpSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  realpathSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const WORKFLOW = ".github/workflows/release-builds.yml";
const unix = process.platform !== "win32";
const linux = process.platform === "linux";
const scratch = realpathSync(mkdtempSync(join(tmpdir(), "release-builds-")));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));

const read = (path) => readFileSync(join(ROOT, path), "utf8");
const workflow = read(WORKFLOW);

// The script of a workflow's step, by its name: the block under its "run: |", without its indent.
function stepRun(name) {
  const lines = workflow.split("\n");
  const at = lines.findIndex((l) => l.trim() === `- name: ${name}`);
  if (at < 0) throw new Error(`${WORKFLOW} has no step named ${name}`);
  const keys = " ".repeat(lines[at].indexOf("-") + 2);
  let i = at + 1;
  while (i < lines.length && lines[i] !== `${keys}run: |`) {
    if (lines[i].startsWith(`${keys.slice(2)}- `)) throw new Error(`${WORKFLOW}'s step ${name} has no run: | block`);
    i++;
  }
  const body = [];
  for (i++; i < lines.length && (lines[i].trim() === "" || lines[i].search(/\S/) > keys.length); i++) body.push(lines[i]);
  const pad = Math.min(...body.filter((l) => l.trim()).map((l) => l.search(/\S/)));
  return `${body.map((l) => l.slice(pad)).join("\n").trimEnd()}\n`;
}

// A job's lines, from its "  <id>:" to the next job.
function job(id) {
  const lines = workflow.split("\n");
  const start = lines.indexOf(`  ${id}:`);
  if (start < 0) throw new Error(`${WORKFLOW} has no job ${id}`);
  const end = lines.findIndex((l, i) => i > start && /^ {2}\S/.test(l));
  return lines.slice(start, end < 0 ? lines.length : end).join("\n");
}
const JOBS = [...workflow.split("\njobs:\n")[1].matchAll(/^ {2}([\w-]+):$/gm)].map((m) => m[1]);
const jobName = (id) => job(id).match(/^ {4}name: (.*)$/m)[1];

// Writes an executable stand-in.
function tool(dir, name, script) {
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, name), script);
  chmodSync(join(dir, name), 0o755);
}

// A stand-in that writes its name and its arguments, each in [], to $RAN, and exits with $FAIL_<NAME> (or 0).
const RECORDS = `#!/bin/sh
name="\${0##*/}"
{ printf '%s' "$name"; for a in "$@"; do printf ' [%s]' "$a"; done; echo; } >> "$RAN"
var="FAIL_$(printf '%s' "$name" | tr 'a-z-' 'A-Z_')"
eval "exit \\"\\\${$var:-0}\\""
`;

// A step's script, run in dir as GitHub runs shell: bash, with $GITHUB_OUTPUT. Returns its exit status, what it printed,
// what the stand-ins ran and what it wrote to $GITHUB_OUTPUT.
function runStep(script, dir, env) {
  writeFileSync(join(dir, "step.sh"), script);
  const output = join(dir, "github-output.txt");
  rmSync(output, { force: true });
  const run = spawnSync("bash", ["--noprofile", "--norc", "-eo", "pipefail", join(dir, "step.sh")], {
    cwd: dir,
    encoding: "utf8",
    env: {
      ...process.env,
      PATH: `${join(dir, "bin")}:${process.env.PATH}`,
      RAN: join(dir, "ran.txt"),
      GITHUB_OUTPUT: output,
      GITHUB_REPOSITORY: "Yotech-AI/hersir",
      ...env,
    },
  });
  const lines = (file) => (existsSync(file) ? readFileSync(file, "utf8").replace(/\n$/, "").split("\n") : []);
  const outputs = Object.fromEntries(lines(output).filter(Boolean).map((l) => [l.split("=")[0], l.slice(l.indexOf("=") + 1)]));
  return { status: run.status, out: `${run.stdout}${run.stderr}`, ran: lines(join(dir, "ran.txt")), outputs };
}

const folder = (name) => mkdtempSync(join(scratch, `${name}-`));
const sha256 = (file) => createHash("sha256").update(readFileSync(file)).digest("hex");

describe("Release builds: when it runs, and who may write to a release", () => {
  const on = workflow.split("\njobs:")[0];

  it("runs on a published release, by hand with a release's tag, and on a pull request that changes it or an installer", () => {
    expect(on).toContain("  release:\n    types: [published]\n");
    expect(on).toContain("  workflow_dispatch:\n    inputs:\n      tag:\n");
    expect(on).toMatch(/ {6}tag:\n(?: {8}.*\n)*? {8}required: true\n/);
    expect(on).toContain(
      "  pull_request:\n    paths:\n      - .github/workflows/release-builds.yml\n      - install.sh\n      - install.ps1\n",
    );
    expect(on).not.toMatch(/^ {2}(push|schedule|pull_request_target|workflow_run):/m);
  });

  it("reads with its token everywhere, and only 'upload to the release' may write, never on a pull request", () => {
    expect(on).toContain("\npermissions:\n  contents: read\n");
    expect(JOBS).toEqual(["what", "build", "sums", "upload"]);
    expect(workflow.split("contents: write")).toHaveLength(2);
    for (const id of JOBS.filter((j) => j !== "upload")) expect(job(id), id).not.toContain("permissions:");
    const upload = job("upload");
    expect(upload).toContain("    if: github.event_name != 'pull_request'\n");
    expect(upload).toContain("    permissions:\n      contents: write\n");
    expect(upload).toContain("    needs: [what, sums]\n");
  });

  it("uploads to a release only in that job, with the workflow's own token and no other secret", () => {
    expect(workflow.split("gh release upload").length - 1).toBe(2);
    expect(job("upload").split("gh release upload").length - 1).toBe(2);
    expect(workflow).not.toMatch(/secrets\./);
    const tokens = [...workflow.matchAll(/^\s*(GH_TOKEN|GITHUB_TOKEN|token):\s*(.*)$/gm)].map((m) => `${m[1]}: ${m[2]}`);
    expect(tokens).toEqual(["GH_TOKEN: ${{ github.token }}", "GH_TOKEN: ${{ github.token }}"]);
    // the build's checkout keeps no token for the build's own scripts
    expect(job("build")).toContain("          persist-credentials: false\n");
  });

  it("names its checks as RELEASING.md does: What to build, build <system>, SHA256SUMS and upload to the release", () => {
    expect(JOBS.map(jobName)).toEqual(["What to build", "build ${{ matrix.system }}", "SHA256SUMS", "upload to the release"]);
    const releasing = read("docs/RELEASING.md");
    for (const check of [
      "What to build",
      "build linux-x86_64",
      "build macos-arm64",
      "build windows-x86_64",
      "SHA256SUMS",
      "upload to the release",
    ]) {
      expect(releasing).toContain(`\`${check}\``);
    }
    expect(releasing).toContain("`upload to the release`, which a pull request skips: `COMPLETED SKIPPED`");
  });
});

describe("Release builds: the three systems and their archives (the contract)", () => {
  const build = job("build");

  it("builds linux-x86_64 on ubuntu-22.04, macos-arm64 on macos-15 and windows-x86_64 on windows-latest", () => {
    expect(build).toContain(
      [
        "          - system: linux-x86_64",
        "            runner: ubuntu-22.04",
        "            arch: X64",
        "            ext: tar.gz",
        "          - system: macos-arm64",
        "            runner: macos-15",
        "            arch: ARM64",
        "            ext: tar.gz",
        "          - system: windows-x86_64",
        "            runner: windows-latest",
        "            arch: X64",
        "            ext: zip",
      ].join("\n"),
    );
    expect(build).toContain("    runs-on: ${{ matrix.runner }}\n");
    expect(workflow).not.toContain("macos-latest");
    expect(workflow).not.toContain("ubuntu-latest");
  });

  it("checks out what 'What to build' chose, and builds it with that commit's own installer, as people do", () => {
    expect(build).toContain("          ref: ${{ needs.what.outputs.ref }}\n");
    expect(build).toContain("        run: /bin/bash ./install.sh --build-only\n");
    expect(build).toContain("        shell: powershell\n        run: |\n          powershell -NoProfile -ExecutionPolicy Bypass -File .\\install.ps1 -BuildOnly\n");
    // no caches: each build starts clean
    expect(workflow).not.toMatch(/actions\/cache|Swatinem\/rust-cache|cache: /);
  });

  it("packs exactly the files --skip-build installs, at the paths the build leaves them", () => {
    const pack = stepRun("Pack (Linux, macOS)");
    expect(pack).toContain("target/release/gizai target/release/gizai-mcp target/release/bundle/macos/Hersir.app\n");
    expect(pack).toContain("-czf \"$out\" target/release/gizai target/release/gizai-mcp\n");
    const zip = stepRun("Pack (Windows)");
    expect(zip).toContain("foreach ($p in @('gizai.exe', 'gizai-mcp.exe'))");
    expect(zip).toContain('"target/release/$p"');
  });

  it("signs nothing: no codesign, signtool or certificate in the workflow", () => {
    expect(workflow).not.toMatch(/codesign|signtool|notarytool|certificate|APPLE_|\.p12/i);
  });

  it("keeps each archive as an artifact named like the file, replaced when the run runs again", () => {
    const upload = (name) => workflow.match(new RegExp(`- uses: actions/upload-artifact@v4\\n {8}with:\\n {10}name: ${name}\\n(?: {10}.*\\n)*`))?.[0];
    expect(upload("\\$\\{\\{ steps\\.version\\.outputs\\.archive \\}\\}")).toContain("overwrite: true");
    expect(upload("SHA256SUMS")).toContain("overwrite: true");
  });
});

// The "What to build" step, with a stand-in gh that has the release, or not ($FAIL_GH).
describe.runIf(unix)("Release builds: what it builds", () => {
  const script = stepRun("The release's tag, or the pull request's commit");
  const step = (env) => {
    const dir = folder("what");
    tool(join(dir, "bin"), "gh", RECORDS);
    return runStep(script, dir, { RELEASE_TAG: "", INPUT_TAG: "", HEAD_SHA: "", ...env });
  };

  it("is a script with no expressions in it, so what runs here is what runs in CI", () => {
    expect(script).not.toContain("${{");
  });

  it("on a pull request: the pull request's last commit, no tag, and no question to GitHub", () => {
    const r = step({ GITHUB_EVENT_NAME: "pull_request", HEAD_SHA: "0123456789abcdef0123456789abcdef01234567" });
    expect(r.status, r.out).toBe(0);
    expect(r.outputs).toEqual({ ref: "0123456789abcdef0123456789abcdef01234567" });
    expect(r.ran).toEqual([]);
  });

  it.each([
    ["release", { RELEASE_TAG: "v0.16.0" }],
    ["workflow_dispatch", { INPUT_TAG: "v0.16.0" }],
  ])("on %s: the release's tag, once GitHub says the release is there", (event, env) => {
    const r = step({ GITHUB_EVENT_NAME: event, ...env });
    expect(r.status, r.out).toBe(0);
    expect(r.outputs).toEqual({ tag: "v0.16.0", ref: "refs/tags/v0.16.0" });
    expect(r.ran).toEqual(["gh [release] [view] [v0.16.0] [--repo] [Yotech-AI/hersir] [--json] [tagName]"]);
  });

  it.each(["", "0.16.0", "v0.16", "v0.16.0-beta.1", "production", "v0.16.0; echo hi"])(
    "by hand with the tag '%s', which isn't a version like v1.2.3: fails, and asks GitHub nothing",
    (tag) => {
      const r = step({ GITHUB_EVENT_NAME: "workflow_dispatch", INPUT_TAG: tag });
      expect(r.status).toBe(1);
      expect(r.out).toContain("isn't a version like v1.2.3, so there is no release to build.");
      expect(r.outputs).toEqual({});
      expect(r.ran).toEqual([]);
    },
  );

  it("by hand with a version that has no release: fails, and says so", () => {
    const r = step({ GITHUB_EVENT_NAME: "workflow_dispatch", INPUT_TAG: "v9.9.9", FAIL_GH: "1" });
    expect(r.status).toBe(1);
    expect(r.out).toContain("Yotech-AI/hersir has no release v9.9.9.");
    expect(r.outputs).toEqual({});
  });
});

// The "The version" step, in a folder with a tauri.conf.json and a stand-in git.
describe.runIf(unix)("Release builds: the version and the archive's name", () => {
  const script = stepRun("The version");
  const step = (env) => {
    const dir = folder("version");
    mkdirSync(join(dir, "src-tauri"));
    writeFileSync(join(dir, "src-tauri", "tauri.conf.json"), JSON.stringify({ productName: "Hersir", version: "1.2.3" }));
    tool(join(dir, "bin"), "git", "#!/bin/sh\necho 0123456789abcdef\n");
    return runStep(script, dir, { TAG: "", SYSTEM: "linux-x86_64", ARCH: "X64", EXT: "tar.gz", RUNNER_ARCH: "X64", ...env });
  };

  it("names the archive after the version in src-tauri/tauri.conf.json and the system", () => {
    expect(step({}).outputs).toEqual({ version: "1.2.3", archive: "hersir-1.2.3-linux-x86_64.tar.gz" });
    const mac = step({ SYSTEM: "macos-arm64", ARCH: "ARM64", RUNNER_ARCH: "ARM64" });
    expect(mac.outputs.archive).toBe("hersir-1.2.3-macos-arm64.tar.gz");
    const windows = step({ SYSTEM: "windows-x86_64", EXT: "zip" });
    expect(windows.outputs.archive).toBe("hersir-1.2.3-windows-x86_64.zip");
  });

  it("for a release: passes when the tag is that version, fails when it isn't", () => {
    expect(step({ TAG: "v1.2.3" }).status).toBe(0);
    const r = step({ TAG: "v1.2.4" });
    expect(r.status).toBe(1);
    expect(r.out).toContain("The release is v1.2.4, but its src-tauri/tauri.conf.json says version 1.2.3.");
    expect(r.outputs).toEqual({});
  });

  it("fails when the runner isn't the system the name says", () => {
    const r = step({ SYSTEM: "macos-arm64", ARCH: "ARM64", RUNNER_ARCH: "X64" });
    expect(r.status).toBe(1);
    expect(r.out).toContain("This runner is X64, not ARM64, so its build isn't macos-arm64.");
  });
});

// install.sh from where it reads /etc/os-release to where it prints the package line, with that os-release file
// instead: the check as it prints on a system missing WebKitGTK, then exit 1, as install.sh --check does.
function checkPrinting(osRelease) {
  const lines = read("install.sh").split("\n");
  const from = lines.indexOf('distro=""');
  const to = lines.indexOf('  say "  $(deps_command)"');
  expect(from).toBeGreaterThan(0);
  expect(lines[to + 1]).toBe("fi");
  return [
    "#!/usr/bin/env bash",
    lines.find((l) => l.startsWith("say() {")),
    "missing_cmds=() missing_libs=(webkit2gtk-4.1)",
    ...lines.slice(from, to + 2).map((l) => l.replaceAll("/etc/os-release", osRelease)),
    "exit 1",
    "",
  ].join("\n");
}

describe.runIf(linux)("Release builds: the system packages on ubuntu-22.04, as install.sh --check names them", () => {
  const script = stepRun("System packages, as install.sh --check names them");
  const step = (installSh) => {
    const dir = folder("packages");
    writeFileSync(join(dir, "os-release"), 'ID=ubuntu\nID_LIKE=debian\nVERSION_ID="22.04"\n');
    writeFileSync(join(dir, "install.sh"), installSh ?? checkPrinting(join(dir, "os-release")));
    for (const name of ["sudo", "apt", "apt-get"]) tool(join(dir, "bin"), name, RECORDS);
    return runStep(script, dir, {});
  };

  it("is a script with no expressions in it, and runs the release's own install.sh --check", () => {
    expect(script).toContain("bash ./install.sh --check");
    expect(script).not.toContain("${{");
  });

  it("runs exactly the line install.sh --check prints for Ubuntu, with apt -y, after apt-get update", () => {
    const r = step();
    expect(r.status, r.out).toBe(0);
    expect(r.ran).toEqual([
      "sudo [apt-get] [update]",
      "sudo [apt] [install] [-y] [libwebkit2gtk-4.1-dev] [build-essential] [curl] [wget] [file] [libxdo-dev] [libssl-dev] [libayatana-appindicator3-dev] [librsvg2-dev] [pkg-config] [git]",
    ]);
  });

  it("installs nothing when install.sh --check names no packages", () => {
    const r = step('#!/bin/sh\necho "Everything Hersir needs to build is here."\n');
    expect(r.status, r.out).toBe(0);
    expect(r.out).toContain("install.sh --check names no system packages to install.");
    expect(r.ran).toEqual([]);
  });
});

// Pack (Linux, macOS), then the check: the archive unpacked into a copy of the source (install.sh, tauri.conf.json and
// the icons), installed with the real install.sh --skip-build into a scratch prefix, home and data folder. The build is
// a stand-in: a gizai that says its version.
describe.runIf(linux)("Release builds: the Linux archive, and its check with install.sh --skip-build", () => {
  const pack = stepRun("Pack (Linux, macOS)");
  const check = stepRun("Check the archive (Linux, macOS)");
  const ARCHIVE = "hersir-1.2.3-linux-x86_64.tar.gz";

  // A build in target/release whose gizai says gizai <says>, packed; and a fresh copy of the source to unpack it into.
  function packed({ says = "1.2.3", files = ["gizai", "gizai-mcp"] } = {}) {
    const dir = folder("archive");
    const build = join(dir, "build");
    for (const f of files) {
      tool(join(build, "target", "release"), f, `#!/bin/sh\n[ "$1" = --version ] && echo "${f} ${says}"\nexit 0\n`);
    }
    writeFileSync(join(build, "target", "release", "gizai.d"), "not in the archive\n");
    const temp = join(dir, "runner-temp");
    mkdirSync(temp);
    const r = runStep(pack, build, { RUNNER_OS: "Linux", RUNNER_TEMP: temp, ARCHIVE });
    const fresh = join(temp, "fresh");
    mkdirSync(join(fresh, "src-tauri"), { recursive: true });
    cpSync(join(ROOT, "install.sh"), join(fresh, "install.sh"));
    cpSync(join(ROOT, "src-tauri", "tauri.conf.json"), join(fresh, "src-tauri", "tauri.conf.json"));
    cpSync(join(ROOT, "src-tauri", "icons"), join(fresh, "src-tauri", "icons"), { recursive: true });
    return { dir, temp, fresh, pack: r, archive: join(temp, "builds", ARCHIVE) };
  }
  const checked = ({ dir, temp }, version = "1.2.3") =>
    runStep(check, dir, {
      ARCHIVE,
      VERSION: version,
      RUNNER_TEMP: temp,
      HOME: join(temp, "home"),
      GIZAI_PREFIX: join(temp, "scratch"),
      XDG_DATA_HOME: join(temp, "scratch", "share"),
      GIZAI_DATA_DIR: join(temp, "scratch", "share", "gizai"),
    });

  it("holds target/release/gizai and target/release/gizai-mcp and nothing else, owned by root", () => {
    const { pack: r, archive } = packed();
    expect(r.status, r.out).toBe(0);
    const list = spawnSync("tar", ["--numeric-owner", "-tzvf", archive], { encoding: "utf8" }).stdout.trim().split("\n");
    expect(list.map((l) => l.split(/\s+/).at(-1))).toEqual(["target/release/gizai", "target/release/gizai-mcp"]);
    for (const line of list) expect(line).toMatch(/^-rwx\S+ 0\/0 /);
  });

  it("unpacked into the source, installs with install.sh --skip-build into the scratch prefix and says the version", () => {
    const build = packed();
    const r = checked(build);
    expect(r.status, r.out).toBe(0);
    expect(r.out).toContain("gizai --version: gizai 1.2.3\n");
    const prefix = join(build.temp, "scratch");
    expect(readdirSync(join(prefix, "lib", "gizai")).sort()).toEqual(["gizai", "gizai-launch", "gizai-mcp"]);
    expect(existsSync(join(prefix, "bin", "gizai"))).toBe(true);
    expect(existsSync(join(prefix, "share", "applications", "gizai.desktop"))).toBe(true);
    // nothing outside the scratch folders: the home it was given stays empty
    expect(readdirSync(build.temp).sort()).toEqual(["builds", "fresh", "scratch"]);
  });

  it("fails when the installed gizai says another version", () => {
    const r = checked(packed({ says: "1.2.2" }));
    expect(r.status).toBe(1);
    expect(r.out).toContain("The installed gizai says gizai 1.2.2, not gizai 1.2.3.");
  });

  it("fails when the build has no gizai-mcp: the pack can't find it", () => {
    const r = packed({ files: ["gizai"] }).pack;
    expect(r.status).not.toBe(0);
  });
});

// Write SHA256SUMS, in a folder with the downloaded archives (each holding its own name).
describe.runIf(linux)("Release builds: SHA256SUMS", () => {
  const script = stepRun("Write SHA256SUMS");
  const NAMES = ["hersir-1.2.3-linux-x86_64.tar.gz", "hersir-1.2.3-macos-arm64.tar.gz", "hersir-1.2.3-windows-x86_64.zip"];
  const step = (names, env = {}) => {
    const dir = folder("sums");
    for (const name of names) writeFileSync(join(dir, name), `${name}\n`);
    return { dir, ...runStep(script, dir, { TAG: "", ...env }) };
  };

  it("lists the three archives, one line each as sha256sum writes it, and sha256sum -c passes", () => {
    const r = step(NAMES, { TAG: "v1.2.3" });
    expect(r.status, r.out).toBe(0);
    const sums = readFileSync(join(r.dir, "SHA256SUMS"), "utf8");
    expect(sums).toBe(NAMES.map((n) => `${sha256(join(r.dir, n))}  ${n}\n`).join(""));
    expect(r.out).toContain(NAMES.map((n) => `${n}: OK`).join("\n"));
    const again = spawnSync("sha256sum", ["-c", "SHA256SUMS"], { cwd: r.dir, encoding: "utf8" });
    expect(again.status, again.stdout).toBe(0);
  });

  it("on a pull request, with no tag: the same", () => {
    expect(step(NAMES).status).toBe(0);
  });

  it.each([
    ["one archive is missing", NAMES.slice(0, 2)],
    ["one is another version", [NAMES[0], "hersir-1.2.4-macos-arm64.tar.gz", NAMES[2]]],
    ["there is one too many", [...NAMES, "hersir-1.2.3-linux-arm64.tar.gz"]],
  ])("fails when %s", (_, names) => {
    const r = step(names);
    expect(r.status).toBe(1);
    expect(r.out).toContain(`, not ${NAMES.join(" ")}.`);
    expect(existsSync(join(r.dir, "SHA256SUMS"))).toBe(false);
  });

  it("fails when the archives aren't the release's version", () => {
    const r = step(NAMES, { TAG: "v1.2.4" });
    expect(r.status).toBe(1);
    expect(r.out).toContain("The archives are version 1.2.3, not the release's, v1.2.4.");
  });
});

// Upload to the release, in a folder with what the run kept, with a stand-in gh whose release lists $ASSETS.
describe.runIf(linux)("Release builds: the upload", () => {
  const script = stepRun("Upload to the release");
  const NAMES = ["hersir-1.2.3-linux-x86_64.tar.gz", "hersir-1.2.3-macos-arm64.tar.gz", "hersir-1.2.3-windows-x86_64.zip"];
  const ALL = [...NAMES, "SHA256SUMS"];
  const GH = `#!/bin/sh
{ printf 'gh'; for a in "$@"; do printf ' [%s]' "$a"; done; echo; } >> "$RAN"
[ "$2" = view ] && printf '%s\\n' "$ASSETS"
exit 0
`;
  const step = ({ assets = ALL, change } = {}) => {
    const dir = folder("upload");
    for (const name of NAMES) writeFileSync(join(dir, name), `${name}\n`);
    writeFileSync(join(dir, "SHA256SUMS"), NAMES.map((n) => `${sha256(join(dir, n))}  ${n}\n`).join(""));
    if (change) writeFileSync(join(dir, change), "changed after SHA256SUMS\n");
    tool(join(dir, "bin"), "gh", GH);
    return runStep(script, dir, { TAG: "v1.2.3", GH_TOKEN: "stand-in", ASSETS: assets.join("\n") });
  };

  it("is a script with no expressions in it", () => {
    expect(script).not.toContain("${{");
  });

  it("uploads the three archives, then SHA256SUMS, replacing any, and checks the release lists the four", () => {
    const r = step({ assets: ["hersir-1.2.2-linux-x86_64.tar.gz", ...ALL] });
    expect(r.status, r.out).toBe(0);
    expect(r.ran).toEqual([
      `gh [release] [upload] [v1.2.3] ${NAMES.map((n) => `[${n}]`).join(" ")} [--clobber] [--repo] [Yotech-AI/hersir]`,
      "gh [release] [upload] [v1.2.3] [SHA256SUMS] [--clobber] [--repo] [Yotech-AI/hersir]",
      "gh [release] [view] [v1.2.3] [--repo] [Yotech-AI/hersir] [--json] [assets] [--jq] [.assets[].name]",
    ]);
  });

  it.each(ALL)("fails when the release doesn't list %s afterwards", (missing) => {
    const r = step({ assets: ALL.filter((a) => a !== missing) });
    expect(r.status).toBe(1);
    expect(r.out).toContain(`The release v1.2.3 has no ${missing}.`);
  });

  it("uploads nothing when an archive doesn't match SHA256SUMS", () => {
    const r = step({ change: NAMES[1] });
    expect(r.status).not.toBe(0);
    expect(r.ran).toEqual([]);
  });
});

describe("Release builds: the docs", () => {
  const releasing = read("docs/RELEASING.md");
  const platforms = read("docs/PLATFORMS.md");

  it("RELEASING.md's check of a release names its four files and how to list them", () => {
    expect(releasing).toContain("gh release view vX.Y.Z --json assets");
    for (const file of [
      "hersir-X.Y.Z-linux-x86_64.tar.gz",
      "hersir-X.Y.Z-macos-arm64.tar.gz",
      "hersir-X.Y.Z-windows-x86_64.zip",
      "SHA256SUMS",
    ]) {
      expect(releasing).toContain(`\`${file}\``);
    }
    expect(releasing).toContain("`<sha256>  <file name>`, as `sha256sum` writes it");
    expect(releasing).toContain("gh workflow run release-builds.yml -f tag=vX.Y.Z");
  });

  it("RELEASING.md's rule: nothing signed beyond the build's own signature, never an account or a certificate", () => {
    const rule = releasing.split("\n").find((l) => l.startsWith("- **Nothing signed beyond"));
    expect(rule).toContain("on macOS, the ad-hoc signature every Apple silicon build has");
    expect(rule).toContain("never set up an account or buy a certificate");
    expect(releasing).not.toContain("No binaries attached yet");
  });

  it("PLATFORMS.md says the same: the workflow, the three archives, and no binaries line left", () => {
    expect(platforms).toContain("`.github/workflows/release-builds.yml` attaches ready-made builds to every release");
    for (const file of ["hersir-X.Y.Z-linux-x86_64.tar.gz", "hersir-X.Y.Z-macos-arm64.tar.gz", "hersir-X.Y.Z-windows-x86_64.zip"]) {
      expect(platforms).toContain(`\`${file}\``);
    }
    expect(platforms).not.toContain("no binaries in releases");
    expect(platforms).not.toContain("Releases stay as they are: no binaries attached.");
  });
});
