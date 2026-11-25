// Home and the Reference: the project at a glance, the declarations in its code (a list with pages, one entry with its
// comment, facts and source), and a file or folder that declares nothing the Reference can name.
import { ArrowLeft, ArrowRight, Bot, BookOpen, Code, File, Globe, History, Link as LinkIcon, Plug, Scale, Search, TriangleAlert } from "lucide-react";
import type { ReactNode } from "react";
import { filtered, filesUnder, folderRow, isOff, moved, MOVED_LABEL, plural, snippet, when } from "./derive";
import { fresh } from "./route";
import type { Entry, SourceFile } from "./types";
import { AgentLink, CodeBlock, CopyButton, Empty, KindBadge, MethodBadge, PathCrumbs, Prose, RailLinks, useDocs, useLoad } from "./ui";

const PER_PAGE = 100;
const MAX_SHOWN_LINES = 500;

/* ---------- Home ---------- */

const SECTION_ICON: Record<string, ReactNode> = {
  guides: <BookOpen className="icon" />, reference: <Code className="icon" />, api: <Globe className="icon" />,
  changelog: <History className="icon" />, decisions: <Scale className="icon" />, mcp: <Plug className="icon" />,
};
const OFF_NAME: Record<string, string> = { guides: "Guides", reference: "Reference", api: "API", changelog: "Changelog", decisions: "Decisions", mcp: "MCP", commits: "Commits" };

export function Home() {
  const { index: i, link } = useDocs();
  const s = i.stats;
  const changes = i.changelog.reduce((n, r) => n + r.changes.length, 0);
  const cards = ([
    ["guides", "Guides", i.guides.length, `${plural(i.guides.length, "guide", "guides")} from the repository`],
    ["reference", "Reference", i.entries.length, i.kinds.slice(0, 4).map((k) => k.name).join(", ")],
    ["api", "API", i.api ? i.api.endpoints.length : 0, i.api ? `from ${i.api.file}` : "no API found"],
    ["changelog", "Changelog", changes, i.changelog[0] ? `latest ${i.changelog[0].version}` : "nothing merged yet"],
    ["decisions", "Decisions", i.decisions.length, "decision records"],
    ["mcp", "MCP", i.mcp.length + i.mcpTools.length, "servers and tools"],
  ] as const).filter(([tab]) => !isOff(i, tab));
  const off = (i.switchedOff ?? []).map((o) => OFF_NAME[o.section] ?? o.section);
  const refOff = isOff(i, "reference");
  return (
    <div className="dd-page narrow">
      <div className="dd-head">
        <div className="dd-crumb">Overview</div>
        <h1>{i.project.title}</h1>
        <p className="dd-lead">The code reference, guides, API, changes, decisions and MCP servers of this project, read from its repository.</p>
      </div>
      {i.problems.length > 0 && (
        <div className="dd-callout warning">
          <TriangleAlert className="icon" />
          <div>
            <b>{plural(i.problems.length, "problem", "problems")} reading this project.</b>
            {i.problems.slice(0, 2).map((p, n) => <div key={n} className="sm"><span className="mono">{p.file}{p.line ? `:${p.line}` : ""}</span>: {p.message}</div>)}
            {i.problems.length > 2 && <div className="sm">and {i.problems.length - 2} more, listed in llms.txt.</div>}
          </div>
        </div>
      )}
      <div className="dd-home">
        <div className="dd-home-main">
          <div className="dd-cards">
            {cards.map(([tab, name, n, note]) => {
              const inner = <>{SECTION_ICON[tab]}<b>{name} <span className="dd-count">{n}</span></b><span className="sm muted">{note}</span></>;
              return n === 0 ? <div key={tab} className="dd-card quiet">{inner}</div> : <a key={tab} className="dd-card" href={link(fresh(tab))}>{inner}</a>;
            })}
          </div>
          {off.length > 0 && <p className="sm muted">Switched off for this project: {off.join(", ")}.</p>}
          {!refOff && <DocChangesPanel />}
          <RecentChanges />
        </div>
        <aside className="dd-home-side">
          <div className="dd-panel">
            <h3>This build</h3>
            <table className="dd-facts sm"><tbody>
              <tr><td>Commit</td><td className="mono">{i.project.commit || "none"}</td></tr>
              <tr><td>Branch</td><td className="mono">{i.project.branch || "none"}</td></tr>
              <tr><td>Read</td><td>{when(i.project.builtAt)}</td></tr>
              <tr><td>Took</td><td>{(s.durationMs / 1000).toFixed(1)} s</td></tr>
              {!refOff && <tr><td>Files</td><td>{s.files}</td></tr>}
            </tbody></table>
          </div>
          {!refOff && (
            <div className="dd-panel">
              <h3>Descriptions</h3>
              <div className="dd-pair">
                <a href={link({ ...fresh("reference"), described: "yes" })}><b>{s.described}</b><span>have one</span></a>
                <a href={link({ ...fresh("reference"), described: "no" })}><b>{s.entries - s.described}</b><span>don't</span></a>
              </div>
              <p className="sm muted">Hersir shows what is written above a declaration and never makes one up, so this counts the comments in the code.</p>
            </div>
          )}
          <div className="dd-panel">
            <h3><Bot className="icon" /> For agents</h3>
            <p className="sm">Every page is also Markdown. Start at <AgentLink path="llms.txt">llms.txt</AgentLink>.</p>
          </div>
        </aside>
      </div>
    </div>
  );
}

/** What changed in the docs since the last commit: who gained or lost a description, what is new, changed or gone. */
function DocChangesPanel() {
  const { index, link } = useDocs();
  const d = index.docChanges;
  const commitsLink = !isOff(index, "commits") && <a href={link(fresh("commits"))}>What the last commits changed <ArrowRight className="icon sm" /></a>;
  if (!d) {
    return <><h2>Changed in the docs</h2><p className="sm muted">There is no commit to compare the docs with yet. {commitsLink}</p></>;
  }
  const since = d.since.commit ? <span className="mono">{d.since.commit}</span> : "the last look";
  if (d.total === 0) {
    return <><h2>Changed in the docs <span className="sm muted">since {since}</span></h2>
      <p className="sm muted">Nothing moved: no declaration gained or lost a description, and none came or went. {commitsLink}</p></>;
  }
  const LIMIT = 12;
  const rows = (["documented", "undocumented", "added", "changed", "removed"] as const).flatMap((bucket) => d[bucket].map((e) => ({ bucket, e }))).slice(0, LIMIT);
  const listable = d.total - d.removed.length;
  return (
    <>
      <h2>Changed in the docs <span className="sm muted">since {since}</span>
        {listable > 0 && <a className="sm" href={link({ ...fresh("reference"), changed: "any" })}>All {listable} <ArrowRight className="icon sm" /></a>}</h2>
      <div className="dd-rows">
        {rows.map(({ bucket, e }) => (
          <div key={`${bucket}:${e.id}`} className="dd-row">
            <span className={`dd-moved ${bucket}`}>{MOVED_LABEL[bucket]}</span>
            <span className="grow">{bucket === "removed" ? <span className="mono">{e.name}</span> : <a className="mono" href={link(fresh("reference", e.id))}>{e.name}</a>}</span>
            <span className="sm muted">{e.file}</span>
          </div>
        ))}
      </div>
      {d.total > rows.length && <p className="sm muted">{d.total - rows.length} more, also listed in llms.txt.</p>}
    </>
  );
}

function RecentChanges() {
  const { index, link } = useDocs();
  if (isOff(index, "changelog")) return null;
  const recent = index.changelog.flatMap((r) => r.changes.map((c) => ({ ...c, version: r.version }))).slice(0, 6);
  if (recent.length === 0) return null;
  return (
    <>
      <h2>Recent changes <a className="sm" href={link(fresh("changelog"))}>All changes <ArrowRight className="icon sm" /></a></h2>
      <div className="dd-rows">
        {recent.map((c, n) => (
          <div key={n} className="dd-row"><KindBadge kind={c.type} /><Prose className="grow" html={c.html} /><span className="sm muted">{c.version}</span></div>
        ))}
      </div>
    </>
  );
}

/* ---------- the Reference's list ---------- */

export function ReferenceList() {
  const { index, route, link, host } = useDocs();
  const found = filtered(index, route);
  const pages = Math.max(1, Math.ceil(found.length / PER_PAGE));
  const page = Math.min(route.page, pages);
  const from = (page - 1) * PER_PAGE;
  const shown = found.slice(from, from + PER_PAGE);
  // nothing found for one file or folder: it may hold no declaration the Reference names, so show the file or its files
  const lone = found.length === 0 && route.folder !== "" && route.kinds.length === 0 && route.described === "" && route.changed === "";
  const src = useLoad(() => (lone ? host.source(route.folder) : Promise.resolve(null)), lone ? route.folder : "");
  const since = index.docChanges?.since.commit || "the last look";
  const movedBy = route.changed === "" ? "" : route.changed === "any" ? ` that moved since ${since}` : ` ${MOVED_LABEL[route.changed] ?? route.changed} since ${since}`;
  const bare = route.described === "no" ? " with no description" : route.described === "yes" ? " with a description" : "";
  const where = route.folder ? ` in ${route.folder}` : "";
  const ofKind = route.kinds.length > 0 ? ` of kind ${route.kinds.join(", ")}` : "";

  if (found.length === 0) {
    if (lone && src.data && src.data.lines.length > 0) return <FileView src={src.data} />;
    const row = lone ? folderRow(index, route.folder) : null;
    if (row?.children && row.children.length > 0) return <FolderView target={row.target} files={filesUnder(row.children)} />;
    if (lone && !src.data && !src.error) return null;
    return (
      <Empty title="Nothing matches" icon={<Search className="icon" />}>
        <p className="muted">No declaration is{where}{ofKind}{bare}{movedBy}.</p>
        <a className="btn" href={link({ folder: "", kinds: [], item: "", described: "", changed: "" })}>Clear the filters</a>
      </Empty>
    );
  }
  return (
    <div className="dd-page">
      <div className="dd-head">
        <div className="dd-crumb">Reference</div>
        <h1>{found.length === index.entries.length ? "Everything in this project" : `${plural(found.length, "declaration", "declarations")}${bare}${movedBy}${where}${ofKind}`}</h1>
        {route.described === "no" && <p className="dd-lead">Nothing is written above these declarations. Hersir shows what the code says and never makes a description up.</p>}
      </div>
      <div className="dd-rows">{shown.map((e) => <EntryRow key={e.id} e={e} />)}</div>
      {pages > 1 && <Pager page={page} pages={pages} from={from} count={shown.length} total={found.length} />}
    </div>
  );
}

function Pager({ page, pages, from, count, total }: { page: number; pages: number; from: number; count: number; total: number }) {
  const { link } = useDocs();
  const first = Math.max(1, Math.min(page - 2, pages - 4));
  const numbers = Array.from({ length: Math.min(5, pages - first + 1) }, (_, n) => first + n);
  const step = (n: number, label: ReactNode, off: boolean) => (off ? <span className="dd-page-btn off">{label}</span> : <a className="dd-page-btn" href={link({ page: n })}>{label}</a>);
  return (
    <nav className="dd-pager">
      <span className="sm muted">{from + 1} to {from + count} of {total}</span>
      <span className="grow" />
      {step(page - 1, <><ArrowLeft className="icon sm" /> Previous</>, page === 1)}
      {numbers.map((n) => (n === page ? <span key={n} className="dd-page-btn on">{n}</span> : <a key={n} className="dd-page-btn" href={link({ page: n })}>{n}</a>))}
      {step(page + 1, <>Next <ArrowRight className="icon sm" /></>, page === pages)}
    </nav>
  );
}

export function EntryRow({ e }: { e: Entry }) {
  const { index, link } = useDocs();
  const how = moved(index).get(e.id);
  return (
    <a className="dd-row dd-entry" href={link(fresh("reference", e.id))}>
      <span className="grow">
        <span className="mono dd-name">{e.name}</span> <KindBadge kind={e.kind} />
        {how && <span className={`dd-moved ${how}`}>{MOVED_LABEL[how]}</span>}
        <span className="sm muted mono dd-block">{e.file}:{e.lineStart}</span>
        {e.hasDescription ? <span className="sm dd-block">{snippet(e.description)}</span> : <span className="dd-nodesc dd-block">no description</span>}
      </span>
    </a>
  );
}

function FileView({ src }: { src: SourceFile }) {
  const { index } = useDocs();
  const problem = index.problems.find((p) => p.file === src.path);
  const shown = Math.min(src.lines.length, MAX_SHOWN_LINES);
  return (
    <div className="dd-page">
      <div className="dd-head">
        <PathCrumbs path={src.path} />
        <h1>{src.path.split("/").pop()}</h1>
        <p className="dd-lead">{problem ? problem.message : "This file declares no class, function or constant, so it has no Reference entries of its own. Its contents are below."}</p>
        <div className="dd-actions"><CopyButton text={() => src.path} label="Copy path" said="Path copied" /></div>
      </div>
      <h2>Source</h2>
      <CodeBlock lines={src.lines} to={shown} />
      {shown < src.lines.length && <p className="sm muted">The first {shown} of {src.lines.length} lines.</p>}
    </div>
  );
}

function FolderView({ target, files }: { target: string; files: string[] }) {
  const { link } = useDocs();
  return (
    <div className="dd-page">
      <div className="dd-head">
        <PathCrumbs path={target} />
        <h1>{target.split("/").pop()}</h1>
        <p className="dd-lead">{plural(files.length, "file", "files")} in this folder. None declares a class, function or constant, so the folder has no Reference entries. Open a file to read it.</p>
      </div>
      <div className="dd-rows">
        {files.map((path) => <a key={path} className="dd-row" href={link({ folder: path, item: "" })}><File className="icon" /><span className="grow mono">{path.slice(target.length + 1)}</span></a>)}
      </div>
    </div>
  );
}

/* ---------- one entry ---------- */

export function EntryPage({ id }: { id: string }) {
  const { index, route, link, host } = useDocs();
  const e = index.entries.find((x) => x.id === id);
  const src = useLoad(() => (e ? host.source(e.file) : Promise.resolve(null)), e?.file ?? "");
  if (!e) return <NotFound id={id} />;
  const byId = (x: string) => index.entries.find((y) => y.id === x);
  const parent = e.parent ? byId(e.parent) : undefined;
  const kids = e.children.map(byId).filter((k): k is Entry => k !== undefined);
  const routes = e.facts.filter((f) => f.label === "Route");
  const facts = e.facts.filter((f) => f.label !== "Route");
  const how = moved(index).get(e.id);
  const since = index.docChanges?.since.commit || "the last look";
  const rail = [
    { id: "summary", label: "Summary" },
    ...(facts.length > 0 || e.tags.length > 0 ? [{ id: "facts", label: "Facts" }] : []),
    ...(kids.length > 0 ? [{ id: "inside", label: `Inside (${kids.length})` }] : []),
    { id: "source", label: "Source" },
  ];
  const missing = index.stats.entries - index.stats.described;
  return (
    <div className="dd-doc">
      <div className="dd-doc-main">
        <PathCrumbs path={e.file} />
        <div className="dd-entry-top">
          <div>
            <KindBadge kind={e.kind} />
            {parent && <a className="sm" href={link({ item: parent.id })}> in {parent.name}</a>}
            {how && <a className={`dd-moved ${how}`} href={link({ ...fresh("reference"), changed: how })} title="Everything that moved the same way">{MOVED_LABEL[how]} since {since}</a>}
          </div>
          <div className="dd-actions">
            <CopyButton text={() => e.file} label="Copy path" said="Path copied" />
            {host.shareable && <CopyButton text={() => host.shareable!(link({ item: e.id }))} label="Copy link" said="Link copied" />}
          </div>
        </div>
        <h1 className="mono" id="summary">{e.name}</h1>
        <div className="sm muted mono">{e.file} · lines {e.lineStart} to {e.lineEnd}</div>
        {e.hasDescription ? (
          <div className="dd-prose">{e.description.split("\n\n").map((p, n) => <p key={n}>{p}</p>)}</div>
        ) : (
          <div className="dd-nodesc-dd-block">
            <b>No description in the source</b>
            <p className="sm muted">Nothing is written above this declaration. Hersir shows what the code says and never makes one up.{" "}
              <a href={link({ ...fresh("reference"), described: "no" })}>{plural(missing, "declaration is", "declarations are")} the same</a>.</p>
          </div>
        )}
        {e.signature && <pre className="dd-code dd-signature"><code>{e.signature}</code></pre>}
        {(facts.length > 0 || e.tags.length > 0) && (
          <>
            <h2 id="facts">Facts</h2>
            <table className="dd-facts"><tbody>
              {facts.map((f, n) => <tr key={n}><td>{f.label}</td><td className="mono">{f.value}</td></tr>)}
              {e.tags.map((t, n) => <tr key={`t${n}`}><td>@{t.name}</td><td className="mono">{t.text}</td></tr>)}
            </tbody></table>
          </>
        )}
        {kids.length > 0 && <><h2 id="inside">{plural(kids.length, "declaration", "declarations")} inside</h2><div className="dd-rows">{kids.map((k) => <EntryRow key={k.id} e={k} />)}</div></>}
        <h2 id="source">Source</h2>
        <div className="dd-code-head">
          <span className="mono sm">{e.file}</span><span className="sm muted">lines {e.lineStart} to {e.lineEnd}</span>
          {src.data && <CopyButton text={() => src.data!.lines.slice(e.lineStart - 1, e.lineEnd).join("\n")} said="Source copied" />}
        </div>
        {src.data ? <CodeBlock lines={src.data.lines} from={e.lineStart} to={e.lineEnd} mark={route.line || e.nameLine} />
          : src.error ? <p className="sm muted">Hersir couldn't read this file: {src.error}</p> : null}
      </div>
      <aside className="dd-rail">
        <h3>On this page</h3>
        <RailLinks items={rail} />
        {routes.length > 0 ? (
          <>
            <h3>Used by routes</h3>
            {routes.map((r, n) => {
              const [verb = "", ...rest] = r.value.split(" ");
              return <div key={n} className="dd-rail-route"><MethodBadge method={verb} /><span className="mono sm">{rest.join(" ").split(" · ")[0]}</span></div>;
            })}
          </>
        ) : null}
        <h3>For agents</h3>
        <AgentLink className="dd-rail-link mono" path={`md/reference/${e.id.replace("#", "/")}.md`}><LinkIcon className="icon sm" /> Markdown</AgentLink>
      </aside>
    </div>
  );
}

function NotFound({ id }: { id: string }) {
  const { index, link } = useDocs();
  const name = id.split("#").pop() || id;
  const guesses = index.entries.filter((e) => e.name.toLowerCase().includes(name.toLowerCase().slice(0, 6))).slice(0, 4);
  return (
    <Empty title={`${name} isn't in this project`} icon={<Search className="icon" />}>
      <p className="muted">The link points to <span className="mono">{id}</span>, which isn't in commit {index.project.commit || "this build"}. It was probably renamed or removed.</p>
      {guesses.length > 0 && <div className="dd-rows">{guesses.map((e) => <EntryRow key={e.id} e={e} />)}</div>}
      <a className="btn" href={link(fresh("reference"))}>All declarations</a>
    </Empty>
  );
}
