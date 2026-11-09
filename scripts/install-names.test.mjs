// Tests for the names install.sh gives the app (GA-233): it is called Hersir, and an install over an installed 0.14.0,
// when it was still Gizai, turns that into Hersir with its data, as Gizai's own update does (install.sh --skip-build in
// the release's source). Each test runs install.sh from a scratch source folder (install.sh, tauri.conf.json, the icons
// and stand-ins for the programs a build makes) with HOME in a scratch folder and nothing else from this environment.
// macOS's install runs here too, with stand-ins for uname (Darwin), ditto and codesign; lsregister isn't there, which
// install.sh allows. Linux only: on a Mac the real lsregister would register the scratch app. The 0.14.0 install is
// made by git's v0.14.0 tag, when the checkout has it (CI's shallow checkout hasn't); install.yml does the update over
// a Gizai install on macOS and Windows in CI.
// GA-249: the data folder install.sh backs up, names and purges is the one Hersir uses: the hersir folder when it exists,
// else a gizai folder of an install from before 0.17.0 that holds gizai.db, else the hersir folder. On macOS
// HERSIR_DATA_DIR wins over GIZAI_DATA_DIR; on Linux the script reads neither, and the backup gets its folder under both
// names. The stand-in build backs up the folder the real one would: HERSIR_DATA_DIR, else GIZAI_DATA_DIR.
import { spawnSync } from "node:child_process";
import { chmodSync, existsSync, lstatSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, readlinkSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { afterAll, describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const linux = process.platform === "linux";
const scratch = realpathSync(mkdtempSync(join(tmpdir(), "install-names-")));
afterAll(() => rmSync(scratch, { recursive: true, force: true }));

const git = (...args) => spawnSync("git", args, { cwd: ROOT, maxBuffer: 64 * 1000 * 1000 });
const OLD = "v0.14.0";
const hasOld = git("rev-parse", "-q", "--verify", `${OLD}^{commit}`).status === 0;

// A file of this checkout (ref null) or of a tag.
function file(ref, path) {
  if (!ref) return readFileSync(join(ROOT, path));
  const r = git("show", `${ref}:${path}`);
  if (r.status !== 0) throw new Error(`git show ${ref}:${path}: ${r.stderr}`);
  return r.stdout;
}
function write(path, text, mode = 0o644) {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
  chmodSync(path, mode);
}
const isLink = (path) => { try { return lstatSync(path).isSymbolicLink(); } catch { return false; } };
const there = (path) => existsSync(path) || isLink(path);

let n = 0;
const next = (what) => join(scratch, `${what}-${++n}`);

// A source folder as the update gets it, from this checkout or from a tag, built: gizai says it is `version` and backs
// up the data as the real one does, from HERSIR_DATA_DIR, else GIZAI_DATA_DIR; with BACKUP_ENV set, it notes both there.
// On macOS the app bundle too, named after productName as Tauri names it.
function source(ref, version, os) {
  const dir = next("src");
  const icons = ref ? git("ls-tree", "-r", "--name-only", ref, "src-tauri/icons").stdout.toString() : git("ls-files", "src-tauri/icons").stdout.toString();
  for (const path of ["install.sh", "src-tauri/tauri.conf.json", ...icons.split("\n").filter(Boolean)]) write(join(dir, path), file(ref, path));
  const gizai = [
    "#!/bin/sh",
    'case "$1" in',
    `  --version) echo "gizai ${version}" ;;`,
    '  --backup) d="${HERSIR_DATA_DIR:-$GIZAI_DATA_DIR}"',
    '    if [ -n "${BACKUP_ENV:-}" ]; then printf "HERSIR_DATA_DIR=%s\\nGIZAI_DATA_DIR=%s\\n" "${HERSIR_DATA_DIR-}" "${GIZAI_DATA_DIR-}" > "$BACKUP_ENV"; fi',
    `    b="$d/backups/gizai-before-install-${version}.db"; mkdir -p "\${b%/*}" && cp "$d/gizai.db" "$b" && echo "$b" ;;`,
    "esac",
    "",
  ].join("\n");
  write(join(dir, "target/release/gizai"), gizai, 0o755);
  write(join(dir, "target/release/gizai-mcp"), "#!/bin/sh\nexit 0\n", 0o755);
  if (os === "Darwin") {
    const name = JSON.parse(file(ref, "src-tauri/tauri.conf.json")).productName;
    const app = join(dir, `target/release/bundle/macos/${name}.app/Contents`);
    write(join(app, "MacOS/gizai"), gizai, 0o755);
    write(join(app, "Info.plist"), `<plist><dict><key>CFBundleName</key><string>${name}</string></dict></plist>\n`);
  }
  return dir;
}

// Stand-ins for the system: uname says `os` (and arm64); on macOS ditto copies, codesign and xcode-select say yes.
function tools(os) {
  const dir = join(scratch, `tools-${os}`);
  if (existsSync(dir)) return dir;
  write(join(dir, "uname"), `#!/bin/sh\ncase "$1" in -m) echo arm64 ;; *) echo ${os} ;; esac\n`, 0o755);
  if (os === "Darwin") {
    write(join(dir, "ditto"), '#!/bin/sh\nexec cp -R "$1" "$2"\n', 0o755);
    write(join(dir, "codesign"), "#!/bin/sh\nexit 0\n", 0o755);
    write(join(dir, "xcode-select"), "#!/bin/sh\necho /Library/Developer/CommandLineTools\n", 0o755);
  }
  return dir;
}

// install.sh from `src`, for the user whose home is `home`. GIZAI_REPO is a folder that isn't there, so an install
// that didn't take `src` as its source fails at once instead of cloning from GitHub.
function install(src, home, os, ...args) {
  return installWith({}, src, home, os, ...args);
}
// The same, with `more` in its environment.
function installWith(more, src, home, os, ...args) {
  const env = { HOME: home, USER: "gizai", PATH: `${tools(os)}:${process.env.PATH}`, GIZAI_REPO: join(scratch, "no-such-repo"), ...more };
  const r = spawnSync("bash", [join(src, "install.sh"), ...args], { cwd: home, env, encoding: "utf8" });
  return { code: r.status, out: `${r.stdout}${r.stderr}` };
}
function newHome() {
  const home = next("home");
  mkdirSync(home, { recursive: true });
  return home;
}
// What the installed gizai command says to --version.
const gizaiVersion = (home) => spawnSync(join(home, ".local/bin/gizai"), ["--version"], { env: { HOME: home, PATH: process.env.PATH }, encoding: "utf8" }).stdout.trim();

describe.runIf(linux)("install.sh on Linux: the launcher shows Hersir", () => {
  it("installs the desktop entry gizai.desktop with Name=Hersir; its Exec, Icon and StartupWMClass stay gizai's, and the gizai command starts it", () => {
    const home = newHome();
    const r = install(source(null, "0.15.0", "Linux"), home, "Linux", "--skip-build");
    expect(r.code, r.out).toBe(0);
    const lib = join(home, ".local/lib/gizai");
    const apps = join(home, ".local/share/applications");
    expect(readdirSync(apps).filter((f) => f.endsWith(".desktop"))).toEqual(["gizai.desktop"]);
    const entry = readFileSync(join(apps, "gizai.desktop"), "utf8").split("\n");
    expect(entry).toContain("Name=Hersir");
    expect(entry.filter((l) => l.startsWith("Name"))).toEqual(["Name=Hersir"]);
    expect(entry).toContain(`Exec=${lib}/gizai-launch`);
    expect(entry).toContain("Icon=gizai");
    expect(entry).toContain("StartupWMClass=gizai");
    expect(readlinkSync(join(home, ".local/bin/gizai"))).toBe(join(lib, "gizai-launch"));
    expect(gizaiVersion(home)).toBe("gizai 0.15.0");
    expect(r.out).toContain("Start Hersir from your app launcher, or run: gizai");
    // a new install's data folder is hersir's (GA-249)
    expect(r.out).toContain(`Your data: ${join(home, ".local/share/hersir")}`);
  });

  it.runIf(hasOld)("an install over an installed 0.14.0 (Gizai in the launcher) makes it Hersir, with its data backed up and kept; the uninstall keeps the data", () => {
    const home = newHome();
    const old = install(source(OLD, "0.14.0", "Linux"), home, "Linux", "--skip-build");
    expect(old.code, old.out).toBe(0);
    const apps = join(home, ".local/share/applications");
    expect(readFileSync(join(apps, "gizai.desktop"), "utf8")).toMatch(/^Name=Gizai$/m);
    const data = join(home, ".local/share/gizai");
    write(join(data, "gizai.db"), "the 0.14.0 data");
    write(join(data, "settings.json"), '{"theme": "dark"}\n');

    const r = install(source(null, "0.15.0", "Linux"), home, "Linux", "--skip-build");
    expect(r.code, r.out).toBe(0);
    expect(readdirSync(apps).filter((f) => f.endsWith(".desktop"))).toEqual(["gizai.desktop"]);
    expect(readFileSync(join(apps, "gizai.desktop"), "utf8")).toMatch(/^Name=Hersir$/m);
    expect(readFileSync(join(apps, "gizai.desktop"), "utf8")).not.toMatch(/Gizai/);
    expect(gizaiVersion(home)).toBe("gizai 0.15.0");
    expect(readFileSync(join(data, "gizai.db"), "utf8")).toBe("the 0.14.0 data");
    expect(readFileSync(join(data, "settings.json"), "utf8")).toBe('{"theme": "dark"}\n');
    expect(readFileSync(join(data, "backups/gizai-before-install-0.15.0.db"), "utf8")).toBe("the 0.14.0 data");

    const u = install(source(null, "0.15.0", "Linux"), home, "Linux", "--uninstall");
    expect(u.code, u.out).toBe(0);
    for (const path of [".local/lib/gizai", ".local/bin/gizai", ".local/share/applications/gizai.desktop", ".local/share/icons/hicolor/scalable/apps/gizai.svg"])
      expect(there(join(home, path)), path).toBe(false);
    expect(readFileSync(join(data, "gizai.db"), "utf8")).toBe("the 0.14.0 data");
  });
});

describe.runIf(linux)("install.sh on macOS (with stand-ins): the app is Hersir.app", () => {
  const appSupport = (home) => join(home, "Library/Application Support/Gizai");

  it("installs ~/.local/lib/gizai/Hersir.app, links it as ~/Applications/Hersir.app, and gizai-launch and lib/gizai's links go into it", () => {
    const home = newHome();
    const r = install(source(null, "0.15.0", "Darwin"), home, "Darwin", "--skip-build");
    expect(r.code, r.out).toBe(0);
    const lib = join(home, ".local/lib/gizai");
    expect(existsSync(join(lib, "Hersir.app/Contents/MacOS/gizai"))).toBe(true);
    expect(existsSync(join(lib, "Hersir.app/Contents/MacOS/gizai-mcp"))).toBe(true);
    expect(readlinkSync(join(lib, "gizai"))).toBe("Hersir.app/Contents/MacOS/gizai");
    expect(readlinkSync(join(lib, "gizai-mcp"))).toBe("Hersir.app/Contents/MacOS/gizai-mcp");
    expect(readFileSync(join(lib, "gizai-launch"), "utf8")).toContain(`exec "${lib}/Hersir.app/Contents/MacOS/gizai" "$@"`);
    expect(readlinkSync(join(home, "Applications/Hersir.app"))).toBe(join(lib, "Hersir.app"));
    expect(there(join(lib, "Gizai.app"))).toBe(false);
    expect(there(join(home, "Applications/Gizai.app"))).toBe(false);
    expect(readdirSync(lib).filter((f) => f.startsWith("."))).toEqual([]);
    expect(gizaiVersion(home)).toBe("gizai 0.15.0");
    expect(r.out).toContain("Start Hersir from Applications in your home folder (Finder: Go > Home), and keep it in the Dock. Or run: gizai");
    // a new install's data folder is Hersir's (GA-249)
    expect(r.out).toContain(`Your data: ${join(home, "Library/Application Support/Hersir")}`);
  });

  it.runIf(hasOld)("an install over an installed 0.14.0 leaves no Gizai.app and no ~/Applications/Gizai.app link, with the data backed up and kept", () => {
    const home = newHome();
    const old = install(source(OLD, "0.14.0", "Darwin"), home, "Darwin", "--skip-build");
    expect(old.code, old.out).toBe(0);
    const lib = join(home, ".local/lib/gizai");
    expect(readlinkSync(join(home, "Applications/Gizai.app"))).toBe(join(lib, "Gizai.app"));
    expect(readlinkSync(join(lib, "gizai"))).toBe("Gizai.app/Contents/MacOS/gizai");
    write(join(appSupport(home), "gizai.db"), "the 0.14.0 data");

    const r = install(source(null, "0.15.0", "Darwin"), home, "Darwin", "--skip-build");
    expect(r.code, r.out).toBe(0);
    expect(there(join(lib, "Gizai.app"))).toBe(false);
    expect(there(join(home, "Applications/Gizai.app"))).toBe(false);
    expect(readdirSync(join(home, "Applications"))).toEqual(["Hersir.app"]);
    expect(readlinkSync(join(home, "Applications/Hersir.app"))).toBe(join(lib, "Hersir.app"));
    expect(readdirSync(lib).sort()).toEqual(["Hersir.app", "gizai", "gizai-launch", "gizai-mcp"]);
    expect(readFileSync(join(lib, "gizai-launch"), "utf8")).not.toContain("Gizai.app");
    expect(gizaiVersion(home)).toBe("gizai 0.15.0");
    expect(r.out).not.toContain("could not remove");
    expect(readFileSync(join(appSupport(home), "gizai.db"), "utf8")).toBe("the 0.14.0 data");
    expect(readFileSync(join(appSupport(home), "backups/gizai-before-install-0.15.0.db"), "utf8")).toBe("the 0.14.0 data");
  });

  it("leaves a ~/Applications/Gizai.app that isn't this install's link: a folder of its own, or a link to another install", () => {
    for (const folder of [false, true]) {
      const home = newHome();
      const elsewhere = join(home, "elsewhere/Gizai.app");
      if (folder) write(join(home, "Applications/Gizai.app/Contents/Info.plist"), "an app of its own\n");
      else {
        mkdirSync(join(home, "Applications"), { recursive: true });
        spawnSync("ln", ["-s", elsewhere, join(home, "Applications/Gizai.app")]);
      }
      const r = install(source(null, "0.15.0", "Darwin"), home, "Darwin", "--skip-build");
      expect(r.code, r.out).toBe(0);
      if (folder) expect(readFileSync(join(home, "Applications/Gizai.app/Contents/Info.plist"), "utf8")).toBe("an app of its own\n");
      else expect(readlinkSync(join(home, "Applications/Gizai.app"))).toBe(elsewhere);
      expect(readlinkSync(join(home, "Applications/Hersir.app"))).toBe(join(home, ".local/lib/gizai/Hersir.app"));
    }
  });

  it("the uninstall removes Hersir.app and Gizai.app with their links in ~/Applications, and keeps the data unless purged", () => {
    const home = newHome();
    const src = source(null, "0.15.0", "Darwin");
    expect(install(src, home, "Darwin", "--skip-build").code).toBe(0);
    const lib = join(home, ".local/lib/gizai");
    // what an update over a Gizai.app that couldn't be removed would leave
    write(join(lib, "Gizai.app/Contents/MacOS/gizai"), "#!/bin/sh\n", 0o755);
    spawnSync("ln", ["-s", join(lib, "Gizai.app"), join(home, "Applications/Gizai.app")]);
    write(join(appSupport(home), "gizai.db"), "data");

    const u = install(src, home, "Darwin", "--uninstall");
    expect(u.code, u.out).toBe(0);
    for (const path of [".local/lib/gizai", ".local/bin/gizai", "Applications/Hersir.app", "Applications/Gizai.app"])
      expect(there(join(home, path)), path).toBe(false);
    expect(readFileSync(join(appSupport(home), "gizai.db"), "utf8")).toBe("data");

    const p = install(src, home, "Darwin", "--uninstall", "--purge");
    expect(p.code, p.out).toBe(0);
    expect(existsSync(appSupport(home))).toBe(false);
  });
});

// GA-249: the backup an install of 0.17.0 makes, in the data folder `data`.
const backupIn = (data) => join(data, "backups/gizai-before-install-0.17.0.db");
// Each folder's own files, sorted.
const files = (dir) => readdirSync(dir).sort();

describe.runIf(linux)("install.sh on Linux: the data folder is the one Hersir uses (GA-249)", () => {
  const share = (home) => join(home, ".local/share");

  it("a new install's data folder is ~/.local/share/hersir: nothing to back up, and the install, the uninstall and the purge name it", () => {
    const home = newHome();
    const [hersir, gizai] = [join(share(home), "hersir"), join(share(home), "gizai")];
    const src = source(null, "0.17.0", "Linux");
    const r = install(src, home, "Linux", "--skip-build");
    expect(r.code, r.out).toBe(0);
    expect(r.out).not.toContain("Backing up your data");
    expect(r.out).toContain(`Your data: ${hersir}\n`);
    expect(there(gizai)).toBe(false);
    const u = install(src, home, "Linux", "--uninstall");
    expect(u.code, u.out).toBe(0);
    expect(u.out).toContain(`Your data stays in ${hersir} (run with --uninstall --purge to delete it).`);
    // the data Hersir made there since
    write(join(hersir, "gizai.db"), "the 0.17.0 data");
    const p = install(src, home, "Linux", "--uninstall", "--purge");
    expect(p.code, p.out).toBe(0);
    expect(p.out).toContain(`Deleted your data (${hersir})`);
    expect(there(hersir)).toBe(false);
  });

  it("a gizai folder that holds gizai.db, with no hersir folder, is used where it is: backed up, named, kept and purged, and nothing moves", () => {
    const home = newHome();
    const [hersir, gizai] = [join(share(home), "hersir"), join(share(home), "gizai")];
    write(join(gizai, "gizai.db"), "the 0.16.0 data");
    write(join(gizai, "settings.json"), '{"theme": "dark"}\n');
    const src = source(null, "0.17.0", "Linux");
    const r = install(src, home, "Linux", "--skip-build");
    expect(r.code, r.out).toBe(0);
    expect(r.out).toContain(`Backed up your data to ${backupIn(gizai)}`);
    expect(readFileSync(backupIn(gizai), "utf8")).toBe("the 0.16.0 data");
    expect(r.out).toContain(`Your data: ${gizai}\n`);
    expect(files(gizai)).toEqual(["backups", "gizai.db", "settings.json"]);
    expect(readFileSync(join(gizai, "gizai.db"), "utf8")).toBe("the 0.16.0 data");
    expect(there(hersir)).toBe(false);

    const u = install(src, home, "Linux", "--uninstall");
    expect(u.code, u.out).toBe(0);
    expect(u.out).toContain(`Your data stays in ${gizai} (run with --uninstall --purge to delete it).`);
    expect(readFileSync(join(gizai, "gizai.db"), "utf8")).toBe("the 0.16.0 data");
    const p = install(src, home, "Linux", "--uninstall", "--purge");
    expect(p.code, p.out).toBe(0);
    expect(p.out).toContain(`Deleted your data (${gizai})`);
    expect(there(gizai)).toBe(false);
    expect(there(hersir)).toBe(false);
  });

  it("the hersir folder wins when it exists, also beside a gizai folder with gizai.db: it is backed up, named and purged, and the gizai one is left as it is", () => {
    for (const withDb of [true, false]) {
      const home = newHome();
      const [hersir, gizai] = [join(share(home), "hersir"), join(share(home), "gizai")];
      write(join(gizai, "gizai.db"), "the 0.16.0 data");
      if (withDb) write(join(hersir, "gizai.db"), "the 0.17.0 data");
      else mkdirSync(hersir, { recursive: true });
      const src = source(null, "0.17.0", "Linux");
      const r = install(src, home, "Linux", "--skip-build");
      expect(r.code, r.out).toBe(0);
      if (withDb) expect(readFileSync(backupIn(hersir), "utf8")).toBe("the 0.17.0 data");
      else expect(r.out, "an empty hersir folder has nothing to back up").not.toContain("Backing up your data");
      expect(r.out).toContain(`Your data: ${hersir}\n`);
      expect(files(gizai), "the gizai folder isn't backed up").toEqual(["gizai.db"]);

      const p = install(src, home, "Linux", "--uninstall", "--purge");
      expect(p.code, p.out).toBe(0);
      expect(p.out).toContain(`Deleted your data (${hersir})`);
      expect(there(hersir)).toBe(false);
      expect(files(gizai)).toEqual(["gizai.db"]);
      expect(readFileSync(join(gizai, "gizai.db"), "utf8")).toBe("the 0.16.0 data");
    }
  });

  it("a gizai folder without gizai.db isn't Hersir's data: the data folder is the hersir one, and a purge leaves the gizai one", () => {
    const home = newHome();
    const [hersir, gizai] = [join(share(home), "hersir"), join(share(home), "gizai")];
    write(join(gizai, "settings.json"), "{}\n");
    const src = source(null, "0.17.0", "Linux");
    const r = install(src, home, "Linux", "--skip-build");
    expect(r.code, r.out).toBe(0);
    expect(r.out).not.toContain("Backing up your data");
    expect(r.out).toContain(`Your data: ${hersir}\n`);
    const p = install(src, home, "Linux", "--uninstall", "--purge");
    expect(p.code, p.out).toBe(0);
    expect(p.out).toContain(`Deleted your data (${hersir})`);
    expect(files(gizai)).toEqual(["settings.json"]);
  });

  it("HERSIR_DATA_DIR or GIZAI_DATA_DIR in the environment can't send the backup elsewhere: the new build gets the folder by the rule under both names", () => {
    const home = newHome();
    const gizai = join(share(home), "gizai");
    const [h, g] = [join(home, "elsewhere-hersir"), join(home, "elsewhere-gizai")];
    write(join(gizai, "gizai.db"), "the data");
    write(join(h, "gizai.db"), "other data");
    write(join(g, "gizai.db"), "more other data");
    const seen = join(home, "backup-env.txt");
    const src = source(null, "0.17.0", "Linux");
    const r = installWith({ HERSIR_DATA_DIR: h, GIZAI_DATA_DIR: g, BACKUP_ENV: seen }, src, home, "Linux", "--skip-build");
    expect(r.code, r.out).toBe(0);
    expect(readFileSync(seen, "utf8")).toBe(`HERSIR_DATA_DIR=${gizai}\nGIZAI_DATA_DIR=${gizai}\n`);
    expect(readFileSync(backupIn(gizai), "utf8")).toBe("the data");
    expect(r.out).toContain(`Your data: ${gizai}\n`);
    expect(files(h)).toEqual(["gizai.db"]);
    expect(files(g)).toEqual(["gizai.db"]);

    // install.sh reads neither on Linux: the purge deletes the folder by the rule, not theirs
    const p = installWith({ HERSIR_DATA_DIR: h, GIZAI_DATA_DIR: g }, src, home, "Linux", "--uninstall", "--purge");
    expect(p.code, p.out).toBe(0);
    expect(there(gizai)).toBe(false);
    expect(readFileSync(join(h, "gizai.db"), "utf8")).toBe("other data");
    expect(readFileSync(join(g, "gizai.db"), "utf8")).toBe("more other data");
  });

  it("the update's install (GIZAI_PREFIX, XDG_DATA_HOME under the prefix) backs up the data folder there by the same rule", () => {
    for (const hersirThere of [false, true]) {
      const home = newHome();
      const prefix = join(home, "prefix");
      const [hersir, gizai] = [join(prefix, "share/hersir"), join(prefix, "share/gizai")];
      write(join(gizai, "gizai.db"), "the 0.16.0 data");
      if (hersirThere) write(join(hersir, "gizai.db"), "the 0.17.0 data");
      const r = installWith({ GIZAI_PREFIX: prefix, XDG_DATA_HOME: join(prefix, "share") }, source(null, "0.17.0", "Linux"), home, "Linux", "--skip-build");
      expect(r.code, r.out).toBe(0);
      const used = hersirThere ? hersir : gizai;
      expect(readFileSync(backupIn(used), "utf8")).toBe(hersirThere ? "the 0.17.0 data" : "the 0.16.0 data");
      expect(r.out).toContain(`Your data: ${used}\n`);
      if (hersirThere) expect(files(gizai)).toEqual(["gizai.db"]);
      else expect(there(hersir)).toBe(false);
      expect(there(join(share(home), "hersir")) || there(join(share(home), "gizai"))).toBe(false);
    }
  });
});

describe.runIf(linux)("install.sh on macOS (with stand-ins): the data folder is the one Hersir uses (GA-249)", () => {
  const support = (home, name) => join(home, "Library/Application Support", name);

  it("Gizai with gizai.db and no Hersir folder is used where it is; a Hersir folder wins once it exists; the purge deletes the one used", () => {
    const home = newHome();
    const [hersir, gizai] = [support(home, "Hersir"), support(home, "Gizai")];
    write(join(gizai, "gizai.db"), "the 0.16.0 data");
    const src = source(null, "0.17.0", "Darwin");
    const r = install(src, home, "Darwin", "--skip-build");
    expect(r.code, r.out).toBe(0);
    expect(readFileSync(backupIn(gizai), "utf8")).toBe("the 0.16.0 data");
    expect(r.out).toContain(`Your data: ${gizai}\n`);
    expect(there(hersir)).toBe(false);

    write(join(hersir, "gizai.db"), "the 0.17.0 data");
    const again = install(src, home, "Darwin", "--skip-build");
    expect(again.code, again.out).toBe(0);
    expect(readFileSync(backupIn(hersir), "utf8")).toBe("the 0.17.0 data");
    expect(again.out).toContain(`Your data: ${hersir}\n`);

    const p = install(src, home, "Darwin", "--uninstall", "--purge");
    expect(p.code, p.out).toBe(0);
    expect(p.out).toContain(`Deleted your data (${hersir})`);
    expect(there(hersir)).toBe(false);
    expect(readFileSync(join(gizai, "gizai.db"), "utf8")).toBe("the 0.16.0 data");
  });

  it("HERSIR_DATA_DIR wins over GIZAI_DATA_DIR, which still works alone, and an empty one gives way: that folder is backed up, named, kept and purged", () => {
    const home = newHome();
    const [h, g, usual] = [join(home, "hersir-data"), join(home, "gizai-data"), support(home, "Hersir")];
    const held = { [h]: "data in HERSIR_DATA_DIR", [g]: "data in GIZAI_DATA_DIR", [usual]: "the usual data" };
    for (const [dir, text] of Object.entries(held)) write(join(dir, "gizai.db"), text);
    const seen = join(home, "backup-env.txt");
    const src = source(null, "0.17.0", "Darwin");
    const cases = [
      [{ HERSIR_DATA_DIR: h, GIZAI_DATA_DIR: g }, h],
      [{ GIZAI_DATA_DIR: g }, g],
      [{ HERSIR_DATA_DIR: "", GIZAI_DATA_DIR: g }, g],
      [{ HERSIR_DATA_DIR: h }, h],
      [{}, usual],
    ];
    for (const [more, used] of cases) {
      for (const dir of Object.keys(held)) rmSync(join(dir, "backups"), { recursive: true, force: true });
      const r = installWith({ ...more, BACKUP_ENV: seen }, src, home, "Darwin", "--skip-build");
      expect(r.code, r.out).toBe(0);
      expect(r.out, JSON.stringify(more)).toContain(`Your data: ${used}\n`);
      expect(readFileSync(seen, "utf8"), JSON.stringify(more)).toBe(`HERSIR_DATA_DIR=${used}\nGIZAI_DATA_DIR=${used}\n`);
      expect(readFileSync(backupIn(used), "utf8")).toBe(held[used]);
      for (const other of Object.keys(held).filter((d) => d !== used)) expect(files(other), `${JSON.stringify(more)}: ${other}`).toEqual(["gizai.db"]);
      const u = installWith(more, src, home, "Darwin", "--uninstall");
      expect(u.code, u.out).toBe(0);
      expect(u.out).toContain(`Your data stays in ${used} (run with --uninstall --purge to delete it).`);
    }

    const p = installWith({ HERSIR_DATA_DIR: h, GIZAI_DATA_DIR: g }, src, home, "Darwin", "--uninstall", "--purge");
    expect(p.code, p.out).toBe(0);
    expect(p.out).toContain(`Deleted your data (${h})`);
    expect(there(h)).toBe(false);
    expect(readFileSync(join(g, "gizai.db"), "utf8")).toBe(held[g]);
    expect(readFileSync(join(usual, "gizai.db"), "utf8")).toBe(held[usual]);
  });
});

describe("the installers' help is their whole header, which names the data folder (GA-249)", () => {
  // the header: the comment lines at the top, after install.sh's #! line
  const header = (path) => {
    const lines = readFileSync(join(ROOT, path), "utf8").split(/\r?\n/);
    const from = lines[0].startsWith("#!") ? 1 : 0;
    const to = lines.findIndex((l, i) => i >= from && !l.startsWith("#"));
    return { lines, from, to };
  };

  it.runIf(linux)("install.sh --help prints its header, from the first line to the last, and nothing after it", () => {
    const { lines, from, to } = header("install.sh");
    const r = install(source(null, "0.17.0", "Linux"), newHome(), "Linux", "--help");
    expect(r.code, r.out).toBe(0);
    expect(r.out).toBe(lines.slice(from, to).map((l) => l.replace(/^# ?/, "")).join("\n") + "\n");
    expect(r.out).toContain("only --uninstall --purge\ndeletes it.");
    expect(r.out).toContain("HERSIR_DATA_DIR, else GIZAI_DATA_DIR (macOS: another data folder)");
  });

  it("install.ps1 -Help prints its header, all of it and nothing after it", () => {
    const { lines, to } = header("install.ps1");
    const count = Number(readFileSync(join(ROOT, "install.ps1"), "utf8").match(/Get-Content -LiteralPath \$PSCommandPath -TotalCount (\d+)/)[1]);
    expect(count).toBe(to);
    const text = lines.slice(0, count).join("\n");
    expect(text).toContain("# Your data lives in %APPDATA%\\Hersir, or in %APPDATA%\\Gizai of an install from before 0.17.0 when it holds gizai.db");
    expect(text).toContain("only -Uninstall -Purge\n# deletes it.");
    expect(text).toContain("HERSIR_DATA_DIR, else GIZAI_DATA_DIR (another\n# data folder).");
  });
});
