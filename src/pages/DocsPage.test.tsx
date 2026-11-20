// The Docs page of one project (the Docs section): its crumbs, and what it says with Docs on and off. Rendered to HTML on
// the server: the data hooks are replaced so the page gets its project at once, and a button's click is tried on the
// element tree (the page called as a function, its hooks being the replaced ones).
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { Project } from "../types";

const state = vi.hoisted(() => ({ project: null as unknown, open: vi.fn() }));
vi.mock("../api", async (orig) => ({ ...(await orig<object>()), getProject: () => state.project }));
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => ({ data: fetch(), error: null, reload: () => {}, setData: () => {} }),
}));
vi.mock("../lib/drawers", async (orig) => ({ ...(await orig<object>()), useDrawer: () => state.open }));

const { DocsPage } = await import("./DocsPage");

const kade = (devDocs: boolean) => ({
  id: "kade", name: "Kade portal", key: "KADE", number: "2026-001", status: "active", defaultBranch: "main", openTasks: 0,
  doneTasks: 0, updatedAt: 1, devDocs,
}) as unknown as Project;
const page = () => renderToStaticMarkup(<DocsPage id="kade" />);

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

beforeEach(() => { state.open.mockReset(); });

describe("the Docs page", () => {
  it("names the section and the project in its crumbs", () => {
    state.project = kade(true);
    expect(page()).toContain('<div class="crumbs"><span>Docs</span><span class="sep">/</span><b>Kade portal</b></div>');
  });
  it("says there are no docs yet while Docs is on", () => {
    state.project = kade(true);
    expect(page()).toContain("<b>No docs yet</b>");
    expect(page()).not.toContain("Docs is off");
  });
  it("says Docs is off, with Edit project, which opens the project's form", () => {
    state.project = kade(false);
    expect(page()).toContain("<b>Docs is off for this project</b>");
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
