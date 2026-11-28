// A project's Docs page, drawn from a small index: every section with what it shows, links under the host's own prefix
// (Hersir's #/devdocs/<id>/ or a browser's #/), a switched-off section left out, and only Hersir's own type and colours.
import { describe, expect, it } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import { DocsBody } from "./DocsView";
import { parseDocsPath } from "./route";
import { sampleIndex } from "./sampleIndex.test-util";
import type { DocsHost, Index } from "./types";

// as the other style tests read a stylesheet (lib/codeTheme.test.ts): Vite hands a test no ?raw CSS, and the app has no Node types
const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const docsCss: string = fs.readFileSync(new URL("../styles/docs.css", import.meta.url), "utf8");

const host: DocsHost = {
  index: () => Promise.resolve(sampleIndex()), source: (path) => Promise.resolve({ path, lines: [] }), commit: () => Promise.resolve(null),
  agentsUrl: (path) => `http://127.0.0.1:4380/p/p1/${path}`, openExternal: () => {}, copy: () => Promise.resolve(),
};
const draw = (path: string, index: Index = sampleIndex()) =>
  renderToStaticMarkup(<DocsBody index={index} host={host} route={parseDocsPath(path)} prefix="#/devdocs/p1/" navigate={() => {}} />);

describe("the Docs page", () => {
  it("opens on Home: each section with its count, what moved since the last commit, and the files for agents", () => {
    const html = draw("");
    expect(html).toContain("<h1>Kade</h1>");
    expect(html).toMatch(/href="#\/devdocs\/p1\/reference"[^>]*>.*Reference.*<span class="dd-count">3<\/span>/);
    expect(html).toContain('<span class="dd-moved documented">now documented</span>');
    expect(html).toContain('href="#/devdocs/p1/reference/app%2FUser.php%23User"');
    expect(html).toContain('href="http://127.0.0.1:4380/p/p1/llms.txt"');
    expect(html).toContain("Hersir shows what is written above a declaration");
  });

  it("lists the Reference by kind, and opens an entry with its description, facts and what is inside it", () => {
    const list = draw("reference?kind=controller");
    expect(list).toContain("1 declaration of kind controller");
    expect(list).toContain("UserController");
    expect(list).not.toContain(">owner<");
    const entry = draw("reference/app%2FUser.php%23User");
    expect(entry).toContain('<h1 class="mono" id="summary">User</h1>');
    expect(entry).toContain("A person who signs in. Owns invoices.");
    expect(entry).toContain("<td>Extends</td><td class=\"mono\">Model</td>");
    expect(entry).toContain("Inside (1)");
    expect(entry).toContain('href="http://127.0.0.1:4380/p/p1/md/reference/app/User.php/User.md"');
  });

  it("draws an endpoint's request body and response fields, and its handler", () => {
    const html = draw("api/post-users");
    expect(html).toContain('<span class="dd-method post">POST</span>');
    expect(html).toContain("application/json");
    expect(html).toMatch(/name <span class="sm muted">string<\/span><span class="dd-req">required<\/span>/);
    expect(html).toContain("string · admin | member");
  });

  it("shows the changelog's lines, the decisions with their related files, the guides, MCP and the commits", () => {
    const changelog = draw("changelog?kind=Fixed");
    expect(changelog).toContain("Totals round <em>once</em>.");
    expect(changelog).not.toContain("Invoices export as CSV.");
    const decision = draw("decisions/0001");
    expect(decision).toContain("<h1>Use Postgres</h1>");
    expect(decision).toContain('href="#/devdocs/p1/reference/app%2FUser.php%23User"');
    expect(decision).toContain("not in the Reference");
    const guide = draw("guides/readme");
    expect(guide).not.toContain('<h1 id="kade">');
    expect(guide).toContain('<h2 id="install">Install</h2>');
    const mcp = draw("mcp");
    expect(mcp).toContain("${GITHUB_TOKEN}");
    expect(mcp).toContain("Values are never shown.");
    expect(draw("mcp/mcp%2Ftool%2Fsearch_users")).toContain("Input schema");
    expect(draw("commits")).toContain("Add users");
  });

  it("lists the decisions by month, the latest first, by status, and opens one wide with the titles on the right, above On this page", () => {
    const base = sampleIndex().decisions[0]!;
    const decisions = [
      { ...base, id: "n1-20261125-1", number: "", title: "Comments live in our own table", date: "2026-11-25", file: "Decisions/Kade",
        html: "<p>Comments live in our own table: Postgres, moderated in admin.</p>", headings: [], relatedFiles: [] },
      { ...base, id: "decisions/0002", number: "0002", title: "Use a queue", date: "2026-11-24", status: "proposed", file: "by Chief in Hersir",
        sections: [{ heading: "Context", html: "<p>Exports are slow.</p>" }, { heading: "Decision", html: "<p>A queue.</p>" }], relatedFiles: [] },
      base,
      { ...base, id: "n1-20261002-1", number: "", title: "Use Laravel", date: "2026-10-02", relatedFiles: [] },
    ];
    const index = sampleIndex({ decisions });
    const list = draw("decisions", index);
    const at = (text: string) => list.indexOf(text);
    expect(at("Comments live in our own table")).toBeLessThan(at("Use a queue"));
    expect(at("Use a queue")).toBeLessThan(at("Use Postgres"));
    expect(at("November 2026")).toBeLessThan(at("October 2026"));
    expect(list).toContain("Proposed · 1");
    expect(list).toContain("25 Nov");
    expect(list).not.toContain("dd-doc-nav");
    const proposed = draw("decisions?kind=proposed", index);
    expect(proposed).toContain("Use a queue");
    expect(proposed).not.toContain("Use Laravel");

    const one = draw("decisions/n1-20261125-1", index);
    expect(one).toContain("<h1>Comments live in our own table</h1>");
    expect(one).toContain("25 November 2026");
    expect(one).not.toContain("dd-doc-nav");
    expect(one).toMatch(/<aside class="dd-rail"><h3>Decisions<\/h3>.*Use Laravel/);
    expect(one).toContain('class="dd-doc-link on" href="#/devdocs/p1/decisions/n1-20261125-1"');
    expect(one).not.toContain("On this page");
    const record = draw("decisions/decisions%2F0002", index);
    expect(record).toContain('<h2 id="context">Context</h2>');
    expect(record).toMatch(/<div class="dd-decision-block">.*A queue/);
    expect(record.indexOf("Use Laravel")).toBeLessThan(record.indexOf("On this page"));
  });

  it("leaves a switched-off section out of the top bar and says so on its page", () => {
    const index = sampleIndex({ switchedOff: [{ section: "decisions", key: "decisions" }] });
    expect(draw("", index)).not.toContain('href="#/devdocs/p1/decisions"');
    expect(draw("decisions", index)).toContain("Switched off for this project");
  });

  it("is drawn in Hersir's own type and colours, never the colours kept for agents at work and for what needs you", () => {
    expect(docsCss).not.toMatch(/Inter|IBM Plex|--yd-/);
    expect(docsCss).not.toMatch(/var\(--live|var\(--needs/);
    expect(docsCss).toContain("var(--font-sans)");
  });
});
