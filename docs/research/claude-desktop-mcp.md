# Claude Desktop: the board as an MCP App (GA-343 spike)

GA-343, 23 Nov 2026. A spike: what Claude Desktop really does with an MCP App from a local server, before GA-345 builds
the full views. It builds on GA-338 (Claude Desktop's lasting token), GA-339 (`hersir-mcp --desktop`) and GA-341
(Claude Desktop works like the Chief). Agents can't run Claude Desktop, so Jefsev answers the checklist at the end after
the merge, on the Mac and on Linux.

## What MCP Apps are

MCP Apps is the MCP extension `io.modelcontextprotocol/ui` (protocol `2026-01-26`, `@modelcontextprotocol/ext-apps`
2.1.0). A tool names a view, a `ui://` resource: one HTML file with MIME type `text/html;profile=mcp-app`. When the
model calls the tool, the host (Claude) reads the resource and shows it in a sandboxed iframe in the chat, and talks to
it with JSON-RPC over `postMessage`. The view can call the server's tools through the host.

## What the spike built

### The server (`crates/gizai-mcp`)

- **Only for a client that declares it.** A client declares MCP Apps at `initialize`, in
  `capabilities.extensions["io.modelcontextprotocol/ui"]` (with `mimeTypes` that include `text/html;profile=mcp-app`, or
  none). Such a client gets, in the `initialize` answer, the `resources` capability and
  `extensions: {"io.modelcontextprotocol/ui": {"mimeTypes": ["text/html;profile=mcp-app"]}}`, then `resources/list`,
  `resources/read` and `resources/templates/list`. Any other client, Claude Code among them, gets exactly what it got
  before: no extension, no resources (`resources/list` is still "method not found"), the same tools.
- **A session per connection** (`Session`): what the client declared, from its `initialize` on. `handle` still answers
  one message as the first of a connection; `handle_in` and `serve_while` keep the session.
- **Claude Desktop's shim** answers `initialize` itself (GA-339), so it advertises the extension too, and tells Hersir
  what the client declared with a notification of its own, `notifications/hersir/client` (the client's `initialize`
  params): right away when connected, and between the hello and the check ping on every later connection. Only for a
  client that declared MCP Apps. While Hersir isn't running, the shim lists no resources, and a `resources/read` gets
  "Open Hersir first" as an error. When Hersir answers, it writes `notifications/resources/list_changed` after
  `notifications/tools/list_changed`.
- **`_meta` on tools** (`ToolDef::meta`, `app_meta`): `ui.resourceUri` and the older flat `ui/resourceUri`, and
  `ui.visibility`.
- **Results with more than text** (`Reply`, `Tools::reply`): `content` (one text), `structuredContent` and `_meta`.
- **Views** (`AppView`, `Tools::apps`): the `ui://` resources, with their `_meta.ui` in both the list and the read.

### Hersir's tools (`src-tauri/src/tools/apps.rs`)

For Claude Desktop's token (GA-341's `GizaiTools::desktop`), and only when it declared MCP Apps:

- `show_board {project?}`, for the model. One sentence in `content` ("The user sees the board of Giz AI (GA) from Hersir
  in this chat: 42 cards in 7 columns, 3 on hold."), the board in `structuredContent`, and both resourceUri keys in the
  tool's `_meta` and in the result's.
- `board_read {project?}`, for the view alone: `_meta.ui.visibility` is `["app"]`. Same answer, without the result's
  `_meta`.

The board: the project's team's columns in board order (each with its `total`), and the cards on them in board order:
only what a card on the board shows (identifier, title, column, priority, assignee, labels, on hold, by hand, waits for,
the agent working on it now). No description, hold reason, branch, pull request or path: some hosts show
`structuredContent` to the model. At most 50 cards a column, 20 in Backlog, Done and Cancelled, and fewer when the whole
would pass 100,000 characters; `onHold` counts the held cards among all of them. Without a project: the first team's board with all its projects' cards (as the Tasks
page shows it).

Every client that declared MCP Apps gets the view `ui://hersir/app.html` (its HTML holds no data); only Claude Desktop
gets the tools. The view's `_meta.ui` is `{"prefersBorder": true}`, with no `csp`: it reaches nothing.

### The view (`src/widget`, `mcp-app.html`)

- `npm run build` makes it (`vite build --mode mcp-app`, vite-plugin-singlefile) into one file,
  `src-tauri/mcp-app/mcp-app.html`: about 507 KB, every script, style and font inside. `src-tauri/build.rs` embeds it in
  Hersir. Built with cargo alone, Hersir serves a page that says the view wasn't built, and cargo warns.
- It draws the board with the app's own `src/components/Board.tsx`, read-only (no dragging).
- Its own bridge (`src/widget/bridge.ts`): `ui/initialize`, then `ui/notifications/initialized`; it takes
  `ui/notifications/tool-input`, `tool-result`, `tool-cancelled` and `host-context-changed`, and answers
  `ui/resource-teardown` and `ping`; it sends `tools/call`, `ui/request-display-mode` and
  `ui/notifications/size-changed` (from a ResizeObserver, as ext-apps' App measures it). `@modelcontextprotocol/ext-apps`
  gives only its types: its App class brings the MCP client, zod and ajv (1.28 MB in Otus).
- It asks `board_read` every 3 seconds while it is shown, one call at a time, slower (up to 15 s) after a failure, and
  shows why ("Open Hersir first: …").
- The theme follows `hostContext.theme` (the app's dark and light tokens); without one, the system's.
- Full screen (and back) is a button, shown when the host lists the mode: hosts allow it only after a click or a key.
- Fonts: Atkinson Hyperlegible Next and Mono, Latin only, one woff2 each, added with the FontFace API from their bytes.
  A host's default CSP for a view may leave out `font-src`, which would refuse a `data:` URL in `@font-face`.
- No network: no `fetch`, `XMLHttpRequest`, `WebSocket` or `EventSource` in its code or in the built file (Vite's
  modulepreload polyfill, which fetches, is off).

### The stand-in host (`mcp-host.html`, `src/widget-host`)

For development and QA only; neither the app's build nor the view's file has it. `npm run build:mcp-app` (or
`npm run build`), then `npm run mcp-host`, and open `http://127.0.0.1:1431` (or the run's preview port). It loads the
built view into an iframe with `sandbox="allow-scripts"` and `srcdoc`, plays Claude's side of the bridge, answers
`board_read` from a made-up board that changes every 6 s, and logs every message. Its panel switches the theme, turns
"Hersir is open" off (then `board_read` says "Open Hersir first"), mounts a new view as a new tool call does, and shows
the number of `board_read` calls, the view's height and the display mode.

## Hand-written server or rmcp 3.11

**Choice: keep the hand-written server for now.** Revisit when Hersir needs much more of MCP.

rmcp 3.11 (the official Rust SDK, Apache-2.0; GitButler's `but mcp` is built on it) has what MCP Apps need at the
protocol level: `_meta` on tools and on resource contents, and `ExtensionCapabilities` on both the client's and the
server's capabilities (its own example declares `io.modelcontextprotocol/ui` with `mimeTypes`). It has no MCP Apps
helpers: the view's resource, `_meta.ui`, `visibility` and the bridge would be ours either way.

Why the hand-written one stays:

- **It is small and does what Hersir needs.** About 340 lines (`crates/gizai-mcp/src/lib.rs` without its Windows pipe
  module) for `initialize`, `ping`, the tools and now the resources and the extension. MCP Apps added the session,
  `_meta`, `structuredContent` and the views: about 250 lines, the shim's part included.
- **Hersir's parts sit around the protocol, not in it:** the hello line with the token, the token checked before each
  message (`serve_while`, GA-338), the refusal line, Claude Code's `claudecode/toolUseId`, and the desktop shim, which
  passes lines on unchanged and answers `initialize` itself (GA-339). rmcp parses and writes every message again, and
  GA-339's tests check that lines pass unchanged.
- **The shim stays small.** rmcp brings tokio-util, futures, schemars, tracing and more (its default features: macros,
  server, schemars); `hersir-mcp` today needs serde_json and tokio.
- **No migration risk for the Chief.** Every tool of the Chief's chat turns and of Claude Desktop answers today as the
  tests of GA-338, GA-339 and GA-341 expect.

Move to rmcp when Hersir needs several of: prompts, resource subscriptions, sampling or elicitation from the server,
progress and cancellation, pagination, MCP's tasks, or a transport other than lines over the socket; or when a new MCP
version changes what `initialize` and the tools look like. Then wrap rmcp's server in the same hello and token check.

## Claude's limits (23 Nov 2026) and what the spike does about them

| Limit | What the spike does |
| --- | --- |
| A result over about 150,000 characters becomes a file pointer, and the view doesn't load. | The board stays under 100,000 characters: 50 cards a column at most (20 in folded ones), fewer when needed. |
| Tool calls end after 240 s; calls from a view after 45 s. | `board_read` is one database read. The view gives up on an answer after 60 s and tries again. |
| Fullscreen opens only after a click or a key press. | Full screen is a button. |
| Every tool call mounts a new iframe, so the state lives in Hersir. | The view keeps nothing; every view asks `board_read` for the board as it is. |
| Without a `csp`, the view can't reach anything. | It reaches nothing: everything is in its one file, and its data comes through `tools/call`. |

## Jefsev's checklist (after the merge)

Agents can't run Claude Desktop. With a Hersir built from main after this merge: Settings → MCP servers → Claude Desktop
→ Connect (GA-338), quit and reopen Claude Desktop, then ask in a new chat "Show me the Giz AI board in Hersir". The
answers decide GA-345.

| Check | Mac | Linux (Omarchy, Claude Desktop beta) |
| --- | --- | --- |
| Does the widget show inline? | | |
| Does fullscreen open after a click? | | |
| Is the side panel offered for a local server? | | |
| Does polling work? (Move a card in Hersir: the view shows it within a few seconds.) | | |
| Do the height and the dark theme look right? | | |

Notes:

-

## For GA-345

- `show_board` and `board_read` become `show {view, id?}` and `view_read`; the protocol parts (the session, the
  extension, `resources/*`, `_meta`, `Reply`, `AppView`, the shim's `notifications/hersir/client`) stay as they are.
- Every view in the chat keeps polling while its chat is open: many `show_board` calls in a long chat mean many views
  asking. If that shows in Hersir, poll only the newest view, or stop after a while without a change (GA-344's change
  feed could tell a view what changed since its last read).
- A card's click does nothing yet; GA-349's card view could open there.
- A column's count in the view is the number of cards sent (at most 50, 20 in a folded column), not its `total`:
  Board.tsx counts the cards it gets.
