# Docs

Each project can have a Docs page of its own, in the sidebar's Docs section below Agents: its code reference, guides,
routes, changelog, decisions and commits, drawn as the Yodocs Figma draws them. Docs is off for every project until you
turn it on.

## Turning it on

- **Project form → Docs → Show this project under Docs.** Off by default, also for every project from before this version.
- **On:** the project shows under Docs in the sidebar, with its colour dot, and its name opens its Docs page.
- **Off:** the project leaves the Docs section. Its Docs page says Docs is off, with Edit project to turn it on.

## The sidebar's Docs section

- It lists the projects with Docs on, and marks the one whose page is open.
- With none on, it says No project has Docs on, with a link to Projects.
- It folds with the caret at the end of its heading, like Agents and Memory, and stays folded after a restart.

## The page

The page is yodocs' own viewer, which draws the Yodocs Figma: Inter and IBM Plex Mono, its own colours, light and dark.
Hersir builds what it shows from the project's linked folder and its own database, each time the page opens.

- **Top bar:** Home, Guides, Reference, API, Changelog, Decisions and MCP. A section with nothing in it is dimmed and
  says what would fill it. Search (⌘K, Ctrl+K or /) looks through all of them. The chip with the commit id opens the
  commit list.
- **In the app** the page fills the Docs page, in Hersir's own theme. **Open in browser**, at the end of Hersir's top
  bar, opens the same page in your browser.
- **Home:** the project's sections with their counts, what this build read (commit, branch, when, how long it took),
  how many declarations have a description, and any file the parser couldn't read in full.

## Reference

The declarations in the project's code, by folder (the sidebar's tree) and by kind (its filter): components, hooks,
functions, classes, interfaces, types, enums, constants, then Laravel's models, controllers, migrations, middleware and
the rest. Each has a page with its signature, the comment above it, what it extends or implements, the routes that reach
it, and its source with line numbers.

- It reads the files git tracks in the project's linked folder, as they are on disk: TypeScript, TSX, JavaScript, PHP,
  Dart, Kotlin, and the script blocks of Vue and Svelte files.
- Left out: folders of installed, built or generated code (vendor, node_modules, dist, build, .next, out, coverage,
  storage, public, .nuxt), minified and declaration files (.min.js, .d.ts), links, files over 512 KB, files that aren't
  UTF-8, and any file past the first 5,000.
- A function named like a type that renders JSX is a component, and `useSomething` is a hook. A PHP class gets its
  Laravel kind from what it extends or implements, by the last part of the name (`Model`, `Controller`,
  `Migration`…), middleware from its handle method, an exception from its name. In Dart, a class extending a Flutter
  widget or a widget's `State` is a widget; in Kotlin, an interface and an enum class get their own kinds. Kotlin's
  grammar is the weakest: some files are only partly read.
- A class's methods and properties, and imports, aren't listed on their own: they are in their class's or file's code.
- **Routes**, under the folders: the routes the project answers on, read from its files without running it. Each opens
  the class that answers it (Laravel's `Controller@method`), else the first declaration in its file (Next.js).
  - Next.js: each `route.*` file of the app router with the methods it exports, each `page.*` file, and the pages
    router's files. A route group `(name)` and a parallel slot `@name` aren't in the URL; a private folder `_name`
    isn't routed.
  - Laravel: the `Route::` calls in `routes/*.php` (not console.php or channels.php), with their prefix groups,
    controller groups, `match`, `view`, `resource` and `apiResource` (with `only` and `except`); `routes/api.php` under
    `/api`. Routes a package or a service provider adds aren't in these files, so they don't show.

## Guides

The project's own Markdown docs, from the files git tracks: the README at its root first, then the Markdown under
`docs/` and `doc/`, by path, grouped by their folder. Each is titled by its first heading, else its file name, and has
an "On this page" list of its headings. A changelog, a licence, other Markdown and Markdown in installed folders aren't
guides; a guide shows its first 256 KB. HTML in a guide shows as text and never runs.

## Changelog

The project's own changelog, then its merged cards, newest first, with a filter by kind of change.

- **CHANGELOG.md** (or `CHANGELOG.markdown`, `changelog.md`, `docs/CHANGELOG.md`), in Keep a Changelog format: each
  `## [version] - date` is a release, Unreleased first, each `### Added` (Changed, Deprecated, Removed, Fixed,
  Security) its kind of change. A section the format doesn't name is grouped under Other, and Home names it as a problem.
- **Merged cards:** one group a month, each with its identifier and its title.

- A card counts as merged once it is in a Deploy or Done column. Archived cards stay in the changelog.
- Its month is when it reached Done. A card in a Deploy column counts from when it last changed.
- It keeps itself up to date: when an agent's card reaches main, it shows here, with nothing to write.

## Decisions

The project's own decision records, the highest number first, then the shared notes in Memory about its decisions.

- **Decision records:** `NNNN-title.md` files in `docs/decisions/` (or `docs/adr/`, `docs/decision-records/`,
  `adr/`, `decisions/`), the ADR format. Each has its number, its title (front matter, else its first heading), its
  status (proposed, accepted, rejected, superseded or deprecated, from front matter or a `Status:` line), its date, its
  `##` sections, and the files its Related section names. They aren't listed among the guides.
- **Memory notes:**

  - A note shows here when it is in `Decisions/`, or has `type: decision`, and its `project:` is the project's key or
  name, in any case.
  - The Chief keeps them. Its answers to the agents' questions go in `Decisions/<project name>` ([Agents ask the Chief
  first](memory.md)), and its Memory block tells it that a decision about one project goes in `Decisions/` with the
  project's key as `project:`. The project's agents get those notes in their runs too.
  - An agent's own notes and the Chief's own aren't shared, so they never show here.

## MCP

The MCP servers the project's agents connect to, and the tools its own MCP server offers.

- **Servers:** from `.mcp.json` (or `.cursor/mcp.json`, `mcp.json`), Claude Code's project file, by name: how each
  connects (a command or a URL), its arguments, and the names of its environment variables and headers. Values never
  show: a URL's user, password and query values, and an argument like `--api-key=…`, become placeholders, and Home
  says which. A `##` section of `docs/mcp.md` named after a server says what it is for.
- **Tools:** the tools, resources and prompts of the project's own Laravel MCP server, from their classes: a class that
  extends Laravel's `Tool`, `Resource` or `Prompt`, or a class of the project that does. Each has its name, title,
  description, annotations (`IsReadOnly` and the like) and inputs, from its attributes and its `schema` method.
  An abstract one isn't listed.

A class of the project takes the kind of the project class it extends, when no rule names its own: a tool extending
an abstract `FactoryTool`, which extends Laravel's `Tool`, is a tool too.

## Commits

The newest 50 commits on main, newest first: each with its short id, its first line, who made it and when, and the files
it changed. A code file the Reference reads opens to its changed lines, at most 400 a file; other files are only named.

- Main is main as last fetched, as a card's changes measure from it: origin/main for a project linked to GitHub or
  Bitbucket, else the local main branch. Hersir doesn't fetch for it.
- A merge into main is one commit, with its pull request's files.
- A project without a git repository still has its changelog and decisions; its Home says how to link one.

## In a browser

**Open in browser** opens the same page in your browser.

- Hersir serves the page itself, at `http://127.0.0.1:4380/p/<project id>/`, or the next free port up to 4399. Nothing is
  uploaded and nothing is hosted elsewhere. It keeps serving until Hersir quits. Its address on its own lists the
  projects with Docs on.
- Only this computer gets it. A request from another computer, or from a page that reaches it under another name, is
  refused. Only Hersir's own window may show it in a frame.
- It is read only, and only for the projects with Docs on.

## Not there yet

What yodocs also read, and Hersir doesn't yet: an OpenAPI file's request and response bodies, the llms.txt and Markdown pages for agents, what moved in the docs since the last commit, and switching
single sections off.

## Not to be confused with

A project's documents (project page → Docs, and the Doc page) are separate: notes and specs you and the Chief write. The
Docs section is the project's developer docs.
