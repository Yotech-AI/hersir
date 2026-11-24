import { readFile } from "node:fs";
import { fileURLToPath } from "node:url";
import { defineConfig, type Plugin } from "vite";
import react from "@vitejs/plugin-react";
import { viteSingleFile } from "vite-plugin-singlefile";

const watch = { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] };

/** GA-185: in the browser mode the address without a page is browser.html, GizAi's screens with the fake backend. */
const browserPage = (): Plugin => ({
  name: "gizai-browser-page",
  configureServer(server) {
    server.middlewares.use((req, _res, next) => {
      if (req.url === "/" || req.url?.startsWith("/?")) req.url = `/browser.html${req.url.slice(1)}`;
      next();
    });
  },
});

/** GA-343: the board view for Claude Desktop, as `npm run build` makes it (`vite build --mode mcp-app`). */
const MCP_APP = fileURLToPath(new URL("src-tauri/mcp-app/mcp-app.html", import.meta.url));

/** GA-343: in the mcp-host mode the address without a page is mcp-host.html, the stand-in host, and /built/mcp-app.html
 *  is the board view's one file as built, read again at each request. */
const mcpHostPage = (): Plugin => ({
  name: "hersir-mcp-host-page",
  configureServer(server) {
    server.middlewares.use((req, res, next) => {
      if (req.url === "/" || req.url?.startsWith("/?")) req.url = `/mcp-host.html${req.url.slice(1)}`;
      if (req.url?.split("?")[0] !== "/built/mcp-app.html") return next();
      readFile(MCP_APP, (err, html) => {
        res.setHeader("Cache-Control", "no-store");
        if (err) {
          res.statusCode = 404;
          res.setHeader("Content-Type", "text/plain; charset=utf-8");
          res.end("The board view isn't built yet: run npm run build:mcp-app (or npm run build).");
          return;
        }
        res.setHeader("Content-Type", "text/html; charset=utf-8");
        res.end(html);
      });
    });
  },
});

/** A dev server for a run's preview: 127.0.0.1 at the port in HERSIR_PREVIEW_PORT (GIZAI_PREVIEW_PORT before 0.17.0). */
const preview = (port: number) => ({ host: "127.0.0.1", port: Number(process.env.HERSIR_PREVIEW_PORT || process.env.GIZAI_PREVIEW_PORT) || port,
  strictPort: true, watch });

// Tauri expects a fixed dev port; production builds are served by Tauri's custom protocol.
// `vite --mode browser` (npm run browser, GA-185): GizAi's screens in a plain browser with a fake backend (browser.html,
// src/browser/), on 127.0.0.1 at the port a run's preview gets in HERSIR_PREVIEW_PORT (or GIZAI_PREVIEW_PORT, the name before
// 0.17.0; 1430 without either). The build below
// has only index.html, so the app never contains it.
// GA-343: `vite build --mode mcp-app` (in npm run build) makes the board view for Claude Desktop, mcp-app.html and
// src/widget, into one HTML file in src-tauri/mcp-app, which Hersir embeds (src-tauri/build.rs). `vite --mode mcp-host`
// (npm run mcp-host) serves the stand-in host page for development and QA (mcp-host.html, src/widget-host/), at the
// preview port (1431 without one); neither build has it.
export default defineConfig(({ mode }) => {
  if (mode === "mcp-app") {
    return {
      plugins: [react(), viteSingleFile()],
      clearScreen: false,
      build: {
        // Claude Desktop draws a view in its Chromium; one file, every script, style and font inlined. No modulepreload
        // polyfill: one file has nothing to preload, and the view makes no network call (that polyfill fetches).
        target: "es2022", outDir: "src-tauri/mcp-app", emptyOutDir: false, chunkSizeWarningLimit: 2000, modulePreload: false,
        rolldownOptions: { input: "mcp-app.html" },
      },
    };
  }
  if (mode === "mcp-host") {
    return {
      plugins: [mcpHostPage()],
      cacheDir: "node_modules/.vite-mcp-host",
      clearScreen: false,
      server: preview(1431),
    };
  }
  return {
    plugins: mode === "browser" ? [react(), browserPage()] : [react()],
    // its own prebundled dependencies, apart from the app's dev server (npm run dev), so the two never mix
    ...(mode === "browser" ? { cacheDir: "node_modules/.vite-browser" } : {}),
    clearScreen: false,
    server: mode === "browser" ? preview(1430) : { port: 1420, strictPort: true, watch },
    // Desktop app: local files, so a large chunk costs nothing over the network; what counts is the code a start parses.
    // GA-178: the pages but the board and the self-test load when first needed (App.tsx); GA-190: the drawers too
    // (lib/drawers.tsx), and with them the Markdown editor and CodeMirror. What a start needs stays in one file instead of
    // many small ones.
    build: {
      target: "safari16", outDir: "dist", chunkSizeWarningLimit: 2000,
      rolldownOptions: { output: { codeSplitting: { groups: [
        { name: "index", tags: ["$initial"], priority: 1 },
      ] } } },
    },
  };
});
