// A project's docs index, as Hersir builds it in Rust (crates/gizai-docs/src/index.rs, yodocs' schema 1): the one value
// the Docs page draws from. Fields are camelCase on the wire.

export type Heading = { level: number; text: string; slug: string };
export type SwitchedOff = { section: string; key: string };
export type ProjectInfo = { title: string; builtAt: string; commit: string; branch: string; yodocsVersion: string; probes: string[]; changelogFrom: string };
export type Stats = { files: number; entries: number; described: number; problems: number; durationMs: number };
export type KindCount = { name: string; count: number };
export type GroupRow = { label: string; target: string; count: number; badge?: string; children?: GroupRow[] };
export type Group = { name: string; type: "tree" | "flat"; count: number; rows: GroupRow[] };
export type Fact = { label: string; value: string; target?: string };
export type DocTag = { name: string; text: string };
export type Entry = {
  id: string; name: string; kind: string; syntacticKind: string; file: string; lineStart: number; lineEnd: number; nameLine: number;
  signature: string; description: string; hasDescription: boolean; tags: DocTag[]; facts: Fact[]; parent?: string; children: string[];
};
export type Problem = { file: string; message: string; line?: number };
export type Guide = { id: string; title: string; group: string; order: number; file: string; updated: string; html: string; headings: Heading[] };
export type Change = { type: string; text: string; html: string };
export type Release = { version: string; date: string; unreleased: boolean; compareUrl: string; changes: Change[] };
export type Decision = {
  id: string; number: string; title: string; date: string; status: string; relatedFiles: string[]; file: string; html: string; headings: Heading[];
};
export type McpServer = { name: string; transport: string; target: string; args: string[]; envNames: string[]; headerNames: string[]; purpose: string };
export type McpInput = { name: string; type: string; description: string; required: boolean };
export type McpTool = {
  id: string; name: string; title: string; description: string; annotations: { name: string; value: string }[]; inputs: McpInput[];
  sort: string; file: string; entry: string;
};
/** An OpenAPI schema, as the spec gives it. */
export type Schema = { [key: string]: unknown };
export type Parameter = { name: string; in: string; required: boolean; description: string };
export type ApiResponse = { status: string; description: string; schema: Schema | null };
export type Endpoint = {
  id: string; method: string; path: string; summary: string; description: string; tags: string[]; auth: string[];
  parameters: Parameter[]; requestBody: Schema | null; responses: ApiResponse[];
};
export type Api = { title: string; version: string; file: string; endpoints: Endpoint[]; schemas: Record<string, Schema> };
export type DocChange = { id: string; name: string; file: string; kind: string };
export type DocChanges = {
  since: { commit: string; takenAt: string }; added: DocChange[]; removed: DocChange[]; documented: DocChange[]; undocumented: DocChange[];
  changed: DocChange[]; total: number;
};
export type CommitFile = { status: string; path: string; from?: string; diff: boolean };
export type Commit = { hash: string; date: string; author: string; subject: string; files: CommitFile[] };

export type Index = {
  schema: number; project: ProjectInfo; stats: Stats; kinds: KindCount[]; groups: Group[]; entries: Entry[]; problems: Problem[];
  guides: Guide[]; changelog: Release[]; decisions: Decision[]; mcp: McpServer[]; mcpTools: McpTool[]; api: Api | null;
  docChanges: DocChanges | null; history: Commit[]; switchedOff?: SwitchedOff[];
};

/** A file's lines (source/<path>.json); none for a file the docs can't read. */
export type SourceFile = { path: string; lines: string[] };
/** A commit's changed lines per file (commits/<hash>.json). */
export type CommitPatches = { files: Record<string, { lines: string[]; truncated: boolean }> };

/** Where the page gets its data and how it acts outside itself: in Hersir's window (Tauri commands) or in a browser (the
 *  docs server, over HTTP). */
export type DocsHost = {
  index: () => Promise<Index>;
  source: (path: string) => Promise<SourceFile>;
  commit: (hash: string) => Promise<CommitPatches | null>;
  /** The address of a file for agents next to the page (llms.txt, md/…), or null while it isn't known. */
  agentsUrl: (path: string) => string | null;
  /** Opens a link that leaves the page: a new tab in a browser, the browser from Hersir's window. */
  openExternal: (url: string) => void;
  copy: (text: string) => Promise<void>;
  /** The full address of a link on this page, to copy; only a browser has one worth sharing. */
  shareable?: (href: string) => string;
  /** The browser has its own theme switch; Hersir's window follows Hersir's theme. */
  themeSwitch?: { theme: "dark" | "light"; toggle: () => void };
};
