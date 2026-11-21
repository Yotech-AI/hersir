// A project's Docs page (the Docs section): its crumbs; with Docs on, the project's docs page (yodocs' viewer, which
// Hersir serves on this computer) in a frame, in this window's theme, on the section in the address, and Open in
// browser, which opens the same page in the browser; with Docs off, the way to turn it on. Rendered to HTML on the
// server: the data hooks are replaced so the page gets its data at once (a thrown error is the hook's error), and
// Open in browser's click is tried on the element tree (the page called as a function, its own state replaced).
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import type { DocsTab } from "../router";
import type { Project } from "../types";

const state = vi.hoisted(() => ({
  project: null as unknown, serve: null as unknown, open: vi.fn(), opened: [] as string[], refuse: null as string | null,
  direct: false, sets: [] as unknown[],
}));
// The page called as a function (state.direct): useState gives the first value, and its setter records into sets.
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useState = ((init: unknown) => (state.direct ? [init, (v: unknown) => state.sets.push(v)] : R.useState(init))) as typeof R.useState;
  return { ...R, default: { ...R, useState }, useState };
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
const page = (tab?: DocsTab) => renderToStaticMarkup(<DocsPage id="kade" tab={tab} />);

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
function click(label: string, tab?: DocsTab) {
  state.direct = true;
  try { buttons(DocsPage({ id: "kade", tab })).find((b) => text(b.props.children) === label)!.props.onClick!(); }
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
  it("shows the project's docs page in a frame, in this window's theme, on the section in the address", () => {
    expect(page("api")).toContain('<iframe title="Kade portal dev docs" src="http://127.0.0.1:4380/p/kade/?theme=dark#/api"'
      + ' sandbox="allow-scripts allow-same-origin allow-popups" allow="clipboard-write"></iframe>');
    expect(page()).toContain('src="http://127.0.0.1:4380/p/kade/?theme=dark#/"');
  });
  it("says Docs is off, with Edit project, which opens the project's form", () => {
    state.project = kade(false);
    expect(page()).toContain("<b>Docs is off for this project</b>");
    expect(page()).not.toContain("<iframe");
    expect(page()).not.toContain("Open in browser");
    click("Edit project");
    expect(state.open).toHaveBeenCalledWith({ kind: "project", id: "kade" });
  });
  it("says why when Hersir can't serve the docs", () => {
    state.serve = new Error("Hersir couldn't find a free port for the docs from 4380 to 4399");
    expect(page()).toContain('<div class="error-banner">Hersir couldn&#x27;t find a free port for the docs from 4380 to 4399</div>');
    expect(page()).not.toContain("<iframe");
  });
  it("shows nothing while the project loads", () => {
    state.project = null;
    expect(page()).toBe("");
  });
});

describe("Open in browser", () => {
  it("opens the same page in the browser, in the browser's own theme", async () => {
    click("Open in browser", "decisions");
    await settle();
    expect(state.opened).toEqual(["http://127.0.0.1:4380/p/kade/#/decisions"]);
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
