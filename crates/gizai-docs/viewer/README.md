# The docs page (yodocs' viewer)

The page that shows a project's docs in Hersir: in the app's Docs section, and in a browser through Open in browser.
It is the viewer of the yodocs npm package (github.com/Yotech-AI/yodocs, `viewer/` at commit bce200d, 18 Nov 2026),
which draws the Yodocs Figma file (83F8VprKDr4FVSFst14WPm). Hersir builds the index it reads (`../src/index.rs`,
yodocs' `index.json`, schema 1) and serves both (`src-tauri/src/docs_server.rs`).

## What Hersir changed

- `index.html` loads `fonts.css`.
- `fonts.css` and `fonts/` are new: Inter and IBM Plex Mono, the Figma's fonts, from fontsource 5.3.0 (SIL Open Font
  License 1.1, `fonts/*-LICENSE.txt`).
- `js/theme.js` takes `?theme=dark` or `?theme=light` first. The app passes its own theme.
- `js/search.js` builds the search index in the page from `index.json`, with yodocs' own `documents()`. yodocs fetched a
  prebuilt `search.json` instead.
- `js/sidebar.js`: the footer names Hersir, and links `index.json` instead of `search.json`.
- `js/main.js`, `js/reference.js` and `js/sections.js`: messages that said to run a yodocs command say what to do in
  Hersir, and a switched-off section names where Hersir switches it (`Edit project → Docs`) instead of a `DOCS_` key. The Changelog's first line names where its changes come from (`project.changelogFrom`).
- `vendor/minisearch.js` is MiniSearch 7.2.0 (MIT, `vendor/minisearch-LICENSE.txt`), its ESM build without the line
  that names a source map.

Every other file is as yodocs has it. To take a newer viewer, copy yodocs' `viewer/` over this folder and make these
changes again. `tests/it/viewer_test.rs` checks that they hold, and that every file here is built into Hersir.
