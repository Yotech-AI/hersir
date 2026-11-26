// What the Docs page works out from the index: its tabs, the Reference's filters, an endpoint's fields, the search and
// the small wording helpers. No drawing here, so each piece can be tested on its own.
import type { DocsRoute, Section } from "./route";
import type { Entry, GroupRow, Index, Schema } from "./types";

/** The top bar's sections: id, label, how many, without the ones the project switched off. Home has no count. */
export function tabs(index: Index): { id: Section; label: string; count: number | null }[] {
  const all: { id: Section; label: string; count: number | null }[] = [
    { id: "home", label: "Home", count: null },
    { id: "guides", label: "Guides", count: index.guides.length },
    { id: "reference", label: "Reference", count: index.entries.length },
    { id: "api", label: "API", count: index.api ? index.api.endpoints.length : 0 },
    { id: "changelog", label: "Changelog", count: index.changelog.reduce((n, r) => n + r.changes.length, 0) },
    { id: "decisions", label: "Decisions", count: index.decisions.length },
    // servers and tools: a project with tools of its own and no .mcp.json still has an MCP page
    { id: "mcp", label: "MCP", count: index.mcp.length + index.mcpTools.length },
  ];
  return all.filter((t) => !isOff(index, t.id));
}

/** The switch (as Hersir names it) that left `section` out of this project's docs, or null while it shows. */
export const offKey = (index: Index, section: string): string | null =>
  (index.switchedOff ?? []).find((o) => o.section === section)?.key ?? null;
export const isOff = (index: Index, section: string) => offKey(index, section) !== null;

/** What each bucket of "Changed in the docs" is called, in the list, on Home and on an entry. */
export const MOVED_LABEL: Record<string, string> = {
  documented: "now documented", undocumented: "lost its description", added: "new", changed: "declaration changed", removed: "gone",
};

const movedCache = new WeakMap<Index, Map<string, string>>();
/** How each entry moved since the last commit, by entry id; built once per index. */
export function moved(index: Index): Map<string, string> {
  const cached = movedCache.get(index);
  if (cached) return cached;
  const map = new Map<string, string>();
  const d = index.docChanges;
  if (d) for (const bucket of ["documented", "undocumented", "added", "changed"] as const) for (const e of d[bucket]) map.set(e.id, bucket);
  movedCache.set(index, map);
  return map;
}

/** The Reference's entries after the folder, kind, description and moved filters. */
export function filtered(index: Index, r: DocsRoute): Entry[] {
  const how = r.changed === "" ? null : moved(index);
  return index.entries.filter((e) =>
    (r.folder === "" || e.file === r.folder || e.file.startsWith(`${r.folder}/`))
    && (r.kinds.length === 0 || r.kinds.includes(e.kind))
    && (r.described === "" || (r.described === "no" ? !e.hasDescription : e.hasDescription))
    && (how === null || (r.changed === "any" ? how.has(e.id) : how.get(e.id) === r.changed)));
}

/** The sidebar's row for folder `target`, or null when there is no such folder. */
export function folderRow(index: Index, target: string): GroupRow | null {
  const find = (rows: GroupRow[]): GroupRow | null => {
    for (const row of rows) {
      if (row.target === target) return row;
      const below = row.children ? find(row.children) : null;
      if (below) return below;
    }
    return null;
  };
  for (const g of index.groups) {
    const row = g.type === "tree" ? find(g.rows) : null;
    if (row) return row;
  }
  return null;
}

/** The files under a tree row, leaves only. */
export function filesUnder(rows: GroupRow[]): string[] {
  return rows.flatMap((r) => (r.children && r.children.length > 0 ? filesUnder(r.children) : [r.target]));
}

/** Every file the Reference can open. */
export const indexedFiles = (index: Index) => new Set(index.groups.filter((g) => g.type === "tree").flatMap((g) => filesUnder(g.rows)));

/** A schema with its $ref followed in the spec's own schemas (at most five deep). */
export function resolved(index: Index, schema: unknown): Schema | null {
  let s = schema;
  for (let n = 0; isSchema(s) && typeof s.$ref === "string" && n < 5; n++) s = index.api?.schemas[s.$ref.split("/").pop() ?? ""] ?? null;
  return isSchema(s) ? s : null;
}
const isSchema = (s: unknown): s is Schema => typeof s === "object" && s !== null && !Array.isArray(s);

export type Field = { name: string; detail: string; required: boolean; description: string };
/** An object schema's fields (an array's items' fields): name, type with its format or allowed values, required, description. */
export function fieldsOf(index: Index, schema: unknown): Field[] {
  const s = resolved(index, schema);
  if (!s) return [];
  if (s.type === "array") return fieldsOf(index, s.items);
  const required = Array.isArray(s.required) ? (s.required as unknown[]) : [];
  const props = isSchema(s.properties) ? s.properties : {};
  const refName = (x: unknown) => (isSchema(x) && typeof x.$ref === "string" ? x.$ref.split("/").pop() ?? "" : "");
  return Object.entries(props).map(([name, raw]) => {
    const f = resolved(index, raw) ?? {};
    const type = f.type === "array" ? `array of ${String(resolved(index, f.items)?.type ?? (refName(f.items) || "items"))}` : String(f.type ?? refName(raw));
    const detail = [type, typeof f.format === "string" ? f.format : "", Array.isArray(f.enum) ? f.enum.join(" | ") : ""].filter(Boolean).join(" · ");
    return { name, detail, required: required.includes(name), description: typeof f.description === "string" ? f.description : "" };
  });
}

/** The request body's media type and fields, when the spec gives a schema for it. */
export function bodyOf(index: Index, body: Schema | null): { type: string; fields: Field[] } | null {
  const content = body && isSchema(body.content) ? body.content : null;
  const first = content ? Object.entries(content)[0] : undefined;
  if (!first) return null;
  const fields = isSchema(first[1]) ? fieldsOf(index, first[1].schema) : [];
  return fields.length > 0 ? { type: first[0], fields } : null;
}

/** Rendered HTML without a leading heading that repeats `title`: nearly every Markdown file starts with its title. */
export function withoutLeadHeading(html: string, title: string): string {
  const m = /^\s*<h1[^>]*>([\s\S]*?)<\/h1>/.exec(html);
  if (!m) return html;
  return (m[1] ?? "").replace(/<[^>]+>/g, "").trim().toLowerCase() === title.trim().toLowerCase() ? html.slice(m[0].length) : html;
}

/** The headings for "On this page", down to level 3, without that same leading title. */
export function railHeadings<T extends { level: number; text: string }>(headings: T[], title: string): T[] {
  const out = headings.filter((h) => h.level <= 3);
  const first = out[0];
  return first && first.level === 1 && first.text.trim().toLowerCase() === title.trim().toLowerCase() ? out.slice(1) : out;
}

export const plural = (n: number, one: string, many: string) => `${n} ${n === 1 ? one : many}`;

/** The first sentence of a description, for a list row, cut at a word near `limit`. */
export function snippet(text: string, limit = 140): string {
  const flat = text.split("\n\n")[0]?.replace(/\s+/g, " ").trim() ?? "";
  const stop = flat.search(/[.!?](\s|$)/);
  const sentence = stop !== -1 && stop < limit ? flat.slice(0, stop + 1) : flat;
  if (sentence.length <= limit) return sentence;
  const cut = sentence.slice(0, limit);
  return `${cut.slice(0, cut.lastIndexOf(" "))}…`;
}

/** A date and time nobody can read the wrong way round, or "" for none. */
export function when(iso: string): string {
  const d = new Date(iso);
  return Number.isNaN(d.getTime()) ? "" : d.toLocaleString(undefined, { day: "numeric", month: "short", year: "numeric", hour: "2-digit", minute: "2-digit" });
}

/** A kind's colour, from Hersir's palette: kinds are per project, so a pattern picks the family and the rest are grey. */
const KIND_COLOURS: [RegExp, string][] = [
  [/model|twill module/, "--c-violet"], [/migration/, "--c-orange"], [/route|endpoint|added/, "--c-teal"],
  [/resource|repository|changed/, "--c-blue"], [/component|widget|block|view|page/, "--c-pink"], [/config|provider|setting|server|deprecated/, "--c-yellow"],
  [/test|factory|seeder/, "--c-grey"], [/method|function|hook|property|constant|fixed/, "--c-teal"], [/removed|security/, "--c-red"],
  [/class|interface|trait|enum|controller|middleware|job|command|exception/, "--c-blue"],
];
export function kindColour(kind: string): string {
  const found = KIND_COLOURS.find(([re]) => re.test(kind.toLowerCase()));
  return `var(${found ? found[1] : "--c-grey"})`;
}

/* ---------- search ---------- */

export type SearchSection = "reference" | "guides" | "api" | "changelog" | "decisions" | "mcp";
export type SearchDoc = { id: string; section: SearchSection; title: string; subtitle: string; kind: string; body: string };

/** Rendered HTML back to words. */
const words = (html: string) => html.replace(/<[^>]+>/g, " ").replace(/&[a-z#0-9]+;/gi, " ").replace(/\s+/g, " ").trim();

/** One search document per entry, guide, decision, change, MCP server and endpoint (yodocs' own list). */
export function searchDocs(index: Index): SearchDoc[] {
  const docs: SearchDoc[] = [];
  for (const e of index.entries) {
    docs.push({ id: e.id, section: "reference", title: e.name, subtitle: e.file, kind: e.kind, body: `${e.signature} ${e.description} ${e.tags.map((t) => t.text).join(" ")}` });
  }
  for (const g of index.guides) docs.push({ id: g.id, section: "guides", title: g.title, subtitle: g.file, kind: "guide", body: `${g.headings.map((h) => h.text).join(" ")} ${words(g.html)}` });
  for (const d of index.decisions) docs.push({ id: d.id, section: "decisions", title: `${d.number} ${d.title}`, subtitle: d.file, kind: d.status, body: words(d.html) });
  for (const r of index.changelog) {
    r.changes.forEach((c, i) => docs.push({ id: `changelog/${r.version}/${i}`, section: "changelog", title: c.text, subtitle: `${r.version} ${r.date}`.trim(), kind: c.type.toLowerCase(), body: "" }));
  }
  for (const m of index.mcp) docs.push({ id: `mcp/${m.name}`, section: "mcp", title: m.name, subtitle: m.target, kind: m.transport, body: `${m.args.join(" ")} ${m.envNames.join(" ")} ${words(m.purpose)}` });
  for (const t of index.mcpTools) docs.push({ id: t.id, section: "mcp", title: t.name, subtitle: t.file, kind: t.sort, body: `${t.title} ${t.description}` });
  for (const e of index.api?.endpoints ?? []) {
    docs.push({ id: e.id, section: "api", title: `${e.method} ${e.path}`, subtitle: e.tags.join(", "), kind: e.method.toLowerCase(), body: `${e.summary} ${e.description}` });
  }
  return docs.filter((d) => !isOff(index, d.section));
}

const terms = (text: string) => text.toLowerCase().split(/[^\p{L}\p{N}_$]+/u).filter((t) => t !== "");

/** The documents that match every word of `query`, best first: a word that starts a word of the title counts most, then
 *  the subtitle's, then the body's; a word found inside another word counts less than one that starts it. */
export function search(docs: SearchDoc[], query: string, limit = 40): SearchDoc[] {
  const want = terms(query);
  if (want.length === 0) return [];
  const scored: { doc: SearchDoc; score: number }[] = [];
  for (const doc of docs) {
    const fields: [string[], number][] = [[terms(doc.title), 4], [terms(doc.subtitle), 2], [terms(doc.body), 1]];
    let score = 0;
    for (const w of want) {
      let best = 0;
      for (const [ts, weight] of fields) {
        for (const t of ts) {
          const s = t === w ? weight * 3 : t.startsWith(w) ? weight * 2 : t.includes(w) ? weight : 0;
          if (s > best) best = s;
        }
      }
      if (best === 0) { score = 0; break; }
      score += best;
    }
    if (score > 0) scored.push({ doc, score });
  }
  return scored.sort((a, b) => b.score - a.score || a.doc.title.length - b.doc.title.length).slice(0, limit).map((s) => s.doc);
}
