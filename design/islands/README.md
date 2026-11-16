# Hersir in islands (1 of 3): the look, the window and a sidebar that closes (GA-284)

The design is in Figma, in the file **Giz-AI**, on the page **Hersir Islands 1: look and window** (node `268:2`):
https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=268-2

These are the notes for the builders. Nothing in the app changes in this card: building waits for Jefsev's OK on all
three island cards (this one, the Chief Chat's side island and the other pages). The one exception is the chip's hover
card: a frontend card builds it as soon as this design is approved. What a page holds and does stays as it is: only how
it is laid out and drawn changes.

## Round 2 (Jefsev's review of 16 Nov)

1. **The background is lighter than the islands** in dark, as in JetBrains' Islands, with a soft glow from the top left
   in Hersir's cool blue. In light the islands are the lightest surface, on a soft grey a step below them.
2. **No island has a border.** The step in fill alone sets an island apart. `island-line` and `island-line-focus` are gone.
3. **Focus shows no sign on the island** (no border, no glow, no change of fill). What has the focus inside shows it, as
   today: the composer keeps its accent border. See [Focus](#focus).
4. **What floats** over the islands is `float`: lighter than the islands, a thin `line-2` border and `shadow-pop`.
5. **New: the chip's hover card.** Resting the pointer on an @ chip shows a small card instead of the browser's tooltip.
6. Kept from round 1: Ctrl+\\ (Cmd+\\ on macOS), the mountains as part of the gradient, the page top bars on the
   background, the closed sidebar's toolbar, `island-radius` 10 and `island-gap` 6.

Every frame of round 1 is redrawn with this: the islands lost their border, the background has its new values and the
glow, and buttons and faint text on the background follow [On the background](#on-the-background).

## The frames

Open a frame with `https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=<id with - for :>`.

| Frame | Node | Size |
|---|---|---|
| Look · Dark | `275:1972` | 1440 × 2362 (a sheet, not a screen) |
| Look · Light | `288:2657` | 1440 × 2362 |
| Chat · Islands · Dark (sidebar open, terminal panel on the right) | `271:136` | 1440 × 900 |
| Chat · Islands · Light | `272:421` | 1440 × 900 |
| Tasks · Board · Islands · Dark | `273:1172` | 1440 × 900 |
| Editor · Islands · Dark (files, code, terminal panel; the resize gap under the pointer) | `274:1564` | 1440 × 900 |
| Chat · Sidebar closed · Dark | `272:575` | 1440 × 900 |
| Chat · Sidebar closed · Light | `272:792` | 1440 × 900 |
| Editor · Sidebar closed · Dark (the resize gap while dragging) | `274:2270` | 1440 × 900 |
| Sidebar toolbar · States · Dark (hover with tooltips, the Inbox count, focus, Close sidebar) | `276:2120` | 1440 × 760 |
| Sidebar toolbar · States · Light | `288:2972` | 1440 × 760 |
| Chat · 900 × 600 · Sidebar open · Dark | `272:896` | 900 × 600 |
| Chat · 900 × 600 · Sidebar closed · Dark | `272:1287` | 900 × 600 |
| Chat · Hover card on a card chip · Dark | `292:2541` | 1440 × 900 |
| Chat · Hover card on a card chip · Light | `292:2956` | 1440 × 900 |
| Islands components (sheet) | `268:3` | |

In the look frames: The background `275:1977`, An island `275:1990`, Focus `275:1997`, Which surfaces change `275:2160`,
What floats `288:2545`, A tooltip `288:2621`, the token table `275:2184`.

The light frames are the dark ones with the GizAi colour collection set to Light on the frame: every colour is a variable,
so a frame switches theme as a whole. The glow is drawn in Figma as an ellipse (`Glow`, locked, the frame's first layer)
in `canvas-glow` with a layer blur; in CSS it is a radial gradient (below).

### Components (on the sheet `268:3`)

| Component | Node | Built on |
|---|---|---|
| `Islands/Island` (one component now: the Focus variant went with the border) | `269:116` | Island header |
| `Islands/Island header` | `269:67` | Island tab, App/Icon button |
| `Islands/Island tab` (State=Default `269:34`, Hover `269:44`, Selected `269:54`) | `269:64` | Icons, App/* text styles |
| `Islands/Tool button` (State=Default `269:5`, Hover `269:12`, Focus `269:19`) | `269:26` | Icons, the nav count |
| `Islands/Sidebar toolbar` (the closed sidebar's toolbar) | `269:259` | 4 × Tool button (exposed) |
| `Islands/Tooltip` | `269:29` | `float`, App/shadow-pop |
| `Islands/Resize gap` (Orientation × State: Rest, Hover, Dragging, Focus) | `269:209` | |
| `Islands/Scrollbar` (State=Rest `269:212`, Hover `269:214`) | `269:216` | |
| `Islands/Hover card` (Kind=Card `291:2536`, Project `291:2564`, Agent `291:2582`, Client `291:2601`, Person `291:2611`, Doc `291:2624`, Not found `291:2635`; Card has the boolean On hold) | `291:2641` | App/Avatar, Status glyphs, App/shadow-pop |
| New Lucide icons: `panel-left-close` `268:9`, `panel-left-open` `268:14`, `panel-left` `268:18`, `maximize-2` `268:24`, `git-branch` `268:30`, `pin` `268:34`, `chevron-up` `268:37` | `268:4` | 16 px, 1.75 stroke at 24, text-2 |

The page frames use App/Nav item, App/Nav label, App/Button, App/Icon button, App/Kbd, App/Avatar, App/Live tag,
App/Board card and the Status glyphs, and these components. Islands in the page frames are frames drawn exactly as
`Islands/Island` (an instance can't hold a page's own content); their names start with `Island ·`.

## What changed in the GizAi variables

Round 2:

- `canvas/canvas-from` and `canvas/canvas-to` have new values (below).
- New: `canvas/canvas-glow`, `canvas/canvas-hover` and `surface/float`, in Dark and Light.
- Gone: `island/island-line` and `island/island-line-focus`, and every stroke that used them (66, all on this page).

Round 1 (still true):

- **The old pink and amber are gone** (GA-230 had changed only the code): `status/needs` is `#f2a53a` / `#9a5800`,
  `status/needs-soft` `rgba(242,165,58,.12)` / `rgba(154,88,0,.08)`, `status/warning` `#dcd839` / `#756a18`,
  `status/warning-soft` `rgba(220,216,57,.13)` / `rgba(117,106,24,.10)`, `glyph/st-progress` `#dcd839` / `#756a18`.
- **Every colour of `design/tokens.json` is a variable**, with its value in Dark and Light: `glyph/pr-urgent`, `diff/*`
  (soft, strong, fold), `syntax/syn-*`, `git/git-*`, `terminal/term-*` and `band/band-subtle-*`; the tokens that follow
  another (`syn-meta` = `syn-keyword`, `term-bg` = `bg`, …) are aliases, as in tokens.json. `band/band-line` and
  `band/band-glow` follow `band-subtle-*`. `brand/pink`, `band/band-blue` and `band/band-hero-*` are the website's.
- **Text styles** so every text in the frames has an App/* style: `App/t-count` (11px, 600, line 14: the counts in a
  pill), `App/t-avatar` (9.5px, 700, letter-spacing 2%) and `App/t-avatar-sm` (8.5px, 700).
- **`shadow/shadow-pop`**: the colour of App/shadow-pop as a variable, so the light theme gets its own.

## The new tokens

For `design/tokens.json` and `src/styles/tokens.css` when this is built. The colours are in **GizAi colour**, the sizes in
**GizAi size**.

| Token | Dark | Light | Use |
|---|---|---|---|
| `canvas-from` | `#1c1e23` | `#ebedf1` | The window's background at its top: a clear step lighter than `island` in dark, a soft grey a step below it in light. |
| `canvas-to` | `#182126` | `#e2ecf1` | The background at its bottom edge: the light of the mountains, hue 232° in OKLCH. |
| `canvas-glow` | `#202e3b` | `#dce9f4` | The glow from the top left, over the gradient, fading to transparent: Hersir's cool blue, hue 245°, low in chroma. |
| `canvas-hover` | `#282a30` | `#dddfe6` | Hover fill of a button that sits on the background (the closed sidebar's toolbar, the top bar's buttons). |
| `island` | `= bg` (`#0f1013`) | `= bg` (`#ffffff`) | An island's fill: bg's value, so code, a terminal (`term-bg` = bg) and inputs in it are one surface. No border, no shadow. |
| `float` | `#191b20` | `#ffffff` | What floats over the islands: menus, popovers, the palette, tooltips, drawers, toasts, the chip's hover card. Takes `overlay`'s place. |
| `scroll-thumb` | `#33353a` | `#caced3` | A scrollbar's thumb at rest. |
| `scroll-thumb-hover` | `#585b61` | `#9b9fa5` | The thumb under the pointer and while dragged. |
| `island-radius` | `10px` (= `radius-l`) | | An island's corners. |
| `island-gap` | `6px` (= `size-handle`) | | Between islands, and between an island and the window's edge. |
| `size-island-head` | `40px` | | An island's header. |
| `size-island-tab` | `28px` | | A tab in an island's header. |
| `size-topbar` | `44px` (was `52px`) | | The top bar, now on the background with no border. |
| `size-tool` | `28px` | | A button of the closed sidebar's toolbar, and Close sidebar. |
| `size-scroll` | `6px` | | A scrollbar's thumb. |

`island-line` and `island-line-focus` (round 1) are not tokens any more. `overlay` goes when this is built: every use
becomes `float` (its light value is the same, `#ffffff`).

**Contrast** (WCAG 2). The step from island to background: 1.14 in dark, 1.17 in light (JetBrains' Islands Dark: 1.22).
Dark, on the background: text-2 7.02 (5.83 in the glow), accent 5.99, focus 6.17 in the glow; text-3 4.39 (3.65 in the
glow), enough for icons only, so faint text on the background is text-2 (below). On `float`: text-3 4.54, live 9.23,
needs 8.38. Light, on the background: text-2 6.99 (6.64 in the glow), text-3 4.82 to 4.58, accent 4.92, focus 4.67 in the
glow; on `float` (white) text-3 5.65, live 5.01, needs 5.57.

## The look

### The background

```css
background:
  radial-gradient(ellipse 60% 75% at 0 0, var(--canvas-glow) 45%, transparent),
  linear-gradient(180deg, var(--canvas-from), var(--canvas-to));
```

- On the window (`html` or `.app`), behind everything, the full window: the islands sit on it with `island-gap` all round.
- **The gradient** runs straight down: `canvas-from` at the top (0%), `canvas-to` at the bottom edge (100%).
- **The glow** falls from the top left corner: `canvas-glow`, full for 45% of the way, then fading to transparent at 60%
  of the window's width and 75% of its height. It sits on the background only: the islands cover it.
- **In dark** the background is a clear step lighter than the islands; **in light** the islands are the lightest surface
  and the background a soft grey a step below them, with the glow a soft blue tint, as JetBrains' Islands Light.
- One cool blue, low in chroma: the window's own light, as the band was. Never `accent`, `live` or `needs`.
- **The mountains are part of the gradient** (round 1's choice, kept). Their lines go: the islands are solid, so only
  bits of the lines would show in the 6px gaps. Their light lives on as `canvas-to` at the bottom of the window. So
  `Band.tsx`, `band.css`, `mountains.svg` and Settings → Appearance → Background's "Show mountains along the bottom" go
  when this is built (`gizai-band` is no longer read); the website keeps `mountains-hero.svg`. The first start's
  onboarding (no islands) is card 3's: until then it keeps its band.

### On the background

The top bars, the closed sidebar's toolbar and the gaps sit on the background, which is lighter than the islands in dark:

- **Faint text is `text-2`** on the background (a top bar's count, `.crumbs .sep`, `.topbar .faint`): text-3 is only
  4.39 there. Icons keep `text-3` or `text-2` (3:1 or more).
- **Buttons on the background have no fill**: a secondary button keeps its `line-2` border, the Chief's chip its `line`
  border, ghost buttons stay ghost. Hover: `canvas-hover`. (Their `raised` fill would sit darker than the background.)
- Keyboard focus: the 2px `focus` ring, as everywhere.

### An island

- Fill `island`, radius `island-radius` (10), `island-gap` (6) to the next island and to the window's edge.
  `overflow: hidden`, so content keeps to the round corners.
- **No border and no shadow.** The step in fill from the background alone sets an island apart.
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

**No sign on the island.** An island looks the same with the focus or without it: no border, no glow, no change of fill
(round 1's `island-line-focus` is gone, and nothing takes its place). What has the focus shows it, as today:

- the composer: its accent border and halo (`.composer-box:focus-within`), kept;
- a field or a button: its 2px `focus` ring;
- a terminal: a filled block cursor with the focus, an outline without it (xterm does this already);
- the code: its caret; a list: its selected row.

That is enough to tell where the keys go when the chat and a terminal sit side by side (the look frame shows both:
the focus in the chat, and the focus in the terminal).

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

### What floats

Menus, popovers (`.pop`), the palette (`.pal`), tooltips, drawers, toasts, a dragged card and the chip's hover card float
over the islands and the background:

- Fill **`float`**: lighter than the islands in dark (a hair below the background, as JetBrains' search popup); white in
  light, as the islands are.
- A **1px `line-2` border** inside and **`shadow-pop`** (`shadow-drawer` for a drawer), as `.pop` and `.pal` have today.
  In light the border and the shadow alone set it apart from a white island.
- Each keeps its own size, radius and content: the palette `radius-xl`, a menu `radius-l`, a tooltip `radius-s`.
- The look frame shows the palette over two islands and a tooltip over the background and an island. Card 3 draws the
  drawers, menus and dialogs.

### Which surfaces change

| Today | In islands |
|---|---|
| `side`: the sidebar, the chat list, the Editor's tree, the terminal rail (dimmest) | The **background**: `canvas-from` → `canvas-to` with `canvas-glow`. In dark a step lighter than the islands; in light a soft grey below them. `side` stays for board rails and code blocks inside an island, no longer for a column. |
| `bg`: the canvas, the main column | **The sidebar island and every work island**: `island` (= bg). The darkest surface in dark, the lightest in light. |
| `raised`: cards, panels, the text box | Unchanged, inside an island. |
| `overlay`: drawers, the palette, popovers, toasts | **`float`**, with a `line-2` border and its shadow. |

## The window in islands

- **The window:** `island-gap` (6) padding all round, islands 6 apart. Hersir has the system's title bar: the frames draw
  only what is inside it.
- **The sidebar island** (`size-side`, 248) runs the window's full height on the left. Inside it is today's sidebar:
  side-main (padding `6px 10px 14px`, scrolling) and side-foot (Company, its 1px `line` top border).
- **The top bar sits on the background** (all pages): `size-topbar` 44 high, no fill, no border, padding
  `0 4px 0 12px`, over the islands of the main column (right of the sidebar island). Crumbs on the left, the page's
  actions on the right, as today, drawn as in [On the background](#on-the-background). It belongs to all of a page's
  islands (the Chat has three), so it isn't one island's header; and with the sidebar closed the toolbar shares its row.
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
  `aria-label="Close sidebar"`, `aria-expanded="true"`, `aria-controls` the sidebar. In the sidebar island its hover
  fill is `hover`, as the sidebar's rows.
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
- **States** (`276:2120`, light `288:2972`): hover: fill **`canvas-hover`** (they sit on the background), icon `text`;
  keyboard focus: the 2px `focus` ring outside; pressed: as hover. The tooltip is each button's `title` (the system's
  tooltip, as on every Hersir button); the frames draw it so it can be read: `Islands/Tooltip`, `float`, `line-2` border,
  radius 4, `shadow-pop`, the name in `text` and the key in `text-3` (`t-caption`), 6px under the button.
- **Kept on this computer**, as the sidebar's folded sections are (`gizai.sidebar.folded`): `localStorage`
  `gizai.sidebar.closed` = `"1"` while closed; nothing kept means open.
- **Under 900 × 600** nothing extra: the window doesn't go smaller (`tauri.conf.json`). At 900 × 600 the sidebar open
  leaves the main column 634px: the chat list 220 and the conversation 408 (frames `272:896`, `272:1287`); with the
  terminal panel open there the chat list hides, as today (`no-list`).

## The chip's hover card

Today an @ chip (`ItemChip` in `src/components/MarkdownView.tsx`: a link to a card, a project, a client, an agent, a
person or a doc, in a message, a description or a note) shows the browser's tooltip **Open task** (its `title`). Instead,
resting the pointer on a chip shows a small card beside it. Frames: `292:2541` (dark) and `292:2956` (light), a card chip
in a Chief answer; the component `Islands/Hover card` (`291:2641`) has every kind side by side.

**When it shows**

- The pointer rests on the chip for **350 ms** (as the Memory page's note preview, `MemoryPage.tsx`). Moving off the chip
  hides it after **250 ms**, unless the pointer moves onto the card: it stays while the pointer is on the card, so its
  text can be selected. Leaving the card hides it after 250 ms. Only one card shows at a time.
- The chip getting the **keyboard focus** shows it too (after the same 350 ms); it hides on blur and on **Escape**.
- A click on the chip opens the item, as today. The card itself isn't a button: it is information (`role="tooltip"`,
  the chip's `aria-describedby` points to it while it shows). The chip loses its `title`.
- It reads the item from what the app already holds (tasks, projects, agents, clients, people, docs); one it doesn't
  hold is read first, and the card shows when it arrives (no loading state). If the pointer has left by then, it doesn't.

**Where**

- Rendered in a portal on `document.body`, `position: fixed`, `z-index: 1000` (as `.note-preview` and `.pop.item-picker`).
- **Below the chip**, 6px under its bottom edge, its left edge on the chip's left edge (for a chip that wraps over two
  lines: the line the pointer is on).
- **Above the chip**, 6px over its top edge, when the room below the chip is less than the card's height + 8px and the
  room above is larger (near the window's bottom edge, as in the composer's last lines).
- Kept 8px inside the window on the left and right (`left = clamp(8, chip.left, innerWidth - 8 - 300)`).

**The look**

- Width **300px** (`max-width: calc(100vw - 16px)`). Padding **12px 12px 10px** (`space-3`), **8px** (`space-2`) between the
  heading, the rows and the last line.
- Fill `float`, 1px `line-2` border inside, radius `radius-l` (10), `shadow-pop`. It shows at once (no fade).
- **Heading:** `t-body-strong` in `text`, wrapping, at most 3 lines (then an ellipsis). For a card, its id above it in
  `t-id`, `text-3`, 2px apart.
- **Rows:** 4px apart; each a label in `t-small`, `text-3`, 56px wide, then 8px, then the value in `t-small`, `text`, with
  its mark 6px before it (the project's colour dot 8px, an `App/Avatar` small, a status glyph 16px, the live dot 6px).
  A value that doesn't fit wraps (an email breaks anywhere).
- **Last line:** `t-caption`, `text-3`: **Click to open**.
- The chip under the pointer keeps its hover look (`selected` fill, `accent-hover` text), as today.

**What each kind shows** (sample values in brackets)

| Kind | Heading | Rows |
|---|---|---|
| Card | Its id (`GA-284`) above its full title | **Project**: colour dot + name (`Hersir`). **Agent**: avatar + name of whoever it is assigned to, a person too (`Design Agent`); **Not assigned** in `text-2` when nobody. **Status**: the column's status glyph + the column's name (`In progress`); when it is held, a second line in `needs`: **On hold:** + the hold's name as the Properties panel shows it (`HOLD_NAMES`: `On hold: Needs your decision`); with the Chief: **On hold: With the Chief**. |
| Project | Colour dot + name + key in `t-id`, `text-3` (`Kade portal KP`) | **Client**: its name, or **None**. **Status**: Planned, Active, Paused, Done or Archived. |
| Agent | Avatar + name (`Design Agent`) | **Role**: its role's name (`Design`). **Status**: **Active**, **Paused**, or the live dot + **Working on GA-12** in `live` (the card it runs on now). |
| Client | Name (`Kade`) | **Status**: Lead, Active or Inactive. |
| Person | Avatar + name (`Sanne de Vries`) | **Email**: the address, or **None**. |
| Doc | Title (`Release checklist`) | **Project**: colour dot + name, or **None**. |
| Not found | The chip's text (`GA-999`) | One line, **Not found** in `t-small`, `text-2`. No rows and no last line. |

Every kind but Not found ends with **Click to open**.

## Every text, word for word

New in this design:

- Close sidebar (the button's label for screen readers) and its tooltip: **Close sidebar (Ctrl+\\)**
- Show sidebar and its tooltip: **Show sidebar (Ctrl+\\)**
- New task's tooltip: **New task (N)**
- Search's tooltip: **Search (Ctrl K)** — Cmd K on macOS
- Inbox's tooltip: **Inbox (3 need you)** — the number is the count; **Inbox** when it is 0. Its label for screen
  readers: **Inbox, 3 need you**, or **Inbox**.
- The toolbar's group label for screen readers: **Sidebar actions**
- The hover card's row labels: **Project**, **Agent**, **Status**, **Client**, **Role**, **Email**
- The hover card's words: **Not assigned**, **On hold:** (then the hold's name), **Active**, **Paused**,
  **Working on GA-12** (the card's id), **None**, **Not found**, **Click to open**
- The statuses, as the app names them today: Planned, Active, Paused, Done, Archived (a project); Lead, Active,
  Inactive (a client); the column names (a card); the hold names of `HOLD_NAMES` in `Properties.tsx`.

Gone: the chip's browser tooltip **Open task** (and Open project, Open client, …).

Unchanged: every other text in the frames is today's (the sidebar, the top bars, the chat, the board's toolbar, the
Editor). The chats, cards, files, code and terminal output in the frames are sample data, not texts to build.

## Not in this card

- The Editor, the board and cards beside the Chief Chat (card 2 of 3), and the other pages, drawers, menus and dialogs
  (card 3 of 3), the first start's onboarding among them.
- Building it: the hover card is built by a frontend card once this design is approved; the rest after Jefsev's OK on
  all three cards.
