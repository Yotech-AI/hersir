// Tests for scripts/get.sh (GA-150), the one-line install for Linux and macOS: curl -fsSL <url>/get.sh | sh. Each
// test pipes get.sh into /bin/sh as curl does (dash on Ubuntu, macOS's own sh), with HOME in a scratch folder. Most
// run against a small git repository with releases v0.1.0 and v0.2.0, whose install.sh only says how it was run and
// writes that down; the last ones run this repository's own install.sh --check. Not on Windows, where get.ps1 runs
// instead (CI checks that one in both PowerShells).
import { spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import {
  chmodSync,
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
import { dirname, join } from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { afterAll, describe, expect, it } from "vitest";

const GET_SH = fileURLToPath(new URL("./get.sh", import.meta.url));
const REPO_ROOT = fileURLToPath(new URL("..", import.meta.url));
const unix = process.platform !== "win32";
const scratch = realpathSync(mkdtempSync(join(tmpdir(), "get-sh-")));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));
// git never looks above the scratch folder for a repository, so a get.sh that lost its --git-dir can't check out or
// fetch tags into a repository around it (the temp folder may be inside a checkout of Gizai).
const CEILING = { GIT_CEILING_DIRECTORIES: scratch };

function git(dir, ...args) {
  const run = spawnSync(
    "git",
    ["-c", "user.name=Gizai test", "-c", "user.email=test@gizai.invalid", "-c", "commit.gpgsign=false",
      "-c", "tag.gpgsign=false", "-c", "core.autocrlf=false", ...args],
    { cwd: dir, encoding: "utf8", env: { ...process.env, ...CEILING } },
  );
  if (run.status !== 0) throw new Error(`git ${args.join(" ")} failed: ${run.stderr}`);
  return run.stdout.trim();
}

// Writes the files (null removes one) and commits them. Returns the commit.
function commit(dir, files, message) {
  for (const [path, text] of Object.entries(files)) {
    if (text === null) {
      rmSync(join(dir, path));
      continue;
    }
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), text);
  }
  git(dir, "add", "-A");
  git(dir, "commit", "-q", "--allow-empty", "-m", message);
  return git(dir, "rev-parse", "HEAD");
}

// A stand-in for install.sh: it says how it was run, writes a line to GET_TEST_LOG (its options, GIZAI_PREFIX,
// XDG_DATA_HOME, its own folder and what it read from stdin), and exits with FAKE_CHECK (with --check) or FAKE_INSTALL.
const FAKE_INSTALL = [
  "#!/usr/bin/env bash",
  'here="$(cd "${0%/*}" && pwd -P)"',
  'echo "install.sh $* (from $here)"',
  'input="$(cat)"',
  `printf '%s|%s|%s|%s|%s\\n' "$*" "\${GIZAI_PREFIX-}" "\${XDG_DATA_HOME-}" "$here" "$input" >> "$GET_TEST_LOG"`,
  'case " $* " in *" --check "*) exit "${FAKE_CHECK:-0}" ;; esac',
  'exit "${FAKE_INSTALL:-0}"',
  "",
].join("\n");

// The releases: v0.1.0, v0.2.0 (main), a branch next after it, and v0.9.0, which has no install.sh.
const fake = join(scratch, "gizai.git");
mkdirSync(fake);
git(fake, "init", "-q", "-b", "main");
const commits = {};
if (unix) {
  commits["v0.1.0"] = commit(fake, { "install.sh": FAKE_INSTALL, VERSION: "0.1.0\n", ".gitignore": "node_modules/\ntarget/\n" }, "0.1.0");
  git(fake, "tag", "-a", "-m", "v0.1.0", "v0.1.0");
  commits["v0.2.0"] = commit(fake, { VERSION: "0.2.0\n" }, "0.2.0");
  git(fake, "tag", "-a", "-m", "v0.2.0", "v0.2.0");
  git(fake, "checkout", "-q", "-b", "next");
  commits.next = commit(fake, { VERSION: "0.3.0-next\n" }, "Next");
  git(fake, "checkout", "-q", "-b", "no-installer", "v0.2.0");
  commits["v0.9.0"] = commit(fake, { "install.sh": null }, "Without install.sh");
  git(fake, "tag", "-a", "-m", "v0.9.0", "v0.9.0");
  git(fake, "checkout", "-q", "main");
}

// The release signature (GA-252): get.sh installs a release only once hersir-X.Y.Z.sig checks, over the line
// "hersir vX.Y.Z <commit>" and a newline, then SHA256SUMS. The fake releases are signed with a throwaway key made
// here, which HERSIR_RELEASE_SIGNERS names in place of Hersir's release keys, never with those. "other" is a second
// throwaway key, which no signers file names.
const NAMESPACE = "hersir-release";
const keys = join(scratch, "keys");
const signers = join(keys, "signers");
function sshKeygen(...args) {
  const run = spawnSync("ssh-keygen", args, { encoding: "utf8" });
  if (run.status !== 0) throw new Error(`ssh-keygen ${args.join(" ")} failed: ${run.stderr}`);
}
if (unix) {
  mkdirSync(keys);
  for (const name of ["test", "other"]) sshKeygen("-q", "-t", "ed25519", "-N", "", "-C", `get.test ${name}`, "-f", join(keys, name));
  writeFileSync(signers, `${NAMESPACE} namespaces="${NAMESPACE}" ${readFileSync(join(keys, "test.pub"), "utf8").trim()}\n`);
}

// Signs SHA256SUMS `sums` as release `tag` of `commit` with the key `key`: the .sig's text.
let signed = 0;
function signature(tag, commit, sums, key = "test") {
  const message = join(keys, `message-${++signed}`);
  writeFileSync(message, `hersir ${tag} ${commit}\n${sums}`);
  sshKeygen("-q", "-Y", "sign", "-f", join(keys, key), "-n", NAMESPACE, message);
  return readFileSync(`${message}.sig`, "utf8");
}

// A folder of a release's downloads, as HERSIR_BUILDS_URL reads them: SHA256SUMS, the archives and hersir-X.Y.Z.sig
// for each tag. Returns its file:// address. The default one, `builds`, lists only a Windows archive, so get.sh finds
// no ready-made build for Linux or macOS there and builds from source, as before GA-252.
function downloads(name, { sums, files = {}, sign = Object.keys(commits), key = "test", commit = (tag) => commits[tag] }) {
  const dir = join(scratch, `builds-${name}`);
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "SHA256SUMS"), sums);
  for (const [file, data] of Object.entries(files)) writeFileSync(join(dir, file), data);
  for (const tag of sign.filter((t) => /^v\d/.test(t))) {
    writeFileSync(join(dir, `hersir-${tag.slice(1)}.sig`), signature(tag, commit(tag), sums, key));
  }
  return pathToFileURL(dir).href;
}
const WINDOWS_ONLY = `${"ab".repeat(32)}  hersir-0.1.0-windows-x86_64.zip\n`;
const builds = unix ? downloads("default", { sums: WINDOWS_ONLY }) : "";

// GitHub's answer for the latest release, as a file: GIZAI_RELEASES_URL takes file:// addresses.
function release(tag) {
  const file = join(scratch, `release-${tag}.json`);
  writeFileSync(file, `{\n  "url": "https://api.github.com/repos/Yotech-AI/hersir/releases/1",\n  "tag_name": "${tag}",\n  "name": "${tag}",\n  "draft": false,\n  "prerelease": false\n}\n`);
  return pathToFileURL(file).href;
}

let cases = 0;

// A scratch HOME, with the folder get.sh uses in it when XDG_DATA_HOME isn't set.
function newCase() {
  const root = join(scratch, `case-${++cases}`);
  const home = join(root, "home");
  mkdirSync(home, { recursive: true });
  const share = join(home, ".local", "share");
  // TMPDIR, where get.sh keeps the release's downloads while it runs
  const tmp = join(root, "tmp");
  mkdirSync(tmp);
  // hersir-src (GA-248); gizai-src was its name before 0.17.0
  return { root, home, share, tmp, src: join(share, "hersir-src"), oldSrc: join(share, "gizai-src"), log: join(root, "install.log") };
}

// get.sh piped into /bin/sh, as curl -fsSL <url> | sh -s -- <args> runs it, or with stdin run as a file
// (sh get.sh <args>) with that on stdin. env adds or (with undefined) removes variables.
function getSh(c, args = [], env = {}, stdin = undefined) {
  const all = {
    HOME: c.home,
    PATH: process.env.PATH,
    GIZAI_REPO: fake,
    GIZAI_RELEASES_URL: release("v0.1.0"),
    HERSIR_BUILDS_URL: builds,
    HERSIR_RELEASE_SIGNERS: signers,
    TMPDIR: c.tmp,
    GET_TEST_LOG: c.log,
    ...env,
    ...CEILING,
  };
  const asFile = stdin !== undefined;
  const run = spawnSync("/bin/sh", asFile ? [GET_SH, ...args] : ["-s", "--", ...args], {
    input: asFile ? stdin : readFileSync(GET_SH),
    encoding: "utf8",
    env: Object.fromEntries(Object.entries(all).filter(([, v]) => v !== undefined)),
    timeout: 60_000,
  });
  // last: get.sh's own last line (git's progress goes to stderr)
  return { code: run.status, out: `${run.stdout}${run.stderr}`, last: run.stdout.trimEnd().split("\n").at(-1) };
}

// How the stand-in install.sh was run, one entry per run.
function runs(c) {
  if (!existsSync(c.log)) return [];
  return readFileSync(c.log, "utf8").trim().split("\n").map((line) => {
    const [args, prefix, xdg, from, input] = line.split("|");
    return { args, prefix, xdg, from, input };
  });
}

// A folder of programs that run the real ones by their full path, for a PATH with only these.
function tools(c, names, extra = {}) {
  const bin = join(c.root, "bin");
  mkdirSync(bin, { recursive: true });
  for (const name of names) {
    const found = spawnSync("/bin/sh", ["-c", `command -v ${name}`], { encoding: "utf8" }).stdout.trim();
    if (!found.startsWith("/")) throw new Error(`${name} isn't installed as a program`);
    writeFileSync(join(bin, name), `#!/bin/sh\nexec '${found}' "$@"\n`);
    chmodSync(join(bin, name), 0o755);
  }
  for (const [name, script] of Object.entries(extra)) {
    writeFileSync(join(bin, name), script);
    chmodSync(join(bin, name), 0o755);
  }
  return bin;
}

describe.skipIf(!unix)("scripts/get.sh", { timeout: 60_000 }, () => {
  it("gets the latest release into ~/.local/share/hersir-src, runs its install.sh --check, then install.sh", () => {
    const c = newCase();
    const r = getSh(c);
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain("Latest release: v0.1.0");
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.1.0"]);
    expect(runs(c).map((run) => run.args)).toEqual(["--check", ""]);
    expect(runs(c).every((run) => run.from === c.src)).toBe(true);
    // the check's result shows, before the install's
    expect(r.out).toMatch(/install\.sh --check \(from .*hersir-src\)[\s\S]*install\.sh  \(from .*hersir-src\)/);
    // the source folder has a repository of its own, and get.sh wrote nothing else
    expect(existsSync(join(c.src, ".git", "HEAD"))).toBe(true);
    expect(readdirSync(c.home)).toEqual([".local"]);
    expect(readdirSync(c.share)).toEqual(["hersir-src"]);
  });

  it("gives install.sh nothing on stdin, so neither run reads what was sent to get.sh", () => {
    const c = newCase();
    expect(getSh(c).code).toBe(0);
    const r = getSh(c, [], {}, "y\ny\n");
    expect(r.code, r.out).toBe(0);
    expect(runs(c).map((run) => run.input)).toEqual(["", "", "", ""]);
  });

  it("run again, updates the folder to the new release and installs again: changes go, target/ stays", () => {
    const c = newCase();
    expect(getSh(c).code).toBe(0);
    writeFileSync(join(c.src, "VERSION"), "changed by hand\n");
    writeFileSync(join(c.src, "stray.txt"), "left behind\n");
    mkdirSync(join(c.src, "node_modules", "left"), { recursive: true });
    mkdirSync(join(c.src, "target", "release"), { recursive: true });
    writeFileSync(join(c.src, "target", "release", "gizai"), "an earlier build\n");

    const r = getSh(c, [], { GIZAI_RELEASES_URL: release("v0.2.0") });
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain("Latest release: v0.2.0");
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.2.0"]);
    expect(readFileSync(join(c.src, "VERSION"), "utf8")).toBe("0.2.0\n");
    expect(existsSync(join(c.src, "stray.txt"))).toBe(false);
    expect(existsSync(join(c.src, "node_modules"))).toBe(false);
    expect(readFileSync(join(c.src, "target", "release", "gizai"), "utf8")).toBe("an earlier build\n");
    expect(git(c.src, "status", "--porcelain")).toBe("");
    expect(runs(c).map((run) => run.args)).toEqual(["--check", "", "--check", ""]);
  });

  it("updates a gizai-src that install.sh's own curl | bash cloned before, which becomes hersir-src with its build (GA-248)", () => {
    const c = newCase();
    mkdirSync(c.share, { recursive: true });
    git(c.share, "clone", "-q", "--depth", "1", "--branch", "main", pathToFileURL(fake).href, "gizai-src");
    mkdirSync(join(c.oldSrc, "target", "release"), { recursive: true });
    writeFileSync(join(c.oldSrc, "target", "release", "gizai"), "an earlier build\n");
    const r = getSh(c);
    expect(r.code, r.out).toBe(0);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.1.0"]);
    expect(runs(c).map((run) => run.args)).toEqual(["--check", ""]);
    expect(readFileSync(join(c.src, "target", "release", "gizai"), "utf8")).toBe("an earlier build\n");
    expect(readdirSync(c.share)).toEqual(["hersir-src"]);
  });

  it("removes a gizai-src from before 0.17.0 when there is a hersir-src already (GA-248)", () => {
    const c = newCase();
    expect(getSh(c).code).toBe(0);
    mkdirSync(join(c.oldSrc, "target"), { recursive: true });
    writeFileSync(join(c.oldSrc, "target", "old"), "old\n");
    const r = getSh(c, [], { GIZAI_RELEASES_URL: release("v0.2.0") });
    expect(r.code, r.out).toBe(0);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.2.0"]);
    expect(readdirSync(c.share)).toEqual(["hersir-src"]);
  });

  it("reads the HERSIR_ names, which win over the GIZAI_ ones from before 0.17.0 (GA-248)", () => {
    const c = newCase();
    const r = getSh(c, [], { GIZAI_RELEASES_URL: release("v0.1.0"), HERSIR_RELEASES_URL: release("v0.2.0") });
    expect(r.code, r.out).toBe(0);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.2.0"]);
    const ref = getSh(c, [], { GIZAI_REF: "v0.1.0", HERSIR_REF: "next" });
    expect(ref.code, ref.out).toBe(0);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits.next);
    const repo = getSh(c, [], { GIZAI_REPO: join(c.root, "no-such-repo"), HERSIR_REPO: fake, HERSIR_REF: "v0.1.0" });
    expect(repo.code, repo.out).toBe(0);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.1.0"]);
    const alone = getSh(c, [], { GIZAI_REPO: undefined, GIZAI_RELEASES_URL: undefined, HERSIR_REPO: fake, HERSIR_RELEASES_URL: release("v0.2.0") });
    expect(alone.code, alone.out).toBe(0);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.2.0"]);
  });

  it("makes a .git that was cut short again", () => {
    const c = newCase();
    mkdirSync(join(c.src, ".git"), { recursive: true });
    writeFileSync(join(c.src, ".git", "HEAD"), "");
    const r = getSh(c);
    expect(r.code, r.out).toBe(0);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.1.0"]);
  });

  it("works in the source folder's own repository only, never in one around it", () => {
    const c = newCase();
    git(c.home, "init", "-q", "-b", "main");
    const outer = commit(c.home, { "notes.txt": "mine\n" }, "My home folder");
    const r = getSh(c);
    expect(r.code, r.out).toBe(0);
    expect(git(c.src, "rev-parse", "--absolute-git-dir")).toBe(join(c.src, ".git"));
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.1.0"]);
    expect(git(c.home, "rev-parse", "HEAD")).toBe(outer);
    expect(existsSync(join(c.home, ".git", "FETCH_HEAD"))).toBe(false);
    expect(readFileSync(join(c.home, "notes.txt"), "utf8")).toBe("mine\n");
    expect(git(c.home, "status", "--porcelain", "--untracked-files=no")).toBe("");
  });

  it("writes nothing in Hersir's data folder", () => {
    const c = newCase();
    const data = join(c.share, "gizai");
    mkdirSync(join(data, "backups"), { recursive: true });
    writeFileSync(join(data, "gizai.db"), "my data\n");
    expect(getSh(c).code).toBe(0);
    expect(getSh(c, [], { GIZAI_RELEASES_URL: release("v0.2.0") }).code).toBe(0);
    expect(readdirSync(data).sort()).toEqual(["backups", "gizai.db"]);
    expect(readdirSync(join(data, "backups"))).toEqual([]);
    expect(readFileSync(join(data, "gizai.db"), "utf8")).toBe("my data\n");
    expect(readdirSync(c.share).sort()).toEqual(["gizai", "hersir-src"]);
    expect(existsSync(join(c.home, "Library"))).toBe(false);
  });

  it("uses XDG_DATA_HOME's hersir-src, and passes GIZAI_PREFIX and XDG_DATA_HOME on to install.sh", () => {
    const c = newCase();
    const xdg = join(c.root, "xdg");
    const prefix = join(c.root, "opt", "gizai");
    const r = getSh(c, [], { XDG_DATA_HOME: xdg, GIZAI_PREFIX: prefix });
    expect(r.code, r.out).toBe(0);
    expect(git(join(xdg, "hersir-src"), "rev-parse", "HEAD")).toBe(commits["v0.1.0"]);
    expect(existsSync(c.share)).toBe(false);
    expect(runs(c)).toEqual([
      { args: "--check", prefix, xdg, from: join(xdg, "hersir-src"), input: "" },
      { args: "", prefix, xdg, from: join(xdg, "hersir-src"), input: "" },
    ]);
  });

  it("passes its options on to install.sh, to the check and the install alike", () => {
    const c = newCase();
    expect(getSh(c, ["--build-only"]).code).toBe(0);
    expect(getSh(c, ["--skip-build"]).code).toBe(0);
    expect(runs(c).map((run) => run.args)).toEqual([
      "--check --build-only",
      "--build-only",
      "--check --skip-build",
      "--skip-build",
    ]);
  });

  it("with --check, runs only the check and ends with its result", () => {
    const c = newCase();
    expect(getSh(c, ["--check"]).code).toBe(0);
    const missing = getSh(c, ["--check"], { FAKE_CHECK: "1" });
    expect(missing.code).toBe(1);
    expect(missing.out).toContain("install.sh --check --check");
    expect(missing.out).not.toContain("Install what is missing above");
    expect(runs(c).map((run) => run.args)).toEqual(["--check --check", "--check --check"]);
  });

  it("stops when the check finds something missing, and says so, except with --skip-build", () => {
    const c = newCase();
    const r = getSh(c, [], { FAKE_CHECK: "1" });
    expect(r.code).toBe(1);
    expect(r.out).toContain("Install what is missing above, then run this again.");
    expect(runs(c).map((run) => run.args)).toEqual(["--check"]);

    const skip = getSh(c, ["--skip-build"], { FAKE_CHECK: "1" });
    expect(skip.code, skip.out).toBe(0);
    expect(runs(c).map((run) => run.args)).toEqual(["--check", "--check --skip-build", "--skip-build"]);
  });

  it("stops with the check's code when it fails otherwise, and ends with the install's code", () => {
    const c = newCase();
    expect(getSh(c, [], { FAKE_CHECK: "2" }).code).toBe(2);
    expect(runs(c).map((run) => run.args)).toEqual(["--check"]);
    expect(getSh(c, [], { FAKE_INSTALL: "3" }).code).toBe(3);
    expect(runs(c).map((run) => run.args)).toEqual(["--check", "--check", ""]);
  });

  it("runs --uninstall without the check, and refuses --check with --uninstall before getting anything", () => {
    const c = newCase();
    const r = getSh(c, ["--uninstall", "--purge"]);
    expect(r.code, r.out).toBe(0);
    expect(runs(c).map((run) => run.args)).toEqual(["--uninstall --purge"]);

    const d = newCase();
    const both = getSh(d, ["--check", "--uninstall"]);
    expect(both.code).toBe(2);
    expect(both.out).toContain("--check and --uninstall don't go together");
    expect(existsSync(d.share)).toBe(false);
    expect(runs(d)).toEqual([]);
  });

  it("with --help, shows the one line and its options, and does nothing", () => {
    const c = newCase();
    const r = getSh(c, ["--help"]);
    expect(r.code).toBe(0);
    expect(r.out).toContain("curl -fsSL https://hersir.dev/install.sh | sh");
    expect(r.out).not.toContain("raw.githubusercontent.com");
    expect(r.out).toContain("--uninstall");
    expect(readdirSync(c.home)).toEqual([]);
  });

  it("without git, bash or curl, says in one sentence what to install, and does nothing else", () => {
    const c = newCase();
    const none = getSh(c, [], { PATH: tools(c, ["uname"]) });
    expect(none.code).toBe(1);
    expect(none.out.trim()).toBe(
      "To install Hersir, first install git, bash and curl with your package manager, then run this again.",
    );
    expect(readdirSync(c.home)).toEqual([]);

    const d = newCase();
    const noGit = getSh(d, [], { PATH: tools(d, ["uname", "bash", "curl"]) });
    expect(noGit.code).toBe(1);
    expect(noGit.out.trim()).toBe(
      process.platform === "darwin"
        ? "To install Hersir, first install Apple's Command Line Tools, which bring git: run xcode-select --install, then run this again."
        : "To install Hersir, first install git with your package manager, then run this again.",
    );
    expect(readdirSync(d.home)).toEqual([]);

    const e = newCase();
    const noBash = getSh(e, [], { PATH: tools(e, ["uname", "git", "curl"]) });
    expect(noBash.code).toBe(1);
    expect(noBash.out.trim()).toBe("To install Hersir, first install bash with your package manager, then run this again.");
    expect(readdirSync(e.home)).toEqual([]);
    expect(runs(e)).toEqual([]);
  });

  it("needs no curl when GIZAI_REF says what to get", () => {
    const c = newCase();
    const r = getSh(c, [], { PATH: tools(c, ["uname", "git", "bash", "mkdir", "rm", "cat"]), GIZAI_REF: "v0.2.0" });
    expect(r.code, r.out).toBe(0);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.2.0"]);
  });

  it("on another system, points to get.ps1 and does nothing", () => {
    const c = newCase();
    const r = getSh(c, [], { PATH: tools(c, ["git", "bash", "curl"], { uname: "#!/bin/sh\necho MINGW64_NT-10.0-26100\n" }) });
    expect(r.code).toBe(1);
    expect(r.out.trim()).toBe(
      "This installs Hersir on Linux and macOS. On Windows, run this in PowerShell: irm https://hersir.dev/install.ps1 | iex",
    );
    expect(readdirSync(c.home)).toEqual([]);
  });

  it("gets what it downloads from GitHub, never from hersir.dev: the latest release from GitHub's API, the source from github.com", () => {
    // curl and git stand-ins that write down what they were asked and stop there, so nothing goes over the network
    const realGit = spawnSync("/bin/sh", ["-c", "command -v git"], { encoding: "utf8" }).stdout.trim();
    const net = (c) => join(c.root, "net.log");
    const offline = (c) => `${tools(c, [], {
      curl: `#!/bin/sh\nprintf 'curl %s\\n' "$*" >> '${net(c)}'\nexit 22\n`,
      git: `#!/bin/sh\ncase " $* " in *" fetch "*) printf 'git %s\\n' "$*" >> '${net(c)}'; exit 128 ;; esac\nexec '${realGit}' "$@"\n`,
    })}:${process.env.PATH}`;

    const c = newCase();
    const latest = getSh(c, [], { PATH: offline(c), GIZAI_REPO: undefined, GIZAI_RELEASES_URL: undefined });
    expect(latest.code).toBe(1);
    expect(latest.out).toContain(
      "Could not read the latest release from https://api.github.com/repos/Yotech-AI/hersir/releases/latest.",
    );
    const asked = readFileSync(net(c), "utf8").trim().split("\n");
    expect(asked).toHaveLength(1);
    expect(asked[0]).toMatch(/^curl .* https:\/\/api\.github\.com\/repos\/Yotech-AI\/hersir\/releases\/latest$/);
    expect(existsSync(c.share)).toBe(false);

    const d = newCase();
    const source = getSh(d, [], { PATH: offline(d), GIZAI_REPO: undefined, GIZAI_RELEASES_URL: undefined, GIZAI_REF: "v0.2.0" });
    expect(source.code).toBe(1);
    expect(source.out).toContain("Could not get v0.2.0 from https://github.com/Yotech-AI/hersir.git.");
    const fetched = readFileSync(net(d), "utf8").trim().split("\n");
    expect(fetched).toHaveLength(1);
    expect(fetched[0]).toMatch(/ fetch .* -- https:\/\/github\.com\/Yotech-AI\/hersir\.git v0\.2\.0$/);
    expect(runs(d)).toEqual([]);

    for (const log of [...asked, ...fetched]) expect(log).not.toContain("hersir.dev");
  });

  it("gets a branch or a commit instead of the latest release with GIZAI_REF, and stops at one that doesn't exist", () => {
    const c = newCase();
    const branch = getSh(c, [], { GIZAI_REF: "next", GIZAI_RELEASES_URL: undefined });
    expect(branch.code, branch.out).toBe(0);
    expect(branch.out).not.toContain("Latest release");
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits.next);

    const sha = getSh(c, [], { GIZAI_REF: commits["v0.1.0"] });
    expect(sha.code, sha.out).toBe(0);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.1.0"]);

    const missing = getSh(c, [], { GIZAI_REF: "no-such-ref" });
    expect(missing.code).toBe(1);
    expect(missing.out).toContain(`Could not get no-such-ref from ${realpathSync(fake)}.`);
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.1.0"]);
    expect(runs(c)).toHaveLength(4);
  });

  it("stops before getting anything when the latest release can't be read or its tag isn't a version", () => {
    const c = newCase();
    const unread = getSh(c, [], { GIZAI_RELEASES_URL: pathToFileURL(join(scratch, "no-such-release.json")).href });
    expect(unread.code).toBe(1);
    expect(unread.out).toContain("Could not read the latest release from");
    const nightly = getSh(c, [], { GIZAI_RELEASES_URL: release("nightly") });
    expect(nightly.code).toBe(1);
    expect(nightly.out).toContain("The latest release's tag, nightly, isn't a version like v1.2.3");
    const notGitHub = getSh(c, [], { GIZAI_RELEASES_URL: undefined });
    expect(notGitHub.code).toBe(1);
    expect(notGitHub.out).toContain("isn't on GitHub, so there is no latest release to ask it for");
    expect(existsSync(c.share)).toBe(false);
    expect(runs(c)).toEqual([]);
  });

  it("stops at a release without install.sh", () => {
    const c = newCase();
    const r = getSh(c, [], { GIZAI_RELEASES_URL: release("v0.9.0") });
    expect(r.code).toBe(1);
    expect(r.out).toContain("v0.9.0 has no install.sh, so it can't be installed this way.");
    expect(runs(c)).toEqual([]);
  });

  it("runs this repository's own install.sh --check, and its result shows", () => {
    const c = newCase();
    const r = getSh(c, ["--check"], { GIZAI_REPO: REPO_ROOT, GIZAI_REF: "HEAD", GIZAI_RELEASES_URL: undefined });
    expect(r.out).toContain("== Checking what Hersir needs");
    expect(r.out).toMatch(/Claude Code: (found|not found)/);
    expect(r.code === 0, r.out).toBe(r.out.includes("Everything Hersir needs to build is here."));
    expect(git(c.src, "rev-parse", "HEAD")).toBe(git(REPO_ROOT, "rev-parse", "HEAD"));
    expect(readdirSync(c.share)).toEqual(["hersir-src"]);

    // an option install.sh doesn't know stops at its check, with its code
    const typo = getSh(c, ["--chek"], { GIZAI_REPO: REPO_ROOT, GIZAI_REF: "HEAD", GIZAI_RELEASES_URL: undefined });
    expect(typo.code).toBe(2);
    expect(typo.out).toContain("unknown option: --chek");
    expect(readdirSync(c.share)).toEqual(["hersir-src"]);
  });
});

// GA-252: the release's signature first, then its ready-made build. A refused release runs nothing of itself, not
// even install.sh --check, and leaves the source folder as it was. A ready-made build is unpacked into the source folder
// and installed with install.sh --skip-build; without one, or with a wrong checksum, it builds as before.
const REFUSED = {
  unsigned: "v0.1.0 isn't signed yet: try again in a few minutes.",
  otherKey: "The signature of v0.1.0 doesn't check against Hersir's release keys, so nothing was installed.",
  otherCommit: "The signature of v0.1.0 is by Hersir's release key, but not for its commit",
};
const NO_SSH_KEYGEN =
  "Hersir checks each release's signature with ssh-keygen: install OpenSSH's client (openssh-client, openssh-clients or openssh), then run this again.";

// A folder of release downloads in which v0.1.0 has the ready-made build `archive` (a .tar.gz's bytes) for Linux on
// x86_64, listed in SHA256SUMS with `sum` (by default its own), and signed; `upload: false` leaves the archive out.
function readyMade(name, archive, { sum, upload = true, ...rest } = {}) {
  const file = "hersir-0.1.0-linux-x86_64.tar.gz";
  const sha = sum ?? createHash("sha256").update(archive).digest("hex");
  return downloads(name, { sums: `${sha}  ${file}\n${WINDOWS_ONLY}`, files: upload ? { [file]: archive } : {}, ...rest });
}

// A .tar.gz with these files (path: text) at these paths, as Release builds packs one.
function tarGz(name, files) {
  const dir = join(scratch, `archive-${name}`);
  for (const [path, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, path)), { recursive: true });
    writeFileSync(join(dir, path), text);
  }
  const run = spawnSync("tar", ["-czf", "-", "-C", dir, ...Object.keys(files)]);
  if (run.status !== 0) throw new Error(`tar failed: ${run.stderr}`);
  return run.stdout;
}
const BUILD = { "target/release/hersir": "hersir from the archive\n", "target/release/hersir-mcp": "hersir-mcp from the archive\n" };

// uname as on Linux on `machine`, whatever runs the tests: get.sh picks the archive by it.
function linuxOn(c, machine = "x86_64") {
  const bin = join(c.root, "uname-bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "uname"), `#!/bin/sh\ncase "$1" in -s) echo Linux ;; -m) echo ${machine} ;; *) exec uname "$@" ;; esac\n`);
  chmodSync(join(bin, "uname"), 0o755);
  return `${bin}:${process.env.PATH}`;
}

// A copy of the repository whose v0.1.0 tag is on another commit, whose install.sh writes MOVED_RAN when it runs.
const moved = join(scratch, "moved.git");
if (unix) {
  git(scratch, "clone", "-q", fake, moved);
  git(moved, "checkout", "-q", "v0.1.0");
  commits.moved = commit(moved, { "install.sh": '#!/bin/sh\necho ran > "$MOVED_RAN"\n' }, "Not the signed commit");
  git(moved, "tag", "-f", "-a", "-m", "v0.1.0", "v0.1.0");
}

describe.skipIf(!unix)("scripts/get.sh: the release's signature (GA-252)", { timeout: 60_000 }, () => {
  it("checks the signature over 'hersir vX.Y.Z <commit>' and SHA256SUMS, says so, and leaves no temporary files", () => {
    const c = newCase();
    const r = getSh(c);
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain("== Checking the signature of v0.1.0");
    expect(r.out).toContain(`Checked against ${signers} (HERSIR_RELEASE_SIGNERS), not Hersir's release keys.`);
    expect(r.out).toContain(`v0.1.0 is signed with Hersir's release key, for its commit ${commits["v0.1.0"]}.`);
    // the signature comes before anything of the release
    expect(r.out.indexOf("is signed with")).toBeLessThan(r.out.indexOf("install.sh --check"));
    expect(readdirSync(c.tmp)).toEqual([]);
  });

  it.each([
    ["without hersir-X.Y.Z.sig", () => downloads("unsigned", { sums: WINDOWS_ONLY, sign: [] }), REFUSED.unsigned],
    ["signed by another key", () => downloads("other-key", { sums: WINDOWS_ONLY, key: "other" }), REFUSED.otherKey],
    [
      "with SHA256SUMS changed after it was signed",
      () => {
        const url = downloads("changed-sums", { sums: WINDOWS_ONLY });
        writeFileSync(join(fileURLToPath(url), "SHA256SUMS"), WINDOWS_ONLY.replace("ab", "ba"));
        return url;
      },
      REFUSED.otherCommit,
    ],
    ["without SHA256SUMS", () => {
      const url = downloads("no-sums", { sums: WINDOWS_ONLY });
      rmSync(join(fileURLToPath(url), "SHA256SUMS"));
      return url;
    }, REFUSED.unsigned],
  ])("refuses a release %s: nothing of it runs, one line says why, also with --from-source", (_, make, says) => {
    const url = make();
    for (const args of [[], ["--from-source"], ["--check"]]) {
      const c = newCase();
      const r = getSh(c, args, { HERSIR_BUILDS_URL: url });
      expect(r.code, r.out).toBe(1);
      // its last line says why, in full where the test knows it all
      if (says === REFUSED.otherCommit) expect(r.last).toContain(says);
      else expect(r.last).toBe(says);
      expect(runs(c)).toEqual([]);
      expect(r.out).not.toContain("install.sh");
      // the release isn't checked out into the source folder
      expect(existsSync(join(c.src, "install.sh"))).toBe(false);
      expect(readdirSync(c.tmp)).toEqual([]);
    }
  });

  it("refuses a release whose tag is on another commit than the signed one, and runs nothing of it", () => {
    const c = newCase();
    const ran = join(c.root, "moved-ran");
    for (const args of [[], ["--from-source"]]) {
      const r = getSh(c, args, { GIZAI_REPO: moved, MOVED_RAN: ran });
      expect(r.code, r.out).toBe(1);
      expect(r.out).toContain(`${REFUSED.otherCommit} ${commits.moved} and its SHA256SUMS: this isn't the release that was signed, so nothing was installed.`);
      expect(existsSync(ran)).toBe(false);
      expect(existsSync(join(c.src, "install.sh"))).toBe(false);
    }
    expect(runs(c)).toEqual([]);
  });

  it("leaves an installed release as it is when the next one doesn't check", () => {
    const c = newCase();
    expect(getSh(c).code).toBe(0);
    const before = runs(c).length;
    const unsigned = downloads("unsigned-next", { sums: WINDOWS_ONLY, sign: ["v0.1.0"] });
    const r = getSh(c, [], { GIZAI_RELEASES_URL: release("v0.2.0"), HERSIR_BUILDS_URL: unsigned });
    expect(r.code, r.out).toBe(1);
    expect(r.out).toContain("v0.2.0 isn't signed yet: try again in a few minutes.");
    expect(git(c.src, "rev-parse", "HEAD")).toBe(commits["v0.1.0"]);
    expect(readFileSync(join(c.src, "VERSION"), "utf8")).toBe("0.1.0\n");
    expect(runs(c)).toHaveLength(before);
  });

  it("without HERSIR_RELEASE_SIGNERS checks against the built-in release keys, which the test key isn't", () => {
    const c = newCase();
    const r = getSh(c, [], { HERSIR_RELEASE_SIGNERS: undefined });
    expect(r.code, r.out).toBe(1);
    expect(r.out).not.toContain("HERSIR_RELEASE_SIGNERS");
    expect(r.out).toContain(REFUSED.otherKey);
    expect(runs(c)).toEqual([]);
  });

  it("stops when HERSIR_RELEASE_SIGNERS isn't a file", () => {
    const c = newCase();
    const r = getSh(c, [], { HERSIR_RELEASE_SIGNERS: join(c.root, "no-such-file") });
    expect(r.code).toBe(1);
    expect(r.out).toContain(`HERSIR_RELEASE_SIGNERS is ${join(c.root, "no-such-file")}, which isn't a file, so the signature of v0.1.0 can't be checked.`);
    expect(runs(c)).toEqual([]);
  });

  it("without ssh-keygen, says so in one sentence and does nothing else", () => {
    const c = newCase();
    const r = getSh(c, [], { PATH: tools(c, ["uname", "git", "bash", "curl"]) });
    expect(r.code).toBe(1);
    expect(r.out.trim()).toBe(NO_SSH_KEYGEN);
    expect(readdirSync(c.home)).toEqual([]);
    expect(runs(c)).toEqual([]);
  });

  it("with an ssh-keygen from before OpenSSH 8.1, which has no -Y, says the same and installs nothing", () => {
    const c = newCase();
    const old = tools(c, [], { "ssh-keygen": "#!/bin/sh\necho 'ssh-keygen: unknown option -- Y' >&2\necho 'usage: ssh-keygen [-q] [-b bits]' >&2\nexit 1\n" });
    const r = getSh(c, [], { PATH: `${old}:${process.env.PATH}` });
    expect(r.code).toBe(1);
    expect(r.last).toBe(NO_SSH_KEYGEN);
    expect(runs(c)).toEqual([]);
  });

  it("with HERSIR_REF installs a branch without a signature, and says so in one line", () => {
    const c = newCase();
    const r = getSh(c, [], { HERSIR_REF: "next", HERSIR_BUILDS_URL: pathToFileURL(join(scratch, "nothing-here")).href });
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain("next is a branch or a commit (HERSIR_REF), not a release: it has no signature to check, and is built from source.");
    expect(r.out).not.toContain("Checking the signature");
    expect(runs(c).map((run) => run.args)).toEqual(["--check", ""]);
  });

  it("reads GIZAI_BUILDS_URL too, and HERSIR_BUILDS_URL wins", () => {
    const c = newCase();
    const nowhere = pathToFileURL(join(scratch, "nothing-here")).href;
    const old = getSh(c, [], { HERSIR_BUILDS_URL: undefined, GIZAI_BUILDS_URL: builds });
    expect(old.code, old.out).toBe(0);
    const both = getSh(c, [], { HERSIR_BUILDS_URL: nowhere, GIZAI_BUILDS_URL: builds });
    expect(both.code).toBe(1);
    expect(both.out).toContain(REFUSED.unsigned);
  });
});

describe.skipIf(!unix)("scripts/get.sh: the release's ready-made build (GA-252)", { timeout: 60_000 }, () => {
  it("downloads this system's archive, checks it, unpacks it into the source and installs it with --skip-build", () => {
    const c = newCase();
    const url = readyMade("ready", tarGz("ready", { ...BUILD, "README.md": "not installed\n" }));
    const r = getSh(c, [], { PATH: linuxOn(c), HERSIR_BUILDS_URL: url });
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain("== Getting the ready-made build of v0.1.0 (hersir-0.1.0-linux-x86_64.tar.gz)");
    expect(r.out).toContain(`hersir-0.1.0-linux-x86_64.tar.gz matches its checksum in SHA256SUMS, and is unpacked into ${c.src}/target/release.`);
    expect(runs(c).map((run) => run.args)).toEqual(["--check --skip-build", "--skip-build"]);
    expect(readFileSync(join(c.src, "target", "release", "hersir"), "utf8")).toBe(BUILD["target/release/hersir"]);
    expect(readFileSync(join(c.src, "target", "release", "hersir-mcp"), "utf8")).toBe(BUILD["target/release/hersir-mcp"]);
    // only the files --skip-build installs come out of the archive
    expect(readFileSync(join(c.src, "VERSION"), "utf8")).toBe("0.1.0\n");
    expect(existsSync(join(c.src, "README.md"))).toBe(false);
    expect(git(c.src, "status", "--porcelain")).toBe("");
    expect(readdirSync(c.tmp)).toEqual([]);
  });

  it("replaces what an earlier build left in target/release, and keeps the rest of target", () => {
    const c = newCase();
    mkdirSync(join(c.src, "target", "release", "deps"), { recursive: true });
    writeFileSync(join(c.src, "target", "release", "hersir"), "an earlier build\n");
    writeFileSync(join(c.src, "target", "release", "deps", "kept"), "kept\n");
    const r = getSh(c, [], { PATH: linuxOn(c), HERSIR_BUILDS_URL: readyMade("again", tarGz("again", BUILD)) });
    expect(r.code, r.out).toBe(0);
    expect(readFileSync(join(c.src, "target", "release", "hersir"), "utf8")).toBe(BUILD["target/release/hersir"]);
    expect(readFileSync(join(c.src, "target", "release", "deps", "kept"), "utf8")).toBe("kept\n");
  });

  it("with --check, checks for a ready-made build without downloading it, and runs install.sh --check --skip-build only", () => {
    const c = newCase();
    const url = readyMade("check-only", tarGz("check-only", BUILD), { upload: false });
    const r = getSh(c, ["--check"], { PATH: linuxOn(c), HERSIR_BUILDS_URL: url });
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain("v0.1.0 has a ready-made build for Linux on x86_64, hersir-0.1.0-linux-x86_64.tar.gz: the check names what Hersir needs to run it.");
    expect(runs(c).map((run) => run.args)).toEqual(["--check --skip-build --check"]);
    expect(existsSync(join(c.src, "target"))).toBe(false);
  });

  it("stops when install.sh --check --skip-build finds something missing", () => {
    const c = newCase();
    const r = getSh(c, [], { PATH: linuxOn(c), HERSIR_BUILDS_URL: readyMade("missing", tarGz("missing", BUILD)), FAKE_CHECK: "1" });
    expect(r.code).toBe(1);
    expect(r.out).toContain("Install what is missing above, then run this again.");
    expect(runs(c).map((run) => run.args)).toEqual(["--check --skip-build"]);
  });

  it.each([
    ["a wrong checksum", { sum: "0".repeat(64) }, "linux", "hersir-0.1.0-linux-x86_64.tar.gz doesn't match its checksum in SHA256SUMS, so v0.1.0 is built from source."],
    ["no archive to download", { upload: false }, "linux", "v0.1.0 has no hersir-0.1.0-linux-x86_64.tar.gz to download, so it is built from source."],
    ["no archive for the system", {}, "aarch64", "There is no ready-made build of Hersir for Linux on aarch64, so v0.1.0 is built from source."],
    ["--from-source", {}, "from-source", "v0.1.0 is built from source, as --from-source asks."],
  ])("with %s, says why in one line, then builds the checked source", (name, options, kind, says) => {
    const c = newCase();
    const url = readyMade(`fallback-${kind}-${cases}`, tarGz(`fallback-${cases}`, BUILD), options);
    const args = kind === "from-source" ? ["--from-source"] : [];
    const r = getSh(c, args, { PATH: linuxOn(c, kind === "aarch64" ? "aarch64" : "x86_64"), HERSIR_BUILDS_URL: url });
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain(says);
    expect(r.out).toContain("is signed with Hersir's release key");
    expect(runs(c).map((run) => run.args)).toEqual(["--check", ""]);
    expect(existsSync(join(c.src, "target", "release", "hersir"))).toBe(false);
    expect(readdirSync(c.tmp)).toEqual([]);
  });

  it("an archive without the files --skip-build installs: says it can't unpack it, then builds", () => {
    const c = newCase();
    const url = readyMade("empty", tarGz("empty", { "target/release/hersir": "only one of them\n" }));
    const r = getSh(c, [], { PATH: linuxOn(c), HERSIR_BUILDS_URL: url });
    expect(r.code, r.out).toBe(0);
    expect(r.out).toMatch(/Could not unpack hersir-0\.1\.0-linux-x86_64\.tar\.gz \(.*\), so v0\.1\.0 is built from source\./);
    expect(runs(c).map((run) => run.args)).toEqual(["--check", ""]);
  });

  it("refuses --from-source with --skip-build before getting anything", () => {
    const c = newCase();
    const r = getSh(c, ["--from-source", "--skip-build"]);
    expect(r.code).toBe(2);
    expect(r.out).toContain("--from-source and --skip-build don't go together");
    expect(readdirSync(c.home)).toEqual([]);
  });

  it("uses no ready-made build for --uninstall, nor for your own --skip-build or --build-only", () => {
    const c = newCase();
    const url = readyMade("not-for", tarGz("not-for", BUILD));
    for (const args of [["--uninstall"], ["--skip-build"], ["--build-only"]]) {
      const r = getSh(c, args, { PATH: linuxOn(c), HERSIR_BUILDS_URL: url });
      expect(r.code, r.out).toBe(0);
      expect(r.out).not.toContain("ready-made");
    }
    expect(runs(c).map((run) => run.args)).toEqual(["--uninstall", "--check --skip-build", "--skip-build", "--check --build-only", "--build-only"]);
    expect(existsSync(join(c.src, "target", "release", "hersir"))).toBe(false);
  });
});

// The keys built into the one line are release-signers' lines, exactly (GA-252, from GA-260): a key changed in one
// place and not in the other fails here.
describe("the release keys built into get.sh and get.ps1", () => {
  const read = (path) => readFileSync(join(REPO_ROOT, path), "utf8");
  const lines = (text) => text.replace(/\r\n/g, "\n").split("\n").filter((l) => l.trim() && !l.startsWith("#"));
  const signerLines = lines(read("release-signers"));

  it("release-signers has the two keys, in the allowed-signers format", () => {
    expect(signerLines).toHaveLength(2);
    for (const line of signerLines) expect(line).toMatch(/^hersir-release namespaces="hersir-release" ssh-ed25519 AAAA\S+ .+$/);
  });

  it("get.sh's SIGNERS are release-signers' lines", () => {
    const block = read("scripts/get.sh").match(/<<'SIGNERS'\n([\s\S]*?)\nSIGNERS\n/);
    expect(block).not.toBeNull();
    expect(lines(block[1])).toEqual(signerLines);
  });

  it("get.ps1's $ReleaseSigners are release-signers' lines", () => {
    const block = read("scripts/get.ps1").match(/\$ReleaseSigners = @'\r?\n([\s\S]*?)\r?\n'@/);
    expect(block).not.toBeNull();
    expect(lines(block[1])).toEqual(signerLines);
  });

  it.runIf(unix)("get.sh's release_signers prints them as release-signers has them", () => {
    const fn = read("scripts/get.sh").match(/release_signers\(\) \{\n[\s\S]*?\n\}\n/)[0];
    const run = spawnSync("/bin/sh", ["-c", `${fn}release_signers`], { encoding: "utf8" });
    expect(run.status).toBe(0);
    expect(run.stdout).toBe(`${signerLines.join("\n")}\n`);
  });
});

// The one lines people see are hersir.dev's (GA-159), which the website sends on to get.sh and get.ps1 on production;
// what the scripts download stays on GitHub. get.ps1 is read as text here: CI runs it, on Windows.
describe("the one-line installs people see", () => {
  const CURL = "curl -fsSL https://hersir.dev/install.sh | sh";
  const IRM = "irm https://hersir.dev/install.ps1 | iex";
  const read = (path) => readFileSync(join(REPO_ROOT, path), "utf8");

  it("README.md, docs/install.md and the website's design show hersir.dev's lines, not GitHub's raw addresses", () => {
    for (const path of ["README.md", "docs/install.md", "design/website/README.md"]) {
      const text = read(path);
      expect(text, path).toContain(CURL);
      expect(text, path).toContain(IRM);
      expect(text, path).not.toContain("raw.githubusercontent.com");
      expect(text, path).not.toContain("gizai.ai");
    }
  });

  it("get.sh and get.ps1 show hersir.dev's lines in their headers, and only ever show them", () => {
    const sh = read("scripts/get.sh").split("\n");
    expect(sh[2]).toBe(`#   ${CURL}`);
    expect(sh).toContain("GET_PS1=https://hersir.dev/install.ps1");
    expect(sh.filter((line) => line.includes("$GET_PS1")).map((line) => line.trim())).toEqual([
      '*) say "This installs Hersir on Linux and macOS. On Windows, run this in PowerShell: irm $GET_PS1 | iex"; return 1 ;;',
    ]);

    const ps1 = read("scripts/get.ps1").split(/\r?\n/);
    expect(ps1[1]).toBe(`#   ${IRM}`);
    expect(ps1).toContain("  $GetPs1 = 'https://hersir.dev/install.ps1'");
    expect(ps1).toContain("  $GetSh = 'https://hersir.dev/install.sh'");
    const shown = ps1.filter((line) => /\$GetPs1|\$GetSh/.test(line) && !/^\s*\$Get(Ps1|Sh) = /.test(line));
    expect(shown.map((line) => line.trim())).toEqual([
      'Say "  irm $GetPs1 | iex"',
      'Say "This installs Hersir on Windows. On Linux and macOS, run: curl -fsSL $GetSh | sh"',
    ]);
  });

  it("get.sh and get.ps1 download from GitHub: hersir.dev is only in what they show and in comments", () => {
    for (const path of ["scripts/get.sh", "scripts/get.ps1"]) {
      const lines = read(path).split(/\r?\n/);
      const site = lines.filter((line) => line.includes("hersir.dev") && !line.trim().startsWith("#"));
      expect(site.map((line) => line.trim()), path).toEqual(
        path.endsWith(".sh")
          ? ["GET_PS1=https://hersir.dev/install.ps1", CURL]
          : ["$GetPs1 = 'https://hersir.dev/install.ps1'", "$GetSh = 'https://hersir.dev/install.sh'"],
      );
    }
    const ps1 = read("scripts/get.ps1");
    expect(ps1).toContain("$DefaultRepo = 'https://github.com/Yotech-AI/hersir.git'");
    // the latest release from GitHub's API, and its signature, SHA256SUMS and builds from its downloads (GA-252)
    expect(ps1).toContain('$url = "https://api.github.com/repos/$github/releases/latest"');
    expect(ps1).toContain('$builds = "https://github.com/$github/releases/download/$tag"');
    expect(read("scripts/get.sh")).toContain('builds="${builds:-https://github.com/$github/releases/download/$tag}"');
    expect(ps1).toContain("$repo = Get-Setting 'REPO'");
    expect(ps1).toContain("if (-not $repo) { $repo = $DefaultRepo }");
  });
});
