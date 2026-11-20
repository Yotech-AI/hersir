// Docs in a browser: docs.html, which Hersir serves on 127.0.0.1 (src-tauri/src/docs_server.rs). Tauri's mockIPC carries
// the page's calls to that server as POST /api/<command> with JSON, as browser mode's fake backend answers its own
// (src/browser/main.ts); a refused call's text is the error the page shows. Events stay in the page, and a link in a
// guide or a note opens in a new tab: the browser opens it, not Hersir.
import React from "react";
import ReactDOM from "react-dom/client";
import { mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { applyAppearance, getAppearance } from "../lib/appearance";
import { DocsWeb } from "./DocsWeb";
import "../styles/fonts.css";
import "../styles/tokens.css";
import "../styles/components.css";
import "../styles/app.css";
import "../styles/changes.css";

mockWindows("main");
mockIPC(async (cmd, args) => {
  if (cmd === "plugin:opener|open_url") {
    const url = String((args as { url?: unknown } | undefined)?.url ?? "");
    if (/^(https?|mailto):/i.test(url)) window.open(url, "_blank", "noopener,noreferrer");
    return null;
  }
  const r = await fetch(`/api/${encodeURIComponent(cmd)}`, {
    method: "POST", headers: { "Content-Type": "application/json" }, body: JSON.stringify(args ?? {}),
  });
  const text = await r.text();
  if (!r.ok) throw text;
  return JSON.parse(text);
}, { shouldMockEvents: true });
applyAppearance(document.documentElement, getAppearance());
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(<React.StrictMode><DocsWeb /></React.StrictMode>);
