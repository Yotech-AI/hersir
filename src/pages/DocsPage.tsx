// A project's page in the sidebar's Docs section (#/devdocs/<project id>/<tab>). Docs is switched on per project, in the
// project form; while it is off the page says so, with the way to turn it on. With it on, three tabs:
// - Changelog (the first): the project's merged cards, newest first, by month, each linking to its card;
// - Decisions: the shared memory notes about the project's decisions, each linking to it in Memory;
// - Commits: main's newest commits as last fetched (no fetch); a commit opens to the files it changed, each in the
//   Changes view's own box, whose diff loads once the box comes near the screen.
import { Fragment, useState } from "react";
import { ChevronDown, ChevronRight, GitCommitHorizontal, Scale, ScrollText, type LucideIcon } from "lucide-react";
import { docsChangelog, docsCommitFiles, docsCommits, docsDecisions, getProject } from "../api";
import { href, type DocsTab } from "../router";
import { useData } from "../lib/useData";
import { useDrawer } from "../lib/drawers";
import { when } from "../lib/screenshots";
import { useNearScreen } from "../lib/shotUrls";
import { FileBox, type DiffSource } from "../components/changes/FileBox";
import { MarkdownView } from "../components/MarkdownView";
import type { ChangelogEntry, FileChange, LoggedCommit } from "../types";

const TABS: { tab: DocsTab; label: string; icon: LucideIcon }[] = [
  { tab: "changelog", label: "Changelog", icon: ScrollText },
  { tab: "decisions", label: "Decisions", icon: Scale },
  { tab: "commits", label: "Commits", icon: GitCommitHorizontal },
];

export function DocsPage({ id, tab = "changelog" }: { id: string; tab?: DocsTab }) {
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
              {tab === "changelog" ? <Changelog projectId={p.id} /> : tab === "decisions" ? <Decisions projectId={p.id} /> : <Commits projectId={p.id} />}
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
