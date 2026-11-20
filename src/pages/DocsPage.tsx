// A project's page in the sidebar's Docs section (#/devdocs/<project id>/<tab>). Docs is switched on per project, in the
// project form; while it is off the page says so, with the way to turn it on. With it on, six tabs:
// - Reference (the first): the declarations in the project's code, by kind, with a filter; each opens to its comment and
//   its code;
// - Guides: the project's own Markdown docs, the README first and open;
// - API: the routes the project answers on (Next.js's route and page files, Laravel's routes/ files), endpoints first;
// - Changelog: the project's merged cards, newest first, by month, each linking to its card;
// - Decisions: the shared memory notes about the project's decisions, each linking to it in Memory;
// - Commits: main's newest commits as last fetched (no fetch); a commit opens to the files it changed, each in the
//   Changes view's own box, whose diff loads once the box comes near the screen.
import { Fragment, useState } from "react";
import { BookOpen, Braces, ChevronDown, ChevronRight, GitCommitHorizontal, Route as RouteIcon, Scale, ScrollText, type LucideIcon } from "lucide-react";
import { docsApi, docsChangelog, docsGuides, docsCommitFiles, docsCommits, docsDecisions, docsReference, getProject } from "../api";
import { href, type DocsTab } from "../router";
import { useData } from "../lib/useData";
import { useDrawer } from "../lib/drawers";
import { when } from "../lib/screenshots";
import { useNearScreen } from "../lib/shotUrls";
import { FileBox, type DiffSource } from "../components/changes/FileBox";
import { MarkdownView } from "../components/MarkdownView";
import type { ApiRoute, ChangelogEntry, Guide, CodeReference, FileChange, LoggedCommit, ReferenceEntry } from "../types";

const TABS: { tab: DocsTab; label: string; icon: LucideIcon }[] = [
  { tab: "reference", label: "Reference", icon: Braces },
  { tab: "guides", label: "Guides", icon: BookOpen },
  { tab: "api", label: "API", icon: RouteIcon },
  { tab: "changelog", label: "Changelog", icon: ScrollText },
  { tab: "decisions", label: "Decisions", icon: Scale },
  { tab: "commits", label: "Commits", icon: GitCommitHorizontal },
];

export function DocsPage({ id, tab = "reference" }: { id: string; tab?: DocsTab }) {
  const { data: p, error } = useData(() => getProject(id), [id]);
  const open = useDrawer();
  if (error) return <div className="error-banner">{error}</div>;
  if (!p) return null;
  return (
    <>
      <div className="topbar"><div className="crumbs"><span>Docs</span><span className="sep">/</span><b>{p.name}</b></div></div>
      <div className="content">
        <div className="page">
          {p.devDocs ? (
            <>
              <div className="tabs docs-tabs" role="tablist" aria-label="The project's docs">
                {TABS.map(({ tab: t, label, icon: I }) => (
                  <a key={t} className="tab" role="tab" aria-selected={tab === t} href={href({ page: "devdocs", id: p.id, view: t })}><I className="icon" />{label}</a>
                ))}
              </div>
              {tab === "reference" ? <Reference projectId={p.id} /> : tab === "guides" ? <Guides projectId={p.id} /> : tab === "api" ? <Api projectId={p.id} /> : tab === "changelog" ? <Changelog projectId={p.id} />
                : tab === "decisions" ? <Decisions projectId={p.id} /> : <Commits projectId={p.id} />}
            </>
          ) : (
            <div className="empty"><b>Docs is off for this project</b><span>Turn it on in the project's settings, under Docs.</span>
              <button className="btn sm" onClick={() => open({ kind: "project", id: p.id })}>Edit project</button></div>
          )}
        </div>
      </div>
    </>
  );
}

/** The kinds in the order Reference shows them; any other kind follows, by name. */
const KIND_ORDER = ["component", "hook", "function", "class", "interface", "type", "enum", "constant", "model", "controller", "migration",
  "middleware", "api resource", "job", "command", "seeder", "factory", "notification", "mailable", "cast", "validation rule",
  "livewire component", "blade component", "exception", "trait"];
const rank = (kind: string) => { const i = KIND_ORDER.indexOf(kind); return i === -1 ? KIND_ORDER.length : i; };
const PLURAL: Record<string, string> = { class: "Classes", middleware: "Middleware", "api resource": "API resources" };
const kindHeading = (kind: string) => PLURAL[kind] ?? `${kind[0]!.toUpperCase()}${kind.slice(1)}s`;

/** Docs → Reference: the declarations in the project's code, by kind, with a filter. */
function Reference({ projectId }: { projectId: string }) {
  const { data, error } = useData(() => docsReference(projectId), [projectId]);
  const [filter, setFilter] = useState("");
  if (error) return <div className="empty"><b>Hersir can't read the code</b><span>{error}</span></div>;
  if (!data) return <p className="faint">Reading the code…</p>;
  if (data.entries.length === 0) {
    return <div className="empty"><b>No declarations found</b><span>Reference reads the TypeScript, JavaScript, PHP, Dart, Kotlin, Vue and Svelte files git tracks.</span></div>;
  }
  const many = (n: number, one: string) => `${n} ${one}${n === 1 ? "" : "s"}`;
  return (
    <section aria-label="Reference">
      <div className="section-head"><h3>{`${many(data.entries.length, "declaration")} in ${many(data.files, "file")}`}</h3>
        <input className="input docs-filter" type="search" placeholder="Filter by name, kind or file" aria-label="Filter the declarations"
          value={filter} onChange={(e) => setFilter(e.target.value)} /></div>
      {data.partial.length > 0 && <p className="faint">{`${many(data.partial.length, "file")} ${data.partial.length === 1 ? "has" : "have"} syntax the parser can't read: ${data.partial.length === 1 ? "its" : "their"} declarations may be incomplete.`}</p>}
      <ReferenceList reference={data} filter={filter} />
    </section>
  );
}

/** The declarations whose name, kind or file has every word of `filter`, by kind. */
export function ReferenceList({ reference, filter }: { reference: CodeReference; filter: string }) {
  const words = filter.toLowerCase().split(/\s+/).filter(Boolean);
  const kept = reference.entries.filter((e) => words.every((w) => `${e.name} ${e.kind} ${e.path}`.toLowerCase().includes(w)));
  if (kept.length === 0) return <p className="faint">Nothing matches.</p>;
  const kinds = [...new Set(kept.map((e) => e.kind))].sort((a, b) => rank(a) - rank(b) || a.localeCompare(b));
  return (
    <>
      {kinds.map((kind) => {
        const of = kept.filter((e) => e.kind === kind);
        return (
          <Fragment key={kind}>
            <h3 className="docs-month">{kindHeading(kind)} <span className="faint">{of.length}</span></h3>
            <ul className="run-commits docs-commits">{of.map((e) => <EntryRow key={`${e.path}:${e.line}:${e.name}`} e={e} />)}</ul>
          </Fragment>
        );
      })}
    </>
  );
}

function EntryRow({ e }: { e: ReferenceEntry }) {
  const [open, setOpen] = useState(false);
  const Caret = open ? ChevronDown : ChevronRight;
  return (
    <li>
      <button className="docs-commit" aria-expanded={open} onClick={() => setOpen(!open)}>
        <Caret className="icon sm" /><span className="mono">{e.name}</span><span className="faint">{`${e.path}:${e.line}`}</span>
        {e.doc && <span className="doc">{e.doc.split("\n")[0]}</span>}
      </button>
      {open && <div className="docs-entry">{e.doc && <p>{e.doc}</p>}<pre className="docs-code">{e.text}</pre></div>}
    </li>
  );
}

/** Docs → Guides: the project's own Markdown docs, the README first and open. */
function Guides({ projectId }: { projectId: string }) {
  const { data, error } = useData(() => docsGuides(projectId), [projectId]);
  if (error) return <div className="empty"><b>Hersir can't read the guides</b><span>{error}</span></div>;
  if (!data) return null;
  if (data.length === 0) return <div className="empty"><b>No guides found</b><span>Guides shows the README at the project's root and the Markdown under docs/.</span></div>;
  return <section aria-label="Guides"><ul className="run-commits docs-commits">{data.map((g, i) => <GuideRow key={g.path} g={g} first={i === 0} />)}</ul></section>;
}

function GuideRow({ g, first }: { g: Guide; first: boolean }) {
  const [open, setOpen] = useState(first);
  const Caret = open ? ChevronDown : ChevronRight;
  return (
    <li>
      <button className="docs-commit" aria-expanded={open} onClick={() => setOpen(!open)}>
        <Caret className="icon sm" /><span className="subj">{g.title}</span><span className="faint">{g.path}</span>
      </button>
      {open && <div className="docs-entry"><MarkdownView md={withoutTitle(g.bodyMd, g.title)} /></div>}
    </li>
  );
}

/** Docs → API: the routes the project answers on, endpoints first, then pages. */
function Api({ projectId }: { projectId: string }) {
  const { data, error } = useData(() => docsApi(projectId), [projectId]);
  if (error) return <div className="empty"><b>Hersir can't read the routes</b><span>{error}</span></div>;
  if (!data) return <p className="faint">Reading the routes…</p>;
  if (data.length === 0) {
    return <div className="empty"><b>No routes found</b><span>API reads a Next.js app's route and page files, and a Laravel app's routes/ files.</span></div>;
  }
  const endpoints = data.filter((r) => r.method !== "PAGE");
  const pages = data.filter((r) => r.method === "PAGE");
  return (
    <section aria-label="API">
      {endpoints.length > 0 && <><h3 className="docs-month">Endpoints <span className="faint">{endpoints.length}</span></h3><RouteList routes={endpoints} /></>}
      {pages.length > 0 && <><h3 className="docs-month">Pages <span className="faint">{pages.length}</span></h3><RouteList routes={pages} /></>}
    </section>
  );
}

function RouteList({ routes }: { routes: ApiRoute[] }) {
  return (
    <ul className="run-commits docs-routes">
      {routes.map((r) => (
        <li key={`${r.method} ${r.path} ${r.file}:${r.line}`}>
          {r.method !== "PAGE" && <span className={`docs-method ${r.method.toLowerCase()}`}>{r.method}</span>}<span className="mono">{r.path}</span>
          {r.handler && <span>{r.handler}</span>}<span className="faint">{`${r.file}:${r.line}`}</span>
        </li>
      ))}
    </ul>
  );
}

const monthOf = (ms: number) => new Date(ms).toLocaleDateString("en-GB", { month: "long", year: "numeric" });
const dayOf = (ms: number) => new Date(ms).toLocaleDateString("en-GB", { day: "numeric", month: "short" });

/** The entries by month, in their order (newest first). */
function byMonth(entries: ChangelogEntry[]): [string, ChangelogEntry[]][] {
  const out: [string, ChangelogEntry[]][] = [];
  for (const e of entries) {
    const m = monthOf(e.at);
    if (out.at(-1)?.[0] === m) out.at(-1)![1].push(e); else out.push([m, [e]]);
  }
  return out;
}

/** Docs → Changelog: the project's merged cards, newest first, by month. */
function Changelog({ projectId }: { projectId: string }) {
  const { data, error } = useData(() => docsChangelog(projectId), [projectId]);
  if (error) return <div className="empty"><b>Hersir can't show the changelog</b><span>{error}</span></div>;
  if (!data) return null;
  if (data.length === 0) return <div className="empty"><b>Nothing merged yet</b><span>A card shows here once its work reaches main: in a Deploy or Done column.</span></div>;
  return (
    <section aria-label="Changelog">
      {byMonth(data).map(([month, entries]) => (
        <Fragment key={month}>
          <h3 className="docs-month">{month}</h3>
          <ul className="run-commits docs-log">
            {entries.map((e) => <li key={e.taskId}><span className="faint">{dayOf(e.at)}</span><a className="mono" href={href({ page: "task", id: e.taskId })}>{e.identifier}</a><span>{e.title}</span></li>)}
          </ul>
        </Fragment>
      ))}
    </section>
  );
}

/** A note's Markdown without a first `# <title>` line, which the note's header already shows. */
function withoutTitle(md: string, title: string): string {
  const [first, ...rest] = md.split("\n");
  return first?.trim() === `# ${title}` ? rest.join("\n").replace(/^\n+/, "") : md;
}

/** Docs → Decisions: the shared memory notes about the project's decisions, newest first. */
function Decisions({ projectId }: { projectId: string }) {
  const { data, error } = useData(() => docsDecisions(projectId), [projectId]);
  if (error) return <div className="empty"><b>Hersir can't show the decisions</b><span>{error}</span></div>;
  if (!data) return null;
  if (data.length === 0) return <div className="empty"><b>No decisions yet</b><span>The Chief keeps them in Memory, in Decisions/, with this project's key as project:.</span></div>;
  return (
    <section aria-label="Decisions">
      {data.map((n) => (
        <article key={n.id} className="docs-decision">
          <div className="section-head"><h3><a href={href({ page: "memory", id: n.id })}>{n.title}</a></h3><span className="faint">{`${n.path}, ${when(n.updatedAt)}`}</span></div>
          <MarkdownView md={withoutTitle(n.bodyMd, n.title)} />
        </article>
      ))}
    </section>
  );
}

/** Docs → Commits: main's newest commits, newest first, each opening to the files it changed. */
function Commits({ projectId }: { projectId: string }) {
  const { data, error } = useData(() => docsCommits(projectId), [projectId]);
  if (error) return <div className="empty"><b>Hersir can't show the commits</b><span>{error}</span></div>;
  if (!data) return null;
  return (
    <section aria-label="Commits">
      <div className="section-head"><h3>Commits</h3><span className="faint">{data.label}</span></div>
      {data.commits.length === 0
        ? <div className="empty"><b>{`No commits on ${data.label} yet`}</b></div>
        : <ul className="run-commits docs-commits">{data.commits.map((c) => <CommitRow key={c.sha} projectId={projectId} c={c} />)}</ul>}
    </section>
  );
}

function CommitRow({ projectId, c }: { projectId: string; c: LoggedCommit }) {
  const [open, setOpen] = useState(false);
  const Caret = open ? ChevronDown : ChevronRight;
  return (
    <li>
      <button className="docs-commit" aria-expanded={open} title={c.sha} onClick={() => setOpen(!open)}>
        <Caret className="icon sm" /><span className="mono">{c.sha.slice(0, 7)}</span><span className="subj">{c.subject}</span>
        <span className="faint">{`${c.author}, ${when(c.at)}`}</span>
      </button>
      {open && <CommitFileList projectId={projectId} sha={c.sha} />}
    </li>
  );
}

/** The files an opened commit changed, each in its own box; a box's diff loads once it comes near the screen. */
export function CommitFileList({ projectId, sha }: { projectId: string; sha: string }) {
  const { data, error } = useData(() => docsCommitFiles(projectId, sha), [projectId, sha]);
  if (error) return <p className="faint">{error}</p>;
  if (!data) return <p className="faint">Loading the changed files…</p>;
  if (data.files.length === 0) return <p className="faint">This commit changed no files.</p>;
  const source: DiffSource = { projectId, from: data.from, to: data.to };
  return <div className="docs-files">{data.files.map((f) => <NearFileBox key={f.path} f={f} source={source} />)}</div>;
}

const noView = () => {};

function NearFileBox({ f, source }: { f: FileChange; source: DiffSource }) {
  const [ref, near] = useNearScreen<HTMLDivElement>();
  return <div ref={ref}><FileBox f={f} source={source} near={near} onView={noView} /></div>;
}
