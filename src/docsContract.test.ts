// Docs: the UI's call and the page's address match the Rust side, read as text (Vite's ?raw) as contract C1's are
// (changesContract.test.ts): docs_serve is registered; the page is served under /p/<id>/ and takes ?theme=, which the
// app sends; each section the app links to is one the page has; the app may frame the page and the page lets it.
import { describe, expect, it, vi } from "vitest";
import libRs from "../src-tauri/src/lib.rs?raw";
import serverRs from "../src-tauri/src/docs_server.rs?raw";
import tauriConf from "../src-tauri/tauri.conf.json?raw";
import themeJs from "../crates/gizai-docs/viewer/js/theme.js?raw";
import stateJs from "../crates/gizai-docs/viewer/js/state.js?raw";
import mainJs from "../crates/gizai-docs/viewer/js/main.js?raw";
import { DOCS_TABS } from "./router";

const invoked: [string, unknown][] = [];
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => { invoked.push([cmd, args]); return Promise.resolve(null); } }));
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

const api = await import("./api");

describe("the Docs page's contract", () => {
  it("starts the server with docs_serve, registered with Tauri", async () => {
    invoked.length = 0;
    await api.docsServe();
    expect(invoked).toEqual([["docs_serve", undefined]]);
    expect(libRs).toContain("docs_server::docs_serve");
    expect(serverRs).toMatch(/pub async fn docs_serve\(/);
  });

  it("finds the page under /p/<id>/, in the theme the app sends", () => {
    expect(serverRs).toContain('path.strip_prefix("/p/")');
    expect(themeJs).toContain("new URLSearchParams(location.search).get('theme')");
  });

  it("links only to sections the page has", () => {
    for (const tab of DOCS_TABS) expect(`${stateJs}\n${mainJs}`).toMatch(new RegExp(`'${tab}'`));
  });

  it("may frame the page, which only Hersir's window may frame", () => {
    const csp: string = JSON.parse(tauriConf).app.security.csp;
    expect(csp).toMatch(/frame-src [^;]*http:\/\/127\.0\.0\.1:\*/);
    expect(serverRs).toMatch(/frame-ancestors tauri:\/\/localhost http:\/\/tauri\.localhost https:\/\/tauri\.localhost/);
  });
});
