// The Docs page of one project (the Docs section): its crumbs and tabs; with Docs on, Changelog (the merged cards),
// Decisions (the shared decision notes about the project) and Commits (main's newest commits, an opened one showing each
// changed file in its own box); with Docs off, the way to turn it on. Rendered to HTML on the server: the data hooks are
// replaced so the page gets its data at once (a thrown error is the hook's error), and a button's click is tried on the
// element tree (the page called as a function, its hooks being the replaced ones).
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { DocsTab } from "../router";
import type { ApiRoute, ChangelogEntry, Guide, CodeReference, CommitFiles, DecisionNote, DocsCommits, Project } from "../types";

const state = vi.hoisted(() => ({
  project: null as unknown, commits: null as unknown, files: null as unknown, changelog: null as unknown, decisions: null as unknown,
  reference: null as unknown, api: null as unknown, guides: null as unknown, open: vi.fn(),
}));
vi.mock("../api", async (orig) => ({
  ...(await orig<object>()),
  getProject: () => state.project,
  docsCommits: () => { if (state.commits instanceof Error) throw state.commits; return state.commits; },
  docsCommitFiles: () => state.files,
  docsChangelog: () => state.changelog,
  docsDecisions: () => state.decisions,
  docsReference: () => state.reference,
  docsApi: () => state.api,
  docsGuides: () => state.guides,
}));
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    try { return { data: fetch() ?? null, error: null, reload: () => {}, setData: () => {} }; }
    catch (e) { return { data: null, error: e instanceof Error ? e.message : String(e), reload: () => {}, setData: () => {} }; }
  },
}));
vi.mock("../lib/drawers", async (orig) => ({ ...(await orig<object>()), useDrawer: () => state.open }));

const { DocsPage, CommitFileList, ReferenceList } = await import("./DocsPage");

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
  state.reference = { entries: [], files: 0, partial: [] };
  state.api = [];
  state.guides = [];
});

describe("the Docs page", () => {
  it("names the section and the project in its crumbs", () => {
    expect(page()).toContain('<div class="crumbs"><span>Docs</span><span class="sep">/</span><b>Kade portal</b></div>');
  });
  it("opens on Reference, with tabs for Guides, API, Changelog, Decisions and Commits", () => {
    const html = page();
    expect(html).toMatch(/<a class="tab" role="tab" aria-selected="true" href="#\/devdocs\/kade\/reference"><svg[^>]*>.*?<\/svg>Reference<\/a>/);
    for (const [tab, label] of [["guides", "Guides"], ["api", "API"], ["changelog", "Changelog"], ["decisions", "Decisions"], ["commits", "Commits"]]) {
      expect(html).toMatch(new RegExp(`<a class="tab" role="tab" aria-selected="false" href="#/devdocs/kade/${tab}"><svg[^>]*>.*?</svg>${label}</a>`));
    }
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

describe("Reference", () => {
  const ref = {
    entries: [
      { name: "Header", kind: "component", path: "src/Header.tsx", line: 4, endLine: 8, doc: "The page's header.\nWith the site's name.", text: "export function Header() {}" },
      { name: "useMenu", kind: "hook", path: "src/menu.ts", line: 2, endLine: 4, doc: "", text: "export function useMenu() {}" },
      { name: "Invoice", kind: "model", path: "app/Models/Invoice.php", line: 10, endLine: 13, doc: "", text: "class Invoice extends Model {}" },
    ],
    files: 3, partial: ["src/Broken.tsx"],
  } satisfies CodeReference;
  it("groups the declarations by kind, components first, each with where it is and its doc's first line", () => {
    state.reference = ref;
    const html = page();
    expect(html).toContain("3 declarations in 3 files");
    expect(html.indexOf("Components")).toBeLessThan(html.indexOf("Hooks"));
    expect(html.indexOf("Hooks")).toBeLessThan(html.indexOf("Models"));
    expect(html).toContain('<span class="mono">Header</span><span class="faint">src/Header.tsx:4</span><span class="doc">The page&#x27;s header.</span>');
    expect(html).toContain("1 file has syntax the parser can&#x27;t read: its declarations may be incomplete.");
  });
  it("keeps the declarations whose name, kind or file has the words typed", () => {
    const html = renderToStaticMarkup(<ReferenceList reference={ref} filter="menu" />);
    expect(html).toContain("useMenu");
    expect(html).not.toContain("Header");
    expect(html).not.toContain("Invoice");
    expect(renderToStaticMarkup(<ReferenceList reference={ref} filter="model" />)).toContain("Invoice");
    expect(renderToStaticMarkup(<ReferenceList reference={ref} filter="nothing like it" />)).toContain("Nothing matches");
  });
  it("says so while it finds no declarations", () => {
    expect(page()).toContain("<b>No declarations found</b>");
  });
});

describe("Guides", () => {
  it("lists the guides, the README first, each opening to its Markdown", () => {
    state.guides = [
      { path: "README.md", title: "Pearl", bodyMd: "# Pearl\n\nThe corporate site.\n" },
      { path: "docs/setup.md", title: "Setting up", bodyMd: "# Setting up\n" },
    ] satisfies Guide[];
    const html = page("guides");
    expect(html).toMatch(/<button class="docs-commit" aria-expanded="true">.*?<span class="subj">Pearl<\/span><span class="faint">README.md<\/span><\/button>/);
    expect(html).toContain("The corporate site.");
    expect(html).toMatch(/<button class="docs-commit" aria-expanded="false">.*?<span class="subj">Setting up<\/span><span class="faint">docs\/setup.md<\/span><\/button>/);
  });
  it("says where it looks while there are none", () => {
    expect(page("guides")).toContain("<b>No guides found</b><span>Guides shows the README at the project&#x27;s root and the Markdown under docs/.</span>");
  });
});

describe("API", () => {
  it("lists the endpoints, then the pages, each with its method, path, handler and where it is", () => {
    state.api = [
      { method: "PAGE", path: "/about", handler: "", file: "src/app/about/page.tsx", line: 1 },
      { method: "GET", path: "/api/invoices/{invoice}", handler: "InvoiceController@show", file: "routes/api.php", line: 12 },
    ] satisfies ApiRoute[];
    const html = page("api");
    expect(html.indexOf("Endpoints")).toBeLessThan(html.indexOf("Pages"));
    expect(html).toContain('<span class="docs-method get">GET</span><span class="mono">/api/invoices/{invoice}</span><span>InvoiceController@show</span><span class="faint">routes/api.php:12</span>');
    expect(html).toContain('<span class="mono">/about</span><span class="faint">src/app/about/page.tsx:1</span>');
  });
  it("says what it reads while it finds no routes", () => {
    expect(page("api")).toContain("<b>No routes found</b><span>API reads a Next.js app&#x27;s route and page files, and a Laravel app&#x27;s routes/ files.</span>");
  });
});

describe("Changelog", () => {
  it("lists the merged cards newest first, each linking to its card", () => {
    state.changelog = [
      { taskId: "t2", identifier: "KADE-2", title: "The contact form says when it sent", at: Date.now() - HOUR },
      { taskId: "t1", identifier: "KADE-1", title: "Invoices export as CSV", at: Date.now() - 50 * HOUR },
    ] satisfies ChangelogEntry[];
    const html = page("changelog");
    expect(html).toContain('<a class="mono" href="#/task/t2">KADE-2</a><span>The contact form says when it sent</span>');
    expect(html).toContain('<a class="mono" href="#/task/t1">KADE-1</a><span>Invoices export as CSV</span>');
    expect(html.indexOf("KADE-2")).toBeLessThan(html.indexOf("KADE-1"));
  });
  it("says what shows there while nothing is merged", () => {
    expect(page("changelog")).toContain("<b>Nothing merged yet</b><span>A card shows here once its work reaches main: in a Deploy or Done column.</span>");
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
