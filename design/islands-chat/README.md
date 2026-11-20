# Hersir in islands (2 of 4): the Chief Chat's side island and its tool strip (GA-285)

The design is in Figma, in the file **Giz-AI**, on the page **Hersir Islands 2: the Chief Chat's side island** (node
`314:3748`): https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=314-3748

These are the notes for the builders. Nothing in the app changes in this card. It builds on GA-284 (card 1 of 4,
`design/islands/README.md` on its branch): its look, its glass (`Islands/Window background`), its tokens and its Island
components. Its rules stay true here; this card replaces the right-hand side of GA-284's Chat frames (the terminal panel)
with one **side island** that holds four kinds of tabs, and today's terminal rail with a **tool strip** along the
window's right edge, as JetBrains' Islands do.

## The frames

Open a frame with `https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=<id with - for :>`.

| # | Frame | Node | Size |
|---|---|---|---|
| 1 | A chat the Chief started about GA-259 and GA-283: the side island on GA-259's Card tab, GA-283's tab beside it; the strip shows Card 2, selected | `314:3809` | 1440 × 900 |
| 2 | A Chief answer with its row of buttons (two cards, Open board, a file at a line, Open in terminal); nothing open beside the chat, so no strip | `316:5906` | 1440 × 900 |
| 3 | The Board tab: the chat's project, the chat's cards marked; strip Board 1 (selected) and Card 2 | `316:6153` | 1440 × 900 |
| 4 | The Editor tab: `src-tauri/tauri.conf.json` at line 4, the line marked; strip Editor 1 (selected) and Card 2 | `316:6400` | 1440 × 900 |
| 5 | A Terminal tab between a Card tab and an Editor tab; strip Terminal 1 (selected), Editor 1, Card 1 | `316:6647` | 1440 × 900 |
| 6 | The side island hidden: strip Terminal 2, Editor 1, Card 3, none selected; Terminal's tooltip (hover) and Card's right-click menu | `316:6894` | 1440 × 900 |
| 7 | The Card tab of GA-283, in Review: its pull request and QA's verdict first | `316:7141` | 1440 × 900 |
| 8 | Frame 1 in light | `316:7388` | 1440 × 900 |
| 9 | Frame 1 at 900 × 600 with the sidebar closed (the chat list hidden) | `318:13311` | 900 × 600 |
| | Islands 2 components (sheet) | `314:3749` | |

Every frame has `Islands/Window background` first (Wallpaper=Dark, Light in frame 8), the sidebar island and top bar of
GA-284, and the islands drawn as frames exactly as `Islands/Island` (names start with `Island ·`). Frame 8 is frame 1
with the GizAi colour collection set to Light on the frame. Colours are GizAi variables, texts App/* styles.

### Components

| Component | Node | Notes |
|---|---|---|
| `Islands/Strip button` (State=Default `314:3788`, Hover `314:3793`, Selected `314:3798`, Focus `314:3803`; props **Icon**, **Count text**) | `314:3808` | One button of the tool strip. New. |
| New Lucide icons, 16 px, 1.75 stroke at 24, `text-2`: `file-code` `314:3757`, `message-square` `314:3760`, `play` `314:3763`, `step-forward` `314:3767`, `arrow-right-left` `314:3773`, `ellipsis` `314:3778`, `circle-check` `314:3782`, `git-merge` `314:3787` | `314:3749` | `file-code` is the Editor kind's icon. |
| Reused from GA-284 and the App pages | | `Islands/Window background`, `Islands/Island tab`, `Islands/Tooltip`, `Islands/Sidebar toolbar`, `Islands/Scrollbar`, App/Button, App/Icon button, App/Badge, App/Avatar, App/Label pill, App/Kbd, the Status glyphs, `Icon/terminal`, `Icon/square-kanban`, `Icon/columns-3`, `Icon/git-pull-request`. |

## The layout

- **The window** (1440 wide, sidebar open): sidebar island 248, then the main column. The **top bar** (44, on the
  background) spans the whole main column, up to 6 from the window's right edge, as in GA-284.
- **The tool strip** takes a 40 px column at the window's right edge, under the top bar (from the islands' top, y 50 at
  1440 × 900, to 6 above the bottom edge). The islands end where it starts, so the gap to the strip is its own 6 px of
  padding. While nothing is open beside the chat the strip is gone and the islands reach the edge as today (frame 2).
- **Under the top bar**: the chat list island (260; 220 under 1100 px wide), the conversation island, and the **side
  island** on the right, 6 apart. In the frames the side island is 440 wide (conversation 428).
- **The side island's width** is today's terminal panel's: a share of the room right of the chat list, 45% the first
  time, kept on this computer (`gizai.chat.panel`, the same key), at least 360 px; the gap between the conversation and
  the side island is the handle (`Islands/Resize gap`). The conversation keeps **at least 400 px**.
- **A narrow window** hides the chat list first while the side island shows (today's `no-list`), then the side island
  gives up width down to its 360. Frame 9 (900 × 600, sidebar closed): toolbar, conversation 400, side island 448, strip
  40. **The tool strip always stays.**

## The side island

One island in place of today's terminal panel (`ChatTerminal.tsx`), drawn as an Island: fill `island`, radius
`island-radius`, no border.

### Its header (`size-island-head`, 40)

- **Tabs** (`Islands/Island tab`), one per thing open, in the order they opened (a new tab goes at the end). Each kind
  has its icon and its name:

  | Kind | Icon | The tab's name | Tooltip (`title`) |
  |---|---|---|---|
  | Terminal | `terminal` | its folder's name (`gizai`, `gizai 2`), as today | the folder's path |
  | Editor | `file-code` | the file's name (`tauri.conf.json`); two with the same name get their folder as today's Editor tabs do | the file's path |
  | Board | `columns-3` | the project's name (`Hersir`), or **Board** for the team's board | **Board of Hersir** |
  | Card | `square-kanban` | the card's id (`GA-259`) | the card's id and title |

  Selected tab: the `selected` pill (GA-284). Close (×) shows on the shown tab, the hovered one and an Editor tab that
  isn't saved. Middle click closes, as in the Editor. Closing the shown tab shows the one to its right (else its left);
  closing the last tab hides the island and the strip goes.
- **Tabs that don't fit** go in an **overflow** button after the last tab that fits: `chevron-down` and the number
  (**2 more**), a menu listing them with their icons; picking one shows it and moves it into the row in place of the
  last one that fit. The shown tab always stays in the row.
- **Actions** (App/Icon button, 26 × 26), right:
  1. **+**, `plus`: a menu **New terminal** (in the shown terminal's folder, else the folder picker, as Open terminal
     today; key Ctrl+Shift+\`), **Open a file…** (the Editor's Go to file box over the project's folder), **Board**.
     Tooltip **Open beside the chat**.
  2. **Own page**, `maximize-2`: the shown tab on its own page (a terminal on the Terminal page as today, a file in the
     Editor page at its line, the board on the Tasks page, a card on its page); the tab closes here. Tooltip
     **Open GA-259 on its own page** (the tab's name).
  3. **Hide**, `chevron-right`: hides the island; terminals keep running. Tooltip **Hide (Ctrl+\`): terminals keep
     running**.
- Keys inside the island: **Ctrl+PgDn** and **Ctrl+PgUp** switch tabs (today's, now over every kind); **Ctrl+W** closes
  the shown tab except in a terminal (the shell's there) and in the Editor tab it asks as the Editor does.

### Card (frames 1, 7)

For reading and checking a card quickly. Top to bottom, padding `8 16 16`, 16 between blocks, scrolling:

1. **Head**: the status glyph (with the hold, as on the card's page), the id in `t-id` `text-2`, the project's dot and
   name (`t-small`, `text-2`).
2. **Title**: `t-section`, `text`, wrapping.
3. **In Review only, first**: the **Pull request** box (as `PullPanel`: `git-pull-request`, **Pull request**, `#231`
   with `external-link` opening GitHub, the state badge, the diffstat, **Show changes** opening the card's Changes tab
   on its page; under it the hint in `t-small` `text-2`), then **QA's verdict**: `circle-check` in `success`,
   **QA's verdict**, the run's badge (**QA passed** / **QA failed**, `answerOf`), **Run 2 · 1 h ago**, and the QA
   Agent's hand-over (3 lines, then an ellipsis; a click on it opens the run's comment on the card's page). Both boxes:
   `raised`, 1 px `line`, radius 10, padding `10 12`.
4. **Properties** (labels 76 wide, `t-small` `text-3`; values `t-small` `text`):
   - **Column**: the column's glyph and name.
   - **Hold** (only when held): the hold's badge in `needs` (`HOLD_NAMES`: **Needs your decision**, …), under it the
     reason in `t-small` `text-2` (60 characters, then **Read in comments**, as Properties does). GA-314: **Read in
     comments** opens the card's page on its Comments tab, at the latest comment that holds the reason, else at the top
     of its comments; the Card tab stays open beside the chat. Under it (or under the reason) **Clear hold**, a `btn sm`
     with the tooltip **Takes the hold off, so its column's agents can start it**, on every hold (also **With the Chief**
     and the waiting-for-OK hold): it takes the hold off as Clear hold on the card's page does, shows a spinner and
     **Clearing…** until the row goes, and an error in `t-small` `danger` under it (the hold stays).
   - **Assignee**: avatar and name; **Not assigned** in `text-2`.
   - **Labels**: the label pills; the row goes when there are none.
5. **Description**: heading `t-group` `text-2`; the text in `t-body`, **folded after 4 lines** with **Show more** (and
   **Show less**) in `accent`. Read-only here: editing is on the card's page.
6. **Acceptance criteria**, with the count **2 of 6** (checked of all): the first 3, then **Show all 6**. Checked items
   in `text-2` with the `accent` box.
7. **Comments**, with the count: the latest 2 (avatar, name, time, the text folded after 3 lines), then **Show all 4**
   (they unfold in place, oldest first).
8. **Runs**, with the count: the latest 3, newest first: **Run 2**, the agent, the outcome badge (`answerOf`), the time.
   A click opens the run on the card's page.

**The foot** (actions, 1 px `line` above, padding `10 12`), each only where the card allows it, as on the card's page:

- **Comment** (`message-square`, secondary): the foot becomes the comment box (`CommentComposer`: the text box, then
  **Comment** (primary) and **Cancel**); posting adds it under Comments and closes the box.
- **Run** (`play`, primary) when the card can run (`RunPanel`'s rule: not held, an agent to run it), or **Continue**
  (`step-forward`, primary) when its last run can go on (frame 1). Neither on a card in Review (frame 7) or Done.
- **Move to** (secondary, `chevron-down` after): a menu of the board's columns with their glyphs, the card's own
  checked; picking one moves it as a drag on the board does.
- **Own page** is the header's.

A card that is gone or archived: the tab stays, its body says **This card was archived.** (with **Restore** on its
page) or **This card is gone.**

### Board (frame 3)

- The board of **the chat's project**: the project of the cards the chat is about (`ChatThread.tasks`; the first card's
  when they differ), else of the cards an answer named, else **the team's board** (all projects).
- Under the header, a row: the project's dot and name (`t-label`), **the chat's project** (`t-small`, `text-2`) and on
  the right the legend: the mark and **In this chat**.
- The columns, compact: 180 wide, 8 apart, on `side` (radius 8, padding 6), the column's glyph, name and count; the
  board **scrolls sideways** (the 6 px thumb at the bottom, GA-284's scrollbar) and each column down. It opens scrolled
  to the column of the chat's first card.
- **Compact cards** (`raised`, 1 px `line`, radius 8, padding `8 10 9`): the id (`t-id`, `text-3`), the assignee's
  avatar, the title (`t-body`, 3 lines at most), **On hold** (`needs` badge) when held.
- **The chat's cards are marked**: a 1.5 px `accent` border and `messages-square` (12 px, `accent`) after the id. Its
  label for screen readers: **GA-259, in this chat**.
- **A click** on a card opens it in a Card tab (or shows its tab). **Ctrl+click** (Cmd+click on macOS) opens its page.
- **Dragging** between columns works as on the Tasks page (the same drop rules, the dragged card floating as GA-284's
  "what floats"). Archived and cancelled cards stay hidden, as the board's default.

### Editor (frame 4)

- One file at a line. Under the header, a row: the path from the project's folder in `t-id` `text-2`
  (`src-tauri/tauri.conf.json`) and **Save** (secondary, small; disabled until there are changes; **Ctrl+S**, Cmd+S on
  macOS, with the focus in the tab).
- The code: the Editor's CodeMirror view and colours, on the island, line numbers in `text-3`.
- **The marked line** (the line it was opened at): a band of `accent-soft` the code's full width and a 2 px `accent`
  bar on its left edge, its number in `text`. The view scrolls it to a third from the top and puts the caret at its
  start. The mark stays until another line is opened.
- The status bar at its foot (1 px `line` above): **Ln 4, Col 15**, the language, **UTF-8**, the branch.
- **Own page**: the Editor page at that file and line (the files panel stays on the Editor page; there is none here).
- Unsaved: the tab's close asks as the Editor does (**tauri.conf.json has changes that aren't saved.** **Save and
  close**, **Close without saving**, **Cancel**).

### Terminal (frame 5)

As today (`ChatTerminal.tsx`): under the header the folder in mono (`t-id`, `text-2`), then the terminal on `term-bg`
(= `island`). Closing a terminal that runs a program asks today's question.

## The tool strip

A slim vertical strip along the window's right edge, **on the background** (no island, no fill), growing out of today's
rail (`.chat-term-rail`). It is the side island's switch: it shows whether the island is shown (one icon selected) or
hidden (none selected).

- **When it shows**: only while something is open beside the chat (at least one tab). It stays whether the island is
  shown or hidden. With nothing open it is gone.
- **Its icons**: one per kind open in this chat, in a fixed order **Terminal, Editor, Board, Card**. An icon appears when
  the first of its kind opens and goes when the last one closes.
- **The count**: under each icon, the number open of that kind (`App/t-count`), 1 too.
- **The button** (`Islands/Strip button`): 28 wide (`size-tool`), 42 high, radius 6, 4 apart, centred in the 40 px
  column: icon 16, 1 between it and the count.
  - Default: no fill, icon and count `text-2` (on the glass; GA-284's rule for the background).
  - Hover: `canvas-hover`, `text`.
  - **Selected** (the kind on show): a filled pill in **`island`**, `text`: a small piece of the island it shows (darker
    than the glass in dark, white on the grey in light), so the count keeps AA on any wallpaper.
  - Keyboard focus: the 2px `focus` ring outside.
- **Click**: shows that kind's **last shown tab** in the side island (showing the island when hidden). A click on the
  **selected** icon hides the island; terminals keep running.
- **Tooltip** (`Islands/Tooltip`, 6 px left of the button, centred on it): the kind and its count, and its key in
  `text-3`:

  | Kind | Tooltip | Key |
  |---|---|---|
  | Terminal | **Terminal, 2 open** | **Ctrl+\`** (today's) |
  | Editor | **Editor, 1 open** | **Alt+2** |
  | Board | **Board, 1 open** | **Alt+3** |
  | Card | **Card, 3 open** | **Alt+4** |

  The `title` is the same text with the key in brackets: **Terminal, 2 open (Ctrl+\`)**. Label for screen readers:
  **Terminal, 2 open**, `aria-pressed` true on the selected one. The strip: `role="toolbar"`,
  `aria-label="Beside the chat"`, ↑ and ↓ move between its buttons.
- **The keys** (proposed): **Ctrl+\`** Terminal (today's), **Alt+2** Editor, **Alt+3** Board, **Alt+4** Card, and
  **Alt+1** as Terminal's second key, so the numbers follow the strip's order (JetBrains' Alt+number for tool windows).
  On macOS **Option+2** to **4** (read by `e.code` `Digit2`… so the typed character doesn't matter; `preventDefault`).
  No page uses Alt with a digit (checked: App.tsx, TasksPage, EditorPage, ChangesView, ShotViewer, GraphCanvas,
  CodeMirror's default keymap). Each does what a click on its icon does: show that kind, or hide it when it is on show.
  With none of that kind open: Alt+2 opens **Open a file…**, Alt+3 opens the Board tab, Alt+4 does nothing. In a
  terminal they reach the shell (only Ctrl+\` and Ctrl+Shift+\` are Gizai's there, as today).
- **Right click** (or the context menu key, Shift+F10) on an icon: a menu (`float`, 1 px `line-2`, radius 10,
  `shadow-pop`, 216 wide, at the pointer or left of the button):
  - its head, `t-caption` `text-3`: **Card · 3 open** (the kind and its count);
  - **Show** with its key (App/Kbd) while that kind isn't on show, **Hide** while it is;
  - a line, then **Close all** in `danger`: closes every tab of that kind. For terminals, when one runs a program it asks
    first with today's question under the icon: one busy, **npm is still running in gizai 2. Close it and end npm?**;
    more, **npm is still running in gizai 2, and sleep in gizai. Close all and end them?**, with **Cancel** and
    **Close**. For the Editor with a file not saved: **tauri.conf.json has changes that aren't saved.** with **Save and
    close**, **Close without saving** and **Cancel**.

## What opens, and when

- **Why the chat started.** Opening a chat the Chief started about cards (`ChatThread.tasks`, for example a board
  check) opens the side island with those cards as Card tabs, the **first one shown**, the others beside it (frame 1).
  This happens **once per chat**: Hersir keeps the chat's id in `localStorage` `gizai.chat.sideOpened` (a list of ids,
  the newest 200) the first time; once you close those tabs or hide the island, opening the chat again leaves it as you
  left it.
- **What an answer names.** Under a Chief answer, **one row of buttons** for what it names (frame 2), ghost and small as
  Open in terminal today (GA-279), wrapping, in this order:
  1. **Open GA-12** for each card (`square-kanban`), **at most three**, then **and 2 more** (a menu of the rest).
  2. **Open board** (`columns-3`) when the answer made or moved cards.
  3. **Open in Editor** (`file-code`) with the file and line in mono after it (`tauri.conf.json:4`; its title the whole
     path), for files in the project's folder; one button per file, at most three.
  4. **Open in terminal** (GA-279's, unchanged).

  A click opens it in the side island (showing an open tab instead of a second one). **Nothing opens by itself while you
  read.** Which cards, files and commands an answer names is for the build card (data the buttons read).
- **Chips.** Resting on a card chip shows GA-284's hover card. A **click** on a card chip opens the card in a Card tab
  (or shows its tab). **Ctrl+click** (Cmd+click on macOS, `keys.ts`) opens the card's page and leaves the chat, as a
  click does today. The hover card's last line becomes **Click to open beside the chat** on the Chat page.
- **The top bar.** Open terminal becomes **Open** (`panel-right`, `chevron-down` after), one button for the side
  island, before New chat: a menu **New terminal**, **Open a file…**, **Board**, the same as the header's +.
- **Where tabs live.** The side island's tabs belong to the chat they were opened in and are kept while Hersir runs;
  switching chats shows that chat's tabs (terminals keep running in the background). New chat (no chat yet) has its own
  until the first message makes the chat.

## Every text, word for word

New in this design:

- The top bar's button **Open**; its tooltip **Open a terminal, a file or the board beside the chat**; its menu and the
  header's + menu: **New terminal**, **Open a file…**, **Board**.
- The header's actions: **+** tooltip **Open beside the chat**; **Own page** tooltip **Open GA-259 on its own page**
  (the shown tab's name); **Hide** tooltip **Hide (Ctrl+\`): terminals keep running**.
- The tab list's label for screen readers: **Beside the chat**; the overflow **2 more**.
- Card tab: the labels **Column**, **Hold**, **Assignee**, **Labels**; **Not assigned**; **Read in comments**; **Clear
  hold**, its tooltip **Takes the hold off, so its column's agents can start it**, and **Clearing…**; the
  headings **Description**, **Acceptance criteria**, **Comments**, **Runs**; **2 of 6**; **Show more**, **Show less**,
  **Show all 6**, **Show all 4**; **Pull request**, **Show changes**, **QA's verdict**, **Run 2 · 1 h ago**; the
  buttons **Comment**, **Run**, **Continue**, **Move to**, **Cancel**; **This card was archived.**, **This card is
  gone.**
- Board tab: **the chat's project**, **In this chat**, the label **GA-259, in this chat**; the tab **Board** for the
  team's board, tooltip **Board of Hersir**.
- Editor tab: **Save**; **Ln 4, Col 15**.
- The tool strip: **Terminal, 2 open**, **Editor, 1 open**, **Board, 1 open**, **Card, 3 open** (the number is the
  count); the keys **Ctrl+\`**, **Alt+2**, **Alt+3**, **Alt+4**; the strip's label **Beside the chat**; the menu
  **Card · 3 open**, **Show**, **Hide**, **Close all**; the questions above.
- Under an answer: **Open GA-259** (the id), **and 2 more**, **Open board**, **Open in Editor** (then the file and line),
  **Open in terminal** (GA-279's).
- The chip's hover card on the Chat page: **Click to open beside the chat**.

Gone: **Open terminal** in the top bar (it becomes **Open**); the rail's **Show the terminal (Ctrl+\`)**.

Kept: every other text is today's (the chat, the terminal panel's, the card page's, `HOLD_NAMES`, `answerOf`). The
chats, cards, files and terminal output in the frames are sample data.

## Choices this card left open

- **Keys**: Alt+2, Alt+3, Alt+4 (and Alt+1 for Terminal), after JetBrains' Alt+number; no page uses them.
- **The selected pill** is `island`, not a lighter fill: on Hersir's glass the islands are darker than the background in
  dark, so the pill reads as a piece of the island it shows; in light it is white on the grey, as JetBrains' light.
- **The count** sits under the icon, as on today's rail, not as a badge on the icon.
- **The right-click menu** has Show or Hide (whichever applies) and Close all, with a head naming the kind and count.
- **Tabs per chat**, in the order they opened; the side island's width is today's terminal panel's (one width for every
  kind).
- **The Card tab's actions** sit in a foot, so they stay in reach while the card scrolls; it is read-only apart from
  Comment, Run or Continue and Move to (edits stay on the card's page).
- **The Board tab** marks the chat's cards with an `accent` border and a chat icon, not a fill, so a card's state still
  reads.

## Not in this card

- Building it; which cards, files and commands an answer names, as data the buttons read (a build card later).
- The other pages (GA-286 and GA-288).
