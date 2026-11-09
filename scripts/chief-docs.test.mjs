// GA-256 QA: Chief, not Team Lead, in the docs. Hersir's lead agent is the Chief wherever people read the docs (the
// README, the docs hersir.dev shows, the other docs the card lists, SECURITY.md and the feature form), with a capital C;
// the headline's common noun stays. The README opens with the Chief, the feature doc is docs/features/chief.md, titled
// "The Chief", and no link points to team-lead.md any more. The Memory folder `Team Lead/` and the code's names stay,
// and the historical notes keep the name they had.
import { spawnSync } from "node:child_process";
import { existsSync, readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const git = (...args) => spawnSync("git", args, { cwd: ROOT, encoding: "utf8" });

const HEADLINE = "**Your coding agents need a chief, not a babysitter.**";
const OPENING = "Talk to one Chief instead of juggling a dozen agent chats. Tell it what you need, for any client or codebase: it " +
  "turns that into cards on your board, starts the agents, answers their questions and puts only tested work in your review. " +
  "Free and open source, on Claude Code, Codex CLI or Gemini CLI. Made for freelancers and software companies with several " +
  "clients and projects.";
// The card's list: the README, the docs hersir.dev shows, the other docs, SECURITY.md and the feature form.
const FEATURES = readdirSync(join(ROOT, "docs/features")).filter((f) => f.endsWith(".md")).map((f) => `docs/features/${f}`);
const DOCS = ["README.md", "SECURITY.md", ".github/ISSUE_TEMPLATE/feature.yml", "docs/install.md", "docs/settings.md", "docs/memory.md",
  "docs/agent-tools.md", "docs/PLATFORMS.md", "docs/RELEASING.md", "docs/PAID-FEATURES.md", "docs/team-template-format.md",
  "docs/backup-format.md", ...FEATURES];
// The historical notes, left as written (with design/, docs/research/ and docs/superpowers/).
const HISTORICAL = ["docs/HANDOFF-2026-10-07.md", "docs/MORNING-2026-10-07.md", "docs/OPEN-ITEMS.md", "docs/v0.3-notes.md"];
// The one "Team Lead" the docs keep: the Memory folder, as a path (`Team Lead/`, `Team Lead/Notes`, `path:"Team Lead"`).
const MEMORY_PATH = /Team Lead\/|path:"Team Lead"/g;

// The code in a Markdown file: each line of its code blocks, and its `code` spans. A ```markdown block is a sample
// for people (RELEASING.md's release notes), so it is prose, not code.
function codeParts(md) {
  const parts = [];
  let block = null;
  for (const text of md.split("\n")) {
    const fence = text.match(/^ *```(\w*)/);
    if (fence) block = block === null ? fence[1] : null;
    else if (block !== null) { if (block !== "markdown") parts.push(text); }
    else for (const [span] of text.matchAll(/`[^`]*`/g)) parts.push(span);
  }
  return parts;
}
const lines = (doc, test) => read(doc).split("\n").flatMap((line, i) => (test(line) ? [`${doc}:${i + 1}: ${line.trim().slice(0, 140)}`] : []));

describe("the README leads with the Chief", () => {
  const readme = read("README.md");
  const section = (title) => readme.split(`\n## ${title}\n`)[1].split("\n## ")[0];

  it("under the headline, the card's paragraph word for word, as one paragraph", () => {
    const after = readme.split(`\n${HEADLINE}\n\n`);
    expect(after).toHaveLength(2);
    const paragraph = after[1].split("\n\n")[0];
    expect(paragraph.split("\n").join(" ")).toBe(OPENING);
    // and the links come right after it
    expect(after[1].split("\n\n")[1]).toMatch(/^\[Website\]\(https:\/\/hersir\.dev\) · /);
  });

  it("the headline keeps its common noun, a chief", () => {
    expect(readme.split("\n").filter((l) => l.includes("babysitter"))).toEqual([HEADLINE]);
  });

  it("The team starts with the Chief", () => {
    const bullets = section("The team").split("\n").filter((l) => l.startsWith("- "));
    expect(bullets[0]).toMatch(/^- \*\*Chief:\*\* you chat with it \(its chat runs on Claude Code\)\./);
  });

  it("Features' second line is A Chief you talk to, with its Read more to docs/features/chief.md", () => {
    const features = section("Features").split("\n").filter((l) => l.startsWith("- **"));
    expect(features[1]).toMatch(/^- \*\*A Chief you talk to\.\*\* Ask the Chief in chat to plan work, .*\[Read more\]\(docs\/features\/chief\.md\)$/);
  });

  it("Documentation lists the Chief's doc second among the features", () => {
    const features = section("Documentation").split("\n").find((l) => l.startsWith("- Features: "));
    expect(features.match(/\[[^\]]+\]\([^)]+\)/g).slice(0, 2)).toEqual(["[Board](docs/features/board.md)", "[Chief](docs/features/chief.md)"]);
  });
});

describe("the docs say Chief", () => {
  it("the card's list is all there", () => {
    for (const doc of DOCS) expect(existsSync(join(ROOT, doc)), doc).toBe(true);
    expect(FEATURES).toEqual(expect.arrayContaining(["docs/features/board.md", "docs/features/chief.md", "docs/features/inbox.md", "docs/features/testing.md"]));
  });

  it("no doc in the list says Team Lead, but for the Memory folder's path", () => {
    const left = DOCS.flatMap((doc) => lines(doc, (l) => /team.?lead/i.test(l.replace(MEMORY_PATH, ""))));
    expect(left).toEqual([]);
  });

  it("Chief has a capital C wherever it names the agent: only the headline and link targets have chief in lower case", () => {
    const left = DOCS.flatMap((doc) => lines(doc, (l) => l !== HEADLINE && /\bchief\b/.test(l.replace(/\]\([^)]*\)/g, "](link)"))));
    expect(left).toEqual([]);
  });

  it("the card's phrases are there: the Chief's chat, Chief may merge, Ask the Chief first, and the renamed headings", () => {
    const has = (doc, text) => expect(read(doc), `${text} in ${doc}`).toContain(text);
    has("README.md", "The Chief's chat needs Claude Code.");
    has("docs/install.md", "The Chief's chat needs Claude Code.");
    has("docs/settings.md", "Ask the Chief first");
    has("docs/agent-tools.md", "**Chief may merge**");
    has("docs/features/testing.md", "\n## The Chief may merge\n");
    has("docs/agent-tools.md", "\n## The Chief merges pull requests\n");
    has("docs/memory.md", "\n## Agents ask the Chief first (GA-70)\n");
    has("docs/features/chief.md", "\n## Agents ask the Chief first\n");
  });

  it("the feature form's part is Chief, after Board", () => {
    const form = read(".github/ISSUE_TEMPLATE/feature.yml");
    expect(form).toMatch(/\n( +)- Board\n\1- Chief\n\1- CLIs\n/);
    expect(form).not.toMatch(/Team Lead/);
  });
});

describe("what stays", () => {
  it("the Memory folder keeps its path, Team Lead/, and no doc names a Chief/ folder", () => {
    const memory = read("docs/memory.md");
    for (const path of ["`Team Lead/`", "`Team Lead/Notes`", "`Team Lead/Imported/`", '`path:"Team Lead"`', "`Team Lead/Imported/<project folder>/<file name>`"]) {
      expect(memory, path).toContain(path);
    }
    expect(read("docs/team-template-format.md")).toContain("(it keeps its notes in Team Lead/)");
    expect(DOCS.flatMap((doc) => lines(doc, (l) => /Chief\//.test(l.replace(/\]\([^)]*\)/g, ""))))).toEqual([]);
  });

  it("no code name in the docs became Chief: their code spans and code blocks don't say it", () => {
    const left = DOCS.filter((doc) => doc.endsWith(".md")).flatMap((doc) => codeParts(read(doc)).filter((c) => /chief/i.test(c)).map((c) => `${doc}: ${c}`));
    expect(left).toEqual([]);
  });

  it("the headline and the old name's line are as they were", () => {
    const readme = read("README.md");
    expect(readme).toContain(`\n${HEADLINE}\n`);
    expect(readme).toContain("\nHersir was called GizAi until version 0.14.\n");
  });

  it("the historical notes still say Team Lead, as they did", () => {
    for (const doc of HISTORICAL) expect(read(doc), doc).toMatch(/Team Lead/);
  });
});

describe("the feature doc is docs/features/chief.md", () => {
  it('it exists, is in git and is titled "The Chief"', () => {
    expect(read("docs/features/chief.md").split("\n")[0]).toBe("# The Chief");
    expect(git("ls-files", "--error-unmatch", "docs/features/chief.md").status).toBe(0);
  });

  it("docs/features/team-lead.md is gone", () => {
    expect(existsSync(join(ROOT, "docs/features/team-lead.md"))).toBe(false);
    expect(git("ls-files", "docs/features/team-lead.md").stdout).toBe("");
  });

  it("no file links to it or to its anchors any more, but for design/ (the website's card) and the historical notes", () => {
    const found = git("grep", "-n", "-e", "team-lead" + "\\.md", "-e", "\\.md#[a-z0-9-]*team-lead", "--", ".", ":!design", ":!docs/research", ":!docs/superpowers",
      ":!docs/HANDOFF-*", ":!docs/MORNING-*", ":!docs/v0.*", ":!docs/OPEN-ITEMS.md");
    expect(found.stdout).toBe("");
  });

  it("the links that pointed to it reach chief.md: the README, SECURITY.md and the feature docs", () => {
    expect(read("README.md")).toContain("[Read more](docs/features/chief.md)");
    expect(read("SECURITY.md")).toContain("(docs/features/chief.md#approve-with-one-click)");
    for (const [doc, link] of [["docs/features/board.md", "(chief.md#teams)"], ["docs/features/cli.md", "(chief.md#the-chat)"],
      ["docs/features/inbox.md", "(chief.md#agents-ask-the-chief-first)"], ["docs/features/memory.md", "(chief.md#agents-ask-the-chief-first)"],
      ["docs/features/tools.md", "(chief.md#approve-with-one-click)"], ["docs/features/inbox.md", "(../memory.md#agents-ask-the-chief-first-ga-70)"],
      ["docs/features/testing.md", "(../agent-tools.md#the-chief-merges-pull-requests)"], ["docs/features/chief.md", "(testing.md#the-chief-may-merge)"]]) {
      expect(read(doc), `${link} in ${doc}`).toContain(link);
    }
  });
});
