// The Docs page of one project (the Docs section): its crumbs and tabs; with Docs on, Changelog (the merged cards),
// Decisions (the shared decision notes about the project) and Commits (main's newest commits, an opened one showing each
// changed file in its own box); with Docs off, the way to turn it on. Rendered to HTML on the server: the data hooks are
// replaced so the page gets its data at once (a thrown error is the hook's error), and a button's click is tried on the
// element tree (the page called as a function, its hooks being the replaced ones).
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { DocsTab } from "../router";
import type { ChangelogEntry, CommitFiles, DecisionNote, DocsCommits, Project } from "../types";

const state = vi.hoisted(() => ({
  project: null as unknown, commits: null as unknown, files: null as unknown, changelog: null as unknown, decisions: null as unknown, open: vi.fn(),
}));
vi.mock("../api", async (orig) => ({
  ...(await orig<object>()),
  getProject: () => state.project,
  docsCommits: () => { if (state.commits instanceof Error) throw state.commits; return state.commits; },
  docsCommitFiles: () => state.files,
  docsChangelog: () => state.changelog,
  docsDecisions: () => state.decisions,
}));
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    try { return { data: fetch() ?? null, error: null, reload: () => {}, setData: () => {} }; }
    catch (e) { return { data: null, error: e instanceof Error ? e.message : String(e), reload: () => {}, setData: () => {} }; }
  },
}));
vi.mock("../lib/drawers", async (orig) => ({ ...(await orig<object>()), useDrawer: () => state.open }));

const { DocsPage, CommitFileList } = await import("./DocsPage");

const HOUR = 3_600_000;
const kade = (devDocs: boolean) => ({
  id: "kade", name: "Kade portal", key: "KADE", number: "2026-001", status: "active", defaultBranch: "main", openTasks: 0,
  doneTasks: 0, updatedAt: 1, devDocs,
}) as unknown as Project;
const page = (tab?: DocsTab) => renderToStaticMarkup(<DocsPage id="kade" tab={tab} />);

/** The buttons in the page's element tree. */
function buttons(): ReactElement<{ onClick?: () => void; children?: ReactNode }>[] {
  const found: ReactElement<{ onClick?: () => void; children?: ReactNode }>[] = [];
  const walk = (n: ReactNode): void => {
    if (!n || typeof n !== "object") return;
    if (Array.isArray(n)) { n.forEach(walk); return; }
    const el = n as ReactElement<{ children?: ReactNode }>;
    if (el.type === "button") found.push(el as ReactElement<{ onClick?: () => void; children?: ReactNode }>);
    walk(el.props?.children);
  };
  walk(DocsPage({ id: "kade" }));
  return found;
}

beforeEach(() => {
  state.open.mockReset();
  state.project = kade(true);
  state.commits = { label: "origin/main", commits: [] };
  state.files = null;
  state.changelog = [];
  state.decisions = [];
});

describe("the Docs page", () => {
  it("names the section and the project in its crumbs", () => {
    expect(page()).toContain('<div class="crumbs"><span>Docs</span><span class="sep">/</span><b>Kade portal</b></div>');
  });
  it("opens on Changelog, with tabs for Decisions and Commits", () => {
    const html = page();
    expect(html).toMatch(/<a class="tab" role="tab" aria-selected="true" href="#\/devdocs\/kade\/changelog"><svg[^>]*>.*?<\/svg>Changelog<\/a>/);
    expect(html).toMatch(/<a class="tab" role="tab" aria-selected="false" href="#\/devdocs\/kade\/decisions"><svg[^>]*>.*?<\/svg>Decisions<\/a>/);
    expect(html).toMatch(/<a class="tab" role="tab" aria-selected="false" href="#\/devdocs\/kade\/commits"><svg[^>]*>.*?<\/svg>Commits<\/a>/);
    expect(page("decisions")).toMatch(/aria-selected="true" href="#\/devdocs\/kade\/decisions"/);
  });
  it("says Docs is off, with Edit project, which opens the project's form", () => {
    state.project = kade(false);
    expect(page()).toContain("<b>Docs is off for this project</b>");
    expect(page()).not.toContain('role="tablist"');
    const edit = buttons().filter((b) => b.props.children === "Edit project");
    expect(edit).toHaveLength(1);
    edit[0]!.props.onClick!();
    expect(state.open).toHaveBeenCalledWith({ kind: "project", id: "kade" });
  });
  it("shows nothing while the project loads", () => {
    state.project = null;
    expect(page()).toBe("");
  });
});

describe("Changelog", () => {
  it("lists the merged cards newest first, each linking to its card", () => {
    state.changelog = [
      { taskId: "t2", identifier: "KADE-2", title: "The contact form says when it sent", at: Date.now() - HOUR },
      { taskId: "t1", identifier: "KADE-1", title: "Invoices export as CSV", at: Date.now() - 50 * HOUR },
    ] satisfies ChangelogEntry[];
    const html = page();
    expect(html).toContain('<a class="mono" href="#/task/t2">KADE-2</a><span>The contact form says when it sent</span>');
    expect(html).toContain('<a class="mono" href="#/task/t1">KADE-1</a><span>Invoices export as CSV</span>');
    expect(html.indexOf("KADE-2")).toBeLessThan(html.indexOf("KADE-1"));
  });
  it("says what shows there while nothing is merged", () => {
    expect(page()).toContain("<b>Nothing merged yet</b><span>A card shows here once its work reaches main: in a Deploy or Done column.</span>");
  });
});

describe("Decisions", () => {
  it("shows each decision note with a link to it in Memory, without its title twice", () => {
    state.decisions = [
      { id: "n1", path: "Decisions/Kade portal", title: "Kade portal", bodyMd: "# Kade portal\n\n- 2026-11-20 (KADE-3): Postgres, not MySQL.\n", updatedAt: Date.now() - HOUR },
    ] satisfies DecisionNote[];
    const html = page("decisions");
    expect(html).toContain('<h3><a href="#/memory/n1">Kade portal</a></h3><span class="faint">Decisions/Kade portal, 1 h ago</span>');
    expect(html).toContain("Postgres, not MySQL.");
    expect(html).not.toContain("<h1>Kade portal</h1>");
  });
  it("says where the Chief keeps them while there are none", () => {
    expect(page("decisions")).toContain("<b>No decisions yet</b><span>The Chief keeps them in Memory, in Decisions/, with this project&#x27;s key as project:.</span>");
  });
});

describe("Commits", () => {
  it("lists main's newest commits, newest first, each with its short id, subject, author and when", () => {
    state.commits = { label: "origin/main", commits: [
      { sha: "abc1234def0", parent: "0123456aaa1", author: "Remi", at: Date.now() - 2 * HOUR, subject: "Add the contact form" },
      { sha: "0123456aaa1", parent: null, author: "Jeffrey", at: Date.now() - 50 * HOUR, subject: "First commit" },
    ] } satisfies DocsCommits;
    const html = page("commits");
    expect(html).toContain("<h3>Commits</h3><span class=\"faint\">origin/main</span>");
    expect(html).toContain('<span class="mono">abc1234</span><span class="subj">Add the contact form</span><span class="faint">Remi, 2 h ago</span>');
    expect(html.indexOf("Add the contact form")).toBeLessThan(html.indexOf("First commit"));
    expect(html.match(/aria-expanded="false"/g)).toHaveLength(2);
  });
  it("says so while main has no commits yet", () => {
    state.commits = { label: "main", commits: [] } satisfies DocsCommits;
    expect(page("commits")).toContain("<b>No commits on main yet</b>");
  });
  it("says why when it can't read the commits", () => {
    state.commits = new Error("Kade portal has no git repository: link one on its project page → Edit");
    expect(page("commits")).toContain("<b>Hersir can&#x27;t show the commits</b><span>Kade portal has no git repository: link one on its project page → Edit</span>");
  });
});

describe("an opened commit", () => {
  it("shows each file it changed in its own box", () => {
    state.files = { from: "0123456aaa1", to: "abc1234def0", files: [
      { path: "src/form.tsx", oldPath: null, status: "added", additions: 40, deletions: 0, binary: false, uncommitted: false },
      { path: "README.md", oldPath: null, status: "modified", additions: 2, deletions: 1, binary: false, uncommitted: false },
    ] } satisfies CommitFiles;
    const html = renderToStaticMarkup(<CommitFileList projectId="kade" sha="abc1234def0" />);
    expect(html).toContain('aria-label="src/form.tsx"');
    expect(html).toContain('aria-label="README.md"');
    expect(html).toContain("+40");
  });
  it("says so when it changed no files", () => {
    state.files = { from: "0123456aaa1", to: "abc1234def0", files: [] } satisfies CommitFiles;
    expect(renderToStaticMarkup(<CommitFileList projectId="kade" sha="abc1234def0" />)).toContain("This commit changed no files");
  });
});
