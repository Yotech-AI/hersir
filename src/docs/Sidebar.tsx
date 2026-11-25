// The Reference's sidebar, on Home and the Reference: the kind filter, then the index's groups (Folders as a tree, the
// rest as flat lists), then which commit this is. The groups come from the index: a Flutter project has no routes and a
// Kotlin one has modules, so no group is named here.
import { useState } from "react";
import { Bot, ChevronDown, ChevronRight, ChevronsUpDown, File, Folder, FolderOpen, GitCommitHorizontal, ListFilter } from "lucide-react";
import { kindColour, when } from "./derive";
import type { Group, GroupRow } from "./types";
import { AgentLink, MethodBadge, useDocs } from "./ui";

const SHOW_AT_FIRST = 6;

function TreeRow({ row, depth }: { row: GroupRow; depth: number }) {
  const { route, link } = useDocs();
  const open = route.folder === row.target || route.folder.startsWith(`${row.target}/`);
  const kids = row.children && row.children.length > 0 ? row.children : null;
  const current = route.folder === row.target;
  return (
    <>
      <a className={`dd-side-row${current ? " on" : ""}`} style={{ paddingLeft: 8 + depth * 14 }}
        href={link({ tab: "reference", folder: current ? "" : row.target, item: "" })}>
        <span className="dd-twist">{kids && (open ? <ChevronDown className="icon sm" /> : <ChevronRight className="icon sm" />)}</span>
        {kids ? (open ? <FolderOpen className="icon" /> : <Folder className="icon" />) : <File className="icon" />}
        <span className="dd-label">{row.label}</span>
        {row.count > 0 && <span className="dd-count">{row.count}</span>}
      </a>
      {kids && open && kids.map((k) => <TreeRow key={k.target} row={k} depth={depth + 1} />)}
    </>
  );
}

function GroupView({ g }: { g: Group }) {
  const { route, link } = useDocs();
  const [all, setAll] = useState(false);
  const rows = g.type === "tree" || all ? g.rows : g.rows.slice(0, SHOW_AT_FIRST);
  return (
    <div className="dd-group">
      <h2 className="dd-group-head">{g.name}{g.count > 0 && <span className="dd-count"> · {g.count}</span>}</h2>
      {g.type === "tree"
        ? rows.map((r) => <TreeRow key={r.target} row={r} depth={0} />)
        : rows.map((r) => {
          const inner = <><span className="dd-label">{r.label}</span>{r.badge && <MethodBadge method={r.badge} />}</>;
          return r.target === ""
            ? <span key={r.label} className="dd-side-row flat">{inner}</span>
            : <a key={`${r.label}:${r.target}`} className={`dd-side-row flat${route.item === r.target ? " on" : ""}`} href={link({ tab: "reference", item: r.target })}>{inner}</a>;
        })}
      {g.type === "flat" && g.rows.length > SHOW_AT_FIRST && (
        <button className="dd-show-all" onClick={() => setAll(!all)}>
          {all ? "Show fewer" : `Show all ${g.rows.length} ${g.name.toLowerCase()}`}{all ? <ChevronDown className="icon sm" /> : <ChevronRight className="icon sm" />}
        </button>
      )}
    </div>
  );
}

function KindFilter() {
  const { index, route, go } = useDocs();
  const [open, setOpen] = useState(false);
  const picked = route.kinds;
  const shown = picked.length === 0 ? index.entries.length : index.entries.filter((e) => picked.includes(e.kind)).length;
  const pick = (kind: string, on: boolean) => go({ kinds: on ? [...picked, kind] : picked.filter((k) => k !== kind), item: "" });
  return (
    <div className="dd-kind-filter">
      <button className="dd-select" aria-expanded={open} onClick={() => setOpen(!open)}>
        <ListFilter className="icon" /><span className="dd-label">{picked.length === 0 ? "All kinds" : picked.join(", ")}</span>
        <span className="dd-count">{shown}</span><ChevronsUpDown className="icon sm" />
      </button>
      {open && (
        <div className="dd-kind-menu">
          {index.kinds.map((k) => (
            <label key={k.name} className="dd-kind-opt">
              <input type="checkbox" checked={picked.includes(k.name)} onChange={(ev) => pick(k.name, ev.target.checked)} />
              <span className="dd-dot" style={{ background: kindColour(k.name) }} /><span className="dd-label">{k.name}</span><span className="dd-count">{k.count}</span>
            </label>
          ))}
          {picked.length > 0 && <button className="btn sm ghost" onClick={() => go({ kinds: [], item: "" })}>Clear</button>}
        </div>
      )}
    </div>
  );
}

export function Sidebar() {
  const { index, route } = useDocs();
  const p = index.project;
  const id = p.commit || p.branch;
  return (
    <aside className="dd-side">
      {route.tab === "reference" && <KindFilter />}
      {index.groups.map((g) => <GroupView key={g.name} g={g} />)}
      <div className="dd-side-foot">
        {(id || p.builtAt) && <p><GitCommitHorizontal className="icon sm" /><span>{id && <span className="mono">{id}</span>}{id && " · "}read {when(p.builtAt)}</span></p>}
        <p><Bot className="icon sm" /><span>For agents: <AgentLink path="llms.txt">llms.txt</AgentLink></span></p>
      </div>
    </aside>
  );
}
