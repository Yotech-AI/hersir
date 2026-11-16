# Hersir in islands (1 of 3): the look, the window and a sidebar that closes (GA-284)

The design is in Figma, in the file **Giz-AI**, on the page **Hersir Islands 1: look and window** (node `268:2`):
https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=268-2

These are the notes for the builders. Nothing in the app changes in this card: building waits for Jefsev's OK on all
three island cards (this one, the Chief Chat's side island and the other pages). What a page holds and does stays as it
is: only how it is laid out and drawn changes.

## The frames

Open a frame with `https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=<id with - for :>`.

| Frame | Node | Size |
|---|---|---|
| Look · Dark | `275:1972` | 1440 × 1746 (taller than the window: it is a sheet, not a screen) |
| Look · Light | `275:2283` | 1440 × 1746 |
| Chat · Islands · Dark (sidebar open, terminal panel on the right) | `271:136` | 1440 × 900 |
| Tasks · Board · Islands · Dark | `273:1172` | 1440 × 900 |
| Editor · Islands · Dark (files, code, terminal panel; the resize gap under the pointer) | `274:1564` | 1440 × 900 |
| Chat · Islands · Light | `272:421` | 1440 × 900 |
| Chat · Sidebar closed · Dark | `272:575` | 1440 × 900 |
| Editor · Sidebar closed · Dark (the resize gap while dragging) | `274:2270` | 1440 × 900 |
| Chat · Sidebar closed · Light | `272:792` | 1440 × 900 |
| Sidebar toolbar · States · Dark (hover with tooltips, the Inbox count, focus, Close sidebar) | `276:2120` | 1440 × 760 |
| Sidebar toolbar · States · Light | `276:2398` | 1440 × 760 |
| Chat · 900 × 600 · Sidebar open · Dark | `272:896` | 900 × 600 |
| Chat · 900 × 600 · Sidebar closed · Dark | `272:1287` | 900 × 600 |
| Islands components (sheet) | `268:3` | |

The light frames are the dark ones with the GizAi colour collection set to Light on the frame: every colour is a variable,
so a frame switches theme as a whole.

### Components (on the sheet `268:3`)

| Component | Node | Built on |
|---|---|---|
| `Islands/Island` (Focus=Off `269:116`, Focus=On `269:154`) | `269:192` | Island header |
| `Islands/Island header` | `269:67` | Island tab, App/Icon button |
| `Islands/Island tab` (State=Default `269:34`, Hover `269:44`, Selected `269:54`) | `269:64` | Icons, App/* text styles |
| `Islands/Tool button` (State=Default `269:5`, Hover `269:12`, Focus `269:19`) | `269:26` | Icons, the nav count |
| `Islands/Sidebar toolbar` (the closed sidebar's toolbar) | `269:259` | 4 × Tool button (exposed) |
| `Islands/Tooltip` | `269:29` | App/shadow-pop |
| `Islands/Resize gap` (Orientation × State: Rest, Hover, Dragging, Focus) | `269:209` | |
| `Islands/Scrollbar` (State=Rest `269:212`, Hover `269:214`) | `269:216` | |
| New Lucide icons: `panel-left-close` `268:9`, `panel-left-open` `268:14`, `panel-left` `268:18`, `maximize-2` `268:24`, `git-branch` `268:30`, `pin` `268:34`, `chevron-up` `268:37` | `268:4` | 16 px, 1.75 stroke at 24, text-2 |

The page frames use App/Nav item, App/Nav label, App/Button, App/Icon button, App/Kbd, App/Avatar, App/Live tag,
App/Board card and the Status glyphs, and these components. Islands in the page frames are frames drawn exactly as
`Islands/Island` (an instance can't hold a page's own content); their names start with `Island ·`.

## What changed in the GizAi variables

- **The old pink and amber are gone** (GA-230 had changed only the code): `status/needs` is `#f2a53a` / `#9a5800`,
  `status/needs-soft` `rgba(242,165,58,.12)` / `rgba(154,88,0,.08)`, `status/warning` `#dcd839` / `#756a18`,
  `status/warning-soft` `rgba(220,216,57,.13)` / `rgba(117,106,24,.10)`, `glyph/st-progress` `#dcd839` / `#756a18`.
  Every frame that used them (the website, the screenshots, the onboarding) now shows the gold and the yellow.
- **Every colour of `design/tokens.json` is now a variable**, with its value in Dark and Light: `glyph/pr-urgent`, `diff/*`
  (soft, strong, fold), `syntax/syn-*`, `git/git-*`, `terminal/term-*` and `band/band-subtle-*`; the tokens that follow
  another (`syn-meta` = `syn-keyword`, `term-bg` = `bg`, …) are aliases, as in tokens.json. `band/band-line` and
  `band/band-glow` follow `band-subtle-*`, the strength Jefsev picked. `brand/pink` (the website's old brand colour),
  `band/band-blue` and `band/band-hero-*` are the website's and stay.
- **New text styles** so every text in the frames has an App/* style: `App/t-count` (11px, 600, line 14: the counts in a
  pill), `App/t-avatar` (9.5px, 700, letter-spacing 2%) and `App/t-avatar-sm` (8.5px, 700). App/Nav item's count, App/Avatar's
  initials and App/Live tag's words (now `App/t-badge`, the same values) use them.
- **`shadow/shadow-pop`**: the colour of App/shadow-pop as a variable, so the light theme gets its own (as tokens.json says).

## The new tokens

For `design/tokens.json` and `src/styles/tokens.css` when this is built. The colours are in **GizAi colour**, the sizes in
**GizAi size**.

| Token | Dark | Light | Use |
|---|---|---|---|
| `canvas-from` | `#050608` | `#f0f3f6` | The window's background at its top. |
| `canvas-to` | `#040e13` | `#dceaf1` | The window's background at its bottom edge: the band's light, hue 232° in OKLCH. |
| `island` | `= bg` (`#0f1013`) | `= bg` (`#ffffff`) | An island's fill: bg's value, so code, a terminal (`term-bg` = bg) and inputs in it are one surface. |
| `island-line` | `#1b1d22` | `#dce0e5` | An island's 1px border, inside. |
| `island-line-focus` | `#35383e` | `#b9bec6` | The border of the island the focus is in. |
| `scroll-thumb` | `#33353a` | `#caced3` | A scrollbar's thumb at rest. |
| `scroll-thumb-hover` | `#585b61` | `#9b9fa5` | The thumb under the pointer and while dragged. |
| `island-radius` | `10px` (= `radius-l`) | | An island's corners. |
| `island-gap` | `6px` (= `size-handle`) | | Between islands, and between an island and the window's edge. |
| `size-island-head` | `40px` | | An island's header. |
| `size-island-tab` | `28px` | | A tab in an island's header. |
| `size-topbar` | `44px` (was `52px`) | | The top bar, now on the background with no border. |
| `size-tool` | `28px` | | A button of the closed sidebar's toolbar, and Close sidebar. |
| `size-scroll` | `6px` | | A scrollbar's thumb. |

Contrast (WCAG 2): text-3 on the background is 5.34 to 4.98 in dark and 5.08 to 4.60 in light; text-2 7.95 or more in
dark and 6.68 or more in light; accent 9.0 (dark) and 5.18 to 4.69 (light); needs on the light background's bottom 4.53.
canvas-to is 47° of hue from `live`, 33° from `accent` and far from `needs`, with less chroma than any of them.

## The look

### The background

- A linear gradient **straight down (180°)**: `canvas-from` at 0% (the top), `canvas-to` at 100% (the bottom edge). On
  the window (`html` or `.app`), behind everything, the full window: the islands sit on it with `island-gap` all round.
- One cool blue, low in chroma: the window's own light, as the band was. Never `accent`, `live` or `needs`.
- **The mountains become part of the gradient.** Their lines go: the islands are solid (they hold content, and the band
  never ran behind content), so only bits of the lines would show in the 6px gaps. Their glow and hue live on as
  `canvas-to`, the light at the bottom of the window. So `Band.tsx`, `band.css`, `mountains.svg` and Settings →
  Appearance → Background's "Show mountains along the bottom" go when this is built (`gizai-band` is no longer read);
  the website keeps `mountains-hero.svg`. The first start's onboarding (no islands) is card 3's: until then it keeps its band.

### An island

- Fill `island`, a **1px `island-line` border inside**, radius `island-radius` (10), `island-gap` (6) to the next island
  and to the window's edge. `overflow: hidden`, so content keeps to the round corners.
- **No shadow.** Only what floats keeps one: menus, popovers, the palette, drawers, toasts, a dragged card.
- Inside an island nothing changes: rows, cards (`raised`), the text box, code and terminals are drawn as today. The
  1px `line` borders that split the window into regions go: the gaps do that now. Lines inside an island stay (the
  chat list's foot, the Editor's status bar).

### An island's header

- `size-island-head` (40) high, padding `0 6px`, items 2px apart, centred. No border under it: the content starts right
  below, on the same fill.
- Left: **tabs** (Island tab) or a **title** (`t-body-strong`, `text`, 6px in). Right: the island's **actions**, App/Icon
  button (26 × 26, ghost, `text-2`).
- **Tabs** (`size-island-tab`, 28 high, radius `radius` 6, padding `0 6px 0 10px`, 6px between icon, name, count and close,
  2px apart):
  - Default: no fill, name `text-2` (`t-label`), icon 14px `text-3`.
  - Hover: fill `hover`, name `text`, icon `text-2`.
  - Selected: a rounded pill in `selected`, name `text`, icon `text-2`. It replaces the Editor tab's accent top line.
  - With a count: the number after the name, `t-small` in `text-3` (`Changes 12`). A git mark takes that place in
    the Editor (`M` in `git-modified`).
  - Close: an `x` (12px) in an 18px box, radius 4, `text-3` (`text-2` on a selected or hovered tab), hover fill `hover`.
    It shows on the selected tab, the hovered one and one not saved; the others keep no room for it.
- The Chat's and the Editor's terminal tabs, the Editor's open files and the Tasks board's view toolbar are island
  headers. The board's header holds today's toolbar (New task, Search ID or title, Board or List, Filters, Sort,
  Archived cards) at 44px, its controls 28–30px high.

### Focus

- The island the focus is in (`:focus-within`) gets **`island-line-focus`** on its border. Nothing else: no accent, no
  glow, no change of fill. It tells where the keys go when the chat and a terminal sit side by side.
- What has the focus inside keeps its own sign, as today: the text box an accent border, a field or a button its 2px
  `focus` ring, a terminal its block cursor (an outline without the focus), the code its caret.

### The gap as a resize handle

Between two islands that resize (the Chat's terminal panel and the conversation; the Editor's code and its terminal
panel) the 6px gap is the handle (`size-handle` = `island-gap`; `role="separator"` as now).

- Rest: only the gap; the pointer is `col-resize` (or `row-resize`).
- Hover: a **2px `accent` line** down the gap's middle, the islands' full length, round ends (radius 1).
- Dragging: the line and **`accent-soft`** across the whole gap (radius 4); `.is-dragging` keeps the pointer everywhere.
- Keyboard focus: the line in `focus`.

### A scrollbar inside an island

- No track. A **6px thumb** (`size-scroll`), radius pill, 2px from the island's right (or bottom) edge and at least 4px from
  its ends, so it keeps clear of the round corners.
- `scroll-thumb` at rest; `scroll-thumb-hover` under the pointer and while dragged. Horizontal (the board) the same, 6px high.

### Which surfaces change

| Today | In islands |
|---|---|
| `side`: the sidebar, the chat list, the Editor's tree, the terminal rail (dimmest) | The **background**: `canvas-from` → `canvas-to` (now the dimmest). `side` stays for board rails and code blocks inside an island, no longer for a column. |
| `bg`: the canvas, the main column | **The sidebar island and every work island**: `island` (= bg). |
| `raised`: cards, panels, the text box | Unchanged, inside an island. |
| `overlay`: drawers, the palette, popovers, toasts | Unchanged, with their shadows. |

## The window in islands

- **The window:** `island-gap` (6) padding all round, islands 6 apart. Hersir has the system's title bar: the frames draw
  only what is inside it.
- **The sidebar island** (`size-side`, 248) runs the window's full height on the left. Inside it is today's sidebar:
  side-main (padding `6px 10px 14px`, scrolling) and side-foot (Company, its 1px `line` top border).
- **The top bar sits on the background** (chosen for all pages): `size-topbar` 44 high, no fill, no border, padding
  `0 4px 0 12px`, over the islands of the main column (right of the sidebar island). Crumbs on the left, the page's
  actions on the right, as today. It belongs to all of a page's islands (the Chat has three), so it isn't one island's
  header; and with the sidebar closed the toolbar shares its row.
- **Chat** (`271:136`): the chat list island (260; 220 under 1100px wide), the conversation island, the terminal island
  on the right (`--chat-term-share`, 45% of the room right of the chat list, at least 360; the gap between them is the
  handle). The terminal island's header is its tabs with New terminal, Own page and Hide; under it the folder in mono
  (`t-id`, `text-2`, no fill: the `side` strip goes); then the terminal on `term-bg`. Hidden with terminals, the rail is
  a 36px island of its own.
- **Tasks as a board** (`273:1172`): one island; its header is the view toolbar; the columns scroll sideways inside it
  (the frame shows it scrolled to Review and the Done and Cancelled rails).
- **Editor** (`274:1564`): the files island (`size-tree`, 260: its header is Files or Search with Fold the file tree;
  then Go to file and the tree) and on the right the code island over the terminal island (the gap between them is the
  handle). The code island: the open files as tabs in its header, the code, and the status bar at its bottom (it
  belongs to the open file, so it moves from under the terminal panel into the code island). The terminal panel's
  header: its tabs with New terminal, Own page and Hide (`chevron-down`).
- The behind bar, the card bar and the error banners of the Editor sit at the top of the code island, under its header.

## A sidebar that closes

- **Close sidebar**: an `Islands/Tool button` with `panel-left-close`, at the top right of the sidebar island, on the
  row of New task (New task gets the rest of the row, 4px before the button). Tooltip: **Close sidebar (Ctrl+\\)**.
  `aria-label="Close sidebar"`, `aria-expanded="true"`, `aria-controls` the sidebar.
- **The key: Ctrl+\\** (Cmd+\\ on macOS), as in Notion. No page uses it yet: Ctrl+B is the Tasks page's (Board or List)
  and the Editor's (fold the files panel); CodeMirror binds only Ctrl+Alt+\\ and Ctrl+Shift+\\; the Markdown editor's keys
  are Ctrl+B, I, E, K, S and Enter. Read it by `e.code === "Backslash"` (or `e.key === "\\"`), with Ctrl (Cmd on macOS)
  and no Shift or Alt, in App.tsx's window key handler, with `preventDefault`. It toggles: closes an open sidebar,
  shows a closed one. In a terminal the key stays the shell's (Ctrl+\\ is its quit signal), as Ctrl+B is there.
- **Closed:** the sidebar island goes and the main column takes its width (6px from the window's left edge). At the top
  left, on the background, in the top bar's row, **the toolbar** (`Islands/Sidebar toolbar`): 6px from the window's edge,
  centred in the 44px row, buttons 2px apart, then 12px to the crumbs (the top bar's left padding goes while it shows).
  Its buttons are the sidebar's quick actions, each `size-tool` (28 × 28), radius 6, icon 16 in `text-2`:
  1. **Show sidebar**, `panel-left-open`. Tooltip **Show sidebar (Ctrl+\\)**. `aria-expanded="false"`.
  2. **New task**, `square-pen`. Tooltip **New task (N)**.
  3. **Search**, `search`. Tooltip **Search (Ctrl K)** (Cmd K on macOS, as the sidebar's own kbd).
  4. **Inbox**, `inbox`, with its count: the sidebar's pill (`App/t-count`, `needs` on `needs-soft`, at least 18 wide,
     padding `0 5px`, 4px after the icon), the button wider by the pill. With nothing waiting the pill goes. Tooltip
     **Inbox (3 need you)**, or **Inbox** with none. It goes to the Inbox, as the sidebar's row does.
- **States** (`276:2120`): hover: fill `hover`, icon `text`; keyboard focus: the 2px `focus` ring outside; pressed: as
  hover. The tooltip is each button's `title` (the system's tooltip, as on every Hersir button); the frames draw it so
  it can be read: `Islands/Tooltip`, `overlay`, `line-2` border, radius 4, `shadow-pop`, the name in `text` and the key in
  `text-3` (`t-caption`), 6px under the button.
- **Kept on this computer**, as the sidebar's folded sections are (`gizai.sidebar.folded`): `localStorage`
  `gizai.sidebar.closed` = `"1"` while closed; nothing kept means open.
- **Under 900 × 600** nothing extra: the window doesn't go smaller (`tauri.conf.json`). At 900 × 600 the sidebar open
  leaves the main column 634px: the chat list 220 and the conversation 408 (frames `272:896`, `272:1287`); with the
  terminal panel open there the chat list hides, as today (`no-list`).

## Every text, word for word

New in this design:

- Close sidebar (the button's label for screen readers) and its tooltip: **Close sidebar (Ctrl+\\)**
- Show sidebar and its tooltip: **Show sidebar (Ctrl+\\)**
- New task's tooltip: **New task (N)**
- Search's tooltip: **Search (Ctrl K)** — Cmd K on macOS
- Inbox's tooltip: **Inbox (3 need you)** — the number is the count; **Inbox** when it is 0. Its label for screen
  readers: **Inbox, 3 need you**, or **Inbox**.
- The toolbar's group label for screen readers: **Sidebar actions**

Unchanged: every other text in the frames is today's (the sidebar, the top bars, the chat, the board's toolbar, the
Editor). The chats, cards, files, code and terminal output in the frames are sample data, not texts to build.

## Not in this card

- The Editor, the board and cards beside the Chief Chat (card 2 of 3), and the other pages, drawers, menus and dialogs
  (card 3 of 3), the first start's onboarding among them.
- Building it: the build cards come after Jefsev's OK on all three.
