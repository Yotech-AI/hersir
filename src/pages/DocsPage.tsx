// A project's page in the sidebar's Docs section (#/devdocs/<project id>/<tab>): the project's docs as the Yodocs Figma
// draws them. The page is yodocs' viewer, which Hersir serves on this computer (src-tauri/src/docs_server.rs, at
// http://127.0.0.1:<port>/p/<project id>/); here it shows in a frame, in this window's theme, and Open in browser opens
// the same page in the browser. Docs is switched on per project, in the project form; while it is off the page says
// so, with the way to turn it on.
import { useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";
import { docsServe, getProject } from "../api";
import type { DocsTab } from "../router";
import { useData } from "../lib/useData";
import { useDrawer } from "../lib/drawers";
import { getAppearance } from "../lib/appearance";

/** Project `id`'s docs page at the docs server's address `base`, on `tab` (its Home without one), in `theme`. */
export function docsUrl(base: string, id: string, tab?: DocsTab, theme?: "dark" | "light"): string {
  return `${base}/p/${encodeURIComponent(id)}/${theme ? `?theme=${theme}` : ""}#/${tab ?? ""}`;
}

export function DocsPage({ id, tab }: { id: string; tab?: DocsTab }) {
  const { data: p, error } = useData(() => getProject(id), [id]);
  const served = useData(() => docsServe(), []);
  const open = useDrawer();
  const [msg, setMsg] = useState<string | null>(null);
  if (error) return <div className="error-banner">{error}</div>;
  if (!p) return null;
  const base = served.data;
  const openInBrowser = () => {
    setMsg(null);
    if (base) openUrl(docsUrl(base, p.id, tab)).catch((e) => setMsg(String(e)));
  };
  return (
    <>
      <div className="topbar">
        <div className="crumbs"><span>Docs</span><span className="sep">/</span><b>{p.name}</b></div>
        {p.devDocs && base && (
          <div className="docs-actions">
            {msg && <span className="faint" role="status">{msg}</span>}
            <button className="btn sm" onClick={openInBrowser} title="The same page in your browser, served by Hersir on this computer only">
              <ExternalLink className="icon" />Open in browser
            </button>
          </div>
        )}
      </div>
      {!p.devDocs ? (
        <div className="content"><div className="page">
          <div className="empty"><b>Docs is off for this project</b><span>Turn it on in the project's settings, under Docs.</span>
            <button className="btn sm" onClick={() => open({ kind: "project", id: p.id })}>Edit project</button></div>
        </div></div>
      ) : served.error ? (
        <div className="error-banner">{served.error}</div>
      ) : base ? (
        <div className="content docs-frame">
          <iframe title={`${p.name} dev docs`} src={docsUrl(base, p.id, tab, getAppearance().theme)}
            sandbox="allow-scripts allow-same-origin allow-popups" allow="clipboard-write" />
        </div>
      ) : null}
    </>
  );
}
