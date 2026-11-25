// A small docs index for the Docs page's tests: a Laravel-ish project with an entry of each sort, a guide, a decision, a
// changelog, an MCP server and a tool, an API with a request body, a commit, and one entry that moved.
import type { Index } from "./types";

export function sampleIndex(patch: Partial<Index> = {}): Index {
  return {
    schema: 1,
    project: { title: "Kade", builtAt: "2026-11-24T08:13:47Z", commit: "abc1234", branch: "main", yodocsVersion: "0.20.1", probes: [], changelogFrom: "CHANGELOG.md and the merged cards" },
    stats: { files: 2, entries: 3, described: 1, problems: 0, durationMs: 1200 },
    kinds: [{ name: "class", count: 1 }, { name: "method", count: 1 }, { name: "controller", count: 1 }],
    groups: [
      { name: "Folders", type: "tree", count: 2, rows: [{ label: "app", target: "app", count: 3, children: [
        { label: "User.php", target: "app/User.php", count: 2 }, { label: "UserController.php", target: "app/UserController.php", count: 1 }] }] },
      { name: "Routes", type: "flat", count: 1, rows: [{ label: "/users", target: "app/UserController.php#UserController", count: 0, badge: "GET" }] },
    ],
    entries: [
      { id: "app/User.php#User", name: "User", kind: "class", syntacticKind: "class", file: "app/User.php", lineStart: 3, lineEnd: 9, nameLine: 3,
        signature: "class User extends Model", description: "A person who signs in. Owns invoices.", hasDescription: true,
        tags: [{ name: "see", text: "Invoice" }], facts: [{ label: "Extends", value: "Model" }], children: ["app/User.php#User.owner"] },
      { id: "app/User.php#User.owner", name: "owner", kind: "method", syntacticKind: "method", file: "app/User.php", lineStart: 5, lineEnd: 7, nameLine: 5,
        signature: "public function owner()", description: "", hasDescription: false, tags: [], facts: [], parent: "app/User.php#User", children: [] },
      { id: "app/UserController.php#UserController", name: "UserController", kind: "controller", syntacticKind: "class", file: "app/UserController.php",
        lineStart: 1, lineEnd: 4, nameLine: 1, signature: "class UserController", description: "", hasDescription: false, tags: [],
        facts: [{ label: "Route", value: "GET /users · index" }], children: [] },
    ],
    problems: [],
    guides: [
      { id: "readme", title: "Kade", group: "", order: 0, file: "README.md", updated: "2026-11-20T10:00:00Z",
        html: '<h1 id="kade">Kade</h1><p>Invoices for teams. See <a href="#install">Install</a> or <a href="https://example.com">the site</a>.</p><h2 id="install">Install</h2><p>Run it.</p>',
        headings: [{ level: 1, text: "Kade", slug: "kade" }, { level: 2, text: "Install", slug: "install" }] },
    ],
    changelog: [
      { version: "1.1.0", date: "2026-11-21", unreleased: false, compareUrl: "", changes: [
        { type: "Added", text: "Invoices export as CSV.", html: "Invoices export as CSV." },
        { type: "Fixed", text: "Totals round once.", html: "Totals round <em>once</em>." }] },
    ],
    decisions: [
      { id: "0001", number: "0001", title: "Use Postgres", date: "2026-11-23", status: "accepted", relatedFiles: ["app/User.php", "gone.php"],
        file: "Hersir (decided by Chief)", html: "<h2 id=\"context\">Context</h2><p>One database.</p>", headings: [{ level: 2, text: "Context", slug: "context" }] },
    ],
    mcp: [{ name: "github", transport: "http", target: "https://api.example.com/mcp?token=${TOKEN}", args: [], envNames: ["GITHUB_TOKEN"], headerNames: [], purpose: "<p>Reads issues.</p>" }],
    mcpTools: [{ id: "mcp/tool/search_users", name: "search_users", title: "Search users", description: "Finds users by name. Read only.",
      annotations: [{ name: "IsReadOnly", value: "" }], inputs: [{ name: "q", type: "string", description: "Words to find", required: true }],
      sort: "tool", file: "app/Mcp/SearchUsers.php", entry: "app/User.php#User" }],
    api: { title: "Kade API", version: "1", file: "openapi.json", schemas: { User: { type: "object", required: ["name"], properties: { name: { type: "string", description: "Full name" }, role: { type: "string", enum: ["admin", "member"] } } } },
      endpoints: [{ id: "post-users", method: "POST", path: "/users", summary: "Add a user", description: "", tags: ["Users"], auth: ["bearer"], parameters: [],
        requestBody: { content: { "application/json": { schema: { $ref: "#/components/schemas/User" } } } },
        responses: [{ status: "201", description: "Added", schema: { $ref: "#/components/schemas/User" } }] }] },
    docChanges: { since: { commit: "abc1234", takenAt: "2026-11-24T08:00:00Z" }, added: [], removed: [], documented: [{ id: "app/User.php#User", name: "User", file: "app/User.php", kind: "class" }], undocumented: [], changed: [], total: 1 },
    history: [{ hash: "abc1234", date: "2026-11-24T08:13:47Z", author: "Remi", subject: "Add users", files: [{ status: "A", path: "app/User.php", diff: true }, { status: "M", path: "logo.png", diff: false }] }],
    ...patch,
  };
}
