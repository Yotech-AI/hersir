// Where the Docs page is: a section, the item open in it, and the Reference's filters. Every entry and every line has an
// address that holds on a cold reload, so the address is the state. Written as a path with a query, after the host's
// own prefix: #/ in a browser, #/devdocs/<project id>/ in Hersir's window.
//   reference/app%2FModels%2FUser.php%23User@L51   reference?kind=class,model&folder=app   decisions/0003   guides/install

export const SECTIONS = ["home", "guides", "reference", "api", "changelog", "decisions", "mcp", "commits"] as const;
export type Section = (typeof SECTIONS)[number];

export type DocsRoute = {
  tab: Section;
  /** The entry, guide, decision, endpoint or MCP tool open in the section. */
  item: string;
  /** Reference: the folder or file the list is narrowed to. */
  folder: string;
  /** Reference: the kinds picked; Changelog: the change types picked. */
  kinds: string[];
  /** Reference: "no" for only the entries with nothing written above them, "yes" for only those with something. */
  described: "" | "yes" | "no";
  /** Reference: how the entries moved since the last commit, or "any". */
  changed: string;
  /** Reference: the list's page, from 1. */
  page: number;
  /** The source line to mark, from @L51 on the item. */
  line: number;
};

export const HOME: DocsRoute = { tab: "home", item: "", folder: "", kinds: [], described: "", changed: "", page: 1, line: 0 };

/** The route a docs path names; an unknown section is Home. */
export function parseDocsPath(path: string): DocsRoute {
  const [head = "", query = ""] = path.replace(/^\/+/, "").split("?");
  const params = new URLSearchParams(query);
  const parts = head.split("/").filter((p) => p !== "");
  const tab = SECTIONS.find((s) => s === parts[0]) ?? "home";
  let item = "";
  try {
    item = parts.length > 1 ? decodeURIComponent(parts.slice(1).join("/")) : "";
  } catch {
    item = parts.slice(1).join("/");
  }
  const atLine = /@L(\d+)$/.exec(item);
  const described = params.get("described");
  return {
    tab,
    item: atLine ? item.slice(0, atLine.index) : item,
    folder: params.get("folder") ?? "",
    kinds: (params.get("kind") ?? "").split(",").filter((k) => k !== ""),
    described: described === "yes" || described === "no" ? described : "",
    changed: params.get("changed") ?? "",
    page: Math.max(1, Number(params.get("page") ?? 1) || 1),
    line: atLine ? Number(atLine[1]) : 0,
  };
}

/** The docs path of `route` changed by `patch`. A change to what is listed goes back to page 1 unless it names the page. */
export function docsPath(route: DocsRoute, patch: Partial<DocsRoute> = {}): string {
  const next: DocsRoute = { ...route, page: "page" in patch ? route.page : 1, line: 0, ...patch };
  const params = new URLSearchParams();
  if (next.folder) params.set("folder", next.folder);
  if (next.kinds.length > 0) params.set("kind", next.kinds.join(","));
  if (next.described) params.set("described", next.described);
  if (next.changed) params.set("changed", next.changed);
  if (next.page > 1) params.set("page", String(next.page));
  const query = params.toString();
  const item = next.item ? `/${encodeURIComponent(next.item + (next.line > 0 ? `@L${next.line}` : ""))}` : "";
  return `${next.tab === "home" ? "" : next.tab}${item}${query ? `?${query}` : ""}`;
}

/** A section opened afresh: its item and every filter cleared. */
export const fresh = (tab: Section, item = ""): Partial<DocsRoute> => ({ tab, item, folder: "", kinds: [], described: "", changed: "" });
