// The Docs page's shared pieces: what every section reads (the index, the address, the host), badges, code, Markdown the
// index carries as HTML, and the empty states.
import { createContext, useContext, useEffect, useState, type MouseEvent, type ReactNode } from "react";
import { Check, Copy, Inbox } from "lucide-react";
import { docsPath, type DocsRoute } from "./route";
import { kindColour } from "./derive";
import type { DocsHost, Index } from "./types";

export type DocsCtx = {
  index: Index;
  route: DocsRoute;
  host: DocsHost;
  /** The href of this page with the route changed by `patch`. */
  link: (patch: Partial<DocsRoute>) => string;
  /** Goes there now. */
  go: (patch: Partial<DocsRoute>) => void;
};
const Ctx = createContext<DocsCtx | null>(null);
export const DocsProvider = Ctx.Provider;
export function useDocs(): DocsCtx {
  const c = useContext(Ctx);
  if (!c) throw new Error("useDocs outside the Docs page");
  return c;
}

/** What `load` gives, once it has; for the page's own reads (the app's useData listens for Tauri's events, which a
 *  browser hasn't got). A newer `key` drops an older answer. */
export function useLoad<T>(load: () => Promise<T>, key: string): { data: T | null; error: string | null } {
  const [state, setState] = useState<{ key: string; data: T | null; error: string | null }>({ key: "", data: null, error: null });
  useEffect(() => {
    let live = true;
    load().then((data) => live && setState({ key, data, error: null }), (e: unknown) => live && setState({ key, data: null, error: String(e) }));
    return () => { live = false; };
  }, [key]);
  return state.key === key ? state : { data: null, error: null };
}

/** The context's link and go, from the host's own prefix (#/ in a browser, #/devdocs/<id>/ in Hersir). */
export function makeLinks(route: DocsRoute, prefix: string, navigate: (hash: string) => void) {
  const link = (patch: Partial<DocsRoute>) => `${prefix}${docsPath(route, patch)}`;
  return { link, go: (patch: Partial<DocsRoute>) => navigate(link(patch)) };
}

export const KindBadge = ({ kind }: { kind: string }) => (
  <span className="dd-kind" style={{ ["--dd-kind" as string]: kindColour(kind) }}>{kind}</span>
);

export const MethodBadge = ({ method }: { method: string }) => <span className={`dd-method ${method.toLowerCase()}`}>{method.toUpperCase()}</span>;

/** Lines `from` to `to` of a file, numbered, with line `mark` marked. */
export function CodeBlock({ lines, from = 1, to = lines.length, mark }: { lines: string[]; from?: number; to?: number; mark?: number }) {
  const rows: ReactNode[] = [];
  for (let n = from; n <= to && n <= lines.length; n++) {
    rows.push(<span key={n} className={`dd-line${n === mark ? " hit" : ""}`}><span className="dd-ln">{n}</span>{lines[n - 1] || " "}</span>);
  }
  return <pre className="dd-code"><code>{rows}</code></pre>;
}

/** HTML from the index: Markdown Hersir rendered in Rust, with raw HTML escaped and script links disarmed
 *  (crates/gizai-docs/src/markdown.rs). A link inside the page scrolls to its heading; one that leaves it opens outside,
 *  never in Hersir's window. */
export function Prose({ html, className = "" }: { html: string; className?: string }) {
  const { host } = useDocs();
  const onClick = (ev: MouseEvent<HTMLDivElement>) => {
    const a = (ev.target as HTMLElement).closest("a");
    const href = a?.getAttribute("href");
    if (!a || href == null) return;
    ev.preventDefault();
    if (href.startsWith("#")) document.getElementById(decodeURIComponent(href.slice(1)))?.scrollIntoView({ block: "start" });
    else if (/^https?:\/\//i.test(href)) host.openExternal(href);
  };
  return <div className={`dd-prose ${className}`} onClick={onClick} dangerouslySetInnerHTML={{ __html: html }} />;
}

/** "On this page": links that scroll to a part of the page and leave the address alone. */
export function RailLinks({ items }: { items: { id: string; label: string; level?: number }[] }) {
  return (
    <>
      {items.map((it) => (
        <a key={it.id} className={`dd-rail-link lvl${it.level ?? 2}`} href={`#${it.id}`}
          onClick={(ev) => { ev.preventDefault(); document.getElementById(it.id)?.scrollIntoView({ block: "start" }); }}>{it.label}</a>
      ))}
    </>
  );
}

/** A file for agents next to the page (llms.txt, md/…), opened outside; plain text while its address isn't known. */
export function AgentLink({ path, children, className = "mono" }: { path: string; children: ReactNode; className?: string }) {
  const { host } = useDocs();
  const url = host.agentsUrl(path);
  if (!url) return <span className={className}>{children}</span>;
  return <a className={className} href={url} onClick={(ev) => { ev.preventDefault(); host.openExternal(url); }}>{children}</a>;
}

/** A button that copies `text`, and says so for a moment. */
export function CopyButton({ text, label, said }: { text: () => string | Promise<string>; label?: string; said: string }) {
  const { host } = useDocs();
  const [note, setNote] = useState<string | null>(null);
  const click = async () => {
    try {
      await host.copy(await text());
      setNote(said);
    } catch {
      setNote("Couldn't reach the clipboard");
    }
    setTimeout(() => setNote(null), 2200);
  };
  return (
    <button className="btn sm ghost" onClick={() => void click()} title={label ?? said}>
      {note ? <Check className="icon sm" /> : <Copy className="icon sm" />}{note ?? label}
    </button>
  );
}

export function Empty({ title, children, icon }: { title: string; children?: ReactNode; icon?: ReactNode }) {
  return (
    <div className="dd-empty">
      {icon ?? <Inbox className="icon" />}
      <h2>{title}</h2>
      {children}
    </div>
  );
}

/** A section with nothing in the repository yet, and where it would come from. */
export const EmptySection = ({ name, source }: { name: string; source: string }) => (
  <Empty title={`No ${name.toLowerCase()} yet`}>
    <p className="muted">Nothing was found in <span className="mono">{source}</span>. Add it to the repository and open this page again.</p>
  </Empty>
);

/** A path as crumbs, its last part in code type. */
export const PathCrumbs = ({ path }: { path: string }) => {
  const parts = path.split("/");
  return (
    <div className="dd-crumb">
      {parts.map((p, n) => (n === parts.length - 1 ? <span key={n} className="mono">{p}</span> : <span key={n}>{p} <span className="sep">/</span> </span>))}
    </div>
  );
};
