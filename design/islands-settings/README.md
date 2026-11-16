# Hersir in islands (4 of 4): Memory, docs, agents, Team, Users, Usage, Settings and the terminal page (GA-288)

The design is in Figma, in the file **Giz-AI**, on the page **Hersir Islands 4: Memory, team and Settings** (node
`314:3616`):
https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=314-3616

These are the notes for the builders. Nothing in the app changes in this card: building waits for Jefsev's OK. It
builds on GA-284's look and window (`design/islands/README.md` on GA-284's branch, Figma page `268:2`): its tokens, its
components and its glass. What a page holds and does stays as it is: only how it is laid out and drawn changes.

## The frames

Open a frame with `https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=<id with - for :>`. All are
1440 × 900 and dark, except the light frame and the states sheet.

| # | Frame | Node | Page today |
|---|---|---|---|
| 1 | Memory · A note open · Dark | `324:3511` | `src/pages/MemoryPage.tsx` |
| 2 | A doc · Dark | `319:2640` | `src/pages/DocPage.tsx` |
| 3 | An agent · Dark | `323:3267` | `src/pages/AgentPage.tsx` |
| 4 | Team · Dark | `321:2725` | `src/pages/TeamPage.tsx` |
| 5 | Users · Dark | `316:289` | `src/pages/UsersPage.tsx` |
| 6 | Usage · Dark (the Subscription tab) | `321:3339` | `src/pages/UsagePage.tsx` |
| 7a | Settings · General · Dark | `316:4749` | `src/pages/SettingsPage.tsx` |
| 7b | Settings · Appearance · Dark (scrolled to the end, so Background shows) | `316:10401` | `src/components/AppearanceSettings.tsx` |
| 7c | Settings · Notifications · Dark | `316:5383` | |
| 7d | Settings · Agents and runs · Dark | `318:9965` | |
| 7e | Settings · MCP servers · Dark | `318:12028` | `src/components/McpSettings.tsx` |
| 7f | Settings · GitHub and Bitbucket · Dark | `318:12623` | `GithubSettings.tsx`, `BitbucketSettings.tsx` |
| 7b+ | Settings · Appearance · The glass switch, its states · Dark (a sheet, 1440 × 420) | `318:1588` | |
| 8 | Terminal · Its own page · Dark | `319:2266` | `src/pages/TerminalPage.tsx` |
| 9 | Onboarding · Step 1 · Dark | `324:4204` | `src/pages/OnboardingPage.tsx` |
| 10 | Settings · Appearance · Light | `318:1088` | |
| | Islands 4 components (the new icons) | `314:3617` | |

The rows on the page: row 1 (y 0) frames 1 to 4; row 2 (y 1100) Users, Usage, the terminal page and Onboarding; row 3
(y 2200) Settings General, Appearance, Notifications, Agents and runs; row 4 (y 3300) MCP servers, GitHub and
Bitbucket and Appearance in light; the glass switch's states sheet under the light frame. A label sits above each row.

The light frame is frame 7b with the **GizAi colour** collection set to **Light** on the frame, `Islands/Window
background` set to Wallpaper=Light, and Theme's Light pressed. Every colour is a GizAi variable, so the frame switches as
a whole.

## How every frame is built

- **The window** is GA-284's: `Islands/Window background` (`301:3161`) first in each frame, absolute, stretched and
  locked (Wallpaper=Dark in the dark frames, Light in the light one), the frame's own fill empty, `island-gap` (6) all
  round and between islands.
- **The sidebar island and the top bar** are GA-284's Chat frame (`271:136`) as they are, with the current page's row
  in the sidebar: Usage, Team, Users or Settings in Company; the agent's row in Agents; the terminal's row in Terminal;
  for Memory, the Memory section open (Chief, Shared notes, each agent) and Agents folded. A doc has no current row,
  as today.
- **The top bar sits on the background** (44 high, no fill, no border): crumbs, then the page's actions, as today.
  Following GA-284's [On the background](../islands/README.md#on-the-background): faint words there (`.crumbs .sep`,
  a count, the terminal's folder) are `text-2`; secondary and ghost buttons have no fill; a chip with a fill sits on
  `island` (Team's team picker, as the Chief's chip).
- **Islands** are frames drawn as `Islands/Island`: fill `island` (`#181a1c` / white), radius `island-radius` (10), no
  border, no shadow, `overflow: hidden`. The 1px `line` borders that split a page into regions (`.props`,
  `.mem-files`, `.mem-side`, `.mem-note-bar`, `.props-head`, `.tabs`) go: the gaps do that now. Lines inside an island
  stay (table rows, a form section's foot, a panel's border).
- **An island's header** is GA-284's (40 high, padding `0 6px`, 2px between items): `Islands/Island tab` for tabs, a
  title in `t-body-strong` for a title, `App/Icon button` for its actions. Where a page has a toolbar, the toolbar is
  the header (44, as the Tasks board's).
- **Inside an island** everything is as today, with GA-284's round 3 values: cards and panels `raised` with a `line`
  border, inputs and `.seg` on `raised`, hover `hover`, selected `selected`, code and terminals on the island
  (`bg` = `island`).
- **A page that scrolls** scrolls inside its island, under the island's header; the header and the top bar stay. The
  scrollbar is `Islands/Scrollbar`. Frames 7a, 7d and 7f show a page's top; 7b shows the end (it scrolls).
- **What floats** (menus, popovers, the note preview, the palette, toasts) is `float` with its `line-2` border and
  shadow (GA-284). **Drawers and dialogs** follow GA-286's drawer: the agent's form (frame 3's Settings), New note, a
  version (Restore this version), New team, a person, Export template's warnings. None has a frame here.
- Text styles are App/* only; every colour is bound to a GizAi variable (checked on every frame: no unbound fill or
  stroke, no text without an App style).

## Page by page

### 1. Memory (`324:3511`)

Three islands right of the sidebar, with the note open:

| Island | Width | Header | Body |
|---|---|---|---|
| Notes (`.mem-files`) | 260 (as today) | The scope's name as the title (**Shared notes**, the agent's name, …), then New note and New folder | The search box (`Search, path: or tag:`, on `raised`, 8px in), then the tree, or the search results while you type |
| Note (`.mem-note`) | the rest | The note bar: the status (`Saved · version 3 · Jefsev, 2 hours ago`, `t-small`, `text-3`), Read or Edit, Save (small, primary) | The note: its folder in `text-3`, its title, the note read (or the editor), max 860 |
| About this note (`.mem-side`) | 300 (as today) | none: the sections start at the top (Outline, Properties, Tags, History, Backlinks, Outgoing links, each folding) | |

- What changes: `.mem-files` loses its `side` fill and border (the island is its fill); its head was 11px uppercase in
  `text-3` and is now the island's title in `t-body-strong`, `text`, with its two buttons as the header's actions.
  `.mem-note-bar` (44, a bottom border, sticky) becomes the note island's header (40, no border, it stays while the note
  scrolls). `.mem-side` loses its `border-left`. The open note's row in the tree: `selected`, its icon and name `text`.
- The top bar keeps Memory / scope / folders / title, and the note's panel toggle (shown pressed: `selected`), Open
  local graph, Notes or Graph, and New note.
- **States:** no note open: the note island holds Recently changed (or the empty Memory text and New note when there
  are no notes), and there is no About this note island. The note's panel hidden: that island goes and the note island
  takes its width. The local graph open: it takes the panel's place, as an island of its own. Graph: the graph fills one island across
  the main column (no Notes island, as today). Search: the results take the tree's place in the Notes island. The
  preview on a `[[link]]` floats (`float`). Errors: the banner at the top of the note island, under its header.
- New note opens GA-286's drawer.

### 2. A doc (`319:2640`)

- Two islands: the doc (the rest; `.doc-in` centred, max 980, as today) and **History** (320, `--size-props`) on the
  right. The History panel's head (48, a bottom border) becomes the island's header with the title **History**; the
  versions below (the current one `selected`).
- The top bar keeps the crumbs (Projects / project / doc), Editor or Markdown, the status (`Saved · version 4`, now
  `text-2` on the background) and Save.
- **States:** unsaved, saving and not saved as today in the top bar; the conflict banner and an error sit at the top of
  the doc island; a version opens in GA-286's drawer (wide).

### 3. An agent (`323:3267`)

- One island holds the page as it is: the entity head (the 48px avatar, name, the line under it, and its actions:
  Open chat, Notes, Assign task, Run, Pause or Resume, the state, Settings), the banners, Working now or Latest run, the
  three stat cards, Recent runs, Board checks, Instructions. The island has no header: the entity head is its top, 22px
  in. The page scrolls in the island.
- The actions stay in the entity head (not the top bar): the top bar keeps only Team / the agent.
- The frame shows a working agent: the state `running` (`live` on `live-soft`), Run disabled, Working now's live card.
  Idle: Latest run's card and `idle`; no runs: the empty text, as today.
- Settings opens the agent's form in GA-286's drawer, so it has no frame here.

### 4. Team (`321:2725`)

- One island holds the page: Organisation (the chart in its panel, now a `line` border on the island, as `bg` =
  `island`), Workflow (the columns, Add column), Labels and People below it (they scroll).
- The top bar keeps Team / the team picker / the count, Export template, New team and Add agent. The picker is a chip
  on the background, so it gets `island` under it and its `line` border.
- A dragged agent and the toast float; New team and Export template's warnings follow GA-286's drawer and dialog.

### 5. Users (`316:289`)

- One island: its header is today's toolbar (Add person, then the line about people and agents, `t-small`, `text-3`),
  44 high, as the board's toolbar is; then the table on the island (no fill of its own); `.tablefoot` at the island's
  bottom, its `line` top border kept.
- Empty, loading and error as today, inside the island. A person opens GA-286's drawer.

### 6. Usage (`321:3339`)

- One island: its header holds the four tabs (Subscription, Total, Agents, Projects) as Island tabs, and right of them
  what today's toolbar holds: on Subscription the line `The newest numbers each coding CLI reported in your agents'
  runs and chat turns`; on the other tabs the period chips and `UTC days`. The toolbar row above the tabs goes.
- Under it the page as today (the limits blocks as `raised` panels, or the tables and the bars), and the note at the
  foot. The top bar keeps Usage and, on the other tabs, the period's days.
- **States:** no runs in the period: the empty text in the island; an error at the top of the island.

### 7. Settings (`316:4749`, `316:10401`, `316:5383`, `318:9965`, `318:12028`, `318:12623`)

- One island: **its header holds the six tabs** (General, Appearance, Notifications, Agents and runs, MCP servers,
  GitHub and Bitbucket) as Island tabs with their icons; the open one is the `selected` pill, which replaces the tab's
  2px underline, and the `.tabs` line goes. The address stays `#/settings/<tab>`.
- Below the header the open tab's form scrolls, as today: `.page.settings-page` max 1100, left aligned in the island,
  padding `16px 28px 48px` (the top was 24: the tabs above it went to the header); sections in two columns (220 for the title and text, then the fields), a `line` between them.
  The ok and error banners show at the top of this scroll area.
- **Save settings** stays in the top bar (primary).
- Each tab holds what it holds today (the frames draw a typical computer: Claude Code found, no second CLI, one MCP
  server, GitHub signed in, no Bitbucket login, no backup folder). The forms that open inline (Add CLI, Add MCP server)
  open inline as today.

#### Appearance: the glass switch (frames 7b, 10 and the states sheet `318:1588`)

The Background section is GA-284's proposal, in the place of the mountains' checkbox (the mountains go):

- Section **Background**, its text **Behind the islands: a soft gradient, and glass where your system blurs windows.**
- Field **Glass**, a checkbox **Show the desktop through the background**, with the hint **Blurred and tinted, on macOS,
  Windows 11 and Hyprland. Off gives a plain background.**
- **Default: on** where the system blurs windows (macOS, Windows 11, Hyprland); Reset to defaults turns it on again.
  Kept on this computer with the other appearance settings (`glass`, as `band` was), saved at once like them.
- The system gives no blur: the checkbox is off and disabled, with the hint **Your system doesn't blur windows behind
  Hersir, so the background stays plain.**
- Reduce transparency is on: the checkbox keeps its value, with the hint **Plain while your system reduces
  transparency.**

The states sheet draws the three side by side. GA-284's README has how the glass is built and the fallback.

#### Appearance in light (frame 10, `318:1088`)

The same tab in light: white islands on the light glass; Theme shows Light pressed. Nothing else differs.

### 8. A terminal's own page (`319:2266`)

- One island: its header is the terminal's tabs (`TerminalTabs`) as Island tabs with the terminal icon (the open one
  `selected` with its close), and New terminal (`+`) on the right; then the terminal on `term-bg` (= `island`), 6px
  under the header, 14px in.
- The top bar keeps Terminal / the name, the folder, shell and size (`~/Herd/gizai · bash · 138 × 42`, now `text-2`),
  Show in Editor and More.
- **States:** the ended and failed bars (`.ed-bar`) sit at the top of the island, under its header. A terminal that
  isn't open any more: the empty text and New terminal centred in the island, with no tabs in its header. The terminal
  with the focus: the filled block cursor (as drawn); without it, the outline (xterm does this). More's menu and the
  close question float.

### 9. Onboarding (`324:4204`)

The first start's setup gets the window too, with no sidebar:

- The glass (`Islands/Window background`) behind it. The **band goes** here as well (GA-284 kept it until this card).
- **The top row sits on the background**: 44 high (was 52), no border: the Hersir logo (`text-2`, 24 high) on the left,
  the four steps in the middle, Skip setup (ghost) on the right. The current step's number: an `accent` ring on
  `island`, its name `text`; the others `text-2` with a `line-2` ring (on the glass `text-3` would drop under AA).
- **One island** under it, **760 wide** (the 640 column and 60 each side), centred, down to 6px above the window's
  bottom edge: the eyebrow (`Step 1 of 4`, `text-2`), the heading, the lead, the step's parts, and at its foot the
  actions (Back, Next) above a `line`. The window is never narrower than 900, so the island always fits.
- **The other steps** use the same window and island; only the island's content changes, as today. Step 2 (Coding
  CLIs): Found on this computer and its rows on `raised`, the agents' CLI select, the info line. Step 3 (MCP servers):
  the list with its checkboxes and the read-only caption; the Optional badge after the heading. Step 4 (You're all set):
  the summary rows with Change, the message the chat opens with, and Open the Chief's chat (or Open the board). A step's
  error shows above the actions inside the island; a done step in the top row shows its check and goes back to it, as
  today. A step longer than the island scrolls inside it; the actions stay at its foot.

## What changed where a page's layout had to change to fit islands

1. **Tabs move into an island's header** (Settings, Usage): the underlined tab row inside the page becomes Island tabs
   in the header, the selected tab a pill. Usage's toolbar line (or its period chips) joins them on the right.
2. **A side panel becomes its own island** (the doc's History, Memory's Notes and About this note): its own head
   becomes the island's header (History; the scope's name with New note and New folder), or goes (About this note has
   no header: its folding sections start at the top).
3. **A bar under the top bar becomes the island's header** (Memory's note bar, the terminal's tabs, Users' toolbar).
4. **A page's actions stay where they are**: in the top bar (Settings, Memory, Team, a doc, a terminal) or in the
   entity head (an agent).
5. **Onboarding** is one island in the middle of the glass, under a top row on the background.

## New Figma parts

On the sheet **Islands 4 components** (`314:3617`), Lucide icons as components, 16 px, 1.75 stroke at 24, `text-2`
(as GA-284's), for the icons the pages use that the file didn't have:

`file-text` `314:3624`, `file-plus` `314:3630`, `folder-plus` `314:3635`, `folder` `314:3638`, `waypoints` `314:3647`,
`book-open` `314:3651`, `hash` `314:3657`, `notebook-text` `314:3667`, `play` `314:3670`, `pause` `314:3674`, `gauge`
`314:3678`, `ellipsis` `314:3683`, `rotate-ccw` `314:3687`, `grip-vertical` `314:3695`, `download` `314:3700`,
`link-2` `314:3705`, `sun` `314:3716`, `moon` `314:3719`, `brain` `314:3724`, `circle-check` `314:3728`, `git-merge`
`314:3733`, `key-round` `314:3737`, `log-out` `314:3742`, `archive` `314:3747`, `power` `316:4287`, `circle-arrow-up`
`316:4292`, `folder-open` `316:4295`, `database-backup` `316:4303`, `archive-restore` `316:4310`, `log-in` `316:4315`,
`plug-zap` `316:4322`, `circle-x` `316:4327`.

The frames otherwise use GA-284's `Islands/Window background`, `Islands/Island tab`, `Islands/Scrollbar` and the App
components (Nav item, Nav label, Button, Icon button, Input, Select, Checkbox, Badge, Avatar, Agent chip, Seg, Link,
Live tag). Segmented controls with more than two choices (the text sizes, Notes or Graph, Read or Edit, Dark or Light
with icons) are frames drawn as `.seg`.

## Every text, word for word

**New:** none in the app beyond GA-284's glass switch, which this card draws in Settings → Appearance → Background:

- the section's text **Behind the islands: a soft gradient, and glass where your system blurs windows.**
- the field **Glass**
- the checkbox **Show the desktop through the background**
- its hint **Blurred and tinted, on macOS, Windows 11 and Hyprland. Off gives a plain background.**
- with no blur from the system **Your system doesn't blur windows behind Hersir, so the background stays plain.**
- with reduce transparency on **Plain while your system reduces transparency.**

**Gone:** Settings → Appearance → Background's **A landscape of wireframe mountains along the bottom of the window. It
stays behind text and controls.**, **Mountains**, **Show mountains along the bottom** and **Under the sidebar and the
page, in the theme's colours. Off gives the plain background back.** (as in GA-284's notes).

**Unchanged:** every other text in the frames is today's, moved, not reworded (the island titles **Shared notes** and
**History** are today's scope name and panel head). Names, cards, runs, numbers, notes, paths and terminal output in the
frames are sample data, not texts to build. The row labels on the Figma page and the states sheet's captions are notes
for the builders.

## Not in this card

- Building it.
- Chat, the Tasks board, the Editor, the Chief Chat's side island, the work pages and what floats (GA-284, GA-285 and
  GA-286), and so the drawers and dialogs these pages open.
- What a page holds or does.
