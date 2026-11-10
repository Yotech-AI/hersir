// Tests for the installers' CI (GA-223): the steps of .github/workflows/install.yml and one-line.yml that decide
// something, taken from the workflows as they are and run here with bash, as GitHub runs a bash step (-eo pipefail),
// against stand-ins for the package managers, sudo, id, curl and sleep. Install's "System packages" step must run
// exactly the line install.sh --check prints for each distribution, only made non-interactive and without sudo as
// root. One-line install's step for Linux and macOS must try the line again when it can't read the latest release, at
// most 3 times a minute apart, and pass only when the installed Gizai is the latest release. That the lines really
// install Gizai only CI shows. Not on Windows; the package step only on Linux, where it runs.
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const unix = process.platform !== "win32";
const linux = process.platform === "linux";
const scratch = realpathSync(mkdtempSync(join(tmpdir(), "install-ci-")));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));

const read = (path) => readFileSync(join(ROOT, path), "utf8");

// The script of a workflow's step, by its name: the block under its "run: |", without its indent. With job, that job's
// step of that name (install.yml's build, install and update jobs each have a package step, GA-252).
function stepRun(workflow, name, job) {
  const lines = read(workflow).split("\n");
  const from = job ? lines.indexOf(`  ${job}:`) : 0;
  if (from < 0) throw new Error(`${workflow} has no job ${job}`);
  const at = lines.findIndex((l, i) => i > from && l.trim() === `- name: ${name}`);
  if (at < 0) throw new Error(`${workflow} has no step named ${name}`);
  const keys = " ".repeat(lines[at].indexOf("-") + 2);
  let i = at + 1;
  while (i < lines.length && lines[i] !== `${keys}run: |`) {
    if (lines[i].startsWith(`${keys.slice(2)}- `)) throw new Error(`${workflow}'s step ${name} has no run: | block`);
    i++;
  }
  const body = [];
  for (i++; i < lines.length && (lines[i].trim() === "" || lines[i].search(/\S/) > keys.length); i++) body.push(lines[i]);
  const pad = Math.min(...body.filter((l) => l.trim()).map((l) => l.search(/\S/)));
  return `${body.map((l) => l.slice(pad)).join("\n").trimEnd()}\n`;
}

// The steps of a workflow's job: each one's lines, from its "- " on.
function jobSteps(workflow, job) {
  const lines = read(workflow).split("\n");
  const start = lines.indexOf(`  ${job}:`);
  if (start < 0) throw new Error(`${workflow} has no job ${job}`);
  const end = lines.findIndex((l, i) => i > start && /^ {2}\S/.test(l));
  const job_ = lines.slice(start, end < 0 ? lines.length : end);
  const from = job_.indexOf("    steps:");
  const steps = [];
  for (const line of job_.slice(from + 1)) {
    if (line.startsWith("      - ")) steps.push([line]);
    else if (steps.length && /^ {8}\S/.test(line)) steps.at(-1).push(line);
  }
  return steps.map((s) => ({
    name: (s.find((l) => /^( {6}- | {8})name: /.test(l)) ?? s.find((l) => /uses: /.test(l)) ?? "").replace(/.*?(name|uses): /, ""),
    if: (s.find((l) => /^ {8}if: /.test(l)) ?? "").replace(/^ {8}if: /, ""),
  }));
}

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

// A step's script, run as GitHub runs shell: bash.
function runStep(script, dir, env) {
  writeFileSync(join(dir, "step.sh"), script);
  const run = spawnSync("bash", ["--noprofile", "--norc", "-eo", "pipefail", join(dir, "step.sh")], {
    cwd: dir,
    encoding: "utf8",
    env: { ...process.env, PATH: `${join(dir, "bin")}:${process.env.PATH}`, RAN: join(dir, "ran.txt"), ...env },
  });
  const ran = existsSync(join(dir, "ran.txt")) ? readFileSync(join(dir, "ran.txt"), "utf8").replace(/\n$/, "").split("\n") : [];
  return { status: run.status, out: `${run.stdout}${run.stderr}`, ran };
}

// install.sh from where it reads /etc/os-release to where it prints the package line, with that os-release file
// instead: the check as it prints on a system missing WebKitGTK, then exit 1, as install.sh --check does. With
// skipBuild, as install.sh --check --skip-build prints it, the line to run a ready-made build (GA-252).
function checkPrinting(osRelease, skipBuild = false) {
  const lines = read("install.sh").split("\n");
  const from = lines.indexOf('distro=""');
  const to = lines.indexOf('  if [ "$SKIP_BUILD" = 1 ]; then say "  $(run_deps_command)"; else say "  $(deps_command)"; fi');
  expect(from).toBeGreaterThan(0);
  expect(lines[to + 1]).toBe("fi");
  return [
    "#!/usr/bin/env bash",
    lines.find((l) => l.startsWith("say() {")),
    `SKIP_BUILD=${skipBuild ? 1 : 0} missing_cmds=() missing_libs=(webkit2gtk-4.1)`,
    ...lines.slice(from, to + 2).map((l) => l.replaceAll("/etc/os-release", osRelease)),
    "exit 1",
    "",
  ].join("\n");
}

const PACKAGES = "System packages, as install.sh --check names them";

// The four systems Install runs on, with their os-release, the line install.sh --check prints there, and what the
// step runs: the same words, with -y, --noconfirm or -n after the install command, and without sudo in a container.
const SYSTEMS = [
  {
    system: "ubuntu-24.04",
    osRelease: 'ID=ubuntu\nID_LIKE=debian\nVERSION_ID="24.04"\n',
    uid: "1001",
    printed:
      "sudo apt install libwebkit2gtk-4.1-dev build-essential curl wget file libxdo-dev libssl-dev libayatana-appindicator3-dev librsvg2-dev pkg-config git",
    ran: [
      "sudo [apt] [install] [-y] [libwebkit2gtk-4.1-dev] [build-essential] [curl] [wget] [file] [libxdo-dev] [libssl-dev] [libayatana-appindicator3-dev] [librsvg2-dev] [pkg-config] [git]",
    ],
  },
  {
    system: "fedora",
    osRelease: 'ID=fedora\nVERSION_ID=43\n',
    uid: "0",
    printed:
      "sudo dnf install webkit2gtk4.1-devel openssl-devel curl wget file libappindicator-gtk3-devel librsvg2-devel git && sudo dnf group install c-development",
    ran: [
      "dnf [install] [-y] [webkit2gtk4.1-devel] [openssl-devel] [curl] [wget] [file] [libappindicator-gtk3-devel] [librsvg2-devel] [git]",
      "dnf [group] [install] [-y] [c-development]",
    ],
  },
  {
    system: "archlinux",
    osRelease: "ID=arch\nBUILD_ID=rolling\n",
    uid: "0",
    printed:
      "sudo pacman -S --needed webkit2gtk-4.1 base-devel curl wget file openssl appmenu-gtk-module libayatana-appindicator librsvg git",
    ran: [
      "pacman [-S] [--noconfirm] [--needed] [webkit2gtk-4.1] [base-devel] [curl] [wget] [file] [openssl] [appmenu-gtk-module] [libayatana-appindicator] [librsvg] [git]",
    ],
  },
  {
    system: "opensuse/tumbleweed",
    osRelease: 'ID="opensuse-tumbleweed"\nID_LIKE="opensuse suse"\n',
    uid: "0",
    printed:
      "sudo zypper in 'pkgconfig(webkit2gtk-4.1)' libopenssl-devel curl wget file libappindicator3-1 librsvg-devel git && sudo zypper in -t pattern devel_basis",
    ran: [
      "zypper [-n] [in] [pkgconfig(webkit2gtk-4.1)] [libopenssl-devel] [curl] [wget] [file] [libappindicator3-1] [librsvg-devel] [git]",
      "zypper [-n] [in] [-t] [pattern] [devel_basis]",
    ],
  },
];

// A folder with install.sh's check for this system and the stand-ins, for Install's package step. With skipBuild, the
// check as install.sh --check --skip-build prints it.
function packageStep({ system, osRelease, uid }, skipBuild = false) {
  const dir = mkdtempSync(join(scratch, `${system.replace("/", "-")}-`));
  writeFileSync(join(dir, "os-release"), osRelease);
  writeFileSync(join(dir, "install.sh"), checkPrinting(join(dir, "os-release"), skipBuild));
  for (const name of ["sudo", "apt", "apt-get", "dnf", "pacman", "zypper"]) tool(join(dir, "bin"), name, RECORDS);
  tool(join(dir, "bin"), "id", `#!/bin/sh\nif [ "$1" = -u ]; then echo ${uid}; else exec /usr/bin/id "$@"; fi\n`);
  return dir;
}

describe.runIf(linux)("install.yml: the system packages, as install.sh --check names them", () => {
  const script = stepRun(".github/workflows/install.yml", PACKAGES, "install");

  it("is a script with no expressions in it, so what runs here is what runs in CI", () => {
    expect(script).toContain("bash ./install.sh --check");
    expect(script).not.toContain("${{");
  });

  it.each(SYSTEMS)("on $system runs exactly the printed line, made non-interactive", (s) => {
    const r = runStep(script, packageStep(s), {});
    expect(r.status, r.out).toBe(0);
    expect(r.out).toContain(`Install them with:\n  ${s.printed}\n`);
    expect(r.out).toContain(`install.sh --check says: ${s.printed}\n`);
    expect(r.ran).toEqual(s.ran);
  });

  it("openSUSE's line names WebKitGTK 4.1 by its pkg-config name, not Tauri 1's webkit2gtk3-soup2-devel", () => {
    const suse = SYSTEMS.find((s) => s.system === "opensuse/tumbleweed");
    const r = runStep(script, packageStep(suse), {});
    expect(r.out).toContain("'pkgconfig(webkit2gtk-4.1)'");
    expect(read("install.sh")).not.toContain("webkit2gtk3-soup2-devel");
  });

  it("fails, and says so, when the printed line fails", () => {
    const r = runStep(script, packageStep(SYSTEMS[2]), { FAIL_PACMAN: "1" });
    expect(r.status).toBe(1);
    expect(r.out).toContain("The package line install.sh --check prints failed.");
  });

  it("fails when install.sh --check names no packages to install", () => {
    const dir = packageStep(SYSTEMS[0]);
    writeFileSync(join(dir, "install.sh"), '#!/bin/sh\necho "Everything Gizai needs to build is here."\n');
    const r = runStep(script, dir, {});
    expect(r.status).toBe(1);
    expect(r.out).toContain("install.sh --check named no system packages to install.");
    expect(r.ran).toEqual([]);
  });
});

// GA-252: before the build, each install job installs only what a ready-made build needs to run, with the line
// install.sh --check --skip-build prints for that system (run_deps_command): no compiler, no -dev packages.
const RUN_PACKAGES = "System packages to run Hersir, as install.sh --check --skip-build names them";
const RUN_LINES = {
  "ubuntu-24.04": {
    printed: "sudo apt install libwebkit2gtk-4.1-0 libayatana-appindicator3-1 git openssh-client",
    ran: ["sudo [apt] [install] [-y] [libwebkit2gtk-4.1-0] [libayatana-appindicator3-1] [git] [openssh-client]"],
  },
  fedora: {
    printed: "sudo dnf install webkit2gtk4.1 libappindicator-gtk3 git openssh-clients",
    ran: ["dnf [install] [-y] [webkit2gtk4.1] [libappindicator-gtk3] [git] [openssh-clients]"],
  },
  archlinux: {
    printed: "sudo pacman -S --needed webkit2gtk-4.1 libayatana-appindicator git openssh",
    ran: ["pacman [-S] [--noconfirm] [--needed] [webkit2gtk-4.1] [libayatana-appindicator] [git] [openssh]"],
  },
  "opensuse/tumbleweed": {
    printed: "sudo zypper in libwebkit2gtk-4_1-0 libappindicator3-1 git openssh-clients",
    ran: ["zypper [-n] [in] [libwebkit2gtk-4_1-0] [libappindicator3-1] [git] [openssh-clients]"],
  },
};

describe.runIf(linux)("install.yml: the packages to run a ready-made build, as install.sh --check --skip-build names them (GA-252)", () => {
  const script = stepRun(".github/workflows/install.yml", RUN_PACKAGES, "install");

  it("is a script with no expressions in it, and runs install.sh --check --skip-build", () => {
    expect(script).toContain("bash ./install.sh --check --skip-build");
    expect(script).not.toContain("${{");
  });

  it.each(SYSTEMS)("on $system runs exactly the printed line to run Hersir, made non-interactive", (s) => {
    const r = runStep(script, packageStep(s, true), {});
    expect(r.status, r.out).toBe(0);
    expect(r.out).toContain(`Install them with:\n  ${RUN_LINES[s.system].printed}\n`);
    expect(r.out).toContain(`install.sh --check --skip-build says: ${RUN_LINES[s.system].printed}\n`);
    expect(r.ran).toEqual(RUN_LINES[s.system].ran);
  });

  it("names no compiler, Rust, Node.js or development package", () => {
    for (const { printed } of Object.values(RUN_LINES)) {
      expect(printed).not.toMatch(/-dev\b|-devel\b|build-essential|base-devel|devel_basis|c-development|pkg-config|rust|node/);
      expect(printed).toMatch(/openssh/);
    }
  });

  it("fails, and says so, when the printed line fails or names nothing", () => {
    const failed = runStep(script, packageStep(SYSTEMS[1], true), { FAIL_DNF: "1" });
    expect(failed.status).toBe(1);
    expect(failed.out).toContain("The package line install.sh --check --skip-build prints failed.");
    const dir = packageStep(SYSTEMS[0], true);
    writeFileSync(join(dir, "install.sh"), '#!/bin/sh\necho "Everything Hersir needs to run is here."\n');
    const none = runStep(script, dir, {});
    expect(none.status).toBe(1);
    expect(none.out).toContain("install.sh --check --skip-build named no system packages to install.");
  });

  it("comes before Node.js, Rust and the build's packages, and the ready-made install before them too", () => {
    const names = jobSteps(".github/workflows/install.yml", "install").map((s) => s.name);
    const run = names.indexOf(RUN_PACKAGES);
    const line = names.indexOf("The one line with the ready-made build (get.sh)");
    const lineWindows = names.indexOf("The one line with the ready-made build (get.ps1)");
    for (const later of ["actions/setup-node@v4", "dtolnay/rust-toolchain@stable", PACKAGES]) {
      expect(names.indexOf(later)).toBeGreaterThan(Math.max(run, line, lineWindows));
    }
    expect(Math.min(run, line, lineWindows)).toBeGreaterThan(0);
  });
});

describe("install.yml: the ready-made builds, packed as Release builds packs them (GA-252)", () => {
  it.runIf(unix)("the build job installs its build packages with Release builds' own step", () => {
    expect(stepRun(".github/workflows/install.yml", PACKAGES, "build")).toBe(
      stepRun(".github/workflows/release-builds.yml", PACKAGES),
    );
  });

  it("packs exactly the files --skip-build installs, and signs only with a throwaway key made in the job", () => {
    const text = read(".github/workflows/install.yml");
    expect(text).toContain("target/release/hersir target/release/hersir-mcp target/release/bundle/macos/Hersir.app");
    expect(text).toContain("foreach ($p in @('hersir.exe', 'hersir-mcp.exe'))");
    expect(text).toContain("ssh-keygen -q -t ed25519 -N '' -C 'Install test release key' -f \"$keys/test\"");
    expect(text).toContain('rm -rf "$keys"');
    // nothing reads a secret, and the release keys never come into it
    expect(text).not.toMatch(/secrets\./);
    expect(text).not.toContain(read("release-signers").split("\n")[0].split(" ")[3]);
  });
});

describe("install.yml: when it runs, and a version bump", () => {
  const on = read(".github/workflows/install.yml").split("\njobs:")[0];

  it("runs on pull requests that change an installer, get.sh or get.ps1 included, and by hand", () => {
    for (const path of ["install.sh", "install.ps1", "scripts/get.sh", "scripts/get.ps1", "src-tauri/tauri.conf.json"]) {
      expect(on).toContain(`      - ${path}\n`);
    }
    expect(on).toContain("  workflow_dispatch:");
  });

  it("skips every step that installs or builds on a version bump, one by one, and starts no container then", () => {
    const text = read(".github/workflows/install.yml");
    expect(text).toContain("container: ${{ needs.version.outputs.only != 'true' && matrix.container || '' }}");
    const steps = jobSteps(".github/workflows/install.yml", "install");
    expect(steps.length).toBeGreaterThan(10);
    const unguarded = steps.filter((s) => !s.if.startsWith("needs.version.outputs.only != 'true'"));
    // only a git config on Windows, which takes a second
    expect(unguarded.map((s) => s.name)).toEqual(["Keep LF line endings"]);
  });

  it("runs the install on the six systems, Fedora, Arch and Tumbleweed in containers", () => {
    const text = read(".github/workflows/install.yml");
    expect(text).toContain("name: install on ${{ matrix.system }}");
    for (const system of ["ubuntu-24.04", "fedora", "archlinux", "opensuse/tumbleweed", "macos-15", "windows-latest"]) {
      expect(text).toContain(`          - system: ${system}\n`);
    }
    for (const image of ["fedora:latest", "archlinux:latest", "opensuse/tumbleweed:latest"]) {
      expect(text).toContain(`            container: ${image}\n`);
    }
  });
});

// install.yml's install job's matrix: each system's keys, as written under its "- system:". (The build job has a
// matrix of its own, the three ready-made builds, GA-252.)
function matrix() {
  const systems = {};
  let at;
  const lines = read(".github/workflows/install.yml").split("\n");
  const start = lines.indexOf("  install:");
  const end = lines.findIndex((l, i) => i > start && /^ {2}\S/.test(l));
  for (const line of lines.slice(start, end < 0 ? lines.length : end)) {
    const system = line.match(/^ {10}- system: (.+)$/);
    if (system) systems[(at = system[1])] = {};
    else if (at && /^ {12}\w+: /.test(line)) {
      const [, key, value] = line.match(/^ {12}(\w+): (.*)$/);
      systems[at][key] = value;
    } else at = undefined;
  }
  return systems;
}

describe("install.yml: what a container gets before the package line", () => {
  it("installs git, curl and bash to start, and on Tumbleweed gawk, which git needs, instead of busybox's awk", () => {
    const m = matrix();
    expect(Object.keys(m)).toEqual(["ubuntu-24.04", "fedora", "archlinux", "opensuse/tumbleweed", "macos-15", "windows-latest"]);
    expect(m["ubuntu-24.04"].start).toBeUndefined();
    // and OpenSSH's client, for ssh-keygen, with tar and gzip, to check and unpack a ready-made build (GA-252)
    expect(m.fedora.start).toBe("dnf -y install --allowerasing git curl bash openssh-clients tar gzip");
    expect(m.archlinux.start).toBe("pacman -S --needed --noconfirm git curl bash openssh tar gzip");
    expect(m["opensuse/tumbleweed"].start).toBe("zypper -n install git curl bash gawk openssh-clients tar gzip");
    // the refresh only refreshes (or, on a rolling system, upgrades): it names no package
    expect(m["ubuntu-24.04"].refresh).toBe("sudo apt-get update");
    expect(m.fedora.refresh).toBe("dnf -y makecache");
    expect(m.archlinux.refresh).toBe("pacman -Syu --noconfirm");
    expect(m["opensuse/tumbleweed"].refresh).toBe("zypper -n refresh && zypper -n dup");
  });

  it("writes the container's own PATH to GITHUB_PATH first, before setup-node and Rust add theirs, in a container only", () => {
    const steps = jobSteps(".github/workflows/install.yml", "install");
    const names = steps.map((s) => s.name);
    const path = names.indexOf("The container's PATH");
    expect(path).toBeGreaterThanOrEqual(0);
    expect(path).toBeLessThan(names.indexOf("actions/setup-node@v4"));
    expect(path).toBeLessThan(names.indexOf("dtolnay/rust-toolchain@stable"));
    expect(path).toBeLessThan(names.indexOf(PACKAGES));
    expect(steps[path].if).toBe("needs.version.outputs.only != 'true' && matrix.container");
  });

  it.runIf(unix)("its PATH step adds the whole PATH as one line, in its order", () => {
    const lines = read(".github/workflows/install.yml").split("\n");
    const at = lines.findIndex((l) => l.trim() === "- name: The container's PATH");
    const run = lines.slice(at + 1, at + 5).find((l) => /^ {8}run: /.test(l)).replace(/^ {8}run: /, "");
    const dir = mkdtempSync(join(scratch, "container-path-"));
    const githubPath = join(dir, "github_path");
    writeFileSync(githubPath, "/opt/earlier\n");
    const PATH = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";
    const r = spawnSync("/bin/sh", ["-e", "-c", run], { encoding: "utf8", env: { PATH, GITHUB_PATH: githubPath } });
    expect(r.status, r.stderr).toBe(0);
    expect(readFileSync(githubPath, "utf8")).toBe(`/opt/earlier\n${PATH}\n`);
  });
});

const LINE = "The one line (Linux, macOS)";

// hersir.dev's install.sh, as curl hands it to sh: it can't read the latest release for its first $FAKE_API_FAILS runs;
// after that --check names Ubuntu's packages, --uninstall removes Gizai, and otherwise it installs gizai $FAKE_VERSION
// (or fails with $FAKE_FAIL).
const FAKE_GET = `#!/bin/sh
n=$(( $(cat "$RUNNER_TEMP/runs" 2>/dev/null || echo 0) + 1 )); echo "$n" > "$RUNNER_TEMP/runs"
echo "get.sh\${1:+ $1}" >> "$RAN"
# a release from 0.17.0 on installs the hersir names and reads HERSIR_PREFIX first (GA-248); an older one, gizai and GIZAI_PREFIX
case "$FAKE_VERSION" in 0.1[7-9].*|0.[2-9][0-9].*|[1-9]*) name=hersir P="\${HERSIR_PREFIX:-$GIZAI_PREFIX}" ;; *) name=gizai P="$GIZAI_PREFIX" ;; esac
echo "== Finding Hersir's latest release"
if [ "$n" -le "\${FAKE_API_FAILS:-0}" ]; then
  echo "Could not read the latest release from https://api.github.com/repos/Yotech-AI/hersir/releases/latest. Check your internet connection, then run this again."
  exit 1
fi
case "$1" in
  --check)
    echo "Missing system packages: webkit2gtk-4.1"
    echo "Install them with:"
    echo "  sudo apt install libwebkit2gtk-4.1-dev build-essential"
    exit 1 ;;
  --uninstall)
    rm -rf "$P/lib/$name" "$P/bin/$name" "$XDG_DATA_HOME/applications/$name.desktop" ;;
  *)
    if [ -n "\${FAKE_FAIL:-}" ]; then echo "$FAKE_FAIL"; exit 1; fi
    mkdir -p "$P/lib/$name" "$P/bin" "$XDG_DATA_HOME/applications"
    printf '#!/bin/sh\\necho "%s %s"\\n' "$name" "$FAKE_VERSION" > "$P/bin/$name"
    chmod +x "$P/bin/$name"
    echo "Exec=$P/lib/$name/$name-launch" > "$XDG_DATA_HOME/applications/$name.desktop" ;;
esac
`;

// One-line install's step for Linux and macOS, in a folder of its own, with curl handing out FAKE_GET.
function oneLine(env) {
  const dir = mkdtempSync(join(scratch, "one-line-"));
  writeFileSync(join(dir, "get.sh"), FAKE_GET);
  tool(join(dir, "bin"), "curl", `#!/bin/sh\necho "curl $*" >> "$RAN"\ncat "${join(dir, "get.sh")}"\n`);
  for (const name of ["sleep", "sudo"]) tool(join(dir, "bin"), name, RECORDS);
  const r = runStep(stepRun(".github/workflows/one-line.yml", LINE), dir, {
    RUNNER_OS: "macOS",
    RUNNER_TEMP: dir,
    // the step's env gives the prefix under both names (GA-248)
    HERSIR_PREFIX: join(dir, "hersir"),
    GIZAI_PREFIX: join(dir, "hersir"),
    XDG_DATA_HOME: join(dir, "share"),
    GIZAI_DATA_DIR: join(dir, "share", "gizai"),
    LATEST: "v0.13.0",
    FAKE_VERSION: "0.13.0",
    ...env,
  });
  return { ...r, dir };
}

describe.runIf(unix)("one-line.yml: hersir.dev's line on Linux and macOS", () => {
  it("is a script with no expressions in it, and runs hersir.dev's own line", () => {
    const script = stepRun(".github/workflows/one-line.yml", LINE);
    expect(script).not.toContain("${{");
    expect(script).toContain("line 'curl -fsSL https://hersir.dev/install.sh | sh'");
    expect(script).toContain("line 'curl -fsSL https://hersir.dev/install.sh | sh -s -- --uninstall'");
  });

  it("installs the latest release, then uninstalls it, with nothing tried again", () => {
    const r = oneLine({});
    expect(r.status, r.out).toBe(0);
    expect(r.out).toContain("The latest release: v0.13.0. Installed: gizai 0.13.0.");
    expect(r.ran).toEqual([
      "curl -fsSL https://hersir.dev/install.sh",
      "get.sh",
      "curl -fsSL https://hersir.dev/install.sh",
      "get.sh --uninstall",
    ]);
    expect(existsSync(join(r.dir, "hersir", "bin", "gizai"))).toBe(false);
    expect(existsSync(join(r.dir, "hersir", "lib", "gizai"))).toBe(false);
  });

  it("from 0.17.0 on, checks the hersir command a release installs, and its uninstall (GA-248)", () => {
    const r = oneLine({ LATEST: "v0.17.0", FAKE_VERSION: "0.17.0" });
    expect(r.status, r.out).toBe(0);
    expect(r.out).toContain("The latest release: v0.17.0. Installed: hersir 0.17.0.");
    expect(existsSync(join(r.dir, "hersir", "bin", "hersir"))).toBe(false);
    expect(existsSync(join(r.dir, "hersir", "lib", "hersir"))).toBe(false);
    const wrong = oneLine({ LATEST: "v1.0.0", FAKE_VERSION: "0.17.0" });
    expect(wrong.status).toBe(1);
    expect(wrong.out).toContain("The one line installed hersir 0.17.0, not the latest release, v1.0.0.");
  });

  it.runIf(linux)("on Ubuntu runs the check first, then the packages it names with apt -y, then the line", () => {
    const r = oneLine({ RUNNER_OS: "Linux" });
    expect(r.status, r.out).toBe(0);
    expect(r.ran.filter((l) => !l.startsWith("curl "))).toEqual([
      "get.sh --check",
      "sudo [apt-get] [update]",
      "sudo [apt] [install] [-y] [libwebkit2gtk-4.1-dev] [build-essential]",
      "get.sh",
      "get.sh --uninstall",
    ]);
  });

  it("tries the line again a minute later when it can't read the latest release, and then passes", () => {
    const r = oneLine({ FAKE_API_FAILS: "3" });
    expect(r.status, r.out).toBe(0);
    expect(r.ran.filter((l) => l.startsWith("sleep"))).toEqual(["sleep [60]", "sleep [60]", "sleep [60]"]);
    for (const n of [1, 2, 3]) expect(r.out).toContain(`Could not read the latest release: trying again in a minute (${n} of 3).`);
    expect(r.out).toContain("The latest release: v0.13.0. Installed: gizai 0.13.0.");
  });

  it("tries again at most 3 times, then fails and says why", () => {
    const r = oneLine({ FAKE_API_FAILS: "99" });
    expect(r.status).toBe(1);
    expect(r.ran.filter((l) => l.startsWith("get.sh"))).toHaveLength(4);
    expect(r.ran.filter((l) => l.startsWith("sleep"))).toHaveLength(3);
    expect(r.out).not.toContain("(4 of 3)");
    expect(r.out.trimEnd().split("\n").at(-1)).toBe(
      "The one line could not read Hersir's latest release from GitHub's API, also not after trying again 3 times, a minute apart. GitHub allows 60 calls an hour without a login, and the runners share them: run this again later.",
    );
  });

  it("doesn't try again when the line fails for another reason", () => {
    const r = oneLine({ FAKE_FAIL: "error: could not compile `gizai`" });
    expect(r.status).toBe(1);
    expect(r.ran.filter((l) => l.startsWith("sleep"))).toEqual([]);
    expect(r.ran.filter((l) => l.startsWith("get.sh"))).toEqual(["get.sh"]);
    expect(r.out).toContain("The one line didn't install Hersir.");
  });

  it("fails when what it installed isn't the latest release", () => {
    const r = oneLine({ FAKE_VERSION: "0.12.0" });
    expect(r.status).toBe(1);
    expect(r.out).toContain("The one line installed gizai 0.12.0, not the latest release, v0.13.0.");
    expect(r.ran).not.toContain("get.sh --uninstall");
  });
});

describe("one-line.yml: when it runs", () => {
  const text = read(".github/workflows/one-line.yml");
  const on = text.split("\njobs:")[0];

  // GA-252: the one line installs only a signed release, so it runs once sign-release.sh has uploaded the signature
  // and starts it, not when the release is published, which isn't signed yet then.
  it('is "One-line install", when sign-release.sh starts it, weekly, by hand, and on a pull request that changes it', () => {
    expect(text).toMatch(/^name: One-line install$/m);
    expect(on).not.toContain("  release:");
    expect(on).toMatch(/ {2}schedule:\n {4}- cron: "\d+ \d+ \* \* \d"/);
    expect(on).toContain("  workflow_dispatch:\n");
    expect(on).toContain("  pull_request:\n    paths:\n      - .github/workflows/one-line.yml\n");
    expect(read("scripts/sign-release.sh")).toMatch(/^github workflow run one-line\.yml --repo "\$REPO" \|\|$/m);
  });

  it("sets up no Rust or Node.js, so the line must install the ready-made build", () => {
    expect(text).not.toContain("actions/setup-node");
    expect(text).not.toContain("rust-toolchain");
  });

  it("checks that the installed Hersir has no mark of a download: no quarantine on macOS, no Zone.Identifier on Windows", () => {
    expect(text).toContain('if xattr -r -l "$app" | grep com.apple.quarantine; then fail "$app has a quarantine mark."; fi');
    expect(text).toContain("-Stream 'Zone.Identifier'");
  });

  it("runs on ubuntu-24.04, macos-15 and windows-latest, with nothing from the checkout", () => {
    expect(text).toContain("os: [ubuntu-24.04, macos-15, windows-latest]");
    expect(text).not.toContain("actions/checkout");
  });

  it("on Windows runs hersir.dev's own lines, to install and to uninstall", () => {
    expect(text).toContain("Invoke-Line 'irm https://hersir.dev/install.ps1 | iex'");
    expect(text).toContain("Invoke-Line '& ([scriptblock]::Create((irm https://hersir.dev/install.ps1))) -Uninstall'");
  });
});

// GA-244: GitHub retired the macOS image before 15 on 2 Nov 2026, and a job that asks for it stops before any step. The
// macOS jobs run on macos-15, by that name (macos-latest moves whenever GitHub moves it), and the docs name those checks.
// (The old name is left out on purpose: the card's own search for it must find only lead_merge_test.rs's test data.)
describe("the runners: macos-15, as GitHub retired the image before it", () => {
  const workflows = ["ci.yml", "install.yml", "one-line.yml"].map((f) => `.github/workflows/${f}`);
  const docs = ["docs/RELEASING.md", "docs/PLATFORMS.md", "docs/install.md"];
  // each macOS version a file names, as "macos-15" or "macOS 15"
  const versions = (text) => [...text.matchAll(/\bmacos[- ](\d+)\b/gi)].map((m) => m[1]);

  it("CI's job is named after its runner, so its checks are ubuntu-24.04, macos-15 and windows-latest", () => {
    const text = read(".github/workflows/ci.yml");
    expect(text).toContain("    name: ${{ matrix.os }}\n");
    expect(text).toContain("        os: [ubuntu-24.04, macos-15, windows-latest]\n    runs-on: ${{ matrix.os }}\n");
  });

  it("Install's macOS system runs on macos-15 itself, so its check is install on macos-15", () => {
    expect(read(".github/workflows/install.yml")).toContain("    runs-on: ${{ matrix.runner }}\n");
    expect(matrix()["macos-15"]).toEqual({ runner: "macos-15" });
  });

  it("no workflow asks for another macOS image or macos-latest", () => {
    for (const workflow of workflows) {
      const text = read(workflow);
      expect(versions(text).every((v) => v === "15"), workflow).toBe(true);
      expect(text, workflow).not.toContain("macos-latest");
    }
    // ci.yml's matrix, one-line.yml's, and install.yml's three jobs (the ready-made build, GA-252; install; and the
    // update from 0.16.0, GA-248), the last two each with its system and runner
    expect(workflows.flatMap((w) => versions(read(w)))).toHaveLength(7);
  });

  it("RELEASING.md names the checks as the workflows make them", () => {
    const releasing = read("docs/RELEASING.md");
    // when to release, a version bump's checks, and the release's checks from CI
    expect(releasing.split("`ubuntu-24.04`, `macos-15` and `windows-latest`").length - 1).toBe(3);
    for (const system of Object.keys(matrix())) expect(releasing).toContain(`\`install on ${system}\``);
  });

  it("the docs name macOS 15 and no other macOS version", () => {
    expect(docs.map((doc) => versions(read(doc)))).toEqual([Array(5).fill("15"), Array(3).fill("15"), ["15"]]);
    expect(read("docs/RELEASING.md")).toContain("on Ubuntu 24.04, macOS 15 and Windows, then uninstall it");
    const platforms = read("docs/PLATFORMS.md");
    expect(platforms).toContain("| macOS 15 (Apple Silicon) | Windows (latest) |");
    expect(platforms).toContain("on macOS 15 (Apple silicon) and on Windows, it installs Hersir");
    expect(platforms).toContain("as people run them, on Ubuntu 24.04, macOS 15 and Windows");
    expect(read("docs/install.md")).toContain("openSUSE Tumbleweed, macOS 15 on Apple silicon and Windows");
  });
});
