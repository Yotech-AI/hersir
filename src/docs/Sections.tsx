// The Docs page's other sections: Guides, Decisions, Changelog, Commits, MCP and API. A section the repository has
// nothing for says where it would come from.
import { useState } from "react";
import { ArrowLeft, ArrowRight, ChevronRight, ExternalLink, Info, Link as LinkIcon, Plug } from "lucide-react";
import { bodyOf, byMonth, dayLabel, fieldsOf, indexedFiles, monthLabel, plural, railHeadings, when, withoutLeadHeading, type Field } from "./derive";
import { fresh } from "./route";
import type { Commit, CommitPatches, Decision, McpTool } from "./types";
import { AgentLink, CodeBlock, Empty, EmptySection, KindBadge, MethodBadge, Prose, RailLinks, useDocs } from "./ui";

/* ---------- Guides ---------- */

export function Guides({ id }: { id: string }) {
  const { index, link } = useDocs();
  const all = index.guides;
  const current = all.find((g) => g.id === id) ?? all[0];
  if (!current) return <EmptySection name="Guides" source="README.md or docs/*.md" />;
  const groups = new Map<string, typeof all>();
  for (const g of all) groups.set(g.group || "Guides", [...(groups.get(g.group || "Guides") ?? []), g]);
  const at = all.indexOf(current);
  const previous = all[at - 1];
  const next = all[at + 1];
  return (
    <div className="dd-doc">
      <nav className="dd-doc-nav">
        {[...groups].map(([name, items]) => (
          <div key={name}><h3>{name}</h3>
            {items.map((g) => <a key={g.id} className={`dd-doc-link${g.id === current.id ? " on" : ""}`} href={link(fresh("guides", g.id))}>{g.title}</a>)}</div>
        ))}
      </nav>
      <article className="dd-doc-main narrow">
        <div className="dd-crumb">Guides{current.group ? <> <span className="sep">/</span> {current.group}</> : null}</div>
        <h1>{current.title}</h1>
        <div className="dd-byline"><span className="mono sm">{current.file}</span>{when(current.updated) && <span className="sm muted">updated {when(current.updated).split(",")[0]}</span>}</div>
        <Prose html={withoutLeadHeading(current.html, current.title)} />
        <div className="dd-prev-next">
          {previous ? <a href={link(fresh("guides", previous.id))}><span className="sm muted"><ArrowLeft className="icon sm" /> Previous</span><b>{previous.title}</b></a> : <span />}
          {next && <a className="next" href={link(fresh("guides", next.id))}><span className="sm muted">Next <ArrowRight className="icon sm" /></span><b>{next.title}</b></a>}
        </div>
      </article>
      <aside className="dd-rail">
        <h3>On this page</h3>
        <RailLinks items={railHeadings(current.headings, current.title).map((h) => ({ id: h.slug, label: h.text, level: h.level }))} />
        <h3>For agents</h3>
        <AgentLink className="dd-rail-link mono" path={`md/${current.id}.md`}><LinkIcon className="icon sm" /> Markdown</AgentLink>
      </aside>
    </div>
  );
}

/* ---------- Decisions ---------- */

const STATUS_ORDER = ["proposed", "accepted", "superseded", "deprecated", "rejected"];
const slug = (text: string) => text.toLowerCase().replace(/[^\p{L}\p{N}]+/gu, "-").replace(/^-|-$/g, "");
const capital = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/** The decisions, the latest first (the index's order): a list by month with the statuses on top, or one decision, wide,
 *  with the titles on the right above On this page. The status filter (`kinds`) holds for both. */
export function Decisions({ id }: { id: string }) {
  const { index, route } = useDocs();
  const all = index.decisions;
  if (all.length === 0) return <EmptySection name="Decisions" source="docs/decisions/NNNN-title.md, or the Chief's records" />;
  const shown = route.kinds.length === 0 ? all : all.filter((d) => route.kinds.includes(d.status));
  const current = id ? all.find((d) => d.id === id || d.number === id) : undefined;
  return current ? <DecisionPage d={current} shown={shown} /> : <DecisionList shown={shown} />;
}

function StatusChips() {
  const { index, route, link } = useDocs();
  const counts = new Map<string, number>();
  for (const d of index.decisions) counts.set(d.status, (counts.get(d.status) ?? 0) + 1);
  const rank = (s: string) => (STATUS_ORDER.includes(s) ? STATUS_ORDER.indexOf(s) : STATUS_ORDER.length);
  const pick = (s: string) => (route.kinds.includes(s) ? route.kinds.filter((k) => k !== s) : [...route.kinds, s]);
  return (
    <div className="dd-chips">
      <span className="sm muted dd-chips-label">Status</span>
      <a className={`chip${route.kinds.length === 0 ? " on" : ""}`} href={link({ kinds: [], item: "" })}>All · {index.decisions.length}</a>
      {[...counts].sort(([a], [b]) => rank(a) - rank(b)).map(([s, n]) => (
        <a key={s} className={`chip${route.kinds.includes(s) ? " on" : ""}`} href={link({ kinds: pick(s), item: "" })}>{capital(s)} · {n}</a>
      ))}
    </div>
  );
}

function DecisionList({ shown }: { shown: Decision[] }) {
  const { link } = useDocs();
  return (
    <div className="dd-page">
      <div className="dd-head">
        <div className="dd-crumb">Decisions</div>
        <h1>Decisions</h1>
        <p className="dd-lead">The choices made on this project, the latest first: its own records, the Chief's, and the decisions noted in Memory. A proposed decision waits for a person.</p>
      </div>
      <StatusChips />
      {shown.length === 0 && <p className="muted">No decision has this status.</p>}
      {byMonth(shown).map(([month, items]) => (
        <section key={month}>
          <h2>{month} <span className="sm muted">{plural(items.length, "decision", "decisions")}</span></h2>
          <div className="dd-rows">
            {items.map((d) => (
              <a key={d.id} className="dd-row dd-decision-row" href={link({ item: d.id })}>
                <span className={`dd-status ${d.status}`}>{d.status}</span>
                {d.number && <span className="mono sm muted">{d.number}</span>}
                <span className="grow dd-one-line">{d.title}</span>
                <span className="sm muted">{dayLabel(d.date)}</span>
              </a>
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}

/** The titles beside an open decision, in the right column, by month, the open one marked. */
function DecisionTitles({ current, shown }: { current: Decision; shown: Decision[] }) {
  const { link } = useDocs();
  return (
    <>
      <h3>Decisions</h3>
      <div className="dd-decision-list">
        {byMonth(shown).map(([month, items]) => (
          <div key={month}>
            <div className="dd-decision-month">{month}</div>
            {items.map((d) => (
              <a key={d.id} className={`dd-doc-link${d.id === current.id ? " on" : ""}`} href={link({ item: d.id })}><span className="dd-two-lines">{d.title}</span></a>
            ))}
          </div>
        ))}
      </div>
      <a className="sm" href={link({ item: "" })}>All {plural(shown.length, "decision", "decisions")}</a>
    </>
  );
}

function DecisionPage({ d, shown }: { d: Decision; shown: Decision[] }) {
  const { index, link } = useDocs();
  const title = d.number ? `${d.number} · ${d.title}` : d.title;
  const parts = d.sections.map((s) => ({ ...s, id: slug(s.heading) }));
  const rail = parts.length > 0 ? parts.map((s) => ({ id: s.id, label: s.heading }))
    : railHeadings(d.headings, title).map((h) => ({ id: h.slug, label: h.text, level: h.level }));
  const decided = dayLabel(d.date, true);
  return (
    <div className="dd-doc">
      <article className="dd-doc-main">
        <div className="dd-crumb"><a href={link({ item: "" })}>Decisions</a> <span className="sep">›</span> {monthLabel(d.date)}</div>
        <div className="dd-byline">
          <span className={`dd-status ${d.status}`}>{d.status}</span>
          {d.number && <span className="mono sm">{d.number}</span>}
          <span className="sm muted">{decided && `Decided ${decided}`}{d.file && (d.file.startsWith("by ") ? ` ${d.file}` : ` · ${d.file}`)}</span>
        </div>
        <h1>{d.title}</h1>
        {parts.length > 0 ? parts.map((s) => (
          <section key={s.id}>
            <h2 id={s.id}>{s.heading}</h2>
            {s.heading === "Decision" ? <div className="dd-decision-block"><Prose html={s.html} /></div> : <Prose html={s.html} />}
          </section>
        )) : <Prose html={withoutLeadHeading(d.html, title)} />}
        {d.relatedFiles.length > 0 && (
          <>
            <h2>Related files</h2>
            <div className="dd-rows">
              {d.relatedFiles.map((f) => {
                const e = index.entries.find((x) => x.file === f && x.parent === undefined);
                return e
                  ? <a key={f} className="dd-row" href={link(fresh("reference", e.id))}><span className="mono">{f}</span> <KindBadge kind={e.kind} /></a>
                  : <div key={f} className="dd-row"><span className="mono muted">{f}</span><span className="sm muted">not in the Reference</span></div>;
              })}
            </div>
          </>
        )}
      </article>
      <aside className="dd-rail">
        <DecisionTitles current={d} shown={shown} />
        {rail.length > 0 && <><h3>On this page</h3><RailLinks items={rail} /></>}
      </aside>
    </div>
  );
}

/* ---------- Changelog ---------- */

const CHANGE_TYPES = ["Added", "Changed", "Fixed", "Removed", "Deprecated", "Security", "Other"];

export function Changelog() {
  const { index, route, link, host } = useDocs();
  const all = index.changelog;
  if (all.length === 0) return <EmptySection name="Changelog" source="CHANGELOG.md, or the cards merged with a changelog line" />;
  const picked = route.kinds;
  const present = CHANGE_TYPES.filter((t) => all.some((r) => r.changes.some((c) => c.type === t)));
  return (
    <div className="dd-page">
      <div className="dd-head">
        <div className="dd-crumb">Changelog</div>
        <h1>Changelog</h1>
        <p className="dd-lead">From {index.project.changelogFrom || "CHANGELOG.md"}, newest first.</p>
      </div>
      <div className="dd-chips">
        <a className={`chip${picked.length === 0 ? " on" : ""}`} href={link({ kinds: [] })}>All</a>
        {present.map((t) => (
          <a key={t} className={`chip${picked.includes(t) ? " on" : ""}`} href={link({ kinds: picked.includes(t) ? picked.filter((p) => p !== t) : [...picked, t] })}>{t}</a>
        ))}
      </div>
      {all.map((r) => {
        const changes = picked.length === 0 ? r.changes : r.changes.filter((c) => picked.includes(c.type));
        if (changes.length === 0) return null;
        return (
          <section key={r.version} className="dd-release">
            <h2>{r.version} {r.unreleased ? <span className="dd-status proposed">unreleased</span> : <span className="sm muted">{r.date}</span>}
              {r.compareUrl && <a className="sm" href={r.compareUrl} onClick={(ev) => { ev.preventDefault(); host.openExternal(r.compareUrl); }}>Compare <ExternalLink className="icon sm" /></a>}</h2>
            <div className="dd-rows">
              {/* through Prose: a link in a change opens outside, never in Hersir's window (GA-362) */}
              {changes.map((c, n) => <div key={n} className="dd-row"><KindBadge kind={c.type} /><Prose className="grow" html={c.html} /></div>)}
            </div>
          </section>
        );
      })}
    </div>
  );
}

/* ---------- Commits ---------- */

const TOUCHED: Record<string, string> = { A: "added", M: "changed", D: "removed", R: "renamed" };

export function Commits() {
  const { index } = useDocs();
  const all = index.history;
  if (all.length === 0) {
    return <Empty title="No commits to show"><p className="muted">Hersir found no commits on this project's main branch. Link its repository on its project page, or commit something.</p></Empty>;
  }
  return (
    <div className="dd-page">
      <div className="dd-head">
        <div className="dd-crumb">Commits</div>
        <h1>Commits</h1>
        <p className="dd-lead">The last {plural(all.length, "commit", "commits")} on <span className="mono">{index.project.branch || "this branch"}</span>, newest first. Open a commit to see the files it changed, and a file to see its changed lines.</p>
      </div>
      <div className="dd-rows">{all.map((c) => <CommitFold key={c.hash} c={c} />)}</div>
    </div>
  );
}

/** One commit, its changed lines fetched the first time it opens. */
function CommitFold({ c }: { c: Commit }) {
  const { index, host, link } = useDocs();
  const [patches, setPatches] = useState<CommitPatches | null | "loading" | "failed">(null);
  const files = indexedFiles(index);
  const open = () => {
    if (patches !== null) return;
    setPatches("loading");
    host.commit(c.hash).then((p) => setPatches(p ?? "failed"), () => setPatches("failed"));
  };
  const diff = (path: string) => {
    if (patches === null || patches === "loading") return [<span key="l" className="dd-dl hunk">Loading the changed lines…</span>];
    if (patches === "failed") return [<span key="f" className="dd-dl hunk">Hersir couldn't read this commit's lines.</span>];
    const file = patches.files[path];
    if (!file) return [<span key="n" className="dd-dl hunk">No lines were recorded for this file.</span>];
    const kind = (l: string) => (l.startsWith("@@") ? " hunk" : l.startsWith("+") ? " add" : l.startsWith("-") ? " del" : "");
    return [...file.lines.map((l, n) => <span key={n} className={`dd-dl${kind(l)}`}>{l || " "}</span>),
      ...(file.truncated ? [<span key="t" className="dd-dl hunk">Cut short after {file.lines.length} lines.</span>] : [])];
  };
  return (
    <details className="dd-fold" onToggle={(ev) => (ev.currentTarget.open ? open() : undefined)}>
      <summary>
        <ChevronRight className="icon dd-chev" />
        <span className="mono">{c.hash}</span>
        <span className="grow">{c.subject}</span>
        <span className="sm muted">{when(c.date)} · {c.author} · {plural(c.files.length, "file", "files")}</span>
      </summary>
      <div className="dd-rows">
        {c.files.length === 0 && <div className="dd-row"><span className="grow sm muted">A merge: git lists no files of its own for it.</span></div>}
        {c.files.map((f) => {
          const inner = <>
            <KindBadge kind={TOUCHED[f.status] ?? "changed"} />
            <span className="grow mono sm">{f.path}{f.from && <span className="muted"> from {f.from}</span>}</span>
            {files.has(f.path) && <a className="sm" href={link({ ...fresh("reference"), folder: f.path })}>Open in Reference <ArrowRight className="icon sm" /></a>}
          </>;
          return f.diff
            ? <details key={f.path} className="dd-fold"><summary className="dd-row"><ChevronRight className="icon sm dd-chev" />{inner}</summary><pre className="dd-diff">{diff(f.path)}</pre></details>
            : <div key={f.path} className="dd-row"><span className="dd-chev-gap" />{inner}</div>;
        })}
      </div>
    </details>
  );
}

/* ---------- MCP ---------- */

export function Mcp({ id }: { id: string }) {
  const { index, link } = useDocs();
  const servers = index.mcp;
  const tools = index.mcpTools;
  if (servers.length === 0 && tools.length === 0) return <EmptySection name="MCP connections" source=".mcp.json" />;
  const tool = tools.find((t) => t.id === id);
  if (tool) return <McpToolPage tool={tool} all={tools} />;
  const sorts = new Map<string, McpTool[]>();
  for (const t of tools) sorts.set(t.sort, [...(sorts.get(t.sort) ?? []), t]);
  return (
    <div className="dd-page">
      <div className="dd-head">
        <div className="dd-crumb">MCP</div>
        <h1>{servers.length === 0 ? "MCP tools" : "MCP connections"}</h1>
        <p className="dd-lead">{servers.length === 0 ? "The tools this project's own server offers, read from the classes that declare them."
          : <>The servers agents use on this project, read from <span className="mono">.mcp.json</span>{tools.length > 0 ? ", and the tools this project's own server offers" : ""}.</>}</p>
      </div>
      {servers.length > 0 && (
        <div className="dd-callout info"><Info className="icon" />
          <div><b>Values are never shown.</b> Environment variables and headers are listed by name only, and a credential in a URL or an argument is replaced by a placeholder.</div></div>
      )}
      {servers.map((m) => (
        <section key={m.name} className="dd-panel wide">
          <h2><Plug className="icon" /> {m.name} <KindBadge kind={m.transport} /></h2>
          <table className="dd-facts"><tbody>
            <tr><td>{m.transport === "http" ? "URL" : "Command"}</td><td className="mono">{m.target}</td></tr>
            {m.args.length > 0 && <tr><td>Arguments</td><td className="mono">{m.args.join(" ")}</td></tr>}
            {m.envNames.length > 0 && <tr><td>Environment</td><td className="mono">{m.envNames.map((n) => `\${${n}}`).join(", ")} <span className="muted sm">names only</span></td></tr>}
            {m.headerNames.length > 0 && <tr><td>Headers</td><td className="mono">{m.headerNames.join(", ")} <span className="muted sm">names only</span></td></tr>}
          </tbody></table>
          {m.purpose ? <Prose html={m.purpose} /> : <p className="sm muted">No notes. A heading named after this server in <span className="mono">docs/mcp.md</span> says what it is for.</p>}
        </section>
      ))}
      {[...sorts].map(([sort, items]) => (
        <div key={sort}>
          <h2>{plural(items.length, sort, `${sort}s`)} this project offers</h2>
          <div className="dd-rows">
            {items.map((t) => (
              <a key={t.id} className="dd-row" href={link(fresh("mcp", t.id))}>
                <span className="grow">
                  <span className="mono dd-name">{t.name}</span> {t.annotations.some((a) => a.name === "IsReadOnly") && <KindBadge kind="read only" />}
                  <span className="sm dd-block">{t.description.split(". ")[0]}{t.description.includes(". ") ? "." : ""}</span>
                </span>
                <span className="sm muted">{t.inputs.length ? plural(t.inputs.length, "input", "inputs") : "no inputs"}</span>
              </a>
            ))}
          </div>
        </div>
      ))}
    </div>
  );
}

function McpToolPage({ tool, all }: { tool: McpTool; all: McpTool[] }) {
  const { index, link } = useDocs();
  const entry = index.entries.find((e) => e.id === tool.entry);
  const call = JSON.stringify({ name: tool.name, arguments: Object.fromEntries(tool.inputs.map((i) => [i.name, `<${i.type}>`])) }, null, 2);
  return (
    <div className="dd-doc">
      <nav className="dd-doc-nav">
        <h3>{plural(all.length, "tool", "tools")}</h3>
        {all.map((t) => <a key={t.id} className={`dd-doc-link${t.id === tool.id ? " on" : ""}`} href={link(fresh("mcp", t.id))}><span className="mono sm">{t.name}</span></a>)}
        <h3>Connections</h3>
        <a className="dd-doc-link" href={link(fresh("mcp"))}>All servers</a>
      </nav>
      <article className="dd-doc-main">
        <div className="dd-crumb">MCP <span className="sep">/</span> {tool.sort}</div>
        <h1 className="mono">{tool.name}</h1>
        <div className="dd-byline"><span>{tool.title}</span>
          {tool.annotations.map((a) => { const name = a.name.replace(/^Is/, "").toLowerCase(); return <KindBadge key={a.name} kind={a.value === "false" ? `not ${name}` : name} />; })}</div>
        {tool.description ? <div className="dd-prose"><p>{tool.description}</p></div>
          : <div className="dd-nodesc-dd-block"><b>No description on this tool</b><p className="sm muted">An agent calling it is told nothing about what it does. A Description attribute says it.</p></div>}
        <h2>Input schema</h2>
        {tool.inputs.length === 0 ? <p className="sm muted">Takes no input.</p> : (
          <table className="dd-facts"><tbody>
            {tool.inputs.map((i) => <tr key={i.name}><td className="mono">{i.name}{i.required && <span className="dd-req">required</span>}</td><td><span className="sm muted mono">{i.type}</span> {i.description}</td></tr>)}
          </tbody></table>
        )}
        <h2>Calling it</h2>
        <CodeBlock lines={call.split("\n")} />
        <p className="sm muted">There is no console here: Hersir reads the declaration and never calls it.</p>
      </article>
      <aside className="dd-rail">
        {tool.file && <><h3>Declared in</h3>{entry && <a className="dd-rail-link mono" href={link(fresh("reference", entry.id))}>{entry.name}</a>}<p className="sm muted mono">{tool.file}</p></>}
        <h3>For agents</h3>
        <AgentLink className="dd-rail-link mono" path="md/mcp.md"><LinkIcon className="icon sm" /> Markdown</AgentLink>
      </aside>
    </div>
  );
}

/* ---------- API ---------- */

function FieldTable({ rows, small }: { rows: Field[]; small?: boolean }) {
  if (rows.length === 0) return null;
  return (
    <table className={`dd-facts${small ? " sm" : ""}`}><tbody>
      {rows.map((f) => <tr key={f.name}><td className="mono">{f.name} <span className="sm muted">{f.detail}</span>{f.required && <span className="dd-req">required</span>}</td><td>{f.description}</td></tr>)}
    </tbody></table>
  );
}

export function ApiPage({ id }: { id: string }) {
  const { index, link } = useDocs();
  const spec = index.api;
  const current = spec?.endpoints.find((e) => e.id === id) ?? spec?.endpoints[0];
  if (!spec || !current) return <EmptySection name="API reference" source="openapi.json, or the project's routes" />;
  const want = `${current.method} ${current.path}`;
  const handler = index.entries.find((e) => e.facts.some((f) => f.label === "Route" && (f.value === want || f.value.startsWith(`${want} `))));
  const byTag = new Map<string, typeof spec.endpoints>();
  for (const e of spec.endpoints) byTag.set(e.tags[0] ?? "Endpoints", [...(byTag.get(e.tags[0] ?? "Endpoints") ?? []), e]);
  const body = bodyOf(index, current.requestBody);
  return (
    <div className="dd-doc">
      <nav className="dd-doc-nav">
        {[...byTag].map(([tag, items]) => (
          <div key={tag}><h3>{tag}</h3>
            {items.map((e) => <a key={e.id} className={`dd-doc-link${e.id === current.id ? " on" : ""}`} href={link(fresh("api", e.id))}><MethodBadge method={e.method} /><span className="mono sm">{e.path}</span></a>)}</div>
        ))}
      </nav>
      <article className="dd-doc-main">
        <div className="dd-crumb">API <span className="sep">/</span> {current.tags[0] ?? "Endpoints"}</div>
        <h1>{current.summary || current.path}</h1>
        <div className="dd-endpoint"><MethodBadge method={current.method} /><span className="mono">{current.path}</span></div>
        {current.description && <div className="dd-prose"><p>{current.description}</p></div>}
        {current.auth.length > 0 && <table className="dd-facts"><tbody><tr><td>Auth</td><td className="mono">{current.auth.join(", ")}</td></tr></tbody></table>}
        {current.parameters.length > 0 && (
          <><h2>Parameters</h2>
            <table className="dd-facts"><tbody>
              {current.parameters.map((p) => <tr key={`${p.in}:${p.name}`}><td className="mono">{p.name}{p.required && <span className="dd-req">required</span>}</td><td><span className="sm muted">{p.in}</span> {p.description}</td></tr>)}
            </tbody></table></>
        )}
        {body && <><h2>Body <span className="mono sm dd-inline-code">{body.type}</span></h2><FieldTable rows={body.fields} /></>}
        <h2>Responses</h2>
        {current.responses.length > 0 ? (
          <table className="dd-facts"><tbody>
            {current.responses.map((r) => <tr key={r.status}><td className="mono">{r.status}</td><td>{r.description}<FieldTable rows={fieldsOf(index, r.schema)} small /></td></tr>)}
          </tbody></table>
        ) : <p className="sm muted">This endpoint was read from the project's routes, which give the method and the path and nothing else. Its status codes and bodies are in the handler.</p>}
      </article>
      <aside className="dd-rail">
        <h3>Example</h3>
        <pre className="dd-code sm"><code>{current.method} {current.path}</code></pre>
        {handler && <><h3>Handler</h3><a className="dd-rail-link mono" href={link(fresh("reference", handler.id))}>{handler.file}</a></>}
      </aside>
    </div>
  );
}
