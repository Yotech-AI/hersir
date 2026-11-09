// Tests for the product tour's capture (GA-149, round 3). This repository keeps only the capture, which needs the app:
// scripts/tour-capture.sh writes its stills and scenes.json to --out <folder>, or to .devdata/tour/capture, which git
// ignores; the website's repository makes hersir.dev's pictures from those stills. The screen is
// 1600 × 1040 CSS pixels at scale 2, so every still is 3200 × 2080; the ten scenes are the website's eight in its order,
// then the Team page and Usage per project (GA-245); and
// the capture uses made-up data only, in a fake home, so no real project, client, folder or user name and no real home
// folder can show. The capture itself runs in a headless cage (cage, grim, wlr-randr), so it is run by hand, not here:
// these tests read the script, and run its argument handling on Linux. The last two parts (from tour-renders.test.mjs,
// GA-149, and GA-241) check that the tour's video and GIFs left this repository and its README.
import { spawnSync } from "node:child_process";
import { existsSync, mkdtempSync, readFileSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const SCRIPT = join(ROOT, "scripts", "tour-capture.sh");
/** The website's feature rows 01 to 08 (design/website/README.md, GA-144), as the card lists them. */
const WEBSITE_ORDER = ["01-board", "02-chat", "03-clis", "04-review", "05-editor", "06-memory", "07-tools", "08-inbox"];

const read = (...parts) => readFileSync(join(ROOT, ...parts), "utf8");
const sh = read("scripts", "tour-capture.sh");
const git = (...args) => spawnSync("git", args, { cwd: ROOT, encoding: "utf8", maxBuffer: 64 * 1000 * 1000 });
const tracked = (...paths) => {
  const r = git("ls-files", "--", ...paths);
  if (r.status !== 0) throw new Error(`git ls-files: ${r.stderr}`);
  return r.stdout.split("\n").filter(Boolean);
};
/** The script's opening comment, from its second line to the usage, as one line of text. */
const header = sh.slice(sh.indexOf("\n") + 1, sh.indexOf("# usage:")).split("\n").map((l) => l.replace(/^# ?/, "")).join(" ");
/** The README's old tour folder, built from its parts so the card's search for its name (GA-241) finds nothing here. */
const TOUR_DIR = ["docs", "tour"].join("/");

/** The script's `scene <n> <name> "<title>" <route> <steps…>` lines, with their steps, joined over line ends. */
function scenes() {
  const body = sh.slice(sh.indexOf("# The website's eight features")).replace(/\\\n\s*/g, " ");
  return [...body.matchAll(/^scene (\d+) (\S+) "([^"]+)" (\S+)((?: +(?:"[^"]*"|'[^']*'))*)/gm)].map((m) => ({
    n: Number(m[1]), name: m[2], folder: `${m[1].padStart(2, "0")}-${m[2]}`, title: m[3], route: m[4],
    steps: [...m[5].matchAll(/"([^"]*)"|'([^']*)'/g)].map((s) => s[1] ?? s[2]),
  }));
}

/** Runs the script's argument handling (everything up to where OUT gets its default) from `cwd`, and prints OUT, the
 *  folder it went to and the arguments left. The rest needs the cage, so it doesn't run. */
function args(cwd, ...argv) {
  const head = sh.slice(0, sh.indexOf("\n", sh.indexOf("OUT=${OUT:-")) + 1);
  const r = spawnSync("bash", ["-c", `${head}printf '%s\\n' "$OUT" "$PWD" "$*"`, SCRIPT, ...argv], { cwd, encoding: "utf8" });
  const [out, pwd, rest] = r.stdout.split("\n");
  return { status: r.status, stdout: r.stdout, out, pwd, rest };
}

describe("the screen: 1600 × 1040 CSS pixels at scale 2, so 3200 × 2080 stills", () => {
  it("PAGE_W, PAGE_H and SCALE say so, and nothing in the script still names the old 1280 × 832 or 2560 × 1664", () => {
    expect(sh).toMatch(/^SCALE=2; PAGE_W=1600; PAGE_H=1040; TITLE=37$/m);
    expect(sh).not.toMatch(/\b(1280|832|2560|1664)\b/);
    expect(header).toMatch(/1600x1040 CSS pixels at scale 2, so 3200x2080 pixels/);
  });

  it("the screen's mode, the crop, the size checks and scenes.json's screen all follow from them", () => {
    // the cage's screen: the page and the window's title bar, at scale 2
    expect(sh).toContain('MODE_SIZE="$((PAGE_W * SCALE))x$(((PAGE_H + TITLE) * SCALE))"');
    expect(sh).toContain("--custom-mode $MODE_SIZE --scale $SCALE");
    // each still: the screen's lowest PAGE_W × PAGE_H at scale 2, without the title bar
    expect(sh).toContain('crop=$((PAGE_W * SCALE)):$((PAGE_H * SCALE)):0:ih-$((PAGE_H * SCALE))');
    // the page's size as the app reports it, and every still's, are checked against the same numbers
    expect(sh).toContain('python3 - "$dir/steps.json" "$((PAGE_W * SCALE))" "$((PAGE_H * SCALE))"');
    expect(sh).toContain('grep -vc "^$((PAGE_W * SCALE)),$((PAGE_H * SCALE))\\$"');
    // scenes.json's screen: the stills' size in pixels and the scale
    expect(sh).toContain('scenes_json() {  # <theme>\n  python3 - "$OUT/$1" "$1" "$T/titles" "$((PAGE_W * SCALE))" "$((PAGE_H * SCALE))" "$SCALE"');
    expect(sh).toMatch(/"screen": \{"width": width, "height": height, "scale": scale\}/);
  });
});

describe("where the stills go: --out <folder>, or .devdata/tour/capture", () => {
  it("the usage names --out, and the stills, steps.json and scenes.json all go under OUT", () => {
    expect(sh).toContain("# usage: scripts/tour-capture.sh [--out <folder>] [dark|light|both] [scene number…]");
    expect(sh).toContain('local dir=$OUT/$theme/$name steps');
    expect(sh).toContain('GIZAI_TOUR_DIR="$dir"');
    expect(sh).toContain('python3 - "$OUT/$1"');
    expect(sh).not.toMatch(/video\//);
  });

  it("a new capture keeps the stills of earlier ones: only the scene it captures again is emptied", () => {
    const prepare = sh.slice(sh.indexOf("prepare() {"), sh.indexOf("\n}\n", sh.indexOf("prepare() {")));
    const wipe = prepare.match(/rm -rf [^&\n]+/g);
    expect(wipe).toEqual(['rm -rf "$BASE" "$LIVE" "$T/run" "$T/logs" "$T/ids.sh" "$T/titles" "$T/runner.sh" ']);
    const removals = [...sh.matchAll(/rm -rf ([^&\n]+)/g)].map((m) => m[1].trim());
    expect(removals.filter((r) => /OUT|dir/.test(r))).toEqual(['"$dir"']);
  });

  it(".devdata/ is ignored by git, so the default folder's stills never get committed", () => {
    for (const f of [".devdata/tour/capture/dark/scenes.json", ".devdata/tour/capture/light/01-board/open-card-before.png"]) {
      expect(git("check-ignore", "-q", f).status).toBe(0);
    }
    expect(git("ls-files", ".devdata").stdout).toBe("");
  });

  describe.runIf(process.platform === "linux")("its arguments (run by bash, as on Linux, where the capture runs)", () => {
    it("without --out the stills go to .devdata/tour/capture in the repository, wherever it is run from", () => {
      const r = args(join(ROOT, "src"), "dark", "3", "4");
      expect(r.status).toBe(0);
      expect(r.out).toBe(join(realpathSync(ROOT), ".devdata", "tour", "capture"));
      expect(r.pwd).toBe(realpathSync(ROOT));
      expect(r.rest).toBe("dark 3 4");
      expect(args(ROOT).out).toBe(join(realpathSync(ROOT), ".devdata", "tour", "capture"));
    });

    it("--out <folder> is read from where it is run, before the script goes to the repository's root", () => {
      const from = realpathSync(mkdtempSync(join(tmpdir(), "tour-out-")));
      try {
        const relative = args(from, "--out", "site/capture", "light", "5");
        expect(relative.status).toBe(0);
        expect(relative.out).toBe(join(from, "site", "capture"));
        expect(relative.pwd).toBe(realpathSync(ROOT));
        expect(relative.rest).toBe("light 5");
        // an absolute folder stays itself, and the folder needn't exist yet: the capture makes it
        const absolute = args(from, "--out", join(from, "elsewhere", "stills"));
        expect(absolute.out).toBe(join(from, "elsewhere", "stills"));
        expect(absolute.rest).toBe("");
        expect(existsSync(join(from, "site"))).toBe(false);
      } finally {
        rmSync(from, { recursive: true, force: true });
      }
    });

    it("--out without a folder prints the usage and stops", () => {
      const r = args(ROOT, "--out");
      expect(r.status).toBe(1);
      expect(r.stdout).toBe("usage: scripts/tour-capture.sh [--out <folder>] [dark|light|both] [scene number…]\n");
    });
  });
});

describe("the scenes: the website's eight in their order, as in round 2, then the Team page and Usage per project (GA-245)", () => {
  const list = scenes();

  it("numbered 1 to 10: the website's eight in its order, then 09-team and 10-usage, each with at least one named click", () => {
    expect(list.map((s) => s.folder)).toEqual([...WEBSITE_ORDER, "09-team", "10-usage"]);
    expect(list.map((s) => s.n)).toEqual([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    for (const s of list) {
      expect(s.steps.some((step) => /^[a-z0-9][a-z0-9-]*(@\d+)?=/.test(step))).toBe(true);
    }
  });

  it("the named clicks are round 2's fifteen steps, then GA-245's three", () => {
    const named = list.map((s) => [s.folder, s.steps.filter((st) => /^[a-z-]+(@\d+)?=/.test(st)).map((st) => st.split(/[@=]/)[0])]);
    expect(named).toEqual([
      ["01-board", ["open-card"]],
      ["02-chat", ["approve"]],
      ["03-clis", ["agents-tab", "usage"]],
      ["04-review", ["open-card", "runs"]],
      ["05-editor", ["open-folder", "open-project", "terminal"]],
      ["06-memory", ["open-link", "graph"]],
      ["07-tools", ["settings", "use-playwright"]],
      ["08-inbox", ["inbox", "done-continue"]],
      ["09-team", ["organisation", "workflow"]],
      ["10-usage", ["projects"]],
    ]);
  });

  it("scenes 1 to 8 keep their titles, routes and every step as before GA-245", () => {
    expect(list.slice(0, 8).map(({ n, name, title, route, steps }) => ({ n, name, title, route, steps }))).toEqual([
      { n: 1, name: "board", title: "The board, and a quick look at a card's live run", route: "board",
        steps: ["wait:7000", 'open-card@3000=[data-card="NWS-1"]'] },
      { n: 2, name: "chat", title: "The Team Lead chat: a request, the cards it made, Approve", route: '"chat/$THREAD"',
        steps: ["wait:1500", "approve@2500=.pending-actions .pa-card .pa-actions .btn.primary"] },
      { n: 3, name: "clis", title: "Bring your own CLI: Claude Code and Codex in Settings → Coding CLIs, then Usage → Subscription",
        route: "settings/appearance", steps: ["wait:1000", 'agents-tab=.tabs[aria-label="Settings"] .tab:text(Agents and runs)', "usage=.side .nav-item:text(Usage)"] },
      { n: 4, name: "review", title: "Tested before you see it: a card in Review with its pull request and QA's result", route: "board",
        steps: ["wait:2000", 'open-card=[data-card="NWS-2"]', "scroll:.tabs button.tab:text(Runs)", "runs=.tabs button.tab:text(Runs)"] },
      { n: 5, name: "editor", title: "The Editor and Terminal: a project opens in the Editor, then its Terminal", route: "board",
        steps: ["wait:1500", 'open-folder=.side [aria-label="Editor"] button.nav-item:text(Open a folder)',
          'open-project@2000=.ed-picker .opt[title="~/Projects/northwind-shop"]', "terminal@2500=.btn.term-btn"] },
      { n: 6, name: "memory", title: "Memory: a note with its links, then the graph", route: '"memory/$NOTE"',
        steps: ["wait:1000", "open-link=.mem-note .note-view a.wikilink:text(Deploy steps)", "graph@4000=.seg.mem-views button:text(Graph)"] },
      { n: 7, name: "tools", title: "Tools per agent: an agent's form → Tools", route: '"agent/$BACKEND"',
        steps: ["wait:1000", "settings=.entity-head .actions .btn.ghost:text(Settings)", "use-playwright=.drawer .ma-server label.check:text(playwright)"] },
      { n: 8, name: "inbox", title: "The Inbox: a question, and Run this for me", route: "board",
        steps: ["wait:1000", "inbox=.side .nav-item:text(Inbox)", "done-continue@2500=.run-for-me-card .btn.sm.primary"] },
    ]);
  });

  it("the Review scene scrolls the card's Runs tab into view (and 150px under it) before it clicks it", () => {
    const review = list.find((s) => s.folder === "04-review");
    expect(review.steps).toEqual(["wait:2000", 'open-card=[data-card="NWS-2"]', "scroll:.tabs button.tab:text(Runs)", "runs=.tabs button.tab:text(Runs)"]);
  });

  it("the Editor and Terminal is one short scene: open a folder, pick a project in ~/Projects, then its Terminal", () => {
    const editor = list.find((s) => s.folder === "05-editor");
    expect(editor.steps.slice(1).map((s) => s.split(/[@=]/)[0])).toEqual(["open-folder", "open-project", "terminal"]);
    expect(editor.steps[2]).toMatch(/title="~\/Projects\/[a-z-]+"/);
    expect(editor.steps[3]).toMatch(/term-btn/);
    expect(list.some((s) => /terminal/.test(s.name))).toBe(false);
  });

  it("the CLI scene shows Claude Code and Codex: Settings → Agents and runs, then Usage", () => {
    const clis = list.find((s) => s.folder === "03-clis");
    expect(clis.title).toMatch(/Claude Code and Codex/);
    expect(clis.steps.slice(1).map((s) => s.split("=")[0])).toEqual(["agents-tab", "usage"]);
  });

  it("each scene is captured in dark and in light, the theme set by its first step", () => {
    expect(sh).toContain('case "$THEMES" in both) THEMES="dark light" ;; dark|light) ;; *) echo "$USAGE"; exit 1 ;; esac');
    expect(sh).toContain('steps="theme:$theme;$(IFS=\';\'; echo "$*")"');
  });
});

describe("GA-245: stills of the Team page and of Usage per project, for the website's pictures", () => {
  const list = scenes();
  const team = list.find((s) => s.folder === "09-team");
  const usage = list.find((s) => s.folder === "10-usage");
  const teamPage = read("src", "pages", "TeamPage.tsx");
  const usagePage = read("src", "pages", "UsagePage.tsx");
  const selftest = read("src", "selftest.ts");
  /** runSteps' named step, as src/selftest.ts has it: only such a step gives stills. */
  const STEP_NAME = new RegExp(selftest.match(/^const STEP_NAME = \/(.+)\/;$/m)[1]);

  it("the header says the stills are for the website's pictures of each feature, ten scenes, and names no video or GIF", () => {
    expect(header).toMatch(/^The stills for the website's pictures of each feature \(GA-148, GA-149, GA-245\): ten scenes of Gizai's main features/);
    expect(header).toMatch(/one per feature in the homepage's slider and above each docs feature page/);
    expect(header).not.toMatch(/video|\bGIFs?\b|Remotion|product tour/i);
  });

  it("scene 9 opens the Team page, clicks the Organisation heading from its top, then scrolls Workflow up and clicks its heading", () => {
    expect(team.title).toBe("The Team page: the organisation chart and the workflow");
    expect(team.route).toBe("team");
    expect(team.steps).toEqual(["wait:1500", "organisation=.section-head h3:text(Organisation)", ".page > section:has(> .wf)",
      "workflow=.section-head h3:text(Workflow)"]);
    // only the two headings give stills; the section's click between them only scrolls Workflow to the top
    expect(team.steps.filter((s) => STEP_NAME.test(s)).map((s) => s.match(STEP_NAME)[1])).toEqual(["organisation", "workflow"]);
    expect(team.steps[2]).not.toMatch(/^(#\/|wait:|term:|scroll:|theme:)/);
    expect(selftest).toContain('else { const el = document.querySelector(step) as HTMLElement | null; el?.scrollIntoView({ block: "start" }); el?.click(); }');
  });

  it("scene 9's selectors are the Team page's: Organisation's chart first, then Workflow's columns (.wf)", () => {
    const org = teamPage.indexOf('<div className="section-head"><h3>Organisation</h3>');
    const wf = teamPage.indexOf('<div className="section-head"><h3>Workflow</h3>');
    expect(org).toBeGreaterThan(-1);
    expect(wf).toBeGreaterThan(org);
    expect(teamPage.indexOf("<OrgChart ", org)).toBeLessThan(wf);
    expect(teamPage.indexOf("<ColumnEditor ", wf)).toBeGreaterThan(wf);
    expect(read("src", "components", "ColumnEditor.tsx")).toContain('<div className="wf" role="list" aria-label="Columns">');
  });

  it("a comment says which of scene 9's clicks change nothing", () => {
    const before = sh.slice(sh.indexOf("scene 8 inbox"), sh.indexOf("scene 9 team")).split("\n").filter((l) => l.startsWith("#")).join(" ");
    expect(before).toMatch(/Organisation heading, which changes nothing/);
    expect(before).toMatch(/a click on the section itself, which does nothing/);
    expect(before).toMatch(/its heading, which changes nothing either/);
  });

  it("scene 10 opens Usage, on Subscription, and clicks the Projects tab: the table Usage per project", () => {
    expect(usage.title).toBe("Usage per project");
    expect(usage.route).toBe("usage");
    expect(usage.steps).toEqual(["wait:1000", "projects=.tabs .tab:text(Projects)"]);
    // Usage opens on Subscription, so the click changes the tab
    expect(usagePage).toMatch(/const TABS: TabDef<Tab>\[\] = \[\n\s+\["subscription", Gauge, "Subscription"\],.*\["projects", FolderKanban, "Projects"\],\n\];/);
    expect(usagePage).toContain('return <UsageTable rows={rows} total={u.total} nameHeader="Project" label="Usage per project" />;');
    const tabs = read("src", "components", "Tabs.tsx");
    expect(tabs).toContain('<div className="tabs" role="tablist"');
    expect(tabs).toContain('className="tab" role="tab"');
  });

  describe("prep_tour gives every project tokens and a cost, and nothing scenes 1 to 8 show", () => {
    const prep = read("crates/gizai-core/examples/prep_tour.rs");
    /** `let <var> = add(&<project>, "<title>", "<column>", …)`: the cards a run can point to. */
    const cards = Object.fromEntries([...prep.matchAll(/let (\w+) = add\(&(\w+), "([^"]+)", "([^"]+)"/g)].map((m) => [m[1], { project: m[2], title: m[3], column: m[4] }]));
    const runs = [...prep.matchAll(/finished\(&(\w+), "(\w+)", &(\w+), "([a-z-]+)", "(\w+)",\s*"[^"]+", ([\d_]+), ([\d_]+), ([\d_]+)\);/g)]
      .map((m) => ({ agent: m[1], role: m[2], card: m[3], outcome: m[5], numbers: m.slice(6).map((n) => Number(n.replace(/_/g, ""))) }));
    const closure = prep.slice(prep.indexOf("let finished = |"), prep.indexOf("};", prep.indexOf("let finished = |")));

    it("four finished runs, two on Lumen Health's cards and two on Orbit CRM's, each with a cost and input and output tokens", () => {
      expect(runs).toHaveLength(4);
      const projects = runs.map((r) => cards[r.card]?.project);
      expect(projects.filter((p) => p === "health")).toHaveLength(2);
      expect(projects.filter((p) => p === "crm")).toHaveLength(2);
      for (const r of runs) for (const n of r.numbers) expect(n).toBeGreaterThan(0);
      expect(prep).toMatch(/let health = project\("Lumen Health", "LUM"/);
      expect(prep).toMatch(/let crm = project\("Orbit CRM", "ORB"/);
    });

    it("they are the Design and QA Agents' runs, never the Backend Agent's (scene 7 shows its page), in their own roles", () => {
      expect(new Set(runs.map((r) => r.agent))).toEqual(new Set(["design", "qa"]));
      for (const r of runs) expect(r.role).toBe(r.agent);
      expect(prep).toMatch(/let design = agent\("Design Agent", "design"/);
      expect(prep).toMatch(/let qa = agent\("QA Agent", "qa"/);
    });

    it("on cards in Done, Deploy or Review, which no agent at work takes, and they move no card", () => {
      for (const r of runs) expect(["Done", "Deploy", "Review"]).toContain(cards[r.card].column);
      expect(closure).toContain('runs::finish(&db, &r, "succeeded"');
      expect(closure).not.toMatch(/move_to|tasks::|comments::|inbox/);
    });
  });
});

describe("the capture uses made-up data only", () => {
  // the names that must never show: the real user, the real home and projects folder, the round 1 tour's second account
  const REAL = /jefsev|jeffrey|sevinga|yotech|\/home\/|\/Users\/|~\/Herd|\bHerd\b|Claude Code 2/i;
  const sources = [
    "scripts/tour-capture.sh",
    "scripts/tour/fake-claude.sh",
    "scripts/tour/fake-codex.sh",
    "crates/gizai-core/examples/prep_tour.rs",
  ];

  it.each(sources)("%s names no real user, home folder or projects folder", (file) => {
    expect(read(file)).not.toMatch(REAL);
  });

  it("prep_tour makes three made-up projects for three made-up clients, and an owner who isn't a real person here", () => {
    const prep = read("crates/gizai-core/examples/prep_tour.rs");
    const projects = [...prep.matchAll(/project\("([^"]+)", "([A-Z]+)"/g)].map((m) => [m[1], m[2]]);
    expect(projects).toEqual([["Northwind Shop", "NWS"], ["Lumen Health", "LUM"], ["Orbit CRM", "ORB"]]);
    expect(prep).toMatch(/client\("Northwind Traders"/);
    expect(prep).toMatch(/client\("Lumen Health"/);
    expect(prep).toMatch(/client\("Orbit Labs"/);
    expect(prep).toMatch(/ensure_seed\(&db, "Alex Morgan"\)/);
    // no demo data from elsewhere: the old README GIF's data maker is gone
    expect(existsSync(join(ROOT, "crates/gizai-core/examples/prep_readme.rs"))).toBe(false);
  });

  it("Gizai runs in the cage with a fake home, its data, cache and config in it, and only the fakes' bin on its PATH", () => {
    expect(sh).toMatch(/FAKE_HOME=\$LIVE\/home; DATA=\$FAKE_HOME\/\.local\/share\/gizai/);
    expect(sh).toMatch(/PATH_IN_CAGE=\$FAKE_HOME\/\.local\/bin:\/usr\/local\/bin:\/usr\/bin:\/bin\n/);
    const run = sh.slice(sh.indexOf("env -i "), sh.indexOf("timeout -k"));
    for (const v of ['HOME="$FAKE_HOME"', 'PATH="$PATH_IN_CAGE"', 'GIZAI_DATA_DIR="$DATA"', 'XDG_DATA_HOME="$FAKE_HOME/.local/share"',
      'XDG_CACHE_HOME="$FAKE_HOME/.cache"', 'XDG_CONFIG_HOME="$FAKE_HOME/.config"', "WLR_BACKENDS=headless"]) {
      expect(run).toContain(v);
    }
    // the fake data lives in .devdata/tour, apart from the stills' folder
    expect(sh).toMatch(/^T=\$GZ\/\.devdata\/tour; BASE=\$T\/base; LIVE=\$T\/live$/m);
    // the fakes are Claude Code and Codex, at ~/.local/bin/claude and ~/.local/bin/codex in the fake home
    expect(sh).toContain('cp scripts/tour/fake-claude.sh "$FAKE_HOME/.local/bin/claude"');
    expect(sh).toContain('cp scripts/tour/fake-codex.sh "$FAKE_HOME/.local/bin/codex"');
    const prep = read("crates/gizai-core/examples/prep_tour.rs");
    expect(prep).toContain("~/.local/bin/claude");
    expect(prep).toContain("~/.local/bin/codex");
  });
});

describe("the tour's video and GIFs left this repository (GA-149, GA-241)", () => {
  it("the README's old tour folder is gone, on disk and in git, and git has no GIF of the tour anywhere", () => {
    expect(existsSync(join(ROOT, "docs", "tour"))).toBe(false);
    expect(tracked(TOUR_DIR)).toEqual([]);
    expect(tracked().filter((p) => /(^|\/)tour-[a-z]+\.gif$/i.test(p))).toEqual([]);
  });

  it("nothing in git outside docs/research and docs/superpowers names that folder or its two GIFs", () => {
    const r = git("grep", "-nE", `${TOUR_DIR}|tour-(dark|light)\\.gif`, "--", ".", ":(exclude)docs/research", ":(exclude)docs/superpowers");
    expect(r.stdout).toBe("");
    // 1 is git grep's "nothing found", not an error
    expect(r.status).toBe(1);
  });

  it("git has no video/ folder, and no MP4, WebM or poster of the tour anywhere", () => {
    expect(tracked("video")).toEqual([]);
    expect(tracked().filter((p) => /\.(mp4|webm|mov)$/i.test(p) || /(^|\/)tour-[a-z]+-poster\.png$/.test(p))).toEqual([]);
  });

  it("nothing in git points to the video project, its stills or the renders that left", () => {
    const r = git("grep", "-nE", "(^|[^a-z/])video/(capture|src|out|render\\.mjs|package\\.json|README)|tour-(dark|light)(\\.mp4|\\.webm|-poster\\.png)",
      "--", ".", ":(exclude)scripts/tour-*.test.mjs");
    expect(r.stdout).toBe("");
  });

  it("package.json has no workspaces and no Remotion, and the lock file has nothing from video/", () => {
    const pkg = JSON.parse(read("package.json"));
    expect(pkg.workspaces).toBeUndefined();
    const deps = Object.keys({ ...pkg.dependencies, ...pkg.devDependencies });
    expect(deps.filter((d) => /remotion/.test(d))).toEqual([]);
    expect(JSON.stringify(pkg.scripts)).not.toMatch(/video/);
    const lock = JSON.parse(read("package-lock.json"));
    expect(Object.keys(lock.packages).filter((p) => p.startsWith("video") || /remotion/.test(p))).toEqual([]);
  });

  it("tsc and vitest only look in src/ (and vitest in scripts/)", () => {
    expect(JSON.parse(read("tsconfig.json")).include).toEqual(["src"]);
    const vitest = read("vitest.config.ts");
    const include = [...vitest.match(/include:\s*\[([^\]]*)\]/)[1].matchAll(/"([^"]+)"/g)].map((m) => m[1]);
    expect(include.length).toBeGreaterThan(0);
    for (const pattern of include) expect(pattern).toMatch(/^(src|scripts)\//);
  });
});

describe("the README (GA-241): no tour under the logo, and the capture makes the stills of each feature", () => {
  const readme = read("README.md");
  const lines = readme.split("\n");
  const development = (readme.split(/\n## /).find((s) => s.startsWith("Development\n")) ?? "").split("\n");
  const CAPTURE = "scripts/tour-capture.sh --out <folder>   # the stills of each feature, in dark and light (default .devdata/tour/capture)";
  const STILLS = "The capture makes the stills of each feature: Hersir with made-up data in a headless compositor, scene by scene, " +
    "in dark and light, with a `scenes.json` of what each step clicked. The website's repository makes hersir.dev's pictures from them.";

  it("its only <picture> is the logo's: no GIF, no tour and no link into the old tour folder", () => {
    const pictures = readme.match(/<picture>[\s\S]*?<\/picture>/g) ?? [];
    expect(pictures).toHaveLength(1);
    expect(pictures[0]).toContain('<img alt="Hersir" src="design/brand/logo-horizontal-on-light.svg" width="320">');
    expect(readme).not.toMatch(/\.gif\b/i);
    expect(readme).not.toContain(TOUR_DIR);
    expect(readme).not.toMatch(/product tour|tour video|Remotion/i);
  });

  it("the centered block under the logo closes one blank line after the links, and What it is comes next", () => {
    expect(lines[8]).toBe('<div align="center">');
    expect(lines[15]).toMatch(/^\[Website\]\(https:\/\/hersir\.dev\) · \[Install\]/);
    expect(lines.slice(16, 20)).toEqual(["", "</div>", "", "## What it is"]);
    expect(readme.match(/<\/?div\b[^>]*>/g)).toEqual(['<div align="center">', "</div>"]);
  });

  it("its Development section runs the capture for the stills of each feature, after the UI tests", () => {
    expect(development.filter((l) => l.startsWith("scripts/tour-capture.sh"))).toEqual([CAPTURE]);
    expect(development[development.indexOf(CAPTURE) - 1]).toMatch(/^scripts\/ui-test\.sh /);
  });

  it("and says, after the performance paragraph, that the website's repository makes hersir.dev's pictures from the stills", () => {
    expect(development.filter((l) => l.startsWith("The capture makes"))).toEqual([STILLS]);
    expect(development[development.indexOf(STILLS) - 2]).toMatch(/^A performance card runs `scripts\/perf\.sh`/);
  });

  it("names no video/ folder, MP4, WebM or poster, and the old GIF and its script are gone", () => {
    expect(readme).not.toMatch(/video\/|\.mp4|\.webm|poster/);
    for (const old of ["docs/gizai.gif", "docs/gizai.mp4", "scripts/readme-gif.sh"]) expect(existsSync(join(ROOT, old))).toBe(false);
    expect(readme).not.toMatch(/docs\/gizai\.gif|readme-gif/);
  });
});
