// Docs in a browser: the page Hersir serves (src-tauri/src/docs_server.rs). #/devdocs/<project>/<tab> is a project's
// Docs page as the app shows it; #/ and #/devdocs, the projects with Docs on. Everything else a Docs page links to (a
// card, a Memory note) is only in the app.
import { listProjects } from "../api";
import { DocsPage } from "../pages/DocsPage";
import { DOCS_TABS, href, useRoute } from "../router";
import { useData } from "../lib/useData";

export function DocsWeb() {
  const route = useRoute();
  if (route.page === "devdocs" && route.id) {
    return <DocsPage key={route.id} id={route.id} tab={DOCS_TABS.find((t) => t === route.view)} web />;
  }
  // #/ is the router's default page (tasks), and #/devdocs its list (projects)
  if (route.page === "tasks" || route.page === "projects") return <Projects />;
  return (
    <div className="content"><div className="page"><div className="empty">
      <b>This is in the Hersir app</b><span>A card, a Memory note and the rest open in Hersir itself: this page shows Docs only.</span>
      <a className="btn sm" href="#/">Every project's docs</a>
    </div></div></div>
  );
}

/** The projects with Docs on, each linking to its Docs page. */
export function Projects() {
  const { data, error } = useData(() => listProjects(), []);
  if (error) return <div className="error-banner">{error}</div>;
  if (!data) return null;
  return (
    <>
      <div className="topbar"><div className="crumbs"><b>Docs</b></div></div>
      <div className="content"><div className="page">
        {data.length === 0
          ? <div className="empty"><b>No project has Docs on</b><span>Turn it on in Hersir: a project's Edit, under Docs.</span></div>
          : <ul className="run-commits docs-projects">{data.map((p) => <li key={p.id}><a href={href({ page: "devdocs", id: p.id })}>{p.name}</a></li>)}</ul>}
      </div></div>
    </>
  );
}
