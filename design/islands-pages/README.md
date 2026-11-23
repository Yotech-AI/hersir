# Hersir in islands (3 of 4): the work pages and what floats (GA-286)

The design is in Figma, in the file **Giz-AI**, on the page **Hersir Islands 3: work pages and what floats** (node
`312:10695`):
https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=312-10695

These are the notes for the builders. Nothing in the app changes in this card. It draws the Inbox, a card's page (Card,
Changes and Screenshots), Tasks as a list, Projects, a project, Clients and a client in islands, and what floats over the
islands. It builds on GA-284 (the look, the window, `design/islands/README.md` on its branch): its tokens, its components
and its round 3 (islands `#181a1c` without a border, on the glass). What a page holds and does stays as it is: only how it
is laid out and drawn changes.

## The frames

Open a frame with `https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=<id with - for :>`. Every frame is
1440 × 900.

| Frame | Node |
|---|---|
| Inbox · Islands · Dark | `312:10696` |
| Card · Card tab · Islands · Dark (the Properties island is `315:3560`) | `312:11231` |
| Card · Changes tab · Islands · Dark | `312:11747` |
| Card · Screenshots tab · Islands · Dark | `312:12263` |
| Tasks · List · Islands · Dark | `312:12779` |
| Projects · Islands · Dark | `312:13295` |
| Project · Islands · Dark | `312:13830` |
| Clients · Islands · Dark | `312:14365` |
| Client · Islands · Dark | `312:14901` |
| Inbox · Islands · Light | `318:6930` |
| Card · Card tab · Islands · Light | `318:7070` |
| Floats · Command palette (Ctrl K) · Dark | `318:7391` |
| Floats · Drawer (New task) · Dark (the drawer is `318:10875`) | `318:10747` |
| Floats · Popover (Filters) and a toast · Dark | `318:7589` |
| Floats · Menu (a terminal in the sidebar) · Dark | `318:8015` |
| Floats · The close question · Dark (over the Editor) | `318:10976` |
| Islands 3 components (sheet) | `314:3533` |

Rows on the page: the dark pages (y 0 and 1200), the light frames (y 2400), what floats (y 3600), the components sheet
above them (y −1100).

### How the frames are built

- Every frame has GA-284's `Islands/Window background` as its first layer (Wallpaper=Dark in dark frames, Light in light
  ones, Glass on), the sidebar island (248) with the page's row current, the top bar on the background and the islands
  6 apart and 6 from the window's edge, `#181a1c` (`island`) without a border. They are copies of GA-284's board frame
  (`273:1172`) with the main column redrawn.
- The light frames are the dark ones with the **GizAi colour** collection set to **Light** on the frame (every colour is a
  variable) and the wallpaper set to Light.
- Every colour is bound to a GizAi variable, every text uses an App/* style, shadows are `App/shadow-pop` and
  `App/shadow-drawer`.
- The page frames use App/Nav item, App/Nav label, App/Button, App/Icon button, App/Input, App/Select, App/Checkbox,
  App/Badge, App/Avatar, App/Label pill, App/Tab, App/Task row, App/Live tag, the Status glyphs, Shot/* and GA-284's
  Islands/Island tab, Islands/Scrollbar and Islands/Tool button. As in GA-284, islands are frames drawn exactly as
  `Islands/Island`; their names start with `Island ·`.

### Components (on the sheet `314:3533`)

| Component | Node | For |
|---|---|---|
| `Islands/Menu row` (State=Default, Hover; Label, Dot, Hint, Check) | `314:3576` | A row of a menu or a popover (`.opt`) |
| `Islands/Menu label` | `314:3577` | A heading in a popover (`.pop-label`) |
| `Islands/Toast` (Text, Undo) | `314:3584` | `.toast` |
| `Islands/Close question` (Title) | `314:3598` | CloseGuard's `.ask` |
| `App/Button danger` (Size=Medium, Small) | `314:3583` | `.btn.danger` (App/Button has no danger kind) |
| New Lucide icons: `step-forward` `314:3539`, `message-square` `314:3542`, `activity` `314:3545`, `play` `314:3548`, `file-text` `314:3555`, `upload` `314:3560`, `archive` `314:3565` | | 16 px, 1.75 stroke at 24, text-2, as GA-284's |

## On every page

- **The top bar sits on the background** (GA-284): 44 high, no fill, no border. Crumbs and separators are `text-2` (the
  `.sep` was text-3: it drops under 3:1 on the glass); the current page stays `text`, weight 600. The page's actions stay
  on the right of the top bar (Copy ID, Open in editor, Archive or Restore, Show properties), ghost or with their `line-2`
  border, no fill, hover `canvas-hover`.
- **A page's toolbar becomes its island's header**, 44 high as the board's (GA-284): padding `0 8`, controls 28 to 30
  high, the search box on `raised`. The `.toolbar` strip and its padding `12 20` go.
- **The 1px lines that split the main column go** (under the top bar, under the toolbar, under the card's tabs, the
  border-left of Properties): the gaps do that now. Lines inside an island stay (table rows, the Changes file list's right
  edge, the panels).
- **Panels and cards** (`.panel`, `.run-card`, the file boxes, the Inbox's sections) stay `raised` with their `line`
  border and radius 10: they step up from the island.
- **The empty states** (`.empty`) stay as they are, in the island.

## Per page: what changes and what stays

### The Inbox (`312:10696`, light `318:6930`)

- **One island**, `Island · Inbox`, the main column's full width and height under the top bar.
- Its **header** is today's toolbar: New task, Search ID or title, then on the right Filters, Sort and Group (no Board or
  List and no Archived cards in the Inbox, as today).
- Under it, scrolling inside the island: **From the Chief** and **Run this for me** (padding `20 24 0`, as `.page-pad`),
  then the list (padding `14 12 24`). The rows, the group heads, the screenshots sign and the badges are today's.
- Stays: what the Inbox lists, its counts, its texts, the run-for-me box (on `island` = bg, its 3px `needs` bar).

### A card's page: Card (`312:11231`, light `318:7070`), Changes (`312:11747`), Screenshots (`312:12263`)

- **Card | Changes | Screenshots move into the card island's header**, as GA-284's island tabs (`Islands/Island tab`):
  28 high, the selected tab a pill in `selected`, the others no fill (hover `hover`). They take the place of the
  underlined `.task-tabs` strip. **Changes keeps its diffstat** in the tab (`+964 −112` in `diff-add` and `diff-del`,
  `t-id`) or **No changes**; **Screenshots keeps its count** (`text-3`) or **None**. The tabs have no close. Their links,
  roles and `aria-label` stay.
- The header stays the same on all three tabs, so the tabs don't move when you switch.
- **Properties becomes an island of its own**, right of the card island, 6 apart, `--size-props` 320 wide, the full
  height. Its header is GA-284's island header with a title: **Properties** (`t-body-strong`, 6px in) and the close
  button (`x`, App/Icon button, tooltip **Close properties (])**). Its rows are today's (the 96px label column, the same
  order, `.props-sep`), padding `4 12 24 14`. The 48px `.props-head` with its border goes into the 40px island header.
- With Properties closed (`]`, or always first on Changes and Screenshots) the card island takes the full width and
  **Show properties** (`panel-right`) shows in the top bar's actions, as today.
- **Card tab**: the doc as today (`.doc-in` max 920, centred, padding `22 36 64`), on the island: task head, title,
  Description, Acceptance criteria, Attachments, the run panel, the pull request panel, then Comments, Activity and Runs
  (their underlined tabs stay: they are inside the doc).
- **Changes tab**: the file list (260) stays inside the card island, left, with a 1px `line` on its right. It loses the
  `side` fill: the island is its surface (side is for recesses). Its summary, Filter files…, the tree, the keys, the
  fold to a 36px rail stay. The diffs are on the island, each file box with its sticky header on `raised`.
- **Screenshots tab**: the sets as today, inside the card island; padding `20 18 48` (was `24 28 48`) so four 272px
  thumbnails fit in the island at 1440 wide.
- **Chips keep GA-284's hover card**: an @ chip in the description, a comment or a run summary (`Item chip`, `GA-283` in
  the frame) shows `Islands/Hover card` on rest, as GA-284 and GA-287 say. Builds on and Needed by are links, not chips:
  they stay as they are.

### Tasks as a list (`312:12779`)

- **One island**, `Island · Tasks (list)`, as the board's (GA-284 `273:1172`). Its header is the board's view toolbar,
  with **List** pressed in Board or List and **Group** after Sort.
- The list scrolls inside the island: padding `4 12 24`, group heads with their `+` (New task in {group}), Done (and
  Cancelled) folded, the rows as today; a live card has its **Live** badge (`GA-286` in the frame).
- Stays: the keys (J/K, Ctrl+B), the filters, sort and group, the empty state.

### Projects (`312:13295`) and a project (`312:13830`)

- **Projects: one island.** Its header: New project, then the status chips (All, Planned, Active, Paused, Done) on the
  right. The table (DataTable) fills the island: the header row (36), the rows (40), the foot (**3 of 4 projects** and the
  AI usage note with **Open Usage**) at the island's bottom with a `line` above it.
- **A project: one island with two tabs in its header** (GA-331), Overview and Time tracking, as Island tabs like
  Usage's and Settings', and no toolbar. Overview is the page as before (padding `24 28 48`): the tile, the name and its
  line, Open tasks, New task and Edit; then Goal, Docs and Files on the left, Tasks, Repository and Budget on the right,
  each a `raised` panel. Time tracking is a table (DataTable) that fills the island, as on Projects, with its foot (the
  sums and a note) at the island's bottom.

### Clients (`312:14365`) and a client (`312:14901`)

- **Clients: one island.** Its header: New client, the search (**Name, city, KvK, BTW, contact**), then the chips (All,
  Active, Lead, Inactive) on the right. The table and its foot as on Projects.
- **A client: one island without a header**: the tile, the name and its line, Archive and Edit; Details on the left,
  Contacts and Projects on the right. The Archive question (inline) and the contact drawer are today's; the drawer sits as
  every drawer (below).

## What floats

Everything that floats takes GA-284's floating look: fill **`float`** (`#22242a` in dark, white in light), a **1px
`line-2` border** all round, **`shadow-pop`** (a drawer: `shadow-drawer`), solid (no glass). `overlay` becomes `float`
everywhere. Each keeps its own size, radius, padding and content:

| What | Frame | Where | Size and radius | Scrim |
|---|---|---|---|---|
| The command palette (`.pal`, Search, Ctrl K) | `318:7391` | centred, 14vh from the top | 640 (max 92vw), `radius-xl` 14, padding `6 6 10` | yes |
| A drawer (New task, and every `Drawer`) | `318:10747` | an island of its own at the right (below) | min(1024, 100vw − 48), radius 10 | yes |
| A popover (`.pop`, Filters) | `318:7589` | 6 under its button, aligned right | min 220, max 360 high, radius 10, padding 6 | no |
| A menu (`ContextMenu`, a terminal in the sidebar) | `318:8015` | at the pointer | 220, radius 10, padding 6 | no |
| The close question (`.ask`, CloseGuard) | `318:10976` | centred | 460, `radius-xl` 14, padding `20 22 16` | yes |
| A toast (`.toast`) | `318:7589` | 20 from the window's right and bottom edges | its content, radius 10, padding `10 14` | no |

- **The scrim** stays (`surface/scrim`, `rgba(0,0,0,.55)` dark, `rgba(20,22,27,.28)` light) and covers the whole window:
  the islands, the background and the glass.
- **Menu and popover rows** (`Islands/Menu row`): 30 high, padding `0 8`, radius 6, `t-body`; hover `hover` (a step
  lighter than float); the picked one has a `check` in `accent`; a key or hint on the right in `t-caption` `text-3`
  (**F2**). Headings `t-nav-label` `text-3` (**PROJECT**, **LABELS**, **ASSIGNEE**); a separator is a 1px `line`, 6 above
  and below.
- **The palette**: its input row (`search`, 15px text, a `line` under it), then the rows (34 high, radius 6, the active
  one `hover`): icon, the id for a task, the label, the hint, the kind on the right. The list starts with New task and Chat
  with the Chief, as today.
- **The close question**: the title in `t-drawer-title`, the text in `t-small` `text-2`, **Close** (danger) and **Stay**
  (primary, with the focus; Escape is Stay), right-aligned.
- **The toast**: the text in `t-body`, **Undo** as a small secondary button after archiving a card (8 s); errors without
  it (5 s).

### How a drawer sits

**A drawer is an island of its own, floating over the work islands**, not a panel flush with the window's edge:

- It sits **6px (`island-gap`) in from the window's top, right and bottom edges**, so it lines up with the islands and
  the gaps around them, and has **`island-radius` (10) on all four corners**.
- Width as today, `min(1024px, 100vw − 48px)` (the wide drawer 1280); height `100vh − 12px`.
- It **floats over** the islands and the background: the sidebar island and the main column don't move or shrink. It
  is `float` with a 1px `line-2` border all round (today only the left) and `shadow-drawer`; the scrim dims the rest of
  the window behind it; a click on the scrim closes it, as today.
- It **slides in from the right** as today (32px and a fade, .2s), and out the same way.
- Inside it: today's head (padding `18 28 16`, a `line` under it, the title, the subtitle, `x`), the body (padding
  `24 28 40`, scrolling, inputs on `raised`) and the foot (padding `14 28`, a `line` above it). Its content keeps to the
  round corners (`overflow: hidden`).

Why: along the edge with square corners it would be the one surface in the window that doesn't follow the islands; as an
island of its own it reads as one more island that came to the front, and its float fill, border and shadow say it is on
top. Pushing the work islands aside instead would reflow the board and the list under the drawer each time it opens.

```css
.drawer {
  top: var(--island-gap); right: var(--island-gap); bottom: var(--island-gap); height: auto;
  border: 1px solid var(--line-2); border-radius: var(--island-radius); overflow: hidden;
  background: var(--float); box-shadow: var(--shadow-drawer);
}
.pop, .pal, .ask, .toast { background: var(--float); } /* was overlay */
```

## The states drawn

- Tabs: the selected card tab (Card, Changes, Screenshots) and the others at rest.
- Rows: the hovered popover row (**Hersir** in Filters), the hovered menu row (**Rename**), the palette's active row
  (**New task**), the Changes list's current file (`selected`), the list rows at rest, a live card (**Live**), a card on
  hold (**On hold**), the screenshots sign.
- Buttons: the focused **Stay** (its 2px `focus` ring), the danger **Close**, primary, secondary and ghost buttons,
  checked and unchecked checkboxes, the on chip (**Active** on Projects, **All** on Clients).
- Not drawn (as today, unchanged by islands): loading, errors (`.error-banner` sits at the top of the island, under its
  header), the empty states, an archived card, the unsaved-changes foot of a drawer.

## Every text, word for word

**No new text.** Every text in the frames is today's: the top bars, the toolbars, the Inbox's sections, the card's tabs
and Properties (**Properties**, **Close properties (])**, **Show properties (])**), the tables and their feet, the
project and client pages, the palette's placeholder **Type a task id, title, project, client or note…**, the drawer's
**New task** / **Its column decides which agents pick it up, and when.** and its fields, the popover's and the menu's
rows, the close question's **Close Hersir without saving your changes to {title}?** / **Saving them failed or took too
long. Stay keeps them in the editor, unsaved.** / **Close** / **Stay**, and the toast's **{ID} archived** / **Undo**.

The cards, chats, commands, projects, clients, contacts, files and diffs in the frames are sample data, not texts to
build. Gone: nothing.

## Not in this card

- Building it.
- Chat, the Tasks board, the Editor and the closed sidebar's toolbar (GA-284); the Chief Chat's side island (GA-285);
  Memory, the team pages, Settings and the rest (GA-288).
- What a page holds or does.
