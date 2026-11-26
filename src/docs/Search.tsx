// The Docs page's search: every declaration, guide, decision, change, MCP server and tool, and endpoint, built from the
// index the page already has the first time it opens. / opens it (and ⌘K in a browser: in Hersir's window ⌘K is the
// command palette); arrows move, Enter opens, Tab changes the section, Escape closes.
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { History, Search as SearchIcon } from "lucide-react";
import { isOff, search, searchDocs, type SearchDoc, type SearchSection } from "./derive";
import { fresh } from "./route";
import { KindBadge, MethodBadge, useDocs } from "./ui";

const RECENT_KEY = "gizai.docs.recent";
const SECTIONS: [string, SearchSection | null][] = [["All", null], ["Reference", "reference"], ["Guides", "guides"], ["API", "api"],
  ["Changelog", "changelog"], ["Decisions", "decisions"], ["MCP", "mcp"]];

function recent(): string[] {
  try {
    const list: unknown = JSON.parse(localStorage.getItem(RECENT_KEY) ?? "[]");
    return Array.isArray(list) ? list.filter((q): q is string => typeof q === "string") : [];
  } catch {
    return [];
  }
}
function remember(q: string) {
  if (q.trim() === "") return;
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify([q, ...recent().filter((r) => r !== q)].slice(0, 5)));
  } catch {
    // a private window searches without remembering
  }
}

export function SearchDialog({ onClose }: { onClose: () => void }) {
  const { index, link, go } = useDocs();
  const docs = useMemo(() => searchDocs(index), [index]);
  const sections = SECTIONS.filter(([, s]) => s === null || !isOff(index, s));
  const [q, setQ] = useState("");
  const [section, setSection] = useState<SearchSection | null>(null);
  const [cursor, setCursor] = useState(0);
  const input = useRef<HTMLInputElement>(null);
  useEffect(() => input.current?.focus(), []);
  const hits = useMemo(() => search(section ? docs.filter((d) => d.section === section) : docs, q), [docs, q, section]);
  // a change and an MCP server have no page of their own: their section's page
  const whereTo = (d: SearchDoc) =>
    d.section === "changelog" || (d.section === "mcp" && !index.mcpTools.some((t) => t.id === d.id)) ? fresh(d.section) : fresh(d.section, d.id);
  const open = (d: SearchDoc) => {
    remember(q.trim());
    onClose();
    go(whereTo(d));
  };
  const onKey = (ev: KeyboardEvent) => {
    if (ev.key === "Escape") { ev.preventDefault(); onClose(); }
    else if (ev.key === "ArrowDown") { ev.preventDefault(); setCursor((c) => (hits.length ? (c + 1) % hits.length : 0)); }
    else if (ev.key === "ArrowUp") { ev.preventDefault(); setCursor((c) => (hits.length ? (c - 1 + hits.length) % hits.length : 0)); }
    else if (ev.key === "Tab") {
      ev.preventDefault();
      const at = sections.findIndex(([, s]) => s === section);
      setSection(sections[(at + (ev.shiftKey ? -1 : 1) + sections.length) % sections.length]?.[1] ?? null);
      setCursor(0);
    } else if (ev.key === "Enter" && hits[cursor]) { ev.preventDefault(); open(hits[cursor]); }
  };
  const past = recent();
  return (
    <div className="dd-search" onMouseDown={(ev) => ev.target === ev.currentTarget && onClose()}>
      <div className="dd-search-panel" role="dialog" aria-label="Search the docs" onKeyDown={onKey}>
        <div className="dd-search-input">
          <SearchIcon className="icon" />
          <input ref={input} className="input" value={q} placeholder="Search declarations, routes, guides, decisions…" aria-label="Search the docs"
            onChange={(ev) => { setQ(ev.target.value); setCursor(0); }} spellCheck={false} autoComplete="off" />
          <span className="kbd">Esc</span>
        </div>
        <div className="dd-search-tabs">
          {sections.map(([label, s]) => <button key={label} className={`chip${s === section ? " on" : ""}`} onClick={() => { setSection(s); setCursor(0); }}>{label}</button>)}
        </div>
        <div className="dd-search-list">
          {q.trim() === "" ? (
            past.length > 0 ? <>
              <div className="dd-search-head">Recent</div>
              {past.map((r) => <button key={r} className="dd-search-recent" onClick={() => setQ(r)}><History className="icon sm" /> {r}</button>)}
            </> : <div className="dd-search-hint">Search declarations, guides, decisions, changes, MCP and the API.<div className="sm muted">{index.stats.entries} declarations in this project.</div></div>
          ) : hits.length === 0 ? (
            <div className="dd-search-hint"><b>No results for "{q}"</b><div className="sm muted">{section ? "Nothing in this section. Try All, or another word." : "Nothing in this project matches."}</div></div>
          ) : hits.map((d, n) => (
            <a key={`${d.section}:${d.id}`} className={`dd-search-hit${n === cursor ? " on" : ""}`} href={link(whereTo(d))} onMouseEnter={() => setCursor(n)}
              onClick={(ev) => { ev.preventDefault(); open(d); }}>
              <span className="grow">
                <span className="mono dd-name">{d.title}</span> {d.section === "api" ? <MethodBadge method={d.kind} /> : <KindBadge kind={d.kind} />}
                <span className="sm muted dd-block mono">{d.subtitle}</span>
              </span>
              <span className="dd-search-section">{d.section}</span>
            </a>
          ))}
        </div>
        <div className="dd-search-foot"><span><span className="kbd">↑</span><span className="kbd">↓</span> move</span><span><span className="kbd">↵</span> open</span>
          <span><span className="kbd">Tab</span> section</span><span><span className="kbd">Esc</span> close</span></div>
      </div>
    </div>
  );
}
