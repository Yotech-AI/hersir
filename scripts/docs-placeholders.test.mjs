// GA-240 QA: Markdown reads <you>, <branch> or <project> outside code as an HTML tag, so on GitHub and on the website the
// word vanishes ("Hersir pushed (3 commits)"). No doc in docs/ or docs/features/ has such a tag outside code; the lines
// that had one show their word in code, the kept names in docs/memory.md are in code too, and the code marks left each
// line in its list or table row. The docs are read with what the app renders Markdown with (react-markdown, remark-gfm),
// which follows CommonMark and GFM as GitHub does. Also: src/selftest.ts calls --needs gold, as it is now.
import { readFileSync, readdirSync } from "node:fs";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { createElement } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import Markdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { describe, expect, it } from "vitest";

const ROOT = fileURLToPath(new URL("..", import.meta.url));
const read = (path) => readFileSync(join(ROOT, path), "utf8");
const mdIn = (dir) => readdirSync(join(ROOT, dir)).filter((f) => f.endsWith(".md")).map((f) => `${dir}/${f}`);
const DOCS = [...mdIn("docs"), ...mdIn("docs/features")];

/** The Markdown's syntax tree, and its HTML with quotes as written (so the tests below read like the docs). */
function parse(src) {
  let tree;
  const html = renderToStaticMarkup(createElement(Markdown, { remarkPlugins: [remarkGfm, () => (t) => { tree = t; }] }, src));
  return { tree, html: html.replace(/&#x27;/g, "'").replace(/&quot;/g, '"') };
}
function nodes(node, type, out = []) {
  if (node.type === type) out.push(node);
  for (const c of node.children ?? []) nodes(c, type, out);
  return out;
}
// HTML a doc may really use. Any other tag outside code is a word that won't show.
const REAL_HTML = /^<(!--|\/?(a|br|details|img|kbd|sub|summary|sup)[\s/>])/i;
const vanishing = (src) => nodes(parse(src).tree, "html").map((n) => n.value).filter((v) => !REAL_HTML.test(v));

describe("no <word> outside code in the docs", () => {
  it("the check sees one outside code, also in a type or a path, and none in code", () => {
    expect(vanishing("Hersir pushed <branch> (3 commits)")).toEqual(["<branch>"]);
    expect(vanishing('a title gets " (<agent name>)" added')).toEqual(["<agent name>"]);
    expect(vanishing("TaskPatch uses Option<String>")).toEqual(["<String>"]);
    expect(vanishing("| a | b |\n|---|---|\n| Follow <project> | c |")).toEqual(["<project>"]);
    expect(vanishing("Hersir pushed `<branch>` (3 commits)")).toEqual([]);
    expect(vanishing("```\n<branch>\n```")).toEqual([]);
  });
  it("the check leaves real HTML alone", () => {
    expect(vanishing("<!-- a note -->\n\n<details><summary>More</summary>\n\nText\n\n</details>")).toEqual([]);
    expect(vanishing("One<br>two, and <kbd>Ctrl</kbd>")).toEqual([]);
  });
  it("it covers docs/*.md and docs/features/*.md", () => {
    for (const doc of ["docs/memory.md", "docs/settings.md", "docs/agent-tools.md", "docs/features/testing.md", "docs/features/team-lead.md"]) expect(DOCS).toContain(doc);
  });
  it.each(DOCS)("%s", (doc) => {
    expect(vanishing(read(doc))).toEqual([]);
  });
});

describe("the lines that had one show their word, in code", () => {
  it.each([
    ["docs/features/testing.md", '"Hersir pushed <code>&lt;branch&gt;</code> (3 commits)"'],
    ["docs/features/team-lead.md", '"Needs <code>&lt;you&gt;</code>: why, the options, my advice"'],
    ["docs/memory.md", '"Needs <code>&lt;you&gt;</code>: why"'],
    ["docs/memory.md", '" (<code>&lt;agent name&gt;</code>)" added'],
    ["docs/settings.md", "once set, Follow <code>&lt;project&gt;</code> under it goes back to that."],
    ["docs/agent-tools.md", `"<code>&lt;tool&gt;</code> wasn't run: this answer used <code>&lt;outside tool&gt;</code>, a tool from outside Hersir.`],
    ["docs/agent-tools.md", '"Approved: <code>&lt;what&gt;</code>. <code>&lt;what happened&gt;</code>" (or "It failed: <code>&lt;the error&gt;</code>", in red) or "Discarded: <code>&lt;what&gt;</code>."'],
    ["docs/agent-tools.md", "after using <code>&lt;tool&gt;</code>, a tool from outside Hersir."],
    ["docs/HANDOFF-2026-10-07.md", '"Hersir pushed <code>&lt;branch&gt;</code> (<code>&lt;n&gt;</code> commits)"'],
    ["docs/v0.1-notes.md", '"already checked out at <code>&lt;same path&gt;</code>"'],
    ["docs/v0.1-notes.md", "TaskPatch uses <code>Option&lt;String&gt;</code> where"],
    ["docs/v0.1-notes.md", "via <code>#/tasks?project=&lt;id&gt;</code> plus"],
    ["docs/v0.1-notes.md", "copies the blob to <code>data_dir/open/&lt;id&gt;/&lt;name&gt;</code> (a real copy"],
    ["docs/v0.1-notes.md", "route <code>#/team/&lt;agentId&gt;</code> selects an agent"],
  ])("%s: %s", (doc, line) => {
    expect(parse(read(doc)).html).toContain(line);
  });
});

describe("docs/memory.md's kept names and lists, and docs/settings.md's table", () => {
  const memory = parse(read("docs/memory.md")).html;
  const LINES = [
    "through the <code>gizai</code> tools",
    "(no <code>gizai-mcp</code> helper, its CLI not found)",
    '"Needs <code>&lt;you&gt;</code>: why"',
    '" (<code>&lt;agent name&gt;</code>)" added',
  ];
  it.each(LINES)("%s stays inside its list item", (line) => {
    const at = memory.indexOf(line);
    expect(at).toBeGreaterThan(-1);
    expect(memory.lastIndexOf("<li>", at)).toBeGreaterThan(memory.lastIndexOf("</li>", at));
  });
  it("`<project>` stays in the last cell of one table row, as wide as its table's header", () => {
    const { tree } = parse(read("docs/settings.md"));
    const hasProject = (node) => nodes(node, "inlineCode").some((c) => c.value === "<project>");
    const table = nodes(tree, "table").find(hasProject);
    const rows = table.children.filter(hasProject);
    expect(rows).toHaveLength(1);
    expect(rows[0].children).toHaveLength(table.children[0].children.length);
    expect(hasProject(rows[0].children.at(-1))).toBe(true);
  });
});

describe("src/selftest.ts", () => {
  it("calls --needs gold in its four comments, and magenta is gone", () => {
    const selftest = read("src/selftest.ts");
    expect(selftest).not.toMatch(/magenta/i);
    for (const comment of ["the teal (--live) or gold (--needs) of this theme", "The colours that are teal or gold: within 18° of --live's or --needs's hue", "no teal or gold pixel, in dark and light", "No teal or gold anywhere, also in the light theme"]) expect(selftest).toContain(comment);
  });
});
