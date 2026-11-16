// Tests for the variables install.sh reads when it gets the source itself, run outside a checkout (GA-248): HERSIR_REPO
// and HERSIR_BRANCH win over their names from before 0.17.0, GIZAI_REPO and GIZAI_BRANCH, which still work alone. Each
// test runs this checkout's install.sh, copied alone into a scratch folder, with --skip-build: it clones the source into
// $XDG_DATA_HOME/hersir-src, then stops as nothing is built there. The repositories are small local ones, with a file
// that names each one and its branch. Linux only, as install-names.test.mjs.
import { spawnSync } from "node:child_process";
import { copyFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, beforeAll, describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const linux = process.platform === "linux";
const scratch = realpathSync(mkdtempSync(join(tmpdir(), "install-vars-")));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));
const CEILING = { GIT_CEILING_DIRECTORIES: scratch };

function git(dir, ...args) {
  const run = spawnSync("git", ["-c", "user.name=Hersir test", "-c", "user.email=test@hersir.invalid", "-c", "commit.gpgsign=false", ...args],
    { cwd: dir, encoding: "utf8", env: { ...process.env, ...CEILING } });
  if (run.status !== 0) throw new Error(`git ${args.join(" ")} failed: ${run.stderr}`);
  return run.stdout.trim();
}

// A repository `name` with the branches production, hersir-branch and gizai-branch, each with a file which.txt that says
// "<name> <branch>".
function repo(name) {
  const dir = join(scratch, name);
  mkdirSync(dir, { recursive: true });
  git(dir, "init", "-q", "-b", "production");
  for (const branch of ["production", "hersir-branch", "gizai-branch"]) {
    if (branch !== "production") git(dir, "checkout", "-q", "-b", branch, "production");
    writeFileSync(join(dir, "which.txt"), `${name} ${branch}\n`);
    git(dir, "add", "-A");
    git(dir, "commit", "-q", "-m", branch);
  }
  return dir;
}

let hersirRepo, gizaiRepo, installSh;
beforeAll(() => {
  if (!linux) return;
  hersirRepo = repo("hersir-repo");
  gizaiRepo = repo("gizai-repo");
  mkdirSync(join(scratch, "script"));
  installSh = join(scratch, "script/install.sh");
  copyFileSync(join(ROOT, "install.sh"), installSh);
});

let n = 0;
// install.sh --skip-build outside a checkout, with `vars`: what it cloned into hersir-src (which.txt), or null.
function cloned(vars) {
  const dir = join(scratch, `run-${++n}`);
  const home = join(dir, "home");
  const share = join(dir, "share");
  mkdirSync(home, { recursive: true });
  const env = { HOME: home, USER: "hersir", PATH: process.env.PATH, XDG_DATA_HOME: share, ...CEILING, ...vars };
  const r = spawnSync("bash", [installSh, "--skip-build"], { cwd: dir, env, encoding: "utf8" });
  const out = `${r.stdout}${r.stderr}`;
  expect(out, "it gets the source, then stops as nothing is built there").toContain("== Getting the source");
  const which = join(share, "hersir-src/which.txt");
  return existsSync(which) ? readFileSync(which, "utf8").trim() : null;
}

describe.runIf(linux)("install.sh outside a checkout: HERSIR_REPO and HERSIR_BRANCH", () => {
  it("clones GIZAI_REPO's GIZAI_BRANCH when only the names from before 0.17.0 are set", () => {
    expect(cloned({ GIZAI_REPO: gizaiRepo, GIZAI_BRANCH: "gizai-branch" })).toBe("gizai-repo gizai-branch");
  });

  it("clones HERSIR_REPO's HERSIR_BRANCH when only the HERSIR_ names are set", () => {
    expect(cloned({ HERSIR_REPO: hersirRepo, HERSIR_BRANCH: "hersir-branch" })).toBe("hersir-repo hersir-branch");
  });

  it("takes the HERSIR_ names when both are set", () => {
    const both = { HERSIR_REPO: hersirRepo, HERSIR_BRANCH: "hersir-branch", GIZAI_REPO: gizaiRepo, GIZAI_BRANCH: "gizai-branch" };
    expect(cloned(both)).toBe("hersir-repo hersir-branch");
  });

  it("takes each by itself: HERSIR_REPO with GIZAI_BRANCH, and production when no branch is set", () => {
    expect(cloned({ HERSIR_REPO: hersirRepo, GIZAI_REPO: gizaiRepo, GIZAI_BRANCH: "gizai-branch" })).toBe("hersir-repo gizai-branch");
    expect(cloned({ GIZAI_REPO: gizaiRepo })).toBe("gizai-repo production");
  });
});
