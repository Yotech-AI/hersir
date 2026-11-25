// A project's Docs page, laid out as the Yodocs Figma lays it out and drawn in Hersir's own type and colours: a top bar
// (the project, its sections, search, the commit), the Reference's sidebar on Home and the Reference, and the section.
// The same page shows in Hersir's window (pages/DocsPage.tsx) and in a browser (docs/main.tsx); the host says where its
// data comes from and how a link leaves the page.
import { useEffect, useRef, useState, type ReactNode } from "react";
import { BookOpen, Code, GitCommitHorizontal, Globe, History, House, Inbox, Menu, Moon, Plug, Scale, Search, Sun, TriangleAlert } from "lucide-react";
import { isOff, offKey, tabs } from "./derive";
import { fresh, type DocsRoute, type Section } from "./route";
import type { DocsHost, Index } from "./types";
import { ApiPage, Changelog, Commits, Decisions, Guides, Mcp } from "./Sections";
import { EntryPage, Home, ReferenceList } from "./Reference";
import { SearchDialog } from "./Search";
import { Sidebar } from "./Sidebar";
import { DocsProvider, Empty, makeLinks, useDocs, useLoad } from "./ui";

const TAB_ICON: Record<Section, ReactNode> = {
  home: <House className="icon" />, guides: <BookOpen className="icon" />, reference: <Code className="icon" />, api: <Globe className="icon" />,
  changelog: <History className="icon" />, decisions: <Scale className="icon" />, mcp: <Plug className="icon" />, commits: <GitCommitHorizontal className="icon" />,
};

export type DocsViewProps = {
  host: DocsHost;
  route: DocsRoute;
  /** What comes before a docs path in this host's addresses: #/ in a browser, #/devdocs/<project id>/ in Hersir. */
  prefix: string;
  navigate: (hash: string) => void;
  /** ⌘K opens the search in a browser; in Hersir's window it is the command palette. */
  browser?: boolean;
};

export function DocsView({ host, route, prefix, navigate, browser = false }: DocsViewProps) {
  const loaded = useLoad(host.index, "index");
  if (loaded.error) {
    return (
      <div className="dd-shell"><Empty title="The docs couldn't be read" icon={<TriangleAlert className="icon" />}>
        <p className="muted">Hersir couldn't read this project's docs, so there is nothing to show. It said why:</p>
        <p className="sm muted mono">{loaded.error}</p>
      </Empty></div>
    );
  }
  if (!loaded.data) return <div className="dd-shell"><div className="dd-skeleton" aria-busy="true" aria-label="Reading the docs" /></div>;
  return <DocsBody index={loaded.data} host={host} route={route} prefix={prefix} navigate={navigate} browser={browser} />;
}

/** The page once its index is in. */
export function DocsBody({ index, host, route, prefix, navigate, browser = false }: DocsViewProps & { index: Index }) {
  const { link, go } = makeLinks(route, prefix, navigate);
  return (
    <DocsProvider value={{ index, route, host, link, go }}>
      <Shell browser={browser} />
    </DocsProvider>
  );
}

function Shell({ browser }: { browser: boolean }) {
  const { index, route, link, host } = useDocs();
  const [searching, setSearching] = useState(false);
  const [drawer, setDrawer] = useState(false);
  const body = useRef<HTMLDivElement>(null);
  const where = `${route.tab}/${route.item}/${route.folder}/${route.kinds.join(",")}/${route.described}/${route.changed}/${route.page}`;
  useEffect(() => {
    body.current?.scrollTo?.(0, 0);
    setDrawer(false);
  }, [where]);
  useEffect(() => {
    const onKey = (ev: globalThis.KeyboardEvent) => {
      const typing = ev.target instanceof HTMLElement && (/^(INPUT|TEXTAREA|SELECT)$/.test(ev.target.tagName) || ev.target.isContentEditable);
      if ((ev.key === "/" && !typing) || (browser && ev.key.toLowerCase() === "k" && (ev.metaKey || ev.ctrlKey))) {
        ev.preventDefault();
        setSearching(true);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [browser]);
  const sidebar = ["home", "reference"].includes(route.tab) && index.groups.length > 0 && !isOff(index, "reference");
  const commit = index.project.commit || index.project.branch;
  return (
    <div className={`dd-shell${drawer ? " drawer-open" : ""}`}>
      <header className="dd-bar">
        {sidebar && <button className="btn sm ghost dd-only-narrow" aria-label="Sections" onClick={() => setDrawer(!drawer)}><Menu className="icon" /></button>}
        <a className="dd-brand" href={link(fresh("home"))}><BookOpen className="icon" /><b>{index.project.title.replace(/ dev docs$/i, "")}</b><span className="muted">docs</span></a>
        <nav className="dd-tabs">
          {tabs(index).map((t) => (
            <a key={t.id} className={`dd-tab${t.id === route.tab ? " on" : ""}${t.count === 0 ? " quiet" : ""}`} href={link(fresh(t.id))}
              title={t.count === null ? t.label : t.count === 0 ? `${t.label}: nothing here yet` : `${t.label}: ${t.count}`}>
              {TAB_ICON[t.id]}<span>{t.label}</span>
            </a>
          ))}
        </nav>
        <button className="btn sm dd-search-open" onClick={() => setSearching(true)} title={browser ? "Search (/ or ⌘K)" : "Search (/)"}>
          <Search className="icon" /><span>Search</span><span className="kbd">/</span>
        </button>
        {host.themeSwitch && (
          <button className="btn sm ghost" onClick={host.themeSwitch.toggle} aria-label="Switch the theme" title="Switch the theme">
            {host.themeSwitch.theme === "dark" ? <Sun className="icon" /> : <Moon className="icon" />}
          </button>
        )}
        {commit && (isOff(index, "commits")
          ? <span className="dd-commit mono"><GitCommitHorizontal className="icon sm" />{commit}</span>
          : <a className="dd-commit mono" href={link(fresh("commits"))} title="What the last commits changed"><GitCommitHorizontal className="icon sm" />{commit}</a>)}
      </header>
      <div className="dd-main">
        {sidebar && <Sidebar />}
        <div className="dd-body" ref={body}><Body /></div>
      </div>
      {searching && <SearchDialog onClose={() => setSearching(false)} />}
    </div>
  );
}

function Body() {
  const { index, route } = useDocs();
  const { tab, item } = route;
  if (tab !== "home" && isOff(index, tab)) {
    return (
      <Empty title="Switched off for this project" icon={<Inbox className="icon" />}>
        <p className="muted">This project leaves this section out of its docs: <span className="mono">{offKey(index, tab)}</span>, under Docs in its project settings.</p>
      </Empty>
    );
  }
  switch (tab) {
    case "reference": return item === "" ? <ReferenceList /> : <EntryPage key={item} id={item} />;
    case "guides": return <Guides id={item} />;
    case "decisions": return <Decisions id={item} />;
    case "changelog": return <Changelog />;
    case "commits": return <Commits />;
    case "mcp": return <Mcp id={item} />;
    case "api": return <ApiPage id={item} />;
    default: return <Home />;
  }
}
