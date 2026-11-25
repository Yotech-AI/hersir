// A project's page in the sidebar's Docs section (#/devdocs/<project id>/<docs address>): the project's docs
// (src/docs/DocsView), read through Hersir's own commands. Open in browser opens the same page in a browser, served by
// Hersir on this computer only (src-tauri/src/docs_server.rs, http://127.0.0.1:<port>/p/<project id>/). Docs is switched on
// per project, in the project form; while it is off the page says so, with the way to turn it on.
import { useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink } from "lucide-react";
import { docsCommit, docsIndex, docsServe, docsSource, getProject } from "../api";
import { DocsView } from "../docs/DocsView";
import { parseDocsPath } from "../docs/route";
import type { DocsHost } from "../docs/types";
import { useData } from "../lib/useData";
import { useDrawer } from "../lib/drawers";

/** Project `id`'s docs page at the docs server's address `base`, at docs address `docs` (its Home without one), in `theme`. */
export function docsUrl(base: string, id: string, docs = "", theme?: "dark" | "light"): string {
  return `${base}/p/${encodeURIComponent(id)}/${theme ? `?theme=${theme}` : ""}#/${docs}`;
}

export function DocsPage({ id, docs = "" }: { id: string; docs?: string }) {
  const { data: p, error } = useData(() => getProject(id), [id]);
  const served = useData(() => docsServe(), []);
  const open = useDrawer();
  const [msg, setMsg] = useState<string | null>(null);
  const base = served.data;
  const host = useMemo<DocsHost>(() => ({
    index: () => docsIndex(id),
    source: (path) => docsSource(id, path),
    commit: (hash) => docsCommit(id, hash),
    agentsUrl: (path) => (base ? `${base}/p/${encodeURIComponent(id)}/${path}` : null),
    openExternal: (url) => { openUrl(url).catch((e) => setMsg(String(e))); },
    copy: (text) => navigator.clipboard.writeText(text),
  }), [id, base]);
  const route = useMemo(() => parseDocsPath(docs), [docs]);
  if (error) return <div className="error-banner">{error}</div>;
  if (!p) return null;
  const openInBrowser = () => {
    setMsg(null);
    // in the browser's own theme: the page there has its own theme switch
    if (base) openUrl(docsUrl(base, p.id, docs)).catch((e) => setMsg(String(e)));
  };
  return (
    <>
      <div className="topbar">
        <div className="crumbs"><span>Docs</span><span className="sep">/</span><b>{p.name}</b></div>
        {p.devDocs && (
          <div className="docs-actions">
            {msg && <span className="faint" role="status">{msg}</span>}
            {served.error && <span className="faint" role="status">{served.error}</span>}
            {base && (
              <button className="btn sm" onClick={openInBrowser} title="The same page in your browser, served by Hersir on this computer only">
                <ExternalLink className="icon" />Open in browser
              </button>
            )}
          </div>
        )}
      </div>
      {!p.devDocs ? (
        <div className="content"><div className="page">
          <div className="empty"><b>Docs is off for this project</b><span>Turn it on in the project's settings, under Docs.</span>
            <button className="btn sm" onClick={() => open({ kind: "project", id: p.id })}>Edit project</button></div>
        </div></div>
      ) : (
        <div className="content docs-page">
          <DocsView host={host} route={route} prefix={`#/devdocs/${encodeURIComponent(p.id)}/`} navigate={(hash) => { window.location.hash = hash; }} />
        </div>
      )}
    </>
  );
}
