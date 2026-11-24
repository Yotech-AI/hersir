// GA-30: the project form's New worktrees section: paths to copy, the install switch and the setup command.
// Rendered to HTML on the server, so no data loads: this checks what a new project's form shows and sends.
import { describe, expect, it } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { copyPaths, DOCS_SECTIONS, DocsSections, ProjectDrawer, toProjectInput } from "./ProjectForm";
import devdocsRs from "../../crates/gizai-core/src/devdocs.rs?raw";
import type { Project } from "../types";

describe("New worktrees", () => {
  it("shows one copy list, the install switch (on) and a setup command for a new project", () => {
    const html = renderToStaticMarkup(<ProjectDrawer onClose={() => {}} />);
    expect(html).toContain("New worktrees");
    expect(html).toContain("Copy from the main checkout");
    expect(html.match(/<textarea[^>]*id="p-copy"/g)).toHaveLength(1);
    expect(html).toContain("cp --reflink=auto");
    expect(html).toMatch(/<input type="checkbox" checked=""\/>Install missing dependencies/);
    expect(html).toMatch(/<input id="p-setup"/);
    expect(html).toContain("Setup command");
  });

  it("starts a new project with nothing to copy, the install on and no setup command", () => {
    const v = toProjectInput(null);
    expect(v.worktreeCopy).toEqual([]);
    expect(v.worktreeInstall).toBe(true);
    expect(v.worktreeSetup).toBe("");
  });

  it("keeps a project's own settings when it is edited", () => {
    const p = { id: "p1", number: "P-1", key: "SHOP", name: "Shop", status: "active", defaultBranch: "main", openTasks: 0, doneTasks: 0, updatedAt: 0,
      worktreeCopy: [".env", "vendor/"], worktreeInstall: false, worktreeSetup: "php artisan migrate" } as Project;
    const v = toProjectInput(p);
    expect([v.worktreeCopy, v.worktreeInstall, v.worktreeSetup]).toEqual([[".env", "vendor/"], false, "php artisan migrate"]);
  });

  it("Docs: shows the switch off for a new project, and keeps a project's own switch", () => {
    const html = renderToStaticMarkup(<ProjectDrawer onClose={() => {}} />);
    expect(html).toContain("Off by default. On: the project shows under Docs in the sidebar.");
    expect(html).toMatch(/<input id="p-docs" type="checkbox"\/>Show this project under Docs/);
    expect(toProjectInput(null).devDocs).toBe(false);
    const p = { id: "p1", number: "P-1", key: "SHOP", name: "Shop", status: "active", defaultBranch: "main", openTasks: 0, doneTasks: 0, updatedAt: 0,
      worktreeCopy: [], worktreeInstall: true, devDocs: true } as unknown as Project;
    expect(toProjectInput(p).devDocs).toBe(true);
  });

  it("GA-86: shows Chief may merge off for a new project, and keeps a project's own switch", () => {
    const html = renderToStaticMarkup(<ProjectDrawer onClose={() => {}} />);
    expect(html).toContain("Merging");
    expect(html).toMatch(/<input type="checkbox"\/>Chief may merge/);
    expect(html).toContain("Releases and deploys stay yours");
    expect(toProjectInput(null).leadMayMerge).toBe(false);
    const p = { id: "p1", number: "P-1", key: "SHOP", name: "Shop", status: "active", defaultBranch: "main", openTasks: 0, doneTasks: 0, updatedAt: 0,
      worktreeCopy: [], worktreeInstall: true, leadMayMerge: true } as unknown as Project;
    expect(toProjectInput(p).leadMayMerge).toBe(true);
    expect(toProjectInput({ ...p, leadMayMerge: undefined } as Project).leadMayMerge).toBe(false);
  });

  it("reads the copy list one path per line, skipping blank lines", () => {
    expect(copyPaths(".env\n  node_modules/  \n\n vendor/\ntarget/\n")).toEqual([".env", "node_modules/", "vendor/", "target/"]);
    expect(copyPaths("")).toEqual([]);
  });
});

describe("Docs sections", () => {
  it("are all on for a new project, and only show once Docs is on", () => {
    expect(toProjectInput(null).docsOff).toEqual([]);
    const p = { id: "p1", number: "P-1", key: "SHOP", name: "Shop", status: "active", defaultBranch: "main", openTasks: 0, doneTasks: 0, updatedAt: 0,
      worktreeCopy: [], worktreeInstall: true, devDocs: true, docsOff: ["mcp"] } as unknown as Project;
    expect(toProjectInput(p).docsOff).toEqual(["mcp"]);
    expect(renderToStaticMarkup(<ProjectDrawer onClose={() => {}} />)).not.toContain('id="p-docs-reference"');
  });

  it("show each section of the page, checked unless switched off, and a change gives the new list", () => {
    const html = renderToStaticMarkup(<DocsSections off={["mcp", "api"]} onChange={() => {}} />);
    expect(html).toMatch(/<input id="p-docs-reference" type="checkbox" checked=""\/>Reference/);
    expect(html).toMatch(/<input id="p-docs-api" type="checkbox"\/>API/);
    expect(html).toMatch(/<input id="p-docs-mcp" type="checkbox"\/>MCP/);
    const got: string[][] = [];
    const inputs = (DocsSections({ off: ["mcp"], onChange: (o) => got.push(o) }).props.children as ReactElement<{ children: ReactNode[] }>[])
      .map((label) => (label.props.children as ReactNode[])[0] as ReactElement<{ onChange: (e: { target: { checked: boolean } }) => void }>);
    inputs[0].props.onChange({ target: { checked: false } });
    inputs[5].props.onChange({ target: { checked: true } });
    expect(got).toEqual([["mcp", "reference"], []]);
  });

  it("are the sections Hersir knows, in the page's order", () => {
    const rust = /pub const SECTIONS: \[&str; \d+\] = \[([^\]]*)\]/.exec(devdocsRs)![1].match(/"(\w+)"/g)!.map((s) => s.slice(1, -1));
    expect(DOCS_SECTIONS.map(([key]) => key)).toEqual(rust);
  });
});
