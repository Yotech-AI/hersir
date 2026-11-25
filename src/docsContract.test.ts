// Docs: the UI's calls and the page's addresses match the Rust side, read as text (Vite's ?raw) as contract C1's are
// (changesContract.test.ts): the four commands are registered and called with the names Rust takes; the browser's page is
// the one npm run build makes and the docs server serves, reading its data next to it; nothing frames it any more; and
// the page is TypeScript like the rest of src/.
import { describe, expect, it, vi } from "vitest";
import libRs from "../src-tauri/src/lib.rs?raw";
import devdocsRs from "../src-tauri/src/devdocs.rs?raw";
import serverRs from "../src-tauri/src/docs_server.rs?raw";
import buildRs from "../src-tauri/build.rs?raw";
import tauriConf from "../src-tauri/tauri.conf.json?raw";
import packageJson from "../package.json?raw";
import mainTsx from "./docs/main.tsx?raw";

const invoked: [string, unknown][] = [];
vi.mock("@tauri-apps/api/core", () => ({ invoke: (cmd: string, args?: unknown) => { invoked.push([cmd, args]); return Promise.resolve(null); } }));
vi.mock("@tauri-apps/api/event", () => ({ listen: () => Promise.resolve(() => {}) }));

const api = await import("./api");
// as the other file-reading tests do (lib/codeTheme.test.ts): no Node types in the app's TypeScript
const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));

describe("the Docs page's contract", () => {
  it("reads through four commands registered with Tauri, with the argument names Rust takes", async () => {
    invoked.length = 0;
    await api.docsServe();
    await api.docsIndex("p1");
    await api.docsSource("p1", "app/User.php");
    await api.docsCommit("p1", "abc1234");
    expect(invoked).toEqual([["docs_serve", undefined], ["docs_index", { projectId: "p1" }],
      ["docs_source", { projectId: "p1", path: "app/User.php" }], ["docs_commit", { projectId: "p1", hash: "abc1234" }]]);
    for (const cmd of ["docs_server::docs_serve", "devdocs::docs_index", "devdocs::docs_source", "devdocs::docs_commit"]) expect(libRs).toContain(cmd);
    expect(devdocsRs).toMatch(/pub async fn docs_index\(st: tauri::State<'_, AppState>, project_id: String\)/);
    expect(devdocsRs).toMatch(/pub async fn docs_source\(st: tauri::State<'_, AppState>, project_id: String, path: String\)/);
    expect(devdocsRs).toMatch(/pub async fn docs_commit\(st: tauri::State<'_, AppState>, project_id: String, hash: String\)/);
  });

  it("serves the page npm run build makes, which reads its data next to it", () => {
    expect(JSON.parse(packageJson).scripts.build).toContain("vite build --mode docs-web");
    expect(buildRs).toContain('built("docs-web", "docs.html"');
    expect(serverRs).toContain('include_str!(concat!(env!("OUT_DIR"), "/docs.html"))');
    for (const file of ['"index.json"', "`source/", "`commits/"]) expect(mainTsx).toContain(file);
    for (const route of ['file == "index.json"', 'strip_prefix("source/")', 'strip_prefix("commits/")']) expect(serverRs).toContain(route);
  });

  it("is framed by nothing: the app draws it itself", () => {
    const csp: string = JSON.parse(tauriConf).app.security.csp;
    expect(csp).not.toContain("127.0.0.1");
    expect(serverRs).toContain("frame-ancestors 'none'");
  });

  it("is TypeScript, like the rest of src/", () => {
    const files: string[] = fs.readdirSync(new URL("./docs", import.meta.url));
    expect(files.filter((f) => f.endsWith(".js"))).toEqual([]);
  });
});
