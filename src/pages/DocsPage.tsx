// A project's page in the sidebar's Docs section (#/devdocs/<project id>). Docs is switched on per project, in the project
// form; while it is off the page says so, with the way to turn it on.
import { getProject } from "../api";
import { useData } from "../lib/useData";
import { useDrawer } from "../lib/drawers";

export function DocsPage({ id }: { id: string }) {
  const { data: p, error } = useData(() => getProject(id), [id]);
  const open = useDrawer();
  if (error) return <div className="error-banner">{error}</div>;
  if (!p) return null;
  return (
    <>
      <div className="topbar"><div className="crumbs"><span>Docs</span><span className="sep">/</span><b>{p.name}</b></div></div>
      <div className="content">
        <div className="page">
          {p.devDocs
            ? <div className="empty"><b>No docs yet</b><span>The agents' changelog and the Chief's decisions for this project will show here.</span></div>
            : <div className="empty"><b>Docs is off for this project</b><span>Turn it on in the project's settings, under Docs.</span>
                <button className="btn sm" onClick={() => open({ kind: "project", id: p.id })}>Edit project</button></div>}
        </div>
      </div>
    </>
  );
}
