// Tests for the release signature (GA-260): release-signers, scripts/sign-release.sh and the docs. The script runs as it
// is, with the system's own bash (macOS's 3.2 there) and ssh-keygen, against a stand-in for gh (as install-ci.test.mjs
// fakes it) that holds a fake release v9.9.9 in a scratch folder, and with throwaway keys made here, one with a
// passphrase. Never against a real release and never with Hersir's own keys: HERSIR_RELEASE_SIGNERS names an
// allowed-signers file with the throwaway key, HOME is a scratch folder (so ~/.ssh/hersir-release isn't Jeffrey's), and
// gh gets no token and an empty config folder. Not on Windows, where the script doesn't run.
//
// The script runs gh and ssh-keygen from /usr/bin (on macOS also /opt/homebrew/bin) before the other folders on PATH,
// and GitHub's runners have the real gh there. So the stand-in is a bash function, gh, in a file BASH_ENV names: bash
// reads it before the script, and a function wins over every folder on PATH. ssh-keygen is wrapped the same way, only to
// write down how it ran, and then runs as itself. ssh-keygen asks for the key's passphrase on the terminal: the script
// runs here in a session of its own, without one, so ssh-keygen reads it from stdin, after the script's y.
import { spawn, spawnSync } from "node:child_process";
import { createHash } from "node:crypto";
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const SCRIPT = join(ROOT, "scripts/sign-release.sh");
const unix = process.platform !== "win32";
const read = (path) => readFileSync(join(ROOT, path), "utf8");

// The card's two lines, Jeffrey's public keys of 10 Nov 2026.
const RELEASE_SIGNERS = [
  'hersir-release namespaces="hersir-release" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIPyS5akxwuAio54EJUMpJgss6XvxkyznQ3sxZKpOm4mX Hersir release key',
  'hersir-release namespaces="hersir-release" ssh-ed25519 AAAAC3NzaC1lZDI1NTE5AAAAIJ9ER9k9DUooCPSOMQaWoha6E4IFW+e3t08ZmH1llz3J Hersir spare release key',
];

const TAG = "v9.9.9";
const SIG = "hersir-9.9.9.sig";
const ARCHIVES = ["hersir-9.9.9-linux-x86_64.tar.gz", "hersir-9.9.9-macos-arm64.tar.gz", "hersir-9.9.9-windows-x86_64.zip"];
const COMMIT = "4f0c2a9e1b7d3c5a6e8f0b1d2c3e4f5a6b7c8d9e";
const ELSEWHERE = "0123456789abcdef0123456789abcdef01234567";
const TAG_OBJECT = "9e8d7c6b5a4f3e2d1c0b9a8f7e6d5c4b3a2f1e0d";
const SUBJECT = "Release 9.9.9";
const PASSPHRASE = "throwaway passphrase for QA";

// gh, as a stand-in: writes its arguments, each in [], to $RAN and answers from the fake release in $RELEASE (a folder
// with its assets) and the FAKE_ variables, through jq as gh's own --jq. Anything else is GitHub's 404.
const GH = `#!/bin/sh
{ printf 'gh'; for a in "$@"; do printf ' [%s]' "$a"; done; echo; } >> "$RAN"
jq_expr="" dir="" pattern="" clobber="" prev=""
for a in "$@"; do
  case "$prev" in
    --jq) jq_expr="$a" ;;
    --dir) dir="$a" ;;
    --pattern) pattern="$a" ;;
  esac
  [ "$a" = --clobber ] && clobber=1
  prev="$a"
done
answer() { if [ -n "$jq_expr" ]; then printf '%s' "$1" | jq -r "$jq_expr"; else printf '%s\\n' "$1"; fi; }
commit() { answer "{\\"object\\":{\\"type\\":\\"commit\\",\\"sha\\":\\"$1\\"}}"; }
api=repos/Yotech-AI/hersir
case "$1 $2" in
  "auth status") exit "\${FAKE_AUTH:-0}" ;;
  "release view")
    [ "$3" = "$FAKE_TAG" ] && [ -d "$RELEASE" ] || { echo "release not found" >&2; exit 1; }
    json="" ; for f in $(ls "$RELEASE"); do json="$json\${json:+,}{\\"name\\":\\"$f\\"}"; done
    answer "{\\"assets\\":[$json]}" ;;
  "release download")
    [ -f "$RELEASE/$pattern" ] || { echo "no assets match the file pattern" >&2; exit 1; }
    cp "$RELEASE/$pattern" "$dir/$pattern" ;;
  "release upload")
    name="$(basename "$4")"
    if [ -e "$RELEASE/$name" ] && [ -z "$clobber" ]; then echo "asset under the same name already exists: [$name]" >&2; exit 1; fi
    [ -n "\${FAKE_UPLOAD_LOST:-}" ] || cp "$4" "$RELEASE/$name" ;;
  "workflow run") exit 0 ;;
  "api $api/git/ref/tags/$FAKE_TAG")
    if [ -n "\${FAKE_TAG_OBJECT:-}" ]; then answer "{\\"object\\":{\\"type\\":\\"tag\\",\\"sha\\":\\"$FAKE_TAG_OBJECT\\"}}"; else commit "$FAKE_COMMIT"; fi ;;
  "api $api/git/tags/\${FAKE_TAG_OBJECT:-none}") commit "$FAKE_COMMIT" ;;
  "api $api/git/commits/$FAKE_COMMIT") answer "{\\"sha\\":\\"$FAKE_COMMIT\\",\\"message\\":$FAKE_MESSAGE}" ;;
  "api $api/git/ref/heads/production") commit "$FAKE_PRODUCTION" ;;
  *) echo "HTTP 404: Not Found ($*)" >&2; exit 1 ;;
esac
`;

// What BASH_ENV names: gh is the stand-in, and ssh-keygen writes how it ran to $RAN, then runs as itself.
const STANDINS = `gh() { "$FAKE_GH" "$@"; }
ssh-keygen() { { printf 'ssh-keygen'; printf ' [%s]' "$@"; echo; } >> "$RAN"; command ssh-keygen "$@"; }
`;

const sha256 = (data) => createHash("sha256").update(data).digest("hex");

describe.runIf(unix)("scripts/sign-release.sh, against a fake gh and throwaway keys", () => {
  let scratch, key, plainKey, otherKey, signers, gh, standins, ghOnly;

  beforeAll(() => {
    scratch = realpathSync(mkdtempSync(join(tmpdir(), "sign-release-")));
    const keygen = (file, passphrase) => {
      const r = spawnSync("ssh-keygen", ["-q", "-t", "ed25519", "-N", passphrase, "-C", "throwaway QA key", "-f", file], {
        encoding: "utf8",
      });
      expect(r.status, r.stderr).toBe(0);
      return file;
    };
    mkdirSync(join(scratch, "keys"));
    key = keygen(join(scratch, "keys/release"), PASSPHRASE);
    plainKey = keygen(join(scratch, "keys/plain"), "");
    otherKey = keygen(join(scratch, "keys/other"), PASSPHRASE);
    const pub = readFileSync(`${key}.pub`, "utf8").split(" ").slice(0, 2).join(" ");
    signers = join(scratch, "keys/signers");
    writeFileSync(signers, `hersir-release namespaces="hersir-release" ${pub} throwaway QA key\n`);
    gh = join(scratch, "gh");
    writeFileSync(gh, GH);
    chmodSync(gh, 0o755);
    standins = join(scratch, "standins.bash");
    writeFileSync(standins, STANDINS);
    ghOnly = join(scratch, "gh-only.bash");
    writeFileSync(ghOnly, STANDINS.split("\n")[0] + "\n");
  });
  afterAll(() => rmSync(scratch, { recursive: true, force: true }));

  // A fake release v9.9.9 in a folder of its own: the three archives and SHA256SUMS, as Release builds uploads them,
  // without the ones in `without`, with `sig` as hersir-9.9.9.sig when given, and `sums` instead of SHA256SUMS.
  function release({ without = [], sig, sums } = {}) {
    const dir = mkdtempSync(join(scratch, "release-"));
    const lines = ARCHIVES.map((a) => {
      const data = `the build in ${a}\n`;
      if (!without.includes(a)) writeFileSync(join(dir, a), data);
      return `${sha256(data)}  ${a}\n`;
    });
    if (!without.includes("SHA256SUMS")) writeFileSync(join(dir, "SHA256SUMS"), sums ?? lines.join(""));
    if (sig !== undefined) writeFileSync(join(dir, SIG), sig);
    return dir;
  }

  // Runs scripts/sign-release.sh with `args`, in a session of its own (no terminal), `input` on stdin, and only these
  // variables: what gh ran, ssh-keygen's calls, stdout, stderr, and what it left in its TMPDIR.
  function sign(args, { rel, input = `y\n${PASSPHRASE}\n`, env = {}, bashEnv = standins, path = process.env.PATH } = {}) {
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
        GH_CONFIG_DIR: join(run, "gh-config"),
        BASH_ENV: bashEnv,
        FAKE_GH: gh,
        RAN: ran,
        RELEASE: rel ?? join(run, "no-release"),
        FAKE_TAG: TAG,
        FAKE_COMMIT: COMMIT,
        FAKE_PRODUCTION: COMMIT,
        FAKE_MESSAGE: JSON.stringify(`${SUBJECT}\n\nThe notes of the fake release.\n`),
        HERSIR_RELEASE_KEY: key,
        HERSIR_RELEASE_SIGNERS: signers,
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
      const timer = setTimeout(() => process.kill(-child.pid, "SIGKILL"), 20000);
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

  // The message the one-line install and Update rebuild: the line "hersir v9.9.9 <commit>", a newline and SHA256SUMS.
  const message = (commit, sums) => Buffer.concat([Buffer.from(`hersir ${TAG} ${commit}\n`), Buffer.from(sums)]);
  const verify = (sig, data, file = signers, namespace = "hersir-release") =>
    spawnSync("ssh-keygen", ["-Y", "verify", "-f", file, "-I", "hersir-release", "-n", namespace, "-s", sig], { input: data });

  const uploads = (r) => r.gh.filter((l) => l.startsWith("gh [release] [upload]") || l.startsWith("gh [workflow]"));
  const signs = (r) => r.keygen.filter((l) => l.includes("[-Y] [sign]"));

  // Nothing signed or uploaded, the release as it was, and one line on stderr that says why.
  function refused(r, rel, why) {
    expect(r.status, r.stdout + r.stderr).not.toBe(0);
    expect(r.stderr).toMatch(/^[^\n]+\n$/);
    expect(r.stderr).toMatch(why);
    expect(signs(r)).toEqual([]);
    expect(uploads(r)).toEqual([]);
    if (rel) expect(existsSync(join(rel, SIG))).toBe(false);
    expect(r.tmpLeft).toEqual([]);
  }

  it("a complete release: shows the version, the commit, production and SHA256SUMS, signs after y, uploads hersir-9.9.9.sig and starts one-line.yml", async () => {
    const rel = release();
    const sums = readFileSync(join(rel, "SHA256SUMS"), "utf8");
    const r = await sign([TAG], { rel });
    expect(r.status, r.stdout + r.stderr).toBe(0);

    // What it shows, in this order, before it asks.
    const shown = [
      "  Version     v9.9.9",
      `  Commit      ${COMMIT} ${SUBJECT}`,
      "  Production  its latest commit is this one",
      ...sums.trimEnd().split("\n").map((l, i) => (i === 0 ? `  SHA256SUMS  ${l}` : `              ${l}`)),
      "Sign v9.9.9? [y/N] ",
    ];
    let at = 0;
    for (const line of shown) {
      const next = r.stdout.indexOf(line, at);
      expect(next, `${line} in\n${r.stdout}`).toBeGreaterThanOrEqual(at);
      at = next + line.length;
    }
    expect(r.stdout.slice(at)).toMatch(/^Signed v9\.9\.9: the release has hersir-9\.9\.9\.sig/);

    // Signed with the key, in its namespace, never through ssh-agent; checked, uploaded, listed, then One-line install.
    expect(signs(r)).toEqual([expect.stringMatching(/^ssh-keygen \[-Y\] \[sign\] \[-f\] \[.*\/keys\/release\] \[-n\] \[hersir-release\] \[.*\/hersir-9\.9\.9\]$/)]);
    expect(r.keygen.some((l) => l.includes("[-Y] [verify]") && l.includes(`[${signers}]`))).toBe(true);
    expect(uploads(r)).toEqual([
      expect.stringMatching(/^gh \[release\] \[upload\] \[v9\.9\.9\] \[.*\/hersir-9\.9\.9\.sig\] \[--repo\] \[Yotech-AI\/hersir\]$/),
      "gh [workflow] [run] [one-line.yml] [--repo] [Yotech-AI/hersir]",
    ]);
    const views = r.gh.map((l, i) => [l, i]).filter(([l]) => l.startsWith("gh [release] [view]"));
    expect(views.at(-1)[1]).toBeGreaterThan(r.gh.findIndex((l) => l.startsWith("gh [release] [upload]")));
    expect(r.gh.every((l) => !l.startsWith("gh [auth]") || l === "gh [auth] [status] [--hostname] [github.com]")).toBe(true);
    expect(r.gh.filter((l) => !l.startsWith("gh [auth]")).every((l) => l.includes("[--repo] [Yotech-AI/hersir]") || l.includes("[api] [repos/Yotech-AI/hersir/"))).toBe(true);
    expect(readdirSync(rel).sort()).toEqual([...ARCHIVES, "SHA256SUMS", SIG].sort());
    expect(r.tmpLeft).toEqual([]);
  });

  it("that .sig verifies over 'hersir v9.9.9 <commit>', a newline and SHA256SUMS, and not when one byte of them changes", async () => {
    const rel = release();
    const r = await sign([TAG], { rel });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    const sig = join(rel, SIG);
    const sums = readFileSync(join(rel, "SHA256SUMS"));

    const good = verify(sig, message(COMMIT, sums));
    expect(good.status, good.stderr.toString()).toBe(0);
    expect(good.stdout.toString()).toContain('Good "hersir-release" signature for hersir-release');

    // One byte of SHA256SUMS: a checksum's first digit, a name's letter, the last newline.
    const flipped = Buffer.from(sums);
    flipped[0] = flipped[0] === 0x61 ? 0x62 : 0x61;
    expect(verify(sig, message(COMMIT, flipped)).status).not.toBe(0);
    expect(verify(sig, message(COMMIT, sums.toString().replace("linux", "linuX"))).status).not.toBe(0);
    expect(verify(sig, message(COMMIT, sums.subarray(0, sums.length - 1))).status).not.toBe(0);
    // One character of the commit, another commit, another version, no newline after the line.
    expect(verify(sig, message(`${COMMIT.slice(0, -1)}f`, sums)).status).not.toBe(0);
    expect(verify(sig, message(ELSEWHERE, sums)).status).not.toBe(0);
    expect(verify(sig, Buffer.concat([Buffer.from(`hersir v9.9.8 ${COMMIT}\n`), sums])).status).not.toBe(0);
    expect(verify(sig, Buffer.concat([Buffer.from(`hersir ${TAG} ${COMMIT}`), sums])).status).not.toBe(0);
    // Another namespace, and Hersir's own keys: a throwaway key is neither of them.
    expect(verify(sig, message(COMMIT, sums), signers, "file").status).not.toBe(0);
    expect(verify(sig, message(COMMIT, sums), join(ROOT, "release-signers")).status).not.toBe(0);
  });

  it("says when production's latest commit isn't the tag's, and still signs the tag's commit after y", async () => {
    const rel = release();
    const r = await sign([TAG], { rel, env: { FAKE_PRODUCTION: ELSEWHERE } });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain(`  Production  its latest commit is ${ELSEWHERE}, not this one`);
    expect(r.stdout).not.toContain("its latest commit is this one");
    expect(verify(join(rel, SIG), message(COMMIT, readFileSync(join(rel, "SHA256SUMS")))).status).toBe(0);
  });

  it("an annotated tag: signs the commit it points to, not the tag object", async () => {
    const rel = release();
    const r = await sign([TAG], { rel, env: { FAKE_TAG_OBJECT: TAG_OBJECT } });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.gh).toContain(`gh [api] [repos/Yotech-AI/hersir/git/tags/${TAG_OBJECT}] [--jq] [.object.type + " " + .object.sha]`);
    expect(r.stdout).toContain(`  Commit      ${COMMIT} ${SUBJECT}`);
    const sums = readFileSync(join(rel, "SHA256SUMS"));
    expect(verify(join(rel, SIG), message(COMMIT, sums)).status).toBe(0);
    expect(verify(join(rel, SIG), message(TAG_OBJECT, sums)).status).not.toBe(0);
  });

  it("a commit subject with an escape sequence shows without its control characters", async () => {
    const rel = release();
    const r = await sign([TAG], { rel, env: { FAKE_MESSAGE: JSON.stringify("Release \u001b[2K\u001b[1A9.9.9\n") } });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).not.toContain("\u001b");
    expect(r.stdout).toContain(`  Commit      ${COMMIT} Release [2K[1A9.9.9`);
  });

  it("--again signs a signed release again and replaces its hersir-9.9.9.sig", async () => {
    const rel = release({ sig: "an old signature\n" });
    const r = await sign([TAG, "--again"], { rel });
    expect(r.status, r.stdout + r.stderr).toBe(0);
    expect(r.stdout).toContain("  Replaces    the release's hersir-9.9.9.sig (--again)");
    expect(uploads(r)[0]).toMatch(/^gh \[release\] \[upload\] \[v9\.9\.9\] \[.*\/hersir-9\.9\.9\.sig\] \[--repo\] \[Yotech-AI\/hersir\] \[--clobber\]$/);
    expect(verify(join(rel, SIG), message(COMMIT, readFileSync(join(rel, "SHA256SUMS")))).status).toBe(0);
  });

  it.each(ARCHIVES.concat("SHA256SUMS"))("a release without %s: nothing is signed or uploaded, one line says why", async (missing) => {
    const rel = release({ without: [missing] });
    const r = await sign([TAG], { rel });
    refused(r, rel, `The release v9.9.9 has no ${missing} yet`);
    expect(r.stdout).toBe("");
  });

  it("a release that has hersir-9.9.9.sig, without --again: nothing is signed or uploaded, its .sig stays", async () => {
    const rel = release({ sig: "an old signature\n" });
    const r = await sign([TAG], { rel });
    expect(r.status).not.toBe(0);
    expect(r.stderr).toBe("The release v9.9.9 is signed already (hersir-9.9.9.sig): --again signs it again and replaces it.\n");
    expect(signs(r)).toEqual([]);
    expect(uploads(r)).toEqual([]);
    expect(readFileSync(join(rel, SIG), "utf8")).toBe("an old signature\n");
  });

  it("a key without a passphrase: nothing is signed or uploaded, one line says why", async () => {
    const rel = release();
    const r = await sign([TAG], { rel, env: { HERSIR_RELEASE_KEY: plainKey }, input: "y\n\n" });
    refused(r, rel, /has no passphrase/);
    expect(r.keygen).toEqual([`ssh-keygen [-y] [-P] [] [-f] [${plainKey}]`]);
    expect(r.gh.filter((l) => l.startsWith("gh [release]") || l.startsWith("gh [api]"))).toEqual([]);
  });

  it.each([
    ["no key at HERSIR_RELEASE_KEY", () => join(scratch, "keys/none"), /There is no release key at/],
    ["a public key", () => `${key}.pub`, /is a public key/],
  ])("%s: nothing is signed or uploaded, one line says why", async (_, file, why) => {
    const rel = release();
    refused(await sign([TAG], { rel, env: { HERSIR_RELEASE_KEY: file() } }), rel, why);
  });

  it("no HERSIR_RELEASE_KEY: the key is ~/.ssh/hersir-release, and with none there nothing is signed", async () => {
    const rel = release();
    const r = await sign([TAG], { rel, env: { HERSIR_RELEASE_KEY: undefined } });
    refused(r, rel, /^There is no release key at \/.*\/home\/\.ssh\/hersir-release: set HERSIR_RELEASE_KEY to its file\.\n$/);
  });

  it.each([
    ["n", "n\n"],
    ["an empty line", "\n"],
    ["Y", "Y\n"],
    ["yes", "yes\n"],
    ["nothing (end of input)", ""],
  ])("the answer %s: nothing is signed or uploaded, one line says why", async (_, answer) => {
    const rel = release();
    const r = await sign([TAG], { rel, input: answer });
    refused(r, rel, /^Nothing is signed: the answer wasn't y\.\n$/);
    expect(r.stdout).toContain("Sign v9.9.9? [y/N] ");
  });

  it.each(["9.9.9", "v9.9", "v9.9.9-rc.1", "v9.9.9.1", "vX.Y.Z", "V9.9.9", "latest"])(
    "the tag %s isn't vX.Y.Z: nothing is signed or uploaded, gh isn't asked, one line says why",
    async (tag) => {
      const rel = release();
      const r = await sign([tag], { rel });
      refused(r, rel, /isn't a version like v1\.2\.3/);
      expect(r.gh).toEqual([]);
      expect(r.keygen).toEqual([]);
    },
  );

  it.each([
    ["no version", [], /^Give the release's version/],
    ["two versions", [TAG, "v9.9.8"], /^Give one version, not v9\.9\.9 and v9\.9\.8\./],
    ["an unknown option", [TAG, "--yes"], /^--yes isn't an option/],
  ])("%s: nothing is signed or uploaded, one line says why", async (_, args, why) => {
    const rel = release();
    const r = await sign(args, { rel });
    refused(r, rel, why);
    expect(r.gh).toEqual([]);
  });

  it("gh not logged in: nothing is signed or uploaded, one line says why", async () => {
    const rel = release();
    const r = await sign([TAG], { rel, env: { FAKE_AUTH: "1" } });
    refused(r, rel, /^gh isn't logged in to GitHub: run gh auth login, then this again\.\n$/);
    expect(r.gh).toEqual(["gh [auth] [status] [--hostname] [github.com]"]);
  });

  it("no release v9.9.9 on GitHub: nothing is signed or uploaded, one line says why", async () => {
    refused(await sign([TAG]), undefined, /^Couldn't read the release v9\.9\.9 of Yotech-AI\/hersir: release not found\n$/);
  });

  it.each([
    ["an extra line", (s) => `${s}${"0".repeat(64)}  install.sh\n`],
    ["the archives in another order", (s) => s.split("\n").filter(Boolean).reverse().join("\n") + "\n"],
    ["an uppercase checksum", (s) => s.replace(/^[0-9a-f]{64}/, (h) => h.toUpperCase())],
    ["a carriage return", (s) => s.replace("\n", "\r\n")],
  ])("SHA256SUMS with %s isn't Release builds' SHA256SUMS: nothing is signed or uploaded", async (_, change) => {
    const plain = readFileSync(join(release(), "SHA256SUMS"), "utf8");
    const rel = release({ sums: change(plain) });
    refused(await sign([TAG], { rel }), rel, /^The SHA256SUMS of v9\.9\.9 isn't one line per archive/);
  });

  it("a wrong passphrase: ssh-keygen doesn't sign and nothing is uploaded", async () => {
    const rel = release();
    const r = await sign([TAG], { rel, input: "y\nnot the passphrase\n" });
    expect(r.status).not.toBe(0);
    expect(r.stderr).toMatch(/ssh-keygen didn't sign v9\.9\.9 with .* \(a wrong passphrase\?\), so nothing is uploaded\.\n$/);
    expect(uploads(r)).toEqual([]);
    expect(existsSync(join(rel, SIG))).toBe(false);
  });

  it("a key that isn't in the signers file (by default release-signers, Hersir's own): nothing is uploaded", async () => {
    const other = release();
    const r = await sign([TAG], { rel: other, env: { HERSIR_RELEASE_KEY: otherKey } });
    expect(r.status).not.toBe(0);
    expect(r.stderr).toMatch(/The new signature doesn't check against .*\/keys\/signers, so it isn't uploaded/);
    expect(uploads(r)).toEqual([]);
    expect(existsSync(join(other, SIG))).toBe(false);

    const rel = release();
    const own = await sign([TAG], { rel, env: { HERSIR_RELEASE_SIGNERS: undefined } });
    expect(own.status).not.toBe(0);
    expect(own.stderr).toContain(`The new signature doesn't check against ${join(ROOT, "release-signers")}, so it isn't uploaded`);
    expect(own.stdout).not.toContain("HERSIR_RELEASE_SIGNERS");
    expect(uploads(own)).toEqual([]);
    expect(existsSync(join(rel, SIG))).toBe(false);
  });

  it("an upload the release doesn't list afterwards: it says so, and One-line install doesn't start", async () => {
    const rel = release();
    const r = await sign([TAG], { rel, env: { FAKE_UPLOAD_LOST: "1" } });
    expect(r.status).not.toBe(0);
    expect(r.stderr).toMatch(/Uploaded hersir-9\.9\.9\.sig, but the release v9\.9\.9 doesn't list it/);
    expect(r.gh.filter((l) => l.startsWith("gh [workflow]"))).toEqual([]);
  });

  it.runIf(existsSync("/usr/bin/ssh-keygen"))(
    "an ssh-keygen earlier on PATH never runs: /usr/bin's signs, and never sees the key or its passphrase",
    async () => {
      const bin = join(scratch, "evil-bin");
      mkdirSync(bin, { recursive: true });
      writeFileSync(join(bin, "ssh-keygen"), `#!/bin/sh\n{ printf 'evil'; for a in "$@"; do printf ' [%s]' "$a"; done; echo; } >> "$RAN"\ncat >> "$RAN"\nexit 1\n`);
      chmodSync(join(bin, "ssh-keygen"), 0o755);
      const rel = release();
      const r = await sign([TAG], { rel, bashEnv: ghOnly, path: `${bin}:${process.env.PATH}` });
      expect(r.status, r.stdout + r.stderr).toBe(0);
      expect(r.evil).toEqual([]);
      expect(verify(join(rel, SIG), message(COMMIT, readFileSync(join(rel, "SHA256SUMS")))).status).toBe(0);
    },
  );

  it("never uses ssh-agent or an askpass program: SSH_AUTH_SOCK and SSH_ASKPASS go, SSH_ASKPASS_REQUIRE is never", () => {
    const script = read("scripts/sign-release.sh");
    expect(script).toMatch(/^unset SSH_AUTH_SOCK SSH_ASKPASS .*$/m);
    expect(script).toContain("export SSH_ASKPASS_REQUIRE=never");
    expect(script).toContain("ssh-keygen -Y sign -f \"$key\" -n \"$NAMESPACE\" \"$message\"");
    expect(script).not.toMatch(/ssh-add|-Y sign[^\n]* -P |HERSIR_RELEASE_PASSPHRASE/);
  });
});

describe("release-signers", () => {
  it("holds exactly the card's two lines: Jeffrey's release key and the spare", () => {
    expect(read("release-signers")).toBe(`${RELEASE_SIGNERS.join("\n")}\n`);
  });

  it.runIf(unix)("each line is an ed25519 public key that ssh-keygen reads, and the two differ", () => {
    const dir = realpathSync(mkdtempSync(join(tmpdir(), "release-signers-")));
    try {
      const prints = RELEASE_SIGNERS.map((line, i) => {
        const file = join(dir, `key${i}.pub`);
        writeFileSync(file, `${line.split(" ").slice(2).join(" ")}\n`);
        const r = spawnSync("ssh-keygen", ["-l", "-f", file], { encoding: "utf8" });
        expect(r.status, r.stderr).toBe(0);
        expect(r.stdout).toMatch(/^256 SHA256:\S+ .*\(ED25519\)\n$/);
        return r.stdout.split(" ")[1];
      });
      expect(prints[0]).not.toBe(prints[1]);
    } finally {
      rmSync(dir, { recursive: true, force: true });
    }
  });
});

describe("The release signature: the docs", () => {
  const releasing = read("docs/RELEASING.md");

  it("RELEASING.md has the Sign it step after the builds, with the script, and step 4 lists the five assets", () => {
    const step4 = releasing.indexOf("4. **Check it:**");
    const step5 = releasing.indexOf("5. **Sign it.**");
    expect(step4).toBeGreaterThan(0);
    expect(step5).toBeGreaterThan(step4);
    const check = releasing.slice(step4, step5);
    for (const file of [...ARCHIVES.map((a) => a.replaceAll("9.9.9", "X.Y.Z")), "SHA256SUMS", "hersir-X.Y.Z.sig"]) {
      expect(check).toContain(`\`${file}\``);
    }
    expect(check).toContain("it has five");
    expect(check).toContain("The DevOps Agent's part ends when the builds are uploaded: it asks Jeffrey to sign the release");
    const sign = releasing.slice(step5, releasing.indexOf("\n## ", step5));
    expect(sign).toContain("scripts/sign-release.sh vX.Y.Z");
    expect(sign).toContain("Until then no Hersir offers or installs the release.");
    expect(sign).toContain("the line `hersir vX.Y.Z <commit>` and a newline");
    expect(sign).toContain("[`release-signers`](../release-signers)");
  });

  it("RELEASING.md's Never: the keys stay on Jeffrey's computer, never in ssh-agent, and a lost key means the spare", () => {
    const rule = releasing.split("\n").find((l) => l.startsWith("- **The release keys stay on Jeffrey's computer.**"));
    expect(rule).toContain("on Jeffrey's computer and in their password manager");
    expect(rule).toContain("never on GitHub, in CI, in Hersir's data or with an agent");
    expect(rule).toContain("never loaded into ssh-agent");
    expect(rule).toContain("the next release is signed with the spare, and its Hersir carries a new `release-signers`");
  });

  it("no doc says a release is unsigned any more, and each still says no Apple or Microsoft certificate", () => {
    for (const doc of ["docs/RELEASING.md", "docs/PLATFORMS.md", "docs/install.md"]) {
      const text = read(doc);
      expect(text, doc).not.toMatch(/## Not signed|Hersir isn't signed yet|Signing:\*\* none|signed by nothing|#not-signed/);
      expect(text, doc).toMatch(/Hersir's own release key/);
      expect(text, doc).toMatch(/no Apple Developer account and no Windows certificate|no Apple Developer account or Windows certificate/);
    }
  });

  it("SECURITY.md counts the release signature check", () => {
    const security = read("SECURITY.md");
    const counts = security.slice(security.indexOf("## What counts"), security.indexOf("## What doesn't count"));
    expect(counts).toMatch(/^- The release signature check: .*`hersir-X\.Y\.Z\.sig`.*\[`release-signers`\]\(release-signers\)/m);
  });
});
