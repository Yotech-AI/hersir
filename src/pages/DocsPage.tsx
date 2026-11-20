// A project's page in the sidebar's Docs section (#/devdocs/<project id>). Docs is switched on per project, in the project
// form; while it is off the page says so, with the way to turn it on. With it on, Commits lists main's newest commits as
// last fetched (no fetch), newest first; a commit opens to the files it changed, each in the Changes view's own box, whose
// diff loads once the box comes near the screen.
import { useState } from "react";
import { ChevronDown, ChevronRight } from "lucide-react";
import { docsCommitFiles, docsCommits, getProject } from "../api";
import { useData } from "../lib/useData";
import { useDrawer } from "../lib/drawers";
import { when } from "../lib/screenshots";
import { useNearScreen } from "../lib/shotUrls";
import { FileBox, type DiffSource } from "../components/changes/FileBox";
import type { FileChange, LoggedCommit } from "../types";

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
            ? <Commits projectId={p.id} />
            : <div className="empty"><b>Docs is off for this project</b><span>Turn it on in the project's settings, under Docs.</span>
                <button className="btn sm" onClick={() => open({ kind: "project", id: p.id })}>Edit project</button></div>}
        </div>
      </div>
    </>
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
