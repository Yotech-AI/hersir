// docs.html on its own (src/docsweb/): a project's Docs page as Hersir serves it to a browser (src-tauri/src/
// docs_server.rs). A build of its own, into dist/docs-web/ after the app's, so it shares no chunk with the app: the
// app's start code is never in it. npm run build makes both; Tauri packs dist/ with the app.
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: { target: "safari16", outDir: "dist/docs-web", emptyOutDir: true, chunkSizeWarningLimit: 4000, rolldownOptions: { input: "docs.html" } },
});
