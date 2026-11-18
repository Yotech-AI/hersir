// Tests for the AUR package and its publishing (GA-274), and the Homebrew formula and its publishing (GA-275):
// packaging/aur/hersir-bin, packaging/homebrew, scripts/packages.sh and their docs.
// The script runs as it is, with the system's own bash, git and ssh-keygen, against a stand-in for gh (as
// sign-release.test.mjs fakes it) that holds a fake release v9.9.9 in a scratch folder, signed here with throwaway keys
// over the message scripts/sign-release.sh signs. Never against a real release, GitHub's repository or the AUR, and never
// with Hersir's own keys: HERSIR_RELEASE_SIGNERS names an allowed-signers file with a throwaway key, HERSIR_AUR_REPO a
// local bare repository instead of the AUR's hersir-bin, HERSIR_TAP_REPO a local bare repository instead of
// Yotech-AI/homebrew-tap (each run gets an empty one of its own unless a test gives one), HERSIR_SOURCE_REPO a local repository whose commit holds this
// checkout's install.sh, icons and NOTICE, and HOME is a scratch folder. Not on Windows, where the script doesn't run.
//
// The script runs gh, git and ssh-keygen from /usr/bin before the other folders on PATH, and GitHub's runners have the
// real gh there. So the stand-in is a bash function, gh, in a file BASH_ENV names: bash reads it before the script, and a
// function wins over every folder on PATH. ssh-keygen is wrapped the same way, only to write down how it ran.
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, symlinkSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const SCRIPT = join(ROOT, "scripts/packages.sh");
const unix = process.platform !== "win32";
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const sha256 = (data) => createHash("sha256").update(data).digest("hex");
const esc = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

// The card's line, word for word (Jeffrey, 17 Nov 2026).
const MAINTAINER = "# Maintainer: Jeffrey Sevinga <info@yotech.ai>";

const TAG = "v9.9.9";
const VERSION = "9.9.9";
const SIG = "hersir-9.9.9.sig";
const ARCHIVE = "hersir-9.9.9-linux-x86_64.tar.gz";
const ARCHIVES = [ARCHIVE, "hersir-9.9.9-macos-arm64.tar.gz", "hersir-9.9.9-windows-x86_64.zip"];
const ELSEWHERE = "0123456789abcdef0123456789abcdef01234567";
const TAG_OBJECT = "9e8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3a2f1e0d";
const SUBJECT = "Release 9.9.9";
const IDENTITY = {
  GIT_AUTHOR_NAME: "QA",
  GIT_AUTHOR_EMAIL: "qa@example.invalid",
  GIT_COMMITTER_NAME: "QA",
  GIT_COMMITTER_EMAIL: "qa@example.invalid",
};

// The template: its own release (version and commit), and the files it takes from that commit, in the order of its
// sources.
const PKGBUILD = read("packaging/aur/hersir-bin/PKGBUILD");
const SRCINFO = read("packaging/aur/hersir-bin/.SRCINFO");
const OLD_VERSION = PKGBUILD.match(/^pkgver=(.+)$/m)?.[1];
const OLD_COMMIT = PKGBUILD.match(/^_commit=(.+)$/m)?.[1];
const RAW = `https://raw.githubusercontent.com/Yotech-AI/hersir/${OLD_COMMIT}/`;
const SOURCE_URLS = SRCINFO.split("\n")
  .filter((l) => l.startsWith("\tsource = "))
  .map((l) => l.slice(l.indexOf("::") + 2));
const PATHS = SOURCE_URLS.map((u) => u.slice(RAW.length));
const FILE_SUMS = PATHS.map((p) => sha256(readFileSync(join(ROOT, p))));

// The Homebrew formula's template and the tap's README (GA-275), and the release's macOS archive the formula installs.
const FORMULA = read("packaging/homebrew/hersir.rb");
const TAP_README = read("packaging/homebrew/README.md");
const MAC = "hersir-9.9.9-macos-arm64.tar.gz";
const MAC_URL = `https://github.com/Yotech-AI/hersir/releases/download/${TAG}/${MAC}`;
// Lines of Ruby code, without comments, in the formula `text`.
const rubyCode = (text) =>
  text
    .split("\n")
    .filter((l) => !l.trim().startsWith("#"))
    .join("\n");

// The values of a PKGBUILD as bash (and so makepkg) reads them: each a list.
function sourced(text) {
  const dir = mkdtempSync(join(tmpdir(), "pkgbuild-"));
  try {
    writeFileSync(join(dir, "PKGBUILD"), text);
    const keys = "pkgname pkgver pkgrel pkgdesc url arch license depends optdepends provides conflicts options source sha256sums source_x86_64 sha256sums_x86_64 _commit install";
    const r = spawnSync(
      "bash",
      ["-c", `set -e; . "$1"; for k in ${keys}; do eval 'for v in "\${'"$k"'[@]}"; do printf "%s\\t%s\\n" "$k" "$v"; done'; done`, "bash", join(dir, "PKGBUILD")],
      { encoding: "utf8" },
    );
    expect(r.status, r.stderr).toBe(0);
    const values = {};
    for (const line of r.stdout.split("\n").filter(Boolean)) {
      const [k, ...v] = line.split("\t");
      (values[k] ??= []).push(v.join("\t"));
    }
    return values;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

// A .SRCINFO's pkgbase section, each key a list, and its pkgname.
function srcinfo(text) {
  const values = {};
  const [base, name] = text.split("\n\n");
  for (const line of base.split("\n").slice(1)) {
    const m = line.match(/^\t(\S+) = (.*)$/);
    if (m) (values[m[1]] ??= []).push(m[2]);
  }
  values.pkgbase = [base.split("\n")[0].replace(/^pkgbase = /, "")];
  values.pkgname = [name.trim().replace(/^pkgname = /, "")];
  return values;
}

// What a .SRCINFO must say for a PKGBUILD: what makepkg --printsrcinfo takes from it.
function expectSrcinfoMatches(pkgbuild, text) {
  const p = sourced(pkgbuild);
  const s = srcinfo(text);
  expect(s.pkgbase).toEqual(p.pkgname);
  expect(s.pkgname).toEqual(p.pkgname);
  for (const key of ["pkgver", "pkgrel", "pkgdesc", "url", "arch", "license", "depends", "optdepends", "provides", "conflicts", "options", "source", "sha256sums", "source_x86_64", "sha256sums_x86_64"]) {
    expect(s[key], key).toEqual(p[key]);
  }
}

// gh, as a stand-in: writes its arguments, each in [], to $RAN and answers from the fake release in $RELEASE (a folder
// with its assets) and the FAKE_ variables, through jq as gh's own --jq. Anything else is GitHub's 404.
const GH = String.raw`#!/bin/sh
{ printf 'gh'; for a in "$@"; do printf ' [%s]' "$a"; done; echo; } >> "$RAN"
jq_expr="" dir="" pattern="" prev=""
for a in "$@"; do
  case "$prev" in
    --jq) jq_expr="$a" ;;
    --dir) dir="$a" ;;
    --pattern) pattern="$a" ;;
  esac
  prev="$a"
done
answer() { if [ -n "$jq_expr" ]; then printf '%s' "$1" | jq -r "$jq_expr"; else printf '%s\n' "$1"; fi; }
commit() { answer "{\"object\":{\"type\":\"commit\",\"sha\":\"$1\"}}"; }
api=repos/Yotech-AI/hersir
case "$1 $2" in
  "auth status") exit "$FAKE_AUTH" ;;
  "release view")
    [ "$3" = "$FAKE_TAG" ] && [ -d "$RELEASE" ] || { echo "release not found" >&2; exit 1; }
    assets="$(ls "$RELEASE" | jq -R '{name: .}' | jq -s -c .)"
    answer "{\"isDraft\":$FAKE_DRAFT,\"isPrerelease\":$FAKE_PRERELEASE,\"assets\":$assets}" ;;
  "release download")
    [ "$3" = "$FAKE_TAG" ] && [ -f "$RELEASE/$pattern" ] || { echo "no assets match the file pattern" >&2; exit 1; }
    cp "$RELEASE/$pattern" "$dir/$pattern" ;;
  "api $api/git/ref/tags/$FAKE_TAG")
    if [ -n "$FAKE_TAG_OBJECT" ]; then answer "{\"object\":{\"type\":\"tag\",\"sha\":\"$FAKE_TAG_OBJECT\"}}"; else commit "$FAKE_COMMIT"; fi ;;
  "api $api/git/tags/$FAKE_TAG_OBJECT") commit "$FAKE_COMMIT" ;;
  *) echo "HTTP 404: Not Found ($*)" >&2; exit 1 ;;
esac
`;

// What BASH_ENV names: gh is the stand-in, and ssh-keygen writes how it ran to $RAN, then runs as itself.
const STANDINS = `gh() { "$FAKE_GH" "$@"; }
ssh-keygen() { { printf 'ssh-keygen'; printf ' [%s]' "$@"; echo; } >> "$RAN"; command ssh-keygen "$@"; }
`;

const git = (args, opts = {}) => {
  const r = spawnSync("git", args, { encoding: "utf8", env: { ...process.env, ...IDENTITY, GIT_CONFIG_NOSYSTEM: "1" }, ...opts });
  if (r.status !== 0) throw new Error(`git ${args.join(" ")}: ${r.stderr}`);
  return r.stdout.trim();
};

describe.runIf(unix)("scripts/packages.sh, against a fake gh, throwaway keys and local repositories", { timeout: 30000 }, () => {
  let scratch, key, otherKey, signers, fingerprint, gh, standins, ghOnly, source, COMMIT, NO_NOTICE, NO_LAUNCHER;

  beforeAll(() => {
    scratch = realpathSync(mkdtempSync(join(tmpdir(), "packages-")));
    const keygen = (file) => {
      const r = spawnSync("ssh-keygen", ["-q", "-t", "ed25519", "-N", "", "-C", "throwaway QA key", "-f", file], { encoding: "utf8" });
      expect(r.status, r.stderr).toBe(0);
      return file;
    };
    mkdirSync(join(scratch, "keys"));
    key = keygen(join(scratch, "keys/release"));
    otherKey = keygen(join(scratch, "keys/other"));
    const pub = readFileSync(`${key}.pub`, "utf8").split(" ").slice(0, 2).join(" ");
    signers = join(scratch, "keys/signers");
    writeFileSync(signers, `hersir-release namespaces="hersir-release" ${pub} throwaway QA key\n`);
    fingerprint = spawnSync("ssh-keygen", ["-l", "-f", `${key}.pub`], { encoding: "utf8" }).stdout.split(" ")[1];

    gh = join(scratch, "gh");
    writeFileSync(gh, GH);
    chmodSync(gh, 0o755);
    standins = join(scratch, "standins.bash");
    writeFileSync(standins, STANDINS);
    ghOnly = join(scratch, "gh-only.bash");
    writeFileSync(ghOnly, STANDINS.split("\n")[0] + "\n");

    // The release's commit, in a repository of its own: this checkout's install.sh, icons and NOTICE, at the paths the
    // PKGBUILD takes them from. Two more commits on branches of their own: one without NOTICE, and one whose install.sh
    // has no launcher here-document at the start of a line.
    source = join(scratch, "source");
    git(["-c", "init.defaultBranch=master", "init", "-q", source]);
    for (const path of PATHS) {
      mkdirSync(dirname(join(source, path)), { recursive: true });
      writeFileSync(join(source, path), readFileSync(join(ROOT, path)));
    }
    git(["-C", source, "add", "-A"]);
    git(["-C", source, "commit", "-q", "-m", SUBJECT]);
    COMMIT = git(["-C", source, "rev-parse", "HEAD"]);
    git(["-C", source, "checkout", "-q", "-b", "no-notice"]);
    git(["-C", source, "rm", "-q", "NOTICE"]);
    git(["-C", source, "commit", "-q", "-m", "No NOTICE"]);
    NO_NOTICE = git(["-C", source, "rev-parse", "HEAD"]);
    git(["-C", source, "checkout", "-q", "-b", "no-launcher", COMMIT]);
    writeFileSync(join(source, "install.sh"), read("install.sh").replace(/^cat > "\$new_launch" <<LAUNCH$/m, '  cat > "$new_launch" <<LAUNCH'));
    git(["-C", source, "commit", "-q", "-a", "-m", "An install.sh without the launcher"]);
    NO_LAUNCHER = git(["-C", source, "rev-parse", "HEAD"]);
    git(["-C", source, "checkout", "-q", "master"]);
  });
  afterAll(() => rmSync(scratch, { recursive: true, force: true }));

  const archiveSum = (a = ARCHIVE) => sha256(`the build in ${a}\n`);
  const plainSums = () => ARCHIVES.map((a) => `${archiveSum(a)}  ${a}\n`).join("");

  // A fake release v9.9.9 in a folder of its own: the three archives and SHA256SUMS, as Release builds uploads them, and
  // hersir-9.9.9.sig over "hersir v9.9.9 <commit>", a newline and SHA256SUMS, as sign-release.sh signs it: with `signer`
  // (none: no .sig), over `commit` and `sums`, in `namespace`. `changed` replaces SHA256SUMS after it is signed.
  function release({ without = [], signer = key, commit = COMMIT, sums = plainSums(), namespace = "hersir-release", changed } = {}) {
    const dir = mkdtempSync(join(scratch, "release-"));
    for (const a of ARCHIVES) if (!without.includes(a)) writeFileSync(join(dir, a), `the build in ${a}\n`);
    writeFileSync(join(dir, "SHA256SUMS"), sums);
    if (signer) {
      const message = join(mkdtempSync(join(scratch, "message-")), `hersir-${VERSION}`);
      writeFileSync(message, Buffer.concat([Buffer.from(`hersir ${TAG} ${commit}\n`), Buffer.from(sums)]));
      const r = spawnSync("ssh-keygen", ["-Y", "sign", "-f", signer, "-n", namespace, message], { encoding: "utf8" });
      expect(r.status, r.stderr).toBe(0);
      writeFileSync(join(dir, SIG), readFileSync(`${message}.sig`));
    }
    if (changed !== undefined) writeFileSync(join(dir, "SHA256SUMS"), changed);
    return dir;
  }

  // A bare repository instead of the AUR's hersir-bin: empty, as before the first push, or with one commit of `files`.
  function aur(files) {
    const bare = mkdtempSync(join(scratch, "aur-"));
    git(["-c", "init.defaultBranch=master", "init", "-q", "--bare", bare]);
    if (files) {
      const work = mkdtempSync(join(scratch, "aur-work-"));
      for (const [name, text] of Object.entries(files)) writeFileSync(join(work, name), text);
      git(["--git-dir", bare, "--work-tree", work, "add", "-A"]);
      git(["--git-dir", bare, "--work-tree", work, "commit", "-q", "-m", "what the AUR has"]);
    }
    return bare;
  }
  const refs = (bare) => git(["--git-dir", bare, "for-each-ref", "--format=%(refname) %(objectname)"]);
  const pushed = (bare, file) => git(["--git-dir", bare, "show", `master:${file}`]) + "\n";

  // A bare repository instead of Yotech-AI/homebrew-tap: empty, as Jeffrey makes it, or with one commit of `files` (paths
  // may have folders) on `branch`. `links` are symbolic links in that commit.
  function tap(files, { branch = "main", links = {} } = {}) {
    const bare = mkdtempSync(join(scratch, "tap-"));
    git(["-c", `init.defaultBranch=${branch}`, "init", "-q", "--bare", bare]);
    if (files) {
      const work = mkdtempSync(join(scratch, "tap-work-"));
      for (const [name, text] of Object.entries(files)) {
        mkdirSync(dirname(join(work, name)), { recursive: true });
        writeFileSync(join(work, name), text);
      }
      for (const [name, target] of Object.entries(links)) symlinkSync(target, join(work, name));
      git(["--git-dir", bare, "--work-tree", work, "add", "-A"]);
      git(["--git-dir", bare, "--work-tree", work, "commit", "-q", "-m", "what the tap has"]);
    }
    return bare;
  }
  const onTap = (bare, file) => git(["--git-dir", bare, "show", `main:${file}`]) + "\n";
  // A change to the tap's main, as someone else pushed it: `change` gets the tap's Formula/hersir.rb and returns the new one.
  function changeTap(bare, change, file = "Formula/hersir.rb") {
    const work = mkdtempSync(join(scratch, "tap-work-"));
    git(["clone", "-q", bare, work]);
    writeFileSync(join(work, file), change(readFileSync(join(work, file), "utf8")));
    git(["-C", work, "commit", "-q", "-a", "-m", "a change in the tap"]);
    git(["-C", work, "push", "-q", "origin", "HEAD:main"]);
  }

  // Runs scripts/packages.sh with `args`, in a session of its own (no terminal), `input` on stdin, and only these
  // variables: what gh ran, ssh-keygen's calls, stdout, stderr, and what it left in its TMPDIR.
  function publish(args, { rel, repo, tap, input = "y\n", env = {}, bashEnv = standins, path = process.env.PATH } = {}) {
    const run = mkdtempSync(join(scratch, "run-"));
    // An empty tap of the run's own unless the test gives one: never Yotech-AI/homebrew-tap.
    if (!tap) {
      tap = join(run, "tap.git");
      git(["-c", "init.defaultBranch=main", "init", "-q", "--bare", tap]);
    }
    const tmp = join(run, "tmp");
    mkdirSync(tmp);
    mkdirSync(join(run, "home"));
    const ran = join(run, "ran.txt");
    const child = spawn(SCRIPT, args, {
      detached: true,
      stdio: ["pipe", "pipe", "pipe"],
      env: {
        PATH: path,
        HOME: join(run, "home"),
        TMPDIR: tmp,
        LC_ALL: "C",
        GIT_CONFIG_NOSYSTEM: "1",
        GH_CONFIG_DIR: join(run, "gh-config"),
        BASH_ENV: bashEnv,
        FAKE_GH: gh,
        RAN: ran,
        RELEASE: rel ?? join(run, "no-release"),
        FAKE_TAG: TAG,
        FAKE_COMMIT: COMMIT,
        FAKE_AUTH: "0",
        FAKE_DRAFT: "false",
        FAKE_PRERELEASE: "false",
        FAKE_TAG_OBJECT: "",
        HERSIR_RELEASE_SIGNERS: signers,
        HERSIR_AUR_REPO: repo ?? join(run, "no-aur.git"),
        HERSIR_TAP_REPO: tap,
        HERSIR_SOURCE_REPO: source,
        ...IDENTITY,
        ...env,
      },
    });
    let stdout = "";
    let stderr = "";
    child.stdout.on("data", (d) => (stdout += d));
    child.stderr.on("data", (d) => (stderr += d));
    child.stdin.on("error", () => {});
    child.stdin.end(input);
    return new Promise((resolve) => {
      const timer = setTimeout(() => process.kill(-child.pid, "SIGKILL"), 25000);
      child.on("close", (status) => {
        clearTimeout(timer);
        const lines = existsSync(ran) ? readFileSync(ran, "utf8").replace(/\n$/, "").split("\n") : [];
        resolve({
          status,
          stdout,
          stderr,
          gh: lines.filter((l) => l.startsWith("gh")),
          keygen: lines.filter((l) => l.startsWith("ssh-keygen")),
          evil: lines.filter((l) => l.startsWith("evil")),
          tmpLeft: readdirSync(tmp),
        });
      });
    });
  }

  // Nothing published: the AUR's repository as it was, a status that isn't 0, one line on stderr that says why, and
  // nothing left in TMPDIR.
  function refused(r, repo, before, why) {
    expect(r.status, r.stdout + r.stderr).not.toBe(0);
    expect(r.stderr).toMatch(/^[^\n]+\n$/);
    expect(r.stderr).toMatch(why);
    expect(r.stdout).not.toContain("Published");
    if (repo) expect(refs(repo)).toBe(before);
    expect(r.tmpLeft).toEqual([]);
  }

  // The template's PKGBUILD and .SRCINFO with this release's values: the version, `pkgrel`, the commit, each file's
  // sha256 at that commit and the archive's from the signed SHA256SUMS.
  function expected(pkgrel = 1, sum = archiveSum()) {
    let i = 0;
    const pkgbuild = PKGBUILD.replace(/^pkgver=.*$/m, `pkgver=${VERSION}`)
      .replace(/^pkgrel=.*$/m, `pkgrel=${pkgrel}`)
      .replace(/^_commit=.*$/m, `_commit=${COMMIT}`)
      .replace(/'[0-9a-f]{64}'/g, () => `'${i < FILE_SUMS.length ? FILE_SUMS[i++] : (i++, sum)}'`);
    expect(i).toBe(FILE_SUMS.length + 1);
    let j = 0;
    const info = SRCINFO.split("\n")
      .map((l) => {
        if (l.startsWith("\tpkgver = ")) return `\tpkgver = ${VERSION}`;
        if (l.startsWith("\tpkgrel = ")) return `\tpkgrel = ${pkgrel}`;
        if (l.startsWith("\tsource")) return l.replaceAll(OLD_VERSION, VERSION).replaceAll(OLD_COMMIT, COMMIT);
        if (l.startsWith("\tsha256sums = ")) return `\tsha256sums = ${FILE_SUMS[j++]}`;
        if (l.startsWith("\tsha256sums_x86_64 = ")) return `\tsha256sums_x86_64 = ${sum}`;
        return l;
      })
      .join("\n");
    return { pkgbuild, info };
  }

  it("a good signature and y: fills the version, the sha256s and the commit from the signed release into PKGBUILD and .SRCINFO, shows the change and pushes them to the (test) AUR repository", async () => {
    const rel = release();
    const repo = aur();
    const r = await publish([TAG], { rel, repo });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stderr).toBe("");

    // One commit on master, with the PKGBUILD, .SRCINFO and packaging/LICENSE (0BSD).
    expect(git(["--git-dir", repo, "for-each-ref", "--format=%(refname)"])).toBe("refs/heads/master");
    expect(git(["--git-dir", repo, "rev-list", "--count", "master"])).toBe("1");
    expect(git(["--git-dir", repo, "log", "-1", "--format=%s", "master"])).toBe("hersir-bin 9.9.9-1");
    expect(git(["--git-dir", repo, "ls-tree", "--name-only", "master"]).split("\n").sort()).toEqual([".SRCINFO", "LICENSE", "PKGBUILD"]);
    expect(pushed(repo, "LICENSE")).toBe(read("packaging/LICENSE"));

    // Exactly the template with this release's values; the rest of the template as it is.
    const { pkgbuild, info } = expected();
    expect(pushed(repo, "PKGBUILD")).toBe(pkgbuild);
    expect(pushed(repo, ".SRCINFO")).toBe(info);
    expect(pushed(repo, "PKGBUILD").split("\n")[0]).toBe(MAINTAINER);

    // As makepkg reads it: version 9.9.9-1, the commit, every small file of that commit by its id with its sha256 in
    // git, and the archive of v9.9.9 with its line in the signed SHA256SUMS. .SRCINFO says the same.
    const p = sourced(pushed(repo, "PKGBUILD"));
    expect(p.pkgver).toEqual([VERSION]);
    expect(p.pkgrel).toEqual(["1"]);
    expect(p._commit).toEqual([COMMIT]);
    expect(p.source.length).toBe(PATHS.length);
    p.source.forEach((s, i) => {
      expect(s).toBe(`hersir-${VERSION}-${s.slice(`hersir-${VERSION}-`.length, s.indexOf("::"))}::https://raw.githubusercontent.com/Yotech-AI/hersir/${COMMIT}/${PATHS[i]}`);
      expect(p.sha256sums[i]).toBe(sha256(spawnSync("git", ["-C", source, "cat-file", "blob", `${COMMIT}:${PATHS[i]}`]).stdout));
    });
    expect(p.source_x86_64).toEqual([`${ARCHIVE}::https://github.com/Yotech-AI/hersir/releases/download/${TAG}/${ARCHIVE}`]);
    expect(p.sha256sums_x86_64).toEqual([archiveSum()]);
    expectSrcinfoMatches(pushed(repo, "PKGBUILD"), pushed(repo, ".SRCINFO"));

    // What it shows, in this order, before it asks; then that it published.
    const shown = [
      `Hersir's release ${TAG} (Yotech-AI/hersir), for its packages`,
      `  Version     ${TAG}`,
      `  Commit      ${COMMIT} ${SUBJECT}`,
      `  Signed      by ED25519 key ${fingerprint}, which signers allows`,
      `  Checked by  ${signers} (HERSIR_RELEASE_SIGNERS), not release-signers`,
      `  Files from  ${source} (HERSIR_SOURCE_REPO), not GitHub`,
      `hersir-bin, in ${repo} (HERSIR_AUR_REPO), not the AUR`,
      "  Now         not on the AUR yet: this push makes it",
      "  Becomes     9.9.9-1",
      `  Archive     ${archiveSum()}  ${ARCHIVE} (the signed SHA256SUMS)`,
      // git shows .SRCINFO first, then LICENSE and PKGBUILD.
      `+\tpkgver = ${VERSION}`,
      `+pkgver=${VERSION}`,
      `+_commit=${COMMIT}`,
      `+sha256sums_x86_64=('${archiveSum()}')`,
      "Publish v9.9.9? [y/N] ",
      `Published hersir-bin 9.9.9-1 to ${repo}`,
    ];
    let at = 0;
    for (const line of shown) {
      const next = r.stdout.indexOf(line, at);
      expect(next, `${line} in\n${r.stdout}`).toBeGreaterThanOrEqual(at);
      at = next + line.length;
    }

    // The signature checked as the card says; only SHA256SUMS and the .sig downloaded, never an archive; nothing left.
    expect(r.keygen).toEqual([
      expect.stringMatching(new RegExp(`^ssh-keygen \\[-Y\\] \\[verify\\] \\[-f\\] \\[${esc(signers)}\\] \\[-I\\] \\[hersir-release\\] \\[-n\\] \\[hersir-release\\] \\[-s\\] \\[.*/hersir-9\\.9\\.9\\.sig\\]$`)),
    ]);
    const downloads = r.gh.filter((l) => l.startsWith("gh [release] [download]"));
    expect(downloads.map((l) => l.match(/\[--pattern\] \[([^\]]+)\]/)[1]).sort()).toEqual(["SHA256SUMS", SIG].sort());
    expect(r.gh.filter((l) => !l.startsWith("gh [auth]")).every((l) => l.includes("[--repo] [Yotech-AI/hersir]") || l.includes("[api] [repos/Yotech-AI/hersir/"))).toBe(true);
    // gh only reads: auth status, release view, release download and the tag's refs.
    expect(r.gh.every((l) => /^gh \[(auth\] \[status|release\] \[view|release\] \[download|api\] \[repos\/Yotech-AI\/hersir\/git\/ref\/tags\/v9\.9\.9)\]/.test(l))).toBe(true);
    expect(r.tmpLeft).toEqual([]);
  });

  it("the sha256 of the archive is its line in the signed SHA256SUMS, not the archive the release holds", async () => {
    // A release whose Linux archive isn't the one SHA256SUMS names: the package says the signed sha256, so makepkg
    // refuses that archive on every computer.
    const rel = release();
    writeFileSync(join(rel, ARCHIVE), "another build\n");
    const repo = aur();
    const r = await publish([TAG], { rel, repo });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(sourced(pushed(repo, "PKGBUILD")).sha256sums_x86_64).toEqual([archiveSum()]);
    expect(pushed(repo, "PKGBUILD")).not.toContain(sha256("another build\n"));
  });

  it("an annotated tag: checks and fills in the commit it points to, not the tag object", async () => {
    const rel = release();
    const repo = aur();
    const r = await publish([TAG], { rel, repo, env: { FAKE_TAG_OBJECT: TAG_OBJECT } });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.gh).toContain(`gh [api] [repos/Yotech-AI/hersir/git/tags/${TAG_OBJECT}] [--jq] [.object.type + " " + .object.sha]`);
    expect(sourced(pushed(repo, "PKGBUILD"))._commit).toEqual([COMMIT]);
    expect(pushed(repo, "PKGBUILD")).not.toContain(TAG_OBJECT);
  });

  it("no hersir-9.9.9.sig: nothing is published, one line says why", async () => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish([TAG], { rel: release({ signer: null }), repo });
    refused(r, repo, before, /^The release v9\.9\.9 has no hersir-9\.9\.9\.sig: it isn't signed, so nothing is published\. Sign it first: scripts\/sign-release\.sh v9\.9\.9\.\n$/);
    expect(r.stdout).toBe("");
    expect(r.keygen).toEqual([]);
  });

  it("a signature by another key: nothing is published, one line says why", async () => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish([TAG], { rel: release({ signer: otherKey }), repo });
    refused(r, repo, before, new RegExp(`^hersir-9\\.9\\.9\\.sig is signed with a key that isn't in ${esc(signers)}, so nothing is published\\.\\n$`));
    expect(r.stdout).toBe("");
  });

  it("Hersir's own release-signers (no HERSIR_RELEASE_SIGNERS): a throwaway key's signature publishes nothing", async () => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish([TAG], { rel: release(), repo, env: { HERSIR_RELEASE_SIGNERS: undefined } });
    refused(r, repo, before, new RegExp(`is signed with a key that isn't in ${esc(join(ROOT, "release-signers"))}, so nothing is published`));
    expect(r.keygen.some((l) => l.includes(`[-f] [${join(ROOT, "release-signers")}]`))).toBe(true);
  });

  it.each([
    ["another archive's sha256 in it", (s) => s.replace(/^[0-9a-f]{64}/, sha256("another build\n"))],
    ["a line more", (s) => `${s}${"0".repeat(64)}  hersir-9.9.9-linux-aarch64.tar.gz\n`],
    ["a line less", (s) => s.split("\n").slice(1).join("\n")],
    ["one byte less (its last newline)", (s) => s.slice(0, -1)],
  ])("a SHA256SUMS changed after signing, with %s: nothing is published, one line says why", async (_, change) => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish([TAG], { rel: release({ changed: change(plainSums()) }), repo });
    refused(r, repo, before, new RegExp(`^hersir-9\\.9\\.9\\.sig doesn't sign this SHA256SUMS with the commit v9\\.9\\.9 points to \\(${COMMIT}\\): one of them changed since it was signed, so nothing is published\\.\\n$`));
    expect(r.stdout).toBe("");
  });

  it("the tag on another commit than the signed one: nothing is published, one line says why", async () => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish([TAG], { rel: release(), repo, env: { FAKE_COMMIT: NO_NOTICE } });
    refused(r, repo, before, new RegExp(`^hersir-9\\.9\\.9\\.sig doesn't sign this SHA256SUMS with the commit v9\\.9\\.9 points to \\(${NO_NOTICE}\\)`));
    expect(r.stdout).toBe("");
  });

  it.each([
    ["another version", { tag: "v9.9.8" }],
    ["another namespace", { namespace: "file" }],
  ])("a signature over %s: nothing is published, one line says why", async (_, { tag, namespace }) => {
    const repo = aur();
    const before = refs(repo);
    let rel;
    if (tag) {
      // Signed as v9.9.8: the line in the message names another version.
      rel = release();
      const message = join(mkdtempSync(join(scratch, "message-")), "m");
      writeFileSync(message, Buffer.concat([Buffer.from(`hersir ${tag} ${COMMIT}\n`), readFileSync(join(rel, "SHA256SUMS"))]));
      expect(spawnSync("ssh-keygen", ["-Y", "sign", "-f", key, "-n", "hersir-release", message]).status).toBe(0);
      writeFileSync(join(rel, SIG), readFileSync(`${message}.sig`));
    } else {
      rel = release({ namespace });
    }
    const r = await publish([TAG], { rel, repo });
    refused(r, repo, before, /^hersir-9\.9\.9\.sig doesn't sign this SHA256SUMS with the commit v9\.9\.9 points to/);
  });

  it.each([
    ["n", "n\n"],
    ["an empty line", "\n"],
    ["Y", "Y\n"],
    ["yes", "yes\n"],
    ["nothing (end of input)", ""],
  ])("the answer %s: nothing is published, one line says why", async (_, input) => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish([TAG], { rel: release(), repo, input });
    refused(r, repo, before, /^Nothing is published: the answer wasn't y\.\n$/);
    expect(r.stdout).toContain("+pkgver=9.9.9");
    expect(r.stdout.endsWith("Publish v9.9.9? [y/N] ")).toBe(true);
  });

  it("the same release again: nothing to publish, no question, the AUR's repository as it was", async () => {
    const repo = aur();
    const t = tap();
    expect((await publish([TAG], { rel: release(), repo, tap: t })).status).toBe(0);
    const before = refs(repo);
    const r = await publish([TAG], { rel: release(), repo, tap: t, input: "" });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.9-1");
    expect(r.stdout).toContain("  Stays       9.9.9-1");
    expect(r.stdout).toContain("Nothing to publish: each package has v9.9.9 already.");
    expect(r.stdout).not.toContain("Publish v9.9.9?");
    expect(refs(repo)).toBe(before);
  });

  it("the same release with other packaging files on the AUR: publishes it as the next pkgrel, 9.9.9-2", async () => {
    const repo = aur();
    expect((await publish([TAG], { rel: release(), repo })).status).toBe(0);
    // The AUR's 9.9.9-1 from other packaging files (one more comment line in its PKGBUILD).
    const work = mkdtempSync(join(scratch, "aur-work-"));
    git(["clone", "-q", repo, work]);
    writeFileSync(join(work, "PKGBUILD"), `${readFileSync(join(work, "PKGBUILD"), "utf8")}# older packaging\n`);
    git(["-C", work, "commit", "-q", "-a", "-m", "hersir-bin 9.9.9-1, older packaging"]);
    git(["-C", work, "push", "-q", "origin", "HEAD:master"]);

    const r = await publish([TAG], { rel: release(), repo });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.9-1");
    expect(r.stdout).toContain("  Becomes     9.9.9-2");
    const { pkgbuild, info } = expected(2);
    expect(pushed(repo, "PKGBUILD")).toBe(pkgbuild);
    expect(pushed(repo, ".SRCINFO")).toBe(info);
    expect(git(["--git-dir", repo, "log", "-1", "--format=%s", "master"])).toBe("hersir-bin 9.9.9-2");
  });

  it("an older release on the AUR (9.9.8-3): publishes 9.9.9-1 on top of it", async () => {
    const repo = aur({ PKGBUILD: "pkgver=9.9.8\npkgrel=3\n", ".SRCINFO": "pkgbase = hersir-bin\n\tpkgver = 9.9.8\n\tpkgrel = 3\n\npkgname = hersir-bin\n" });
    const r = await publish([TAG], { rel: release(), repo });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.8-3");
    expect(r.stdout).toContain("  Becomes     9.9.9-1");
    expect(r.stdout).toContain("-pkgver=9.9.8");
    expect(pushed(repo, "PKGBUILD")).toBe(expected().pkgbuild);
    expect(git(["--git-dir", repo, "rev-list", "--count", "master"])).toBe("2");
  });

  it("a newer release on the AUR (10.0.0-1): nothing is published, one line says why", async () => {
    const repo = aur({ ".SRCINFO": "pkgbase = hersir-bin\n\tpkgver = 10.0.0\n\tpkgrel = 1\n\npkgname = hersir-bin\n" });
    const before = refs(repo);
    refused(await publish([TAG], { rel: release(), repo }), repo, before, /^hersir-bin on the AUR is at 10\.0\.0 already, after 9\.9\.9, so nothing is published\.\n$/);
  });

  it.each([
    ["a draft", { FAKE_DRAFT: "true" }, /^The release v9\.9\.9 is a draft, so nothing is published/],
    ["a pre-release", { FAKE_PRERELEASE: "true" }, /^The release v9\.9\.9 is a pre-release, so nothing is published/],
    ["gh not logged in", { FAKE_AUTH: "1" }, /^gh isn't logged in to GitHub: run gh auth login, then this again\.\n$/],
  ])("%s: nothing is published, one line says why", async (_, env, why) => {
    const repo = aur();
    const before = refs(repo);
    refused(await publish([TAG], { rel: release(), repo, env }), repo, before, why);
  });

  it("no release v9.9.9 on GitHub: nothing is published, one line says why", async () => {
    refused(await publish([TAG]), undefined, undefined, /^Couldn't read the release v9\.9\.9 of Yotech-AI\/hersir: release not found\n$/);
  });

  it.each([
    ["no Linux archive", { without: [ARCHIVE] }, /^The release v9\.9\.9 has no hersir-9\.9\.9-linux-x86_64\.tar\.gz, so nothing is published\.\n$/],
    [
      "a signed SHA256SUMS without the Linux archive's line",
      { sums: ARCHIVES.slice(1).map((a) => `${archiveSum(a)}  ${a}\n`).join("") },
      /^The signed SHA256SUMS of v9\.9\.9 has no one line for hersir-9\.9\.9-linux-x86_64\.tar\.gz, so nothing is published\.\n$/,
    ],
  ])("%s: nothing is published, one line says why", async (_, opts, why) => {
    const repo = aur();
    const before = refs(repo);
    refused(await publish([TAG], { rel: release(opts), repo }), repo, before, why);
  });

  it("a signed commit that isn't in the repository: nothing is published, one line says why", async () => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish([TAG], { rel: release({ commit: ELSEWHERE }), repo, env: { FAKE_COMMIT: ELSEWHERE } });
    refused(r, repo, before, new RegExp(`^Couldn't fetch the commit ${ELSEWHERE} from ${esc(source)}: `));
  });

  it("a signed commit without a file the package installs (NOTICE): nothing is published, one line says why", async () => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish([TAG], { rel: release({ commit: NO_NOTICE }), repo, env: { FAKE_COMMIT: NO_NOTICE } });
    refused(r, repo, before, new RegExp(`^The commit ${NO_NOTICE} has no NOTICE, which hersir-bin installs, so nothing is published\\.\\n$`));
  });

  it("a signed commit whose install.sh the PKGBUILD can't make the launcher from: nothing is published, one line says why", async () => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish([TAG], { rel: release({ commit: NO_LAUNCHER }), repo, env: { FAKE_COMMIT: NO_LAUNCHER } });
    refused(r, repo, before, new RegExp(`^hersir-bin's PKGBUILD can't make the launcher and desktop entry from install\\.sh at ${NO_LAUNCHER}, so nothing is published\\.\\n$`));
  });

  it.each(["9.9.9", "v9.9", "v9.9.9-rc.1", "vX.Y.Z", "latest"])(
    "the version %s isn't vX.Y.Z: nothing is published, gh isn't asked, one line says why",
    async (tag) => {
      const repo = aur();
      const before = refs(repo);
      const r = await publish([tag], { rel: release(), repo });
      refused(r, repo, before, /isn't a version like v1\.2\.3, so there is nothing to publish\.\n$/);
      expect(r.gh).toEqual([]);
    },
  );

  it.each([
    ["no version", [], /^Give the release's version/],
    ["a package that isn't one", [TAG, "brew"], /^brew isn't one of the packages \(aur homebrew\), so nothing is published/],
    ["an unknown option", [TAG, "--yes"], /^--yes isn't an option of packages\.sh/],
  ])("%s: nothing is published, gh isn't asked, one line says why", async (_, args, why) => {
    const repo = aur();
    const before = refs(repo);
    const r = await publish(args, { rel: release(), repo });
    refused(r, repo, before, why);
    expect(r.gh).toEqual([]);
  });

  it("names the package: scripts/packages.sh v9.9.9 aur publishes hersir-bin", async () => {
    const repo = aur();
    const r = await publish([TAG, "aur"], { rel: release(), repo });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(pushed(repo, "PKGBUILD")).toBe(expected().pkgbuild);
  });

  it.runIf(existsSync("/usr/bin/ssh-keygen") && existsSync("/usr/bin/git"))(
    "an ssh-keygen or git earlier on PATH never runs: /usr/bin's check the signature and push",
    async () => {
      const bin = join(scratch, "evil-bin");
      mkdirSync(bin, { recursive: true });
      for (const name of ["ssh-keygen", "git"]) {
        writeFileSync(join(bin, name), `#!/bin/sh\n{ printf 'evil ${name}'; for a in "$@"; do printf ' [%s]' "$a"; done; echo; } >> "$RAN"\nexit 0\n`);
        chmodSync(join(bin, name), 0o755);
      }
      const repo = aur();
      const t = tap();
      const r = await publish([TAG], { rel: release(), repo, tap: t, bashEnv: ghOnly, path: `${bin}:${process.env.PATH}` });
      expect(r.status, r.stdout + r.stderr).toBe(0);
      expect(r.evil).toEqual([]);
      expect(pushed(repo, "PKGBUILD")).toBe(expected().pkgbuild);
      expect(onTap(t, "Formula/hersir.rb")).toBe(formula());
    },
  );

  // ---------- homebrew: the formula hersir in the tap (GA-275) ----------

  // The template's formula with this release's values: the url of the release's macOS archive and its sha256 from the
  // signed SHA256SUMS, and `revision` after the licence line from 1 on. The rest as it is.
  function formula(revision = 0, sum = archiveSum(MAC)) {
    expect(FORMULA.match(/^ {2}url "[^"]*"$/gm)).toHaveLength(1);
    expect(FORMULA.match(/^ {2}sha256 "[0-9a-f]{64}"$/gm)).toHaveLength(1);
    let text = FORMULA.replace(/^ {2}url "[^"]*"$/m, () => `  url "${MAC_URL}"`).replace(/^ {2}sha256 "[0-9a-f]{64}"$/m, () => `  sha256 "${sum}"`);
    if (revision > 0) text = text.replace(/^( {2}license "[^"]*"\n)/m, (l) => `${l}  revision ${revision}\n`);
    return text;
  }
  // The template's formula for another release `v`, as an older (or newer) publish left it in the tap.
  const formulaFor = (v, revision = 0) =>
    formula(revision, "f".repeat(64)).replaceAll(`v${VERSION}/hersir-${VERSION}-`, `v${v}/hersir-${v}-`);
  const tapFiles = (bare) => git(["--git-dir", bare, "ls-tree", "-r", "--name-only", "main"]).split("\n").sort();
  const subject = (bare) => git(["--git-dir", bare, "log", "-1", "--format=%s", "main"]);
  const inOrder = (stdout, lines) => {
    let at = 0;
    for (const line of lines) {
      const next = stdout.indexOf(line, at);
      expect(next, `${line} in\n${stdout}`).toBeGreaterThanOrEqual(at);
      at = next + line.length;
    }
  };

  it("homebrew: a good signature and y fills the url and the sha256 from the signed release into the formula, shows the change, asks once and pushes Formula/hersir.rb, README.md and LICENSE to the (test) tap's main", async () => {
    const rel = release();
    const repo = aur();
    const t = tap();
    const r = await publish([TAG], { rel, repo, tap: t });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stderr).toBe("");

    // One commit on main, with the formula, the tap's README and packaging/LICENSE (0BSD), and nothing else.
    expect(git(["--git-dir", t, "for-each-ref", "--format=%(refname)"])).toBe("refs/heads/main");
    expect(git(["--git-dir", t, "rev-list", "--count", "main"])).toBe("1");
    expect(subject(t)).toBe("hersir 9.9.9");
    expect(git(["--git-dir", t, "log", "-1", "--format=%b", "main"])).toBe(`Hersir ${TAG} at ${COMMIT}: the release's macOS build, checked against its signed SHA256SUMS.`);
    expect(tapFiles(t)).toEqual(["Formula/hersir.rb", "LICENSE", "README.md"]);
    expect(onTap(t, "README.md")).toBe(TAP_README);
    expect(onTap(t, "LICENSE")).toBe(read("packaging/LICENSE"));

    // Exactly the template with the release's url and the macOS archive's line in the signed SHA256SUMS; no revision.
    const published = onTap(t, "Formula/hersir.rb");
    expect(published).toBe(formula());
    expect(published).toContain(`  url "${MAC_URL}"\n`);
    expect(published).toContain(`  sha256 "${archiveSum(MAC)}"\n`);
    expect(published).not.toContain(archiveSum(ARCHIVE));
    expect(published).not.toMatch(/^ {2}(revision|version) /m);

    // The AUR package is published in the same run, after the same one question.
    expect(pushed(repo, "PKGBUILD")).toBe(expected().pkgbuild);
    expect(r.stdout.split("Publish v9.9.9? [y/N] ").length - 1).toBe(1);
    inOrder(r.stdout, [
      `  Signed      by ED25519 key ${fingerprint}, which signers allows`,
      `hersir-bin, in ${repo} (HERSIR_AUR_REPO), not the AUR`,
      `hersir, in ${t} (HERSIR_TAP_REPO), not the tap yotech-ai/tap`,
      "  Now         not in the tap yet: this push puts it there",
      "  Becomes     9.9.9",
      `  Archive     ${archiveSum(MAC)}  ${MAC} (the signed SHA256SUMS)`,
      `+  url "${MAC_URL}"`,
      `+  sha256 "${archiveSum(MAC)}"`,
      "Publish v9.9.9? [y/N] ",
      `Published hersir-bin 9.9.9-1 to ${repo}`,
      `Published hersir 9.9.9 to ${t}: brew update && brew upgrade hersir updates it`,
    ]);

    // Only SHA256SUMS and the .sig are downloaded, never the macOS archive; gh only reads; nothing left.
    const downloads = r.gh.filter((l) => l.startsWith("gh [release] [download]"));
    expect(downloads.map((l) => l.match(/\[--pattern\] \[([^\]]+)\]/)[1]).sort()).toEqual(["SHA256SUMS", SIG].sort());
    expect(r.gh.every((l) => /^gh \[(auth\] \[status|release\] \[view|release\] \[download|api\] \[repos\/Yotech-AI\/hersir\/git\/ref\/tags\/v9\.9\.9)\]/.test(l))).toBe(true);
    expect(r.tmpLeft).toEqual([]);
  });

  it("homebrew: the formula's sha256 is the macOS archive's line in the signed SHA256SUMS, not the archive the release holds", async () => {
    const rel = release();
    writeFileSync(join(rel, MAC), "another build\n");
    const t = tap();
    const r = await publish([TAG, "homebrew"], { rel, tap: t });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(onTap(t, "Formula/hersir.rb")).toBe(formula());
    expect(onTap(t, "Formula/hersir.rb")).not.toContain(sha256("another build\n"));
  });

  it.each([
    ["no hersir-9.9.9.sig", { signer: null }, {}, /^The release v9\.9\.9 has no hersir-9\.9\.9\.sig: it isn't signed, so nothing is published\./],
    ["a signature by another key", { signer: "other" }, {}, /^hersir-9\.9\.9\.sig is signed with a key that isn't in .*, so nothing is published\.\n$/],
    ["a signature in another namespace", { namespace: "file" }, {}, /^hersir-9\.9\.9\.sig doesn't sign this SHA256SUMS with the commit v9\.9\.9 points to/],
    ["a SHA256SUMS whose macOS line changed after signing", { changeMac: true }, {}, /^hersir-9\.9\.9\.sig doesn't sign this SHA256SUMS with the commit v9\.9\.9 points to .*: one of them changed since it was signed, so nothing is published\.\n$/],
    ["the tag on another commit than the signed one", {}, { FAKE_COMMIT: "NO_NOTICE" }, /^hersir-9\.9\.9\.sig doesn't sign this SHA256SUMS with the commit v9\.9\.9 points to/],
    ["Hersir's own release-signers (a throwaway key's signature)", {}, { HERSIR_RELEASE_SIGNERS: undefined }, /is signed with a key that isn't in .*release-signers, so nothing is published/],
  ])("homebrew: %s: nothing is published to the tap or the AUR, one line says why", async (_, opts, env, why) => {
    const { changeMac, signer, ...rest } = opts;
    const rel = release({
      ...rest,
      ...(signer === null ? { signer: null } : signer === "other" ? { signer: otherKey } : {}),
      ...(changeMac ? { changed: plainSums().replace(`${archiveSum(MAC)}  ${MAC}`, `${sha256("another build\n")}  ${MAC}`) } : {}),
    });
    const repo = aur();
    const t = tap({ "Formula/hersir.rb": formulaFor("9.9.8"), "README.md": TAP_README, LICENSE: read("packaging/LICENSE") });
    const beforeAur = refs(repo);
    const beforeTap = refs(t);
    for (const args of [[TAG], [TAG, "homebrew"]]) {
      const r = await publish(args, { rel, repo, tap: t, env: { ...env, ...(env.FAKE_COMMIT ? { FAKE_COMMIT: NO_NOTICE } : {}) } });
      refused(r, repo, beforeAur, why);
      expect(refs(t)).toBe(beforeTap);
      expect(r.stdout).toBe("");
    }
    expect(onTap(t, "Formula/hersir.rb")).toBe(formulaFor("9.9.8"));
  });

  it.each([
    ["n", "n\n"],
    ["an empty line", "\n"],
    ["nothing (end of input)", ""],
  ])("homebrew: the answer %s: the tap stays as it was, one line says why", async (_, input) => {
    const t = tap({ "Formula/hersir.rb": formulaFor("9.9.8"), "README.md": TAP_README, LICENSE: read("packaging/LICENSE") });
    const before = refs(t);
    const r = await publish([TAG, "homebrew"], { rel: release(), tap: t, input });
    refused(r, undefined, undefined, /^Nothing is published: the answer wasn't y\.\n$/);
    expect(r.stdout).toContain(`+  sha256 "${archiveSum(MAC)}"`);
    expect(r.stdout).toContain("  Now         9.9.8\n");
    expect(refs(t)).toBe(before);
  });

  it("homebrew: the same release again: Stays, nothing to publish, no question, the tap as it was", async () => {
    const t = tap();
    expect((await publish([TAG, "homebrew"], { rel: release(), tap: t })).status).toBe(0);
    const before = refs(t);
    const r = await publish([TAG, "homebrew"], { rel: release(), tap: t, input: "" });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.9\n");
    expect(r.stdout).toContain("  Stays       9.9.9: it has this release as it would be published, so it isn't published again");
    expect(r.stdout).toContain("Nothing to publish: each package has v9.9.9 already.");
    expect(r.stdout).not.toContain("Publish v9.9.9?");
    expect(refs(t)).toBe(before);
  });

  it("homebrew: the same release with another formula in the tap is its next revision (9.9.9_1, then 9.9.9_2), and the same again stays", async () => {
    const t = tap();
    expect((await publish([TAG, "homebrew"], { rel: release(), tap: t })).status).toBe(0);
    // The tap's 9.9.9 from another formula (one more comment line).
    changeTap(t, (f) => `${f}# older packaging\n`);
    let r = await publish([TAG, "homebrew"], { rel: release(), tap: t });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.9\n");
    expect(r.stdout).toContain("  Becomes     9.9.9_1\n");
    expect(r.stdout).toContain("+  revision 1");
    expect(onTap(t, "Formula/hersir.rb")).toBe(formula(1));
    expect(subject(t)).toBe("hersir 9.9.9_1");
    expect(r.stdout).toContain(`Published hersir 9.9.9_1 to ${t}`);

    // Run again: it has this release at revision 1, so nothing is published.
    const before = refs(t);
    r = await publish([TAG, "homebrew"], { rel: release(), tap: t, input: "" });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Stays       9.9.9_1: it has this release");
    expect(refs(t)).toBe(before);

    // Another formula at revision 1: revision 2.
    changeTap(t, (f) => f.replace("  desc ", "  # older packaging\n  desc "));
    r = await publish([TAG, "homebrew"], { rel: release(), tap: t });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.9_1\n");
    expect(r.stdout).toContain("  Becomes     9.9.9_2\n");
    expect(onTap(t, "Formula/hersir.rb")).toBe(formula(2));
    expect(subject(t)).toBe("hersir 9.9.9_2");
  });

  it("homebrew: a new README alone is published without a new revision, as brew installs nothing new", async () => {
    const t = tap({ "Formula/hersir.rb": formula(), "README.md": "# An older README\n", LICENSE: read("packaging/LICENSE") });
    const r = await publish([TAG, "homebrew"], { rel: release(), tap: t });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.9\n");
    expect(r.stdout).toContain("  Becomes     9.9.9\n");
    expect(r.stdout).toContain("-# An older README");
    expect(onTap(t, "Formula/hersir.rb")).toBe(formula());
    expect(onTap(t, "README.md")).toBe(TAP_README);
    expect(subject(t)).toBe("hersir 9.9.9");
  });

  it("homebrew: an older release in the tap (9.9.8, revision 2): publishes 9.9.9 without a revision on top of it", async () => {
    const t = tap({ "Formula/hersir.rb": formulaFor("9.9.8", 2), "README.md": TAP_README, LICENSE: read("packaging/LICENSE") });
    const r = await publish([TAG, "homebrew"], { rel: release(), tap: t });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.8_2\n");
    expect(r.stdout).toContain("  Becomes     9.9.9\n");
    expect(r.stdout).toContain("-  revision 2");
    expect(r.stdout).toContain(`+  url "${MAC_URL}"`);
    expect(onTap(t, "Formula/hersir.rb")).toBe(formula());
    expect(git(["--git-dir", t, "rev-list", "--count", "main"])).toBe("2");
  });

  it("homebrew: a newer release in the tap (10.0.0): nothing is published, the AUR neither, one line says why", async () => {
    const t = tap({ "Formula/hersir.rb": formulaFor("10.0.0") });
    const repo = aur();
    const before = refs(t);
    const r = await publish([TAG], { rel: release(), repo, tap: t });
    refused(r, repo, "", /^hersir in the tap is at 10\.0\.0 already, after 9\.9\.9, so nothing is published\.\n$/);
    expect(refs(t)).toBe(before);
  });

  it("homebrew: the tap's other files and formulae stay as they are", async () => {
    const other = 'class Other < Formula\n  url "https://example.invalid/other-1.0.tar.gz"\nend\n';
    const t = tap({ "Formula/hersir.rb": formulaFor("9.9.8"), "Formula/other.rb": other, "README.md": "# Old\n", "LICENSE": "old\n", ".github/CODEOWNERS": "* @nobody\n" });
    const r = await publish([TAG, "homebrew"], { rel: release(), tap: t });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(tapFiles(t)).toEqual([".github/CODEOWNERS", "Formula/hersir.rb", "Formula/other.rb", "LICENSE", "README.md"]);
    expect(onTap(t, "Formula/other.rb")).toBe(other);
    expect(onTap(t, ".github/CODEOWNERS")).toBe("* @nobody\n");
    expect(onTap(t, "Formula/hersir.rb")).toBe(formula());
    expect(git(["--git-dir", t, "diff", "--name-only", "main~1", "main"]).split("\n").sort()).toEqual(["Formula/hersir.rb", "LICENSE", "README.md"]);
  });

  it.each([
    ["a tap on another branch than main", () => tap({ "Formula/hersir.rb": formulaFor("9.9.8") }, { branch: "master" }), /^The tap .* is on the branch 'master', not main: look at it first, nothing is published\.\n$/],
    ["a tap whose Formula is a link", () => tap({ "README.md": "# Old\n" }, { links: { Formula: "/etc" } }), /^Formula in .* isn't a folder: look at the tap first, nothing is published\.\n$/],
    ["a tap whose formula has no version in its url", () => tap({ "Formula/hersir.rb": 'class Hersir < Formula\n  url "https://example.invalid/hersir.tar.gz"\nend\n' }), /^hersir in the tap has the version '', which isn't X\.Y\.Z: look at it first, nothing is published\.\n$/],
    ["a tap that can't be cloned", () => join(scratch, "no-such-tap.git"), /^Couldn't clone .*no-such-tap\.git, so nothing is published: /],
  ])("homebrew: %s: nothing is published, the AUR neither, one line says why", async (_, makeTap, why) => {
    const t = makeTap();
    const repo = aur();
    const before = existsSync(t) ? refs(t) : undefined;
    const r = await publish([TAG], { rel: release(), repo, tap: t });
    refused(r, repo, "", why);
    if (before !== undefined) expect(refs(t)).toBe(before);
  });

  it.each([
    ["no macOS archive", { without: [MAC] }, /^The release v9\.9\.9 has no hersir-9\.9\.9-macos-arm64\.tar\.gz, so nothing is published\.\n$/],
    [
      "a signed SHA256SUMS without the macOS archive's line",
      { sums: ARCHIVES.filter((a) => a !== MAC).map((a) => `${archiveSum(a)}  ${a}\n`).join("") },
      /^The signed SHA256SUMS of v9\.9\.9 has no one line for hersir-9\.9\.9-macos-arm64\.tar\.gz, so nothing is published\.\n$/,
    ],
    [
      "a signed SHA256SUMS with two lines for the macOS archive",
      { sums: `${plainSums()}${"0".repeat(64)}  ${MAC}\n` },
      /^The signed SHA256SUMS of v9\.9\.9 has no one line for hersir-9\.9\.9-macos-arm64\.tar\.gz, so nothing is published\.\n$/,
    ],
  ])("homebrew: %s: nothing is published, the AUR neither, one line says why", async (_, opts, why) => {
    const t = tap();
    const repo = aur();
    const r = await publish([TAG], { rel: release(opts), repo, tap: t });
    refused(r, repo, "", why);
    expect(refs(t)).toBe("");
  });

  it("names the package: homebrew publishes only the formula, aur only hersir-bin and never clones the tap", async () => {
    const repo = aur();
    const t = tap();
    let r = await publish([TAG, "homebrew"], { rel: release(), repo, tap: t });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(onTap(t, "Formula/hersir.rb")).toBe(formula());
    expect(refs(repo)).toBe("");
    expect(r.stdout).not.toContain("hersir-bin");

    r = await publish([TAG, "aur"], { rel: release(), repo, tap: join(scratch, "no-such-tap.git") });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(pushed(repo, "PKGBUILD")).toBe(expected().pkgbuild);
    expect(r.stdout).not.toContain("HERSIR_TAP_REPO");
    expect(r.stdout).not.toContain("hersir 9.9.9");
  });

  it.runIf(existsSync("/usr/bin/ruby"))("homebrew: the formula it publishes is valid Ruby (ruby -c)", async () => {
    const t = tap();
    expect((await publish([TAG, "homebrew"], { rel: release(), tap: t })).status).toBe(0);
    changeTap(t, (f) => `${f}# older packaging\n`);
    expect((await publish([TAG, "homebrew"], { rel: release(), tap: t })).status).toBe(0);
    const dir = mkdtempSync(join(scratch, "ruby-"));
    for (const [name, text] of [["template.rb", FORMULA], ["published.rb", onTap(t, "Formula/hersir.rb")]]) {
      writeFileSync(join(dir, name), text);
      const r = spawnSync("/usr/bin/ruby", ["-c", join(dir, name)], { encoding: "utf8" });
      expect(r.status, `${name}: ${r.stderr}`).toBe(0);
      expect(r.stdout).toContain("Syntax OK");
    }
    expect(onTap(t, "Formula/hersir.rb")).toContain('  license "Apache-2.0"\n  revision 1\n');
  });
});

describe("packaging/aur/hersir-bin: the PKGBUILD and .SRCINFO", () => {
  it("the Maintainer line is the card's, word for word, and the first line", () => {
    expect(PKGBUILD.split("\n")[0]).toBe(MAINTAINER);
    expect(PKGBUILD.match(/^# Maintainer:.*$/gm)).toEqual([MAINTAINER]);
  });

  it.runIf(unix)("hersir-bin, x86_64, Apache-2.0, provides and conflicts with hersir, and the dependencies the card names", () => {
    const p = sourced(PKGBUILD);
    expect(p.pkgname).toEqual(["hersir-bin"]);
    expect(p.arch).toEqual(["x86_64"]);
    expect(p.license).toEqual(["Apache-2.0"]);
    expect(p.provides).toEqual(["hersir"]);
    expect(p.conflicts).toEqual(["hersir"]);
    for (const dep of ["git", "webkit2gtk-4.1", "libayatana-appindicator"]) expect(p.depends, dep).toContain(dep);
    expect((p.optdepends ?? []).some((d) => d.startsWith("libayatana-appindicator"))).toBe(false);
    // Claude Code, Codex and Gemini stay the user's own installs.
    expect([...p.depends, ...(p.optdepends ?? [])].some((d) => /claude|codex|gemini/i.test(d))).toBe(false);
    // No install script: removing the package runs nothing of its own, so the data folder stays.
    expect(p.install).toBeUndefined();
    expect(readdirSync(join(ROOT, "packaging/aur/hersir-bin")).sort()).toEqual([".SRCINFO", "PKGBUILD"]);
  });

  it.runIf(unix)("its sources: the release's Linux archive, and the commit's files by the commit's id, never a branch", () => {
    const p = sourced(PKGBUILD);
    expect(p._commit[0]).toMatch(/^[0-9a-f]{40}$/);
    expect(p.source_x86_64).toEqual([`hersir-${p.pkgver[0]}-linux-x86_64.tar.gz::https://github.com/Yotech-AI/hersir/releases/download/v${p.pkgver[0]}/hersir-${p.pkgver[0]}-linux-x86_64.tar.gz`]);
    expect(p.source.length).toBeGreaterThan(0);
    for (const s of p.source) expect(s).toMatch(new RegExp(`::https://raw\\.githubusercontent\\.com/Yotech-AI/hersir/${p._commit[0]}/`));
    expect([...PATHS].sort()).toEqual(
      ["install.sh", "NOTICE", "src-tauri/icons/32x32.png", "src-tauri/icons/64x64.png", "src-tauri/icons/128x128.png", "src-tauri/icons/128x128@2x.png", "src-tauri/icons/icon.png", "src-tauri/icons/gizai.svg"].sort(),
    );
    for (const sum of [...p.sha256sums, ...p.sha256sums_x86_64]) expect(sum).toMatch(/^[0-9a-f]{64}$/);
    expect(PKGBUILD).not.toMatch(/SKIP|\/(main|production|master)\/|releases\/latest/);
  });

  it("installs both programs into /usr/lib/hersir, hersir-package.txt saying aur beside them, the launcher, the desktop entry and the icons", () => {
    const pkg = PKGBUILD.slice(PKGBUILD.indexOf("package() {"));
    expect(pkg).toContain("local lib=/usr/lib/hersir");
    expect(pkg).toContain('install -Dm755 target/release/hersir "$pkgdir$lib/hersir"');
    expect(pkg).toContain('install -Dm755 target/release/hersir-mcp "$pkgdir$lib/hersir-mcp"');
    expect(pkg).toContain("printf 'aur\\n' > hersir-package.txt");
    expect(pkg).toContain('install -Dm644 hersir-package.txt "$pkgdir$lib/hersir-package.txt"');
    expect(pkg).toContain('ln -s "$lib/hersir-launch" "$pkgdir/usr/bin/hersir"');
    expect(pkg).toContain('"$pkgdir/usr/share/applications/hersir.desktop"');
    // The icons in the sizes install.sh installs them in.
    const sizes = (text) => text.match(/for pair in ([^;]+);/)[1];
    expect(sizes(pkg)).toBe(sizes(read("install.sh")));
    // Nothing touches the user's data folder.
    expect(PKGBUILD).not.toMatch(/\.local\/share|XDG_DATA_HOME|\$HOME|dev\.hersir\.app/);
  });

  it.runIf(unix)(".SRCINFO says what the PKGBUILD says (as makepkg --printsrcinfo makes it)", () => {
    expectSrcinfoMatches(PKGBUILD, SRCINFO);
  });

  it.runIf(unix)("_install_sh makes install.sh's own launcher and desktop entry, with /usr/lib/hersir for $LIB", () => {
    const dir = realpathSync(mkdtempSync(join(tmpdir(), "install-sh-")));
    try {
      const pkgbuild = join(dir, "PKGBUILD");
      writeFileSync(pkgbuild, PKGBUILD);
      const make = (installSh, doc) => {
        writeFileSync(join(dir, `hersir-${OLD_VERSION}-install.sh`), installSh);
        return spawnSync("bash", ["-c", '. "$1"; srcdir="$2"; _install_sh "$3"', "bash", pkgbuild, dir, doc], { encoding: "utf8" });
      };
      const installSh = read("install.sh");
      const heredoc = (start, end) => {
        const from = installSh.indexOf(`\n${start}\n`) + start.length + 2;
        return installSh.slice(from, installSh.indexOf(`\n${end}\n`, from) + 1).replaceAll("$LIB", "/usr/lib/hersir").replaceAll("\\$", "$");
      };
      const launch = make(installSh, "LAUNCH");
      expect(launch.status, launch.stderr).toBe(0);
      expect(launch.stdout).toBe(heredoc('cat > "$new_launch" <<LAUNCH', "LAUNCH"));
      expect(launch.stdout).toContain('exec bash -lc \'exec "$0" "$@"\' "/usr/lib/hersir/hersir" "$@"');
      const desktop = make(installSh, "DESKTOP");
      expect(desktop.status, desktop.stderr).toBe(0);
      expect(desktop.stdout).toBe(heredoc('cat > "$APPS/hersir.desktop" <<DESKTOP', "DESKTOP"));
      expect(desktop.stdout).toContain("\nExec=/usr/lib/hersir/hersir-launch\nIcon=hersir\n");

      // An install.sh without that here-document at the start of a line stops the build, and says so.
      const none = make(installSh.replace(/^cat > "\$APPS\/hersir\.desktop" <<DESKTOP$/m, ""), "DESKTOP");
      expect(none.status).not.toBe(0);
      expect(none.stderr).toMatch(/has no single DESKTOP here-document, so its file is not made\./);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("The packages: licence, CI and docs", () => {
  it("packaging/ is under 0BSD, and its README says so", () => {
    const licence = read("packaging/LICENSE");
    expect(licence).toContain("Permission to use, copy, modify, and/or distribute this software for any\npurpose with or without fee is hereby granted.");
    expect(licence).not.toMatch(/above copyright notice and this permission notice appear/);
    const readme = read("packaging/README.md");
    expect(readme).toMatch(/BSD Zero Clause License \(0BSD/);
    expect(readme).toContain("yay -S hersir-bin");
  });

  it("scripts/packages.sh: checks the signature as the card says, publishes to the AUR's hersir-bin by default, runs its programs from /usr/bin first", () => {
    const script = read("scripts/packages.sh");
    expect(script.split("\n")[0]).toBe("#!/bin/bash");
    expect(script).toContain('ssh-keygen -Y verify -f "$signers" -I "$NAMESPACE" -n "$NAMESPACE"');
    expect(script).toContain("NAMESPACE=hersir-release");
    expect(script).toContain('signers="${HERSIR_RELEASE_SIGNERS:-$ROOT/release-signers}"');
    expect(script).toContain('AUR_REPO="${HERSIR_AUR_REPO:-ssh://aur@aur.archlinux.org/hersir-bin.git}"');
    expect(script).toContain("printf 'Publish %s? [y/N] ' \"$tag\"");
    expect(script).toMatch(/^ {2}\*\) PATH="\/usr\/bin:\$PATH" ;;$/m);
  });

  it("no workflow runs scripts/packages.sh or pushes to the AUR: only Jeffrey publishes", () => {
    const dir = join(ROOT, ".github/workflows");
    for (const file of readdirSync(dir)) {
      const code = readFileSync(join(dir, file), "utf8")
        .split("\n")
        .filter((l) => !l.trim().startsWith("#"))
        .join("\n");
      expect(code, file).not.toContain("packages.sh");
      expect(code, file).not.toContain("aur@aur.archlinux.org");
    }
  });

  it("ci.yml's Arch Linux job builds with makepkg, runs namcap, installs the package and checks it, and removing it keeps the data", () => {
    const ci = read(".github/workflows/ci.yml");
    const arch = ci.slice(ci.indexOf("\n  arch:\n"));
    expect(arch).toContain("container: archlinux:latest");
    expect(arch).toContain("makepkg --printsrcinfo | diff -u .SRCINFO -");
    expect(arch).toContain("makepkg --syncdeps --noconfirm");
    expect(arch).toContain("namcap PKGBUILD");
    expect(arch).toContain("namcap hersir-bin-*.pkg.tar.zst");
    expect(arch).toContain("pacman -U --noconfirm");
    expect(arch).toContain('test "$(sudo -u builder hersir --version)" = "hersir $pkgver"');
    expect(arch).toContain('test "$(cat /usr/lib/hersir/hersir-package.txt)" = aur');
    expect(arch).toContain("pacman -Rns --noconfirm hersir-bin");
  });

  it("install.md: Arch Linux and Omarchy install hersir-bin with yay", () => {
    const install = read("docs/install.md");
    const section = install.slice(install.indexOf("### Arch Linux and Omarchy"));
    expect(section).toContain("```sh\nyay -S hersir-bin\n```");
    expect(section).toContain("Your data stays in `~/.local/share/hersir`");
  });

  it("RELEASING.md: step 6, Publish the packages, comes right after Sign it, with scripts/packages.sh vX.Y.Z", () => {
    const releasing = read("docs/RELEASING.md");
    const step5 = releasing.indexOf("5. **Sign it.**");
    const step6 = releasing.indexOf("6. **Publish the packages.**");
    expect(step5).toBeGreaterThan(0);
    expect(step6).toBeGreaterThan(step5);
    // No other numbered step between them.
    expect(step5 + 3 + releasing.slice(step5 + 3).search(/\n\d+\. \*\*/)).toBe(step6 - 1);
    const publish = releasing.slice(step6, releasing.indexOf("\n## ", step6));
    expect(publish).toContain("```sh\n   scripts/packages.sh vX.Y.Z\n   ```");
    expect(publish).toContain("ssh-keygen -Y verify -f release-signers -I hersir-release -n hersir-release");
    expect(publish).toContain("Publish vX.Y.Z? [y/N]");
  });
});

describe("packaging/homebrew: the formula hersir and the tap's README (GA-275)", () => {
  const code = rubyCode(FORMULA);

  it("a formula, never a cask, for Macs with Apple silicon, from a release's macOS archive on GitHub with its sha256", () => {
    expect(code).toMatch(/^class Hersir < Formula$/m);
    expect(code).not.toMatch(/^\s*cask /m);
    const url = code.match(/^ {2}url "([^"]*)"$/m)[1];
    const [, v] = url.match(/^https:\/\/github\.com\/Yotech-AI\/hersir\/releases\/download\/v(\d+\.\d+\.\d+)\/hersir-(\d+\.\d+\.\d+)-macos-arm64\.tar\.gz$/) ?? [];
    expect(v, url).toBeDefined();
    expect(url).toContain(`/v${v}/hersir-${v}-macos-arm64.tar.gz`);
    expect(code).toMatch(/^ {2}sha256 "[0-9a-f]{64}"$/m);
    expect(code).toContain('  license "Apache-2.0"\n');
    expect(code).toContain("  depends_on arch: :arm64\n");
    expect(code).toContain("  depends_on :macos\n");
    // Homebrew reads the version from the url; scripts/packages.sh writes the revision.
    expect(code).not.toMatch(/^ {2}(version|revision) /m);
    expect(readdirSync(join(ROOT, "packaging/homebrew")).sort()).toEqual(["README.md", "hersir.rb"]);
  });

  it("installs Hersir.app in its prefix with hersir-mcp inside it and signed ad hoc again, and hersir and hersir-mcp on the PATH", () => {
    const install = code.slice(code.indexOf("  def install"), code.indexOf("  def caveats"));
    // The paths of Release builds' macOS archive, under its one folder target/.
    const builds = read(".github/workflows/release-builds.yml");
    expect(builds).toContain("target/release/hersir target/release/hersir-mcp target/release/bundle/macos/Hersir.app");
    expect(install).toContain('app = buildpath/"release/bundle/macos/Hersir.app"');
    expect(install).toContain('(app/"Contents/MacOS").install "release/hersir-mcp"');
    // as install.sh signs it
    expect(install).toContain('system "/usr/bin/codesign", "--force", "--deep", "--sign", "-", app');
    expect(read("install.sh")).toContain("codesign --force --deep --sign -");
    expect(install.indexOf("codesign")).toBeGreaterThan(install.indexOf('.install "release/hersir-mcp"'));
    expect(install.indexOf("prefix.install app")).toBeGreaterThan(install.indexOf("codesign"));
    expect(install).toContain('bin.write_exec_script prefix/"Hersir.app/Contents/MacOS/hersir"');
    expect(install).toContain('bin.install_symlink prefix/"Hersir.app/Contents/MacOS/hersir-mcp"');
    // Homebrew's cleaner keeps out of the signed app.
    expect(code).toContain('  skip_clean "Hersir.app"\n');
  });

  it("never strips or sets the quarantine mark, and writes nothing into the user's data", () => {
    expect(code).not.toMatch(/quarantine|xattr|spctl|--no-quarantine/i);
    // install writes only into the build and its prefix (the caveats only name the data folder)
    const install = code.slice(code.indexOf("  def install"), code.indexOf("  def caveats"));
    expect(install).not.toMatch(/Application Support|WebKit|\.local\/share|HOME|~\//);
    expect(code).not.toMatch(/hersir-package\.txt|post_install/);
  });

  it("its caveats say how to link Hersir.app into ~/Applications, once, and how to update; its test runs hersir --version", () => {
    const caveats = code.slice(code.indexOf("  def caveats"), code.indexOf("  test do"));
    expect(caveats).toContain("mkdir -p ~/Applications && ln -sfn #{opt_prefix}/Hersir.app ~/Applications/Hersir.app");
    expect(caveats).toContain("brew update && brew upgrade hersir");
    const test = code.slice(code.indexOf("  test do"));
    expect(test).toContain('assert_equal "hersir #{version}", shell_output("#{bin}/hersir --version").strip');
  });

  it.runIf(existsSync("/usr/bin/ruby"))("the template is valid Ruby (ruby -c)", () => {
    const r = spawnSync("/usr/bin/ruby", ["-c", join(ROOT, "packaging/homebrew/hersir.rb")], { encoding: "utf8" });
    expect(r.status, r.stderr).toBe(0);
  });

  it("the tap's README: install, update and uninstall (the data stays), with links that work in the tap", () => {
    expect(TAP_README.split("\n")[0]).toBe("# Yotech-AI/homebrew-tap");
    const section = (name) => TAP_README.slice(TAP_README.indexOf(`## ${name}\n`), TAP_README.indexOf("\n## ", TAP_README.indexOf(`## ${name}\n`) + 1));
    expect(section("Install")).toContain("```sh\nbrew install yotech-ai/tap/hersir\n```");
    expect(section("Install")).toContain('```sh\nmkdir -p ~/Applications && ln -sfn "$(brew --prefix)/opt/hersir/Hersir.app" ~/Applications/Hersir.app\n```');
    expect(section("Update")).toContain("```sh\nbrew update && brew upgrade hersir\n```");
    expect(section("Uninstall")).toContain("```sh\nbrew uninstall hersir\nrm ~/Applications/Hersir.app\n```");
    expect(section("Uninstall")).toContain("Your data stays in `~/Library/Application Support/Hersir`");
    // In the tap only LICENSE is beside it (scripts/packages.sh puts it there): every other link is a full address.
    const links = [...TAP_README.matchAll(/\]\(([^)]+)\)/g)].map((m) => m[1]);
    expect(links.filter((l) => !/^https:\/\//.test(l))).toEqual(["LICENSE"]);
  });
});

describe("The Homebrew formula in CI, scripts/packages.sh and the docs (GA-275)", () => {
  it("scripts/packages.sh publishes to Yotech-AI/homebrew-tap's main by default, HERSIR_TAP_REPO instead for tests", () => {
    const script = read("scripts/packages.sh");
    expect(script).toContain('PACKAGES="aur homebrew"');
    expect(script).toContain('TAP_REPO="${HERSIR_TAP_REPO:-https://github.com/Yotech-AI/homebrew-tap.git}"');
    expect(script).toContain('push --quiet origin HEAD:refs/heads/main');
    expect(script).not.toMatch(/push[^\n]*--force|push[^\n]* -f\b/);
    // Each package's three functions.
    for (const fn of ["homebrew_prepare", "homebrew_show", "homebrew_publish"]) expect(script).toContain(`\n${fn}() {\n`);
  });

  it("no workflow pushes to the tap or bumps the formula: only Jeffrey publishes", () => {
    const dir = join(ROOT, ".github/workflows");
    for (const file of readdirSync(dir)) {
      const code = readFileSync(join(dir, file), "utf8")
        .split("\n")
        .filter((l) => !l.trim().startsWith("#"))
        .join("\n");
      expect(code, file).not.toMatch(/homebrew-tap|yotech-ai\/tap|bump-formula-pr|brew tap yotech/i);
    }
  });

  it("ci.yml's Homebrew job on macos-15: installs the formula from a local tap and a test build, brew test, brew audit, codesign --verify and no quarantine mark", () => {
    const ci = read(".github/workflows/ci.yml");
    const changes = ci.slice(ci.indexOf("\n  homebrew-changes:\n"), ci.indexOf("\n  homebrew:\n"));
    expect(changes).toContain("name: Homebrew formula changed?");
    expect(changes).toContain("git diff --quiet \"$base\" HEAD -- packaging/homebrew install.sh .github/workflows/ci.yml");
    const job = ci.slice(ci.indexOf("\n  homebrew:\n"));
    expect(job).toContain("    name: macOS (Homebrew formula)\n");
    expect(job).toContain("    runs-on: macos-15\n");
    expect(job).toContain("      TAP: hersir-ci/local\n");
    expect(job).toContain("run: /bin/bash ./install.sh --build-only");
    // packed as Release builds packs the macOS archive
    expect(job).toContain("COPYFILE_DISABLE=1 /usr/bin/tar --no-xattrs -czf \"$archive\" \\\n            target/release/hersir target/release/hersir-mcp target/release/bundle/macos/Hersir.app");
    expect(job).toContain('brew tap-new --no-git "$TAP"');
    expect(job).toContain('brew install --formula "$TAP/hersir"');
    expect(job).toContain('brew test --verbose "$TAP/hersir"');
    expect(job).toContain('brew audit --strict --formula "$TAP/hersir"');
    expect(job).toContain('codesign --verify --deep --strict --verbose=2 "$app"');
    expect(job).toContain('if xattr -p com.apple.quarantine "$path"; then');
    expect(job).toContain('brew uninstall --formula "$TAP/hersir"');
    // brew test and brew audit read the formula as published, with its url on GitHub.
    expect(job.indexOf('cp "$RUNNER_TEMP/published.rb"')).toBeLessThan(job.indexOf("brew test --verbose"));
  });

  it("install.md: macOS: Homebrew, with brew install yotech-ai/tap/hersir, the link line, update and uninstall (the data stays)", () => {
    const install = read("docs/install.md");
    const from = install.indexOf("### macOS: Homebrew\n");
    expect(from).toBeGreaterThan(0);
    const section = install.slice(from, install.indexOf("\n## ", from));
    expect(section).toContain("```sh\nbrew install yotech-ai/tap/hersir\n```");
    expect(section).toContain('```sh\nmkdir -p ~/Applications && ln -sfn "$(brew --prefix)/opt/hersir/Hersir.app" ~/Applications/Hersir.app\n```');
    expect(section).toContain("`brew update && brew upgrade hersir` updates it");
    expect(section).toContain("`brew uninstall hersir` removes it");
    expect(section).toContain("Your data stays in `~/Library/Application Support/Hersir`");
    expect(section).toContain("A formula, not a cask");
    // the macOS section links to it
    expect(install).toContain("[Homebrew](#macos-homebrew)");
  });

  it("PLATFORMS.md, packaging/README.md and RELEASING.md name the formula and its tap", () => {
    const platforms = read("docs/PLATFORMS.md");
    expect(platforms).toContain("- **The Homebrew formula.** `hersir` in the tap `yotech-ai/tap`");
    expect(platforms).toContain("A formula, never a cask");
    const packaging = read("packaging/README.md");
    expect(packaging).toContain("| [`homebrew`](homebrew) | the formula `hersir` in the Homebrew tap `yotech-ai/tap`, for Macs with Apple silicon | `brew install yotech-ai/tap/hersir` |");
    const releasing = read("docs/RELEASING.md");
    expect(releasing).toContain("`Homebrew formula changed?`, then `macOS (Homebrew formula)`");
    expect(releasing).toContain("That's eighteen checks:");
    expect(releasing).toContain("`HERSIR_TAP_REPO`");
  });
});
