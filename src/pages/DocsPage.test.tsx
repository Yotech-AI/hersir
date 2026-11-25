// A project's Docs page (the Docs section): its crumbs; with Docs on, the project's docs drawn in this window (src/docs,
// read through Hersir's own commands) at the address after the project's id, and Open in browser, which opens the same
// page in the browser, served by Hersir on this computer; with Docs off, the way to turn it on. Rendered to HTML on the
// server: the data hooks are replaced so the page gets its data at once (a thrown error is the hook's error), and
// Open in browser's click is tried on the element tree (the page called as a function, its own state replaced).
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { Project } from "../types";

const state = vi.hoisted(() => ({
  project: null as unknown, serve: null as unknown, open: vi.fn(), opened: [] as string[], refuse: null as string | null,
  direct: false, sets: [] as unknown[],
}));
// The page called as a function (state.direct): useState gives the first value, and its setter records into sets.
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = ((init: unknown) => (state.direct ? [init, (v: unknown) => state.sets.push(v)] : R.useState(init))) as typeof R.useState;
  const useMemo = ((f: () => unknown, deps: unknown[]) => (state.direct ? f() : R.useMemo(f, deps))) as typeof R.useMemo;
  return { ...R, default: { ...R, useState, useMemo }, useState, useMemo };
});
vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: (url: string) => (state.refuse ? Promise.reject(state.refuse) : (state.opened.push(url), Promise.resolve())),
}));
vi.mock("../api", async (orig) => ({
  ...(await orig<object>()),
  getProject: () => state.project,
  docsServe: () => { if (state.serve instanceof Error) throw state.serve; return state.serve; },
}));
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    try { return { data: fetch() ?? null, error: null, reload: () => {}, setData: () => {} }; }
    catch (e) { return { data: null, error: e instanceof Error ? e.message : String(e), reload: () => {}, setData: () => {} }; }
  },
}));
vi.mock("../lib/drawers", async (orig) => ({ ...(await orig<object>()), useDrawer: () => state.open }));

const { DocsPage, docsUrl } = await import("./DocsPage");

const kade = (devDocs: boolean) => ({
  id: "kade", name: "Kade portal", key: "KADE", number: "2026-001", status: "active", defaultBranch: "main", openTasks: 0,
  doneTasks: 0, updatedAt: 1, devDocs,
}) as unknown as Project;
const page = (docs?: string) => renderToStaticMarkup(<DocsPage id="kade" docs={docs} />);

type El = ReactElement<{ onClick?: () => void; children?: ReactNode }>;
/** The buttons in an element tree, by their text. */
function buttons(n: ReactNode, found: El[] = []): El[] {
  if (!n || typeof n !== "object") return found;
  if (Array.isArray(n)) { n.forEach((c: ReactNode) => buttons(c, found)); return found; }
  const el = n as El;
  if (el.type === "button") found.push(el);
  buttons(el.props?.children, found);
  return found;
}
const text = (n: ReactNode): string => (typeof n === "string" ? n : Array.isArray(n) ? n.map(text).join("") : "");
/** Clicks the button whose text is `label`, on the page called as a function. */
function click(label: string, docs?: string) {
  state.direct = true;
  try { buttons(DocsPage({ id: "kade", docs })).find((b) => text(b.props.children) === label)!.props.onClick!(); }
  finally { state.direct = false; }
}
const settle = () => new Promise((r) => setTimeout(r, 0));

beforeEach(() => {
  state.open.mockReset();
  state.project = kade(true);
  state.serve = "http://127.0.0.1:4380";
  state.opened = [];
  state.refuse = null;
  state.sets = [];
});

describe("the Docs page", () => {
  it("names the section and the project in its crumbs", () => {
    expect(page()).toContain('<div class="crumbs"><span>Docs</span><span class="sep">/</span><b>Kade portal</b></div>');
  });
  it("draws the project's docs in this window, not in a frame, while they are read", () => {
    const html = page("api/post-users");
    expect(html).toContain('<div class="content docs-page"><div class="dd-shell"><div class="dd-skeleton" aria-busy="true" aria-label="Reading the docs">');
    expect(html).not.toContain("<iframe");
  });
  it("says Docs is off, with Edit project, which opens the project's form", () => {
    state.project = kade(false);
    expect(page()).toContain("<b>Docs is off for this project</b>");
    expect(page()).not.toContain("docs-page");
    expect(page()).not.toContain("Open in browser");
    click("Edit project");
    expect(state.open).toHaveBeenCalledWith({ kind: "project", id: "kade" });
  });
  it("still draws the docs when Hersir can't serve them to a browser, and says why where Open in browser would be", () => {
    state.serve = new Error("Hersir couldn't find a free port for the docs from 4380 to 4399");
    expect(page()).toContain('<span class="faint" role="status">Hersir couldn&#x27;t find a free port for the docs from 4380 to 4399</span>');
    expect(page()).not.toContain("Open in browser");
    expect(page()).toContain("docs-page");
  });
  it("shows nothing while the project loads", () => {
    state.project = null;
    expect(page()).toBe("");
  });
});

describe("Open in browser", () => {
  it("opens the same page in the browser, in the browser's own theme", async () => {
    click("Open in browser", "decisions/0001");
    await settle();
    expect(state.opened).toEqual(["http://127.0.0.1:4380/p/kade/#/decisions/0001"]);
  });
  it("says why when the browser can't be opened", async () => {
    state.refuse = "No browser to open";
    click("Open in browser");
    await settle();
    expect(state.sets).toEqual([null, "No browser to open"]);
  });
});

describe("docsUrl", () => {
  it("puts the project's id in the address as one part", () => {
    expect(docsUrl("http://127.0.0.1:4380", "a b/c", "guides", "light")).toBe("http://127.0.0.1:4380/p/a%20b%2Fc/?theme=light#/guides");
  });
});
