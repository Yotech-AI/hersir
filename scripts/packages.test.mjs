// Tests for the AUR package and its publishing (GA-274): packaging/aur/hersir-bin, scripts/packages.sh and their docs.
// The script runs as it is, with the system's own bash, git and ssh-keygen, against a stand-in for gh (as
// sign-release.test.mjs fakes it) that holds a fake release v9.9.9 in a scratch folder, signed here with throwaway keys
// over the message scripts/sign-release.sh signs. Never against a real release, GitHub's repository or the AUR, and never
// with Hersir's own keys: HERSIR_RELEASE_SIGNERS names an allowed-signers file with a throwaway key, HERSIR_AUR_REPO a
// local bare repository instead of the AUR's hersir-bin, HERSIR_SOURCE_REPO a local repository whose commit holds this
// checkout's install.sh, icons and NOTICE, and HOME is a scratch folder. Not on Windows, where the script doesn't run.
//
// The script runs gh, git and ssh-keygen from /usr/bin before the other folders on PATH, and GitHub's runners have the
// real gh there. So the stand-in is a bash function, gh, in a file BASH_ENV names: bash reads it before the script, and a
// function wins over every folder on PATH. ssh-keygen is wrapped the same way, only to write down how it ran.
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
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
// Scoop's manifest template (GA-276), as scripts/packages.sh fills it in.
const SCOOP_MANIFEST = read("packaging/scoop/hersir.json");

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

  // A bare repository instead of the Scoop bucket Yotech-AI/scoop-bucket (GA-276): empty, as before the first push, or
  // with one commit of `files` (paths with folders) on `branch`. Never the real bucket.
  function bucket(files, branch = "main") {
    const bare = mkdtempSync(join(scratch, "bucket-"));
    git(["-c", `init.defaultBranch=${branch}`, "init", "-q", "--bare", bare]);
    if (files) {
      const work = mkdtempSync(join(scratch, "bucket-work-"));
      for (const [name, text] of Object.entries(files)) {
        mkdirSync(dirname(join(work, name)), { recursive: true });
        writeFileSync(join(work, name), text);
      }
      git(["--git-dir", bare, "--work-tree", work, "add", "-A"]);
      git(["--git-dir", bare, "--work-tree", work, "commit", "-q", "-m", "what the bucket has"]);
    }
    return bare;
  }
  // A file on the bucket's main, byte for byte.
  const inBucket = (bare, file) => {
    const r = spawnSync("git", ["--git-dir", bare, "show", `main:${file}`], { encoding: "utf8" });
    if (r.status !== 0) throw new Error(`git show main:${file}: ${r.stderr}`);
    return r.stdout;
  };

  // Runs scripts/packages.sh with `args`, in a session of its own (no terminal), `input` on stdin, and only these
  // variables: what gh ran, ssh-keygen's calls, stdout, stderr, and what it left in its TMPDIR. `scoop` is the bucket's
  // repository (HERSIR_SCOOP_REPO): without one, an empty local bucket of the run's own, so no run reaches the real one.
  function publish(args, { rel, repo, scoop, input = "y\n", env = {}, bashEnv = standins, path = process.env.PATH } = {}) {
    const run = mkdtempSync(join(scratch, "run-"));
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
        HERSIR_SCOOP_REPO: scoop ?? bucket(),
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

  it("the same release again: nothing to publish, no question, the AUR's repository (and the bucket) as it was", async () => {
    const repo = aur();
    const scoop = bucket();
    expect((await publish([TAG], { rel: release(), repo, scoop })).status).toBe(0);
    const before = refs(repo);
    const bucketBefore = refs(scoop);
    const r = await publish([TAG], { rel: release(), repo, scoop, input: "" });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.9-1");
    expect(r.stdout).toContain("  Stays       9.9.9-1");
    expect(r.stdout).toContain("Nothing to publish: each package has v9.9.9 already.");
    expect(r.stdout).not.toContain("Publish v9.9.9?");
    expect(refs(repo)).toBe(before);
    expect(refs(scoop)).toBe(bucketBefore);
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
    ["a package that isn't one", [TAG, "brew"], /^brew isn't one of the packages \(aur scoop\), so nothing is published/],
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

  // ---------- scoop: hersir in the bucket Yotech-AI/scoop-bucket (GA-276), always a local bucket here ----------

  const ZIP = "hersir-9.9.9-windows-x86_64.zip";
  const ZIP_URL = `https://github.com/Yotech-AI/hersir/releases/download/${TAG}/${ZIP}`;
  const BUCKET_FILES = ["LICENSE", "README.md", "bucket/hersir.json", "scripts/hersir-shortcut.ps1"];
  const bucketFiles = (bare) => git(["--git-dir", bare, "ls-tree", "-r", "--name-only", "main"]).split("\n").sort();
  // The template's hersir.json with this release's version, the zip's address and `sum`; the rest as it is.
  function expectedManifest(sum = archiveSum(ZIP)) {
    const swap = (text, key, value) => {
      const re = new RegExp(`^(\\s*"${key}": ")[^"]*(",?)$`, "m");
      expect(text).toMatch(re);
      return text.replace(re, (_, a, b) => `${a}${value}${b}`);
    };
    return swap(swap(swap(SCOOP_MANIFEST, "version", VERSION), "url", ZIP_URL), "hash", sum);
  }
  // A manifest that the bucket holds for another version.
  const manifestAt = (v) => SCOOP_MANIFEST.replace(/^(\s*"version": ")[^"]*(",?)$/m, (_, a, b) => `${a}${v}${b}`);

  it("scoop: a good signature and y: fills the version, the zip's address and its sha256 from the signed release into hersir.json and pushes the bucket's four files to its main (a local bucket)", async () => {
    const rel = release();
    const repo = aur();
    const scoop = bucket();
    const r = await publish([TAG], { rel, repo, scoop });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stderr).toBe("");

    // One commit on main, with exactly the bucket's four files.
    expect(git(["--git-dir", scoop, "for-each-ref", "--format=%(refname)"])).toBe("refs/heads/main");
    expect(git(["--git-dir", scoop, "rev-list", "--count", "main"])).toBe("1");
    expect(git(["--git-dir", scoop, "log", "-1", "--format=%s", "main"])).toBe("hersir 9.9.9");
    expect(git(["--git-dir", scoop, "log", "-1", "--format=%b", "main"])).toContain(`Hersir ${TAG} at ${COMMIT}`);
    expect(bucketFiles(scoop)).toEqual(BUCKET_FILES);
    expect(inBucket(scoop, "LICENSE")).toBe(read("packaging/LICENSE"));
    expect(inBucket(scoop, "README.md")).toBe(read("packaging/scoop/README.md"));
    expect(inBucket(scoop, "scripts/hersir-shortcut.ps1")).toBe(read("packaging/scoop/scripts/hersir-shortcut.ps1"));

    // The manifest: the template with this release's three values, and nothing else changed; valid JSON, as Scoop reads it.
    expect(inBucket(scoop, "bucket/hersir.json")).toBe(expectedManifest());
    const m = JSON.parse(inBucket(scoop, "bucket/hersir.json"));
    expect(m.version).toBe(VERSION);
    expect(m.architecture["64bit"]).toEqual({ url: ZIP_URL, hash: archiveSum(ZIP) });
    expect(m.bin).toEqual(["hersir.exe", "hersir-mcp.exe"]);
    expect(m.checkver).toBeUndefined();
    expect(m.autoupdate).toBeUndefined();

    // Both packages, as none was named: the AUR too.
    expect(pushed(repo, "PKGBUILD")).toBe(expected().pkgbuild);

    // What it shows for the bucket, in this order, before it asks; then that it published both.
    const shown = [
      `hersir-bin, in ${repo} (HERSIR_AUR_REPO), not the AUR`,
      `hersir, in ${scoop} (HERSIR_SCOOP_REPO), not the bucket Yotech-AI/scoop-bucket`,
      "  Now         not in the bucket yet: this push puts it there",
      "  Becomes     9.9.9",
      `  Archive     ${archiveSum(ZIP)}  ${ZIP} (the signed SHA256SUMS)`,
      `+    "version": "${VERSION}",`,
      `+            "url": "${ZIP_URL}",`,
      `+            "hash": "${archiveSum(ZIP)}"`,
      "Publish v9.9.9? [y/N] ",
      `Published hersir-bin 9.9.9-1 to ${repo}`,
      `Published hersir 9.9.9 to ${scoop}: scoop update; scoop update hersir updates it.`,
    ];
    let at = 0;
    for (const line of shown) {
      const next = r.stdout.indexOf(line, at);
      expect(next, `${line} in\n${r.stdout}`).toBeGreaterThanOrEqual(at);
      at = next + line.length;
    }
    // One question for both.
    expect(r.stdout.split("Publish v9.9.9? [y/N] ").length - 1).toBe(1);

    // gh only reads, and only SHA256SUMS and the .sig are downloaded: never the zip.
    const downloads = r.gh.filter((l) => l.startsWith("gh [release] [download]"));
    expect(downloads.map((l) => l.match(/\[--pattern\] \[([^\]]+)\]/)[1]).sort()).toEqual(["SHA256SUMS", SIG].sort());
    expect(r.gh.every((l) => /^gh \[(auth\] \[status|release\] \[view|release\] \[download|api\] \[repos\/Yotech-AI\/hersir\/git\/ref\/tags\/v9\.9\.9)\]/.test(l))).toBe(true);
    expect(r.tmpLeft).toEqual([]);
  });

  it("scoop: the zip's sha256 is its line in the signed SHA256SUMS, not the zip the release holds", async () => {
    const rel = release();
    writeFileSync(join(rel, ZIP), "another build\n");
    const scoop = bucket();
    const r = await publish([TAG, "scoop"], { rel, scoop });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(JSON.parse(inBucket(scoop, "bucket/hersir.json")).architecture["64bit"].hash).toBe(archiveSum(ZIP));
    expect(inBucket(scoop, "bucket/hersir.json")).not.toContain(sha256("another build\n"));
  });

  it("scoop alone: scripts/packages.sh v9.9.9 scoop publishes the bucket and leaves the AUR alone", async () => {
    const repo = aur();
    const before = refs(repo);
    const scoop = bucket();
    const r = await publish([TAG, "scoop"], { rel: release(), repo, scoop });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(refs(repo)).toBe(before);
    expect(r.stdout).not.toContain("hersir-bin");
    expect(inBucket(scoop, "bucket/hersir.json")).toBe(expectedManifest());
  });

  // The card: publishes nothing on a bad signature. The bucket holds an older release, so a push would show.
  it.each([
    ["no hersir-9.9.9.sig", () => release({ signer: null }), {}, /^The release v9\.9\.9 has no hersir-9\.9\.9\.sig: it isn't signed, so nothing is published/],
    ["a signature by another key", () => release({ signer: otherKey }), {}, /^hersir-9\.9\.9\.sig is signed with a key that isn't in .*, so nothing is published\.\n$/],
    [
      "a SHA256SUMS whose zip line changed after signing",
      () => release({ changed: plainSums().replace(`${archiveSum(ZIP)}  ${ZIP}`, `${sha256("another build\n")}  ${ZIP}`) }),
      {},
      /^hersir-9\.9\.9\.sig doesn't sign this SHA256SUMS with the commit v9\.9\.9 points to/,
    ],
    ["the tag on another commit than the signed one", () => release(), { FAKE_COMMIT: "NO_NOTICE" }, /^hersir-9\.9\.9\.sig doesn't sign this SHA256SUMS with the commit v9\.9\.9 points to/],
    ["Hersir's own release-signers (no HERSIR_RELEASE_SIGNERS)", () => release(), { HERSIR_RELEASE_SIGNERS: undefined }, /is signed with a key that isn't in .*release-signers, so nothing is published/],
  ])("scoop: %s: nothing is published to the bucket (nor the AUR), one line says why", async (_, rel, env, why) => {
    if (env.FAKE_COMMIT === "NO_NOTICE") env = { FAKE_COMMIT: NO_NOTICE };
    const scoop = bucket({ "bucket/hersir.json": manifestAt("9.9.8") });
    const repo = aur();
    const before = refs(scoop);
    const aurBefore = refs(repo);
    for (const args of [[TAG, "scoop"], [TAG]]) {
      const r = await publish(args, { rel: rel(), repo, scoop, env });
      refused(r, scoop, before, why);
      expect(r.stdout).not.toContain("Publish v9.9.9?");
      expect(refs(repo)).toBe(aurBefore);
    }
  });

  it.each([
    ["n", "n\n"],
    ["nothing (end of input)", ""],
  ])("scoop: the answer %s: nothing is published to the bucket", async (_, input) => {
    const scoop = bucket({ "bucket/hersir.json": manifestAt("9.9.8") });
    const before = refs(scoop);
    const r = await publish([TAG, "scoop"], { rel: release(), scoop, input });
    refused(r, scoop, before, /^Nothing is published: the answer wasn't y\.\n$/);
    expect(r.stdout).toContain(`+    "version": "${VERSION}",`);
  });

  it("scoop: the same release again, with the same files: nothing to publish, no question, the bucket as it was", async () => {
    const scoop = bucket();
    expect((await publish([TAG, "scoop"], { rel: release(), scoop })).status).toBe(0);
    const before = refs(scoop);
    const r = await publish([TAG, "scoop"], { rel: release(), scoop, input: "" });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.9");
    expect(r.stdout).toContain("  Stays       9.9.9");
    expect(r.stdout).toContain("Nothing to publish: each package has v9.9.9 already.");
    expect(r.stdout).not.toContain("Publish v9.9.9?");
    expect(refs(scoop)).toBe(before);
  });

  it("scoop: an older release in the bucket (9.9.8), with a file the bucket shouldn't have: publishes 9.9.9 on top of it, with the four files only", async () => {
    const scoop = bucket({ "bucket/hersir.json": manifestAt("9.9.8"), "bucket/other.json": "{}\n", "README.md": "older\n" });
    const r = await publish([TAG, "scoop"], { rel: release(), scoop });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Now         9.9.8");
    expect(r.stdout).toContain("  Becomes     9.9.9");
    expect(r.stdout).toContain(`-    "version": "9.9.8",`);
    expect(git(["--git-dir", scoop, "rev-list", "--count", "main"])).toBe("2");
    expect(bucketFiles(scoop)).toEqual(BUCKET_FILES);
    expect(inBucket(scoop, "bucket/hersir.json")).toBe(expectedManifest());
  });

  it("scoop: the same release with other bucket files: publishes it again, as packaging/scoop has it now", async () => {
    const scoop = bucket();
    expect((await publish([TAG, "scoop"], { rel: release(), scoop })).status).toBe(0);
    const work = mkdtempSync(join(scratch, "bucket-work-"));
    git(["clone", "-q", "-b", "main", scoop, work]);
    writeFileSync(join(work, "README.md"), "an older README\n");
    git(["-C", work, "commit", "-q", "-a", "-m", "an older README"]);
    git(["-C", work, "push", "-q", "origin", "HEAD:main"]);
    const r = await publish([TAG, "scoop"], { rel: release(), scoop });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Becomes     9.9.9 again, with the bucket's files as packaging/scoop has them now");
    expect(inBucket(scoop, "README.md")).toBe(read("packaging/scoop/README.md"));
  });

  it.each([
    ["a newer release in the bucket (10.0.0)", "10.0.0", /^hersir in the bucket is at 10\.0\.0 already, after 9\.9\.9, so nothing is published\.\n$/],
    ["a version in the bucket that isn't X.Y.Z", "9.9.9-beta", /^hersir in the bucket has the version '9\.9\.9-beta', which isn't X\.Y\.Z: look at it first, nothing is published\.\n$/],
  ])("scoop: %s: nothing is published, not even the AUR, one line says why", async (_, v, why) => {
    const scoop = bucket({ "bucket/hersir.json": manifestAt(v) });
    const repo = aur();
    const before = refs(scoop);
    const aurBefore = refs(repo);
    refused(await publish([TAG], { rel: release(), repo, scoop }), scoop, before, why);
    expect(refs(repo)).toBe(aurBefore);
  });

  it.each([
    ["no Windows zip in the release", { without: [ZIP] }, /^The release v9\.9\.9 has no hersir-9\.9\.9-windows-x86_64\.zip, so nothing is published\.\n$/],
    [
      "a signed SHA256SUMS without the zip's line",
      { sums: ARCHIVES.filter((a) => a !== ZIP).map((a) => `${archiveSum(a)}  ${a}\n`).join("") },
      /^The signed SHA256SUMS of v9\.9\.9 has no one line for hersir-9\.9\.9-windows-x86_64\.zip, so nothing is published\.\n$/,
    ],
  ])("scoop: %s: nothing is published, not even the AUR, one line says why", async (_, opts, why) => {
    const scoop = bucket();
    const repo = aur();
    const aurBefore = refs(repo);
    refused(await publish([TAG], { rel: release(opts), repo, scoop }), scoop, "", why);
    expect(refs(repo)).toBe(aurBefore);
  });

  it("scoop: a bucket that isn't there (yet): nothing is published, not even the AUR, one line says why", async () => {
    const repo = aur();
    const aurBefore = refs(repo);
    const missing = join(scratch, "no-bucket.git");
    const r = await publish([TAG], { rel: release(), repo, scoop: missing });
    refused(r, undefined, undefined, new RegExp(`^Couldn't read ${esc(missing)}, so nothing is published: .*Is the bucket there yet\\? To publish the other packages alone, name them \\(see --help\\)\\.\\n$`));
    expect(refs(repo)).toBe(aurBefore);
    expect(existsSync(missing)).toBe(false);
  });

  it("scoop: a bucket whose default branch isn't main: the push goes to main, the other branch stays", async () => {
    const scoop = bucket({ "README.md": "on master\n" }, "master");
    const master = git(["--git-dir", scoop, "rev-parse", "master"]);
    const r = await publish([TAG, "scoop"], { rel: release(), scoop });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(git(["--git-dir", scoop, "rev-parse", "master"])).toBe(master);
    expect(bucketFiles(scoop)).toEqual(BUCKET_FILES);
    expect(git(["--git-dir", scoop, "rev-list", "--count", "main"])).toBe("1");
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
      const r = await publish([TAG], { rel: release(), repo, bashEnv: ghOnly, path: `${bin}:${process.env.PATH}` });
      expect(r.status, r.stdout + r.stderr).toBe(0);
      expect(r.evil).toEqual([]);
      expect(pushed(repo, "PKGBUILD")).toBe(expected().pkgbuild);
    },
  );
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

describe("packaging/scoop (GA-276): the bucket's manifest, its shortcut script, install.ps1's hersir-mcp.exe, CI and docs", () => {
  const manifest = JSON.parse(SCOOP_MANIFEST);
  const shortcut = read("packaging/scoop/scripts/hersir-shortcut.ps1");

  it("hersir.json: the release's Windows zip with a sha256, both programs on the PATH from target\\release, no checkver or autoupdate", () => {
    expect(manifest.version).toMatch(/^\d+\.\d+\.\d+$/);
    const v = manifest.version;
    expect(manifest.architecture).toEqual({
      "64bit": {
        url: `https://github.com/Yotech-AI/hersir/releases/download/v${v}/hersir-${v}-windows-x86_64.zip`,
        hash: expect.stringMatching(/^[0-9a-f]{64}$/),
      },
    });
    // The zip holds target/release/hersir.exe and hersir-mcp.exe, as Release builds packs it.
    expect(read(".github/workflows/release-builds.yml")).toContain('CreateEntryFromFile($zip, $file, "target/release/$p", \'Optimal\')');
    expect(manifest.extract_dir).toBe("target\\release");
    expect(manifest.bin).toEqual(["hersir.exe", "hersir-mcp.exe"]);
    // Only scripts/packages.sh changes the version: no checkver or autoupdate, anywhere in it.
    expect(SCOOP_MANIFEST).not.toMatch(/"(checkver|autoupdate)"\s*:/);
    // No persist and no Scoop shortcuts: the data stays in %APPDATA%\Hersir, and the shortcut comes from the bucket's script.
    expect(manifest.persist).toBeUndefined();
    expect(manifest.shortcuts).toBeUndefined();
    expect(manifest.license).toBe("Apache-2.0");
    // One line each of the three values scripts/packages.sh fills in.
    for (const key of ["version", "url", "hash"]) expect(SCOOP_MANIFEST.match(new RegExp(`^\\s*"${key}": "[^"]*",?$`, "gm")), key).toHaveLength(1);
  });

  it("hersir.json: post_install writes hersir-package.txt saying scoop beside the programs, then makes the shortcut for $dir (Scoop's current folder); pre_uninstall removes that shortcut", () => {
    const post = manifest.post_install.join("\n");
    expect(post).toContain("Set-Content -LiteralPath \"$dir\\hersir-package.txt\" -Value 'scoop' -Encoding Ascii");
    expect(post.indexOf("hersir-package.txt")).toBeLessThan(post.indexOf("hersir-shortcut.ps1"));
    expect(post).toContain('"$bucketsdir\\$bucket\\scripts\\hersir-shortcut.ps1"');
    expect(post).toContain("& $hersirShortcut -Dir $dir -Global:([bool]$global)");
    const pre = manifest.pre_uninstall.join("\n");
    expect(pre).toContain("Programs\\Scoop Apps\\Hersir.lnk");
    // Nothing in the hooks touches Hersir's data.
    expect(`${post}\n${pre}`).not.toMatch(/APPDATA|dev\.hersir\.app\\|Remove-Item[^\n]*\$dir/i);
  });

  it("hersir-shortcut.ps1: Start Menu\\Programs\\Scoop Apps\\Hersir.lnk starts hersir.exe in -Dir and carries dev.hersir.app through PKEY_AppUserModel_ID, as install.ps1's shortcut does", () => {
    expect(shortcut).toContain("$exe = Join-Path $Dir 'hersir.exe'");
    expect(shortcut).toContain("$folder = Join-Path $startMenu 'Programs\\Scoop Apps'");
    expect(shortcut).toContain("$link = Join-Path $folder 'Hersir.lnk'");
    expect(shortcut).toContain("[HersirScoop.Shortcut]::Save($link, $exe, $Dir, $description, 'dev.hersir.app')");
    // The same COM ids and property key as install.ps1's own shortcut.
    const install = read("install.ps1");
    for (const id of ["00021401-0000-0000-C000-000000000046", "000214F9-0000-0000-C000-000000000046", "886D8EEB-8CF2-4446-8D02-CDBA1DBDCF99", "9F4C2855-9F79-4B39-A8D0-E1D42DE1D5F3"]) {
      expect(shortcut, id).toContain(id);
      expect(install, id).toContain(id);
    }
    expect(shortcut).toContain("key.PropertyId = 5;");
    expect(shortcut).toContain("value.VarType = 31; // VT_LPWSTR");
    // The IShellLinkW methods in vtable order, as install.ps1 declares them: one out of place breaks every call after it.
    const methods = (text) => [...text.slice(text.indexOf("interface IShellLinkW")).matchAll(/^\s+void (\w+)\(/gm)].map((m) => m[1]).slice(0, 18);
    expect(methods(shortcut)).toEqual(methods(install));
    expect(methods(shortcut)[17]).toBe("SetPath");
  });

  it("install.ps1 builds hersir-mcp.exe with a static C runtime, for that program alone, so it needs no VCRUNTIME140.dll", () => {
    const install = read("install.ps1");
    expect(install).toContain("Invoke-Step 'cargo' @('rustc', '--release', '-p', 'gizai-mcp', '--bin', 'hersir-mcp', '--', '-C', 'target-feature=+crt-static')");
    expect(install).not.toContain("@('build', '--release', '-p', 'gizai-mcp')");
    expect(install).not.toMatch(/RUSTFLAGS[^\n]*crt-static/);
    // The bin is hersir-mcp, and gizai-mcp has no build script that could bring C code with another C runtime.
    expect(read("crates/gizai-mcp/Cargo.toml")).toMatch(/\[\[bin\]\]\s*\nname = "hersir-mcp"/);
    expect(existsSync(join(ROOT, "crates/gizai-mcp/build.rs"))).toBe(false);
  });

  it("scripts/packages.sh: scoop is a package, its bucket Yotech-AI/scoop-bucket's main, with HERSIR_SCOOP_REPO for tests", () => {
    const script = read("scripts/packages.sh");
    expect(script).toContain('PACKAGES="aur scoop"');
    expect(script).toContain('SCOOP_REPO="${HERSIR_SCOOP_REPO:-https://github.com/Yotech-AI/scoop-bucket.git}"');
    expect(script).toContain('push --quiet "$SCOOP_REPO" HEAD:refs/heads/main');
    expect(script).not.toMatch(/push[^\n]*(--force|-f\b|\+HEAD)/);
    for (const fn of ["scoop_prepare", "scoop_show", "scoop_publish"]) expect(script).toContain(`\n${fn}() {`);
  });

  it("no workflow pushes to the bucket Yotech-AI/scoop-bucket: only scripts/packages.sh, on Jeffrey's computer", () => {
    const dir = join(ROOT, ".github/workflows");
    for (const file of readdirSync(dir)) {
      const code = readFileSync(join(dir, file), "utf8")
        .split("\n")
        .filter((l) => !l.trim().startsWith("#"))
        .join("\n");
      expect(code, file).not.toContain("scoop-bucket.git");
      expect(code, file).not.toContain("github.com/Yotech-AI/scoop-bucket");
    }
  });

  it("ci.yml's Windows (Scoop) job: installs from a local bucket on windows-latest and checks what the card names", () => {
    const ci = read(".github/workflows/ci.yml");
    const changes = ci.slice(ci.indexOf("\n  scoop-changes:\n"), ci.indexOf("\n  scoop:\n"));
    expect(changes).toContain("name: Scoop package changed?");
    expect(changes).toContain("packaging/scoop install.ps1 crates/gizai-mcp/Cargo.toml .github/workflows/ci.yml");
    const job = ci.slice(ci.indexOf("\n  scoop:\n"));
    expect(job).toContain("name: Windows (Scoop)");
    expect(job).toContain("runs-on: windows-latest");
    expect(job).toContain("if: needs.scoop-changes.outputs.changed == 'true'");
    expect(job).toContain("powershell -NoProfile -ExecutionPolicy Bypass -File .\\install.ps1 -BuildOnly");
    expect(job).toContain("/dependents");
    expect(job).toContain("(vcruntime|msvcp)");
    expect(job).toContain("scoop bucket add hersir $url");
    expect(job).toContain("scoop install hersir/hersir");
    expect(job).toContain('if ("$got" -ne "hersir $env:VERSION")');
    expect(job).toContain("if (\"$mark\" -ne 'scoop')");
    expect(job).toContain("ExtendedProperty('System.AppUserModel.ID')");
    expect(job).toContain("if ($id -ne 'dev.hersir.app')");
    expect(job).toContain("-like '*\\apps\\hersir\\current\\hersir.exe'");
    expect(job).toContain("-Stream 'Zone.Identifier'");
    expect(job).toContain("scoop uninstall hersir");
    expect(job).toContain("scoop uninstall hersir --purge");
    expect(job).toContain("if ($after -ne $before)");
    // It uses no secrets and no token.
    expect(job).not.toMatch(/secrets\.|GITHUB_TOKEN|GH_TOKEN/);
  });

  it("docs: install.md's Windows: Scoop, PLATFORMS.md, RELEASING.md's checks and step 6, and the bucket's README", () => {
    const install = read("docs/install.md");
    const section = install.slice(install.indexOf("### Windows: Scoop"), install.indexOf("\n## ", install.indexOf("### Windows: Scoop")));
    expect(section).toContain("```powershell\nscoop bucket add hersir https://github.com/Yotech-AI/scoop-bucket\nscoop install hersir/hersir\n```");
    expect(section).toContain("scoop update; scoop update hersir");
    expect(section).toContain("`scoop uninstall hersir`");
    expect(section).toContain("Your data stays in `%APPDATA%\\Hersir`");
    expect(install).toContain("Smart App Control");
    expect(read("docs/PLATFORMS.md")).toContain("**The Scoop package.**");
    const releasing = read("docs/RELEASING.md");
    expect(releasing).toContain("That's eighteen checks:");
    expect(releasing).toContain("`Scoop package changed?` with `Windows (Scoop)`");
    expect(releasing).toContain("Tests put a local repository in `HERSIR_SCOOP_REPO`, never the bucket.");
    const readme = read("packaging/scoop/README.md");
    for (const text of [
      "scoop bucket add hersir https://github.com/Yotech-AI/scoop-bucket",
      "scoop install hersir/hersir",
      "scoop update; scoop update hersir",
      "scoop uninstall hersir",
      "Your data stays in `%APPDATA%\\Hersir`",
      "Smart App Control",
    ]) {
      expect(readme, text).toContain(text);
    }
  });
});
