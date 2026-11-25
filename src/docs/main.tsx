// A project's Docs page in a browser (docs.html), built into one HTML file by `vite build --mode docs-web`: every script,
// style and font inside it. Hersir embeds that file and its docs server serves it at /t/<key>/p/<project id>/, with the
// project's index.json, source/<path>.json and commits/<hash>.json next to it (src-tauri/src/docs_server.rs): every
// address here is relative, so it keeps the key, new at each start. Its theme: the one Hersir's window sent (?theme=),
// else the one picked here last, else the system's.
import React, { useEffect, useMemo, useState } from "react";
import ReactDOM from "react-dom/client";
import { loadFonts } from "../widget/fonts";
import "../styles/tokens.css";
import "../styles/components.css";
import "../styles/app.css";
import "../styles/docs.css";
import { DocsView } from "./DocsView";
import { parseDocsPath } from "./route";
import type { CommitPatches, DocsHost, Index, SourceFile } from "./types";

const THEME_KEY = "gizai.docs.theme";

function startTheme(): "dark" | "light" {
  const sent = new URLSearchParams(location.search).get("theme");
  if (sent === "dark" || sent === "light") return sent;
  try {
    const kept = localStorage.getItem(THEME_KEY);
    if (kept === "dark" || kept === "light") return kept;
  } catch {
    // a browser that keeps nothing for this address: the system's theme
  }
  return matchMedia("(prefers-color-scheme: light)").matches ? "light" : "dark";
}

async function json<T>(path: string): Promise<T> {
  const res = await fetch(path);
  if (!res.ok) throw new Error(`${path} answered ${res.status}: ${(await res.text()).slice(0, 300)}`);
  return (await res.json()) as T;
}

const encodePath = (path: string) => path.split("/").map(encodeURIComponent).join("/");

function App() {
  const [hash, setHash] = useState(location.hash);
  const [theme, setTheme] = useState(startTheme);
  useEffect(() => {
    const onHash = () => setHash(location.hash);
    window.addEventListener("hashchange", onHash);
    return () => window.removeEventListener("hashchange", onHash);
  }, []);
  useEffect(() => {
    document.documentElement.dataset.theme = theme;
  }, [theme]);
  const host = useMemo<DocsHost>(() => ({
    index: async () => {
      const index = await json<Index>("index.json");
      document.title = index.project.title;
      return index;
    },
    source: (path) => json<SourceFile>(`source/${encodePath(path)}.json`).catch(() => ({ path, lines: [] })),
    commit: (hash) => json<CommitPatches>(`commits/${encodeURIComponent(hash)}.json`).catch(() => null),
    agentsUrl: (path) => path,
    openExternal: (url) => { window.open(url, "_blank", "noopener,noreferrer"); },
    copy: (text) => navigator.clipboard.writeText(text),
    shareable: (href) => `${location.origin}${location.pathname}${href}`,
    themeSwitch: {
      theme,
      toggle: () => {
        const next = theme === "dark" ? "light" : "dark";
        setTheme(next);
        try {
          localStorage.setItem(THEME_KEY, next);
        } catch {
          // kept for this visit only
        }
      },
    },
  }), [theme]);
  const route = useMemo(() => parseDocsPath(hash.replace(/^#\/?/, "")), [hash]);
  return <DocsView host={host} route={route} prefix="#/" navigate={(to) => { location.hash = to; }} browser />;
}

loadFonts();
ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);
