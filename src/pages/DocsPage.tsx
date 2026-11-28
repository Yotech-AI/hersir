// A project's page in the sidebar's Docs section (#/devdocs/<project id>/<docs address>): the project's docs
// (src/docs/DocsView), read through Hersir's own commands. Open in browser opens the same page in a browser, served by
// Hersir on this computer only (src-tauri/src/docs_server.rs, http://127.0.0.1:<port>/t/<key>/p/<project id>/: the
// address docsServe gives carries the key, and so do Open in browser and the links for agents). Build docs starts a chat
// with the Chief that asks it to bring the project's docs up to date. Docs is switched on per project, in the project
// form; while it is off the page says so, with the way to turn it on.
import { useMemo, useState } from "react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { ExternalLink, MessagesSquare } from "lucide-react";
import { docsCommit, docsIndex, docsServe, docsSource, getProject, sendChat } from "../api";
import { go } from "../router";
import { DocsView } from "../docs/DocsView";
import { parseDocsPath } from "../docs/route";
import type { DocsHost } from "../docs/types";
import { useData } from "../lib/useData";
import { useDrawer } from "../lib/drawers";

/** Project `id`'s docs page at the docs server's address `base`, at docs address `docs` (its Home without one), in `theme`. */
export function docsUrl(base: string, id: string, docs = "", theme?: "dark" | "light"): string {
  return `${base}/p/${encodeURIComponent(id)}/${theme ? `?theme=${theme}` : ""}#/${docs}`;
}

/** What Build docs asks the Chief: to tidy the project's decisions into the forms the Docs page reads, and to plan cards
 *  for the docs in the repository it can't change itself. */
export function buildDocsMessage(name: string, key: string): string {
  return `Please bring the docs of ${name} (${key}) up to date. Its Docs page in Hersir reads them from the repository and from what Hersir keeps.

1. Decisions: go through the decisions about ${key}, in its repository (docs/decisions), your records, and the decision notes in Memory (Decisions/). The Docs page shows each one as a short title with its date and status, the latest first.
   - In a Memory note, keep one line per decision, its date first: \`- 2026-11-03: Short title: what was decided (CARD-1)\`. Split a line that holds more than one decision.
   - Record a decision that needs its context and the alternatives weighed with record_decision instead.
   - A record in the repository needs a \`# Title\`, a \`Status:\` line (proposed, accepted, rejected, superseded or deprecated) and a \`Date: YYYY-MM-DD\` line. If some lack them, make one card for an agent to fix them all.
   - Write those labels and the status in English and every date as YYYY-MM-DD, whatever language the rest is in, so the Docs page can read them.
2. Guides: if the README or the docs folder no longer match the code, make a card to update them.

Then say what you changed and which cards you made. Ask me before you start an agent.`;
}

export function DocsPage({ id, docs = "" }: { id: string; docs?: string }) {
  const { data: p, error } = useData(() => getProject(id), [id]);
  const served = useData(() => docsServe(), []);
  const open = useDrawer();
  const [msg, setMsg] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
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
  const buildDocs = () => {
    setMsg(null);
    setAsking(true);
    sendChat(null, buildDocsMessage(p.name, p.key)).then((chat) => go({ page: "chat", id: chat }), (e) => { setMsg(String(e)); setAsking(false); });
  };
  return (
    <>
      <div className="topbar">
        <div className="crumbs"><span>Docs</span><span className="sep">/</span><b>{p.name}</b></div>
        {p.devDocs && (
          <div className="docs-actions">
            {msg && <span className="faint" role="status">{msg}</span>}
            {served.error && <span className="faint" role="status">{served.error}</span>}
            <button className="btn sm" onClick={buildDocs} disabled={asking} title="Asks the Chief, in a new chat, to bring this project's docs up to date">
              <MessagesSquare className="icon" />Build docs
            </button>
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
