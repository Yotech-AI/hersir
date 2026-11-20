// Docs in a browser (src/docsweb/): the page Hersir serves shows a project's Docs page at #/devdocs/<id>/<tab>, without
// Open in browser; at #/ and #/devdocs, the projects Hersir lists for it (those with Docs on: docs_server.rs), each
// linking to its Docs page; anything else, that it opens in the app. Rendered to HTML on the server, with the address in
// a stubbed window and the data hooks answering at once.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { Project } from "../types";

const state = vi.hoisted(() => ({ projects: [] as unknown }));
vi.mock("../api", async (orig) => ({
  ...(await orig<object>()),
  listProjects: () => state.projects,
  getProject: () => ({ id: "kade", name: "Kade portal", key: "KADE", devDocs: true }),
  docsApi: () => [],
}));
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    try { return { data: fetch() ?? null, error: null, reload: () => {}, setData: () => {} }; }
    catch (e) { return { data: null, error: String(e), reload: () => {}, setData: () => {} }; }
  },
}));

const { DocsWeb } = await import("./DocsWeb");
const at = (hash: string) => { vi.stubGlobal("window", { location: { hash } }); return renderToStaticMarkup(<DocsWeb />); };

beforeEach(() => { state.projects = []; });
afterEach(() => { vi.unstubAllGlobals(); });

describe("Docs in a browser", () => {
  it("shows a project's Docs page from its address, without Open in browser", () => {
    const html = at("#/devdocs/kade/api");
    expect(html).toContain("<b>Kade portal</b>");
    expect(html).toMatch(/aria-selected="true" href="#\/devdocs\/kade\/api"/);
    expect(html).toContain("<b>No routes found</b>");
    expect(html).not.toContain("Open in browser");
  });
  it("lists the projects Hersir serves docs for, each linking to its Docs page", () => {
    state.projects = [{ id: "kade", name: "Kade portal" }, { id: "pearl", name: "Pearl" }] as Project[];
    for (const hash of ["", "#/", "#/devdocs"]) {
      const html = at(hash);
      expect(html).toContain('<a href="#/devdocs/kade">Kade portal</a>');
      expect(html).toContain('<a href="#/devdocs/pearl">Pearl</a>');
    }
  });
  it("says how to turn Docs on while no project has it", () => {
    expect(at("#/")).toContain("<b>No project has Docs on</b><span>Turn it on in Hersir: a project&#x27;s Edit, under Docs.</span>");
  });
  it("says a card, a Memory note and the rest open in the app", () => {
    for (const hash of ["#/task/t1", "#/memory/n1", "#/project/kade", "#/settings"]) {
      expect(at(hash)).toContain("<b>This is in the Hersir app</b>");
    }
  });
});
