# Hersir on the phone: the PWA's layouts (GA-321)

The design is in Figma, in the file **Giz-AI**, on the page **Hersir on the phone (GA-321)** (node `400:2`):
https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=400-2

These are the notes for the builders: card 12 (the phone layouts, `src/styles/app.css` and `src/components/phone/*`) and
card 11 (the PWA's own screens, push and install). Nothing in the app changes in this card. It is research card 10 of
GA-67 (`docs/research/hersir-cloud-personal.md`, sections 3.11 to 3.14 and 5.8, requirement R1) and builds on GA-317
(`design/hersir-cloud.md`, on GA-317's branch until it is merged): its hosts, their states and texts, the offline page and
the band. It uses the islands (GA-284 to GA-288) and `design/tokens.json`: no new colour.

## The frames

Every place is drawn four times: 375 × 812 and 430 × 932, dark and light. Open a frame with
`https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=<id with - for :>`. The rows on the page: 375 dark
(y 0), 430 dark (y 1000), 375 light (y 2100), 430 light (y 3100); one column per place. The light frames are the dark ones
with the **GizAi colour** collection set to **Light** on the frame, so every colour follows its variable. The 430 frames
are the 375 ones resized: they show what stretches (everything across, the island down) and what stays (bars, rows,
buttons, sheets' contents).

| # | Place | 375 dark | 430 dark | 375 light | 430 light | Adapts (desktop) |
|---|---|---|---|---|---|---|
| P1 | The board, one column at a time | `402:113` | `405:1804` | `405:4086` | `405:4986` | `TasksPage.tsx`, `Board.tsx` |
| P2 | A card's actions (long press) | `402:274` | `405:1828` | `405:4110` | `405:5010` | Board's drag, `RunPanel`'s Run, ChatSide's Move to menu |
| P3 | The menu with the host switcher | `402:491` | `405:1886` | `405:4168` | `405:5068` | `Sidebar.tsx`, `HostRail.tsx` (GA-317) |
| P4 | A card: Details | `403:553` | `405:1940` | `405:4222` | `405:5122` | `TaskPage.tsx` (Card tab) |
| P5 | A card: the Properties sheet | `403:679` | `405:1998` | `405:4280` | `405:5180` | `Properties.tsx` |
| P6 | A card: Runs | `403:860` | `405:2097` | `405:4379` | `405:5279` | `RunHistory.tsx` |
| P7 | A run's output, live, with Stop | `403:980` | `405:2161` | `405:4443` | `405:5343` | `RunPanel.tsx` (Show output) |
| P8 | A run's output, ended, with Continue | `403:1030` | `405:2188` | `405:4470` | `405:5370` | `RunPanel.tsx` |
| P9 | Chat: the threads | `404:775` | `405:2216` | `405:4498` | `405:5398` | `ChatPage.tsx`'s list |
| P10 | Chat: a thread with a call waiting for your OK | `404:869` | `405:2259` | `405:4541` | `405:5441` | `ChatThread.tsx`, `PendingActions.tsx`, `ChatEditor.tsx` |
| P11 | The Inbox, two hosts | `404:961` | `405:2296` | `405:4578` | `405:5478` | `TasksPage.tsx` (Inbox), `RunForMe.tsx` |
| P12 | A host that is off: its offline page | `404:1104` | `405:2357` | `405:4639` | `405:5539` | `HostOffline.tsx` (GA-317 C2) |
| P13 | The host went offline while you work: the band | `404:1201` | `405:2382` | `405:4664` | `405:5564` | GA-317 C4 |
| P14 | A place without a phone layout | `404:1368` | `405:2409` | `405:4691` | `405:5591` | new: `phone/OpenOnComputer.tsx` |
| S1 | Sign in | `405:1170` | `405:2424` | `405:4706` | `405:5606` | GA-317 W pages (card 11) |
| S2 | Sign in: the authenticator's code | `405:1210` | `405:2445` | `405:4727` | `405:5627` | GA-317 W3 (card 11) |
| S3 | Pair with a computer (none yet) | `405:1260` | `405:2466` | `405:4748` | `405:5648` | `AddComputer.tsx` (GA-317 C5) |
| S4 | Pairing: waiting for the computer, then paired | `405:1305` | `405:2496` | `405:4778` | `405:5678` | GA-317 B6/B7 seen from the device |
| S5 | Add to Home Screen (iPhone, Safari) | `405:1355` | `405:2522` | `405:4804` | `405:5704` | new (card 11) |
| S6 | Settings: allow notifications | `405:1428` | `405:2560` | `405:4842` | `405:5742` | `SettingsPage.tsx` → Notifications |
| S7 | Settings: notifications on | `405:1540` | `405:2597` | `405:4879` | `405:5779` | same |
| S8 | No connection | `405:1656` | `405:2637` | `405:4919` | `405:5819` | new (card 11's app shell) |
| N | The notifications on the lock screen | `405:1734` | `405:2651` | `405:4933` | `405:5833` | `notifications.rs`'s texts (card 8 seals them) |
| | Phone components (sheet) | `400:3` | | | | |

## New Figma parts

On the sheet **Phone components (GA-321)** (`400:3`):

| Component | Node | What it is | Built as |
|---|---|---|---|
| Lucide icons, 16 px, 1.75 at 24, `text-2` | `menu` `400:9`, `camera` `400:13`, `paperclip` `400:16`, `share` `400:21`, `wifi-off` `400:30`, `scan-line` `400:37`, `square-plus` `400:42`, `bell-ring` `400:48`, `bell-off` `400:54`, `ellipsis-vertical` `400:59`, `chevrons-up-down` `400:63`, `qr-code` `400:77`, `image-plus` `400:84` | the phone's new icons | `lucide-react` |
| `Phone/Status bar` | `400:85` | the phone's own status bar (not ours), 44 high | `env(safe-area-inset-top)` |
| `Phone/Home indicator` | `400:95` | the phone's home indicator (not ours), 34 high | `env(safe-area-inset-bottom)` |
| `Phone/Icon button` (Icon swap) | `400:97` | 44 × 44, a 16 px icon, no fill | `.icon-btn` at `--size-touch` |
| `Phone/Button` (Kind = Primary, Secondary, Danger, Ghost; Label, Icon, Show icon) | `400:118` | 44 high, radius 6, padding 0 16, `t-button` | `.btn` with `min-height: var(--size-touch)` |
| `Phone/Bottom bar` (Current = Board, Inbox, Chat, None × Host = Connected, Offline) | `401:257` | the bottom bar, 56 high | `phone/BottomBar.tsx` |
| `Phone/Column tab` (State = Default, Selected; Label, Count, Glyph) | `401:272` | a column as a tab | `phone/ColumnTabs.tsx` |
| `Phone/Host row` (State = Selected, Connected, Connecting, Offline; Name, Initials) | `401:309` | a host in the menu's switcher, 56 high | `phone/HostSwitcher.tsx` |
| `Phone/Nav row` (State = Default, Current; Label, Count, Icon, Show count) | `401:322` | a menu row, 44 high | `.nav-item` at touch size |
| `Phone/Board card` (State = Default, Live, On hold, Pressed; ID, Title) | `401:379` | a card at the column's width | the board card's CSS |
| `Phone/Sheet grabber` | `401:380` | the grabber of a bottom sheet | `phone/Sheet.tsx` |

Everything else is the file's own: `App/Badge`, `App/Avatar`, `App/Label pill`, `App/Checkbox`, `App/Input`, `App/Live tag`,
the Status glyphs, the Lucide icons of GA-284 to GA-317 (`monitor-off`, `loader-circle`, …), `Hersir mark`, the App/*
text styles and the GizAi variables. Shadows: `App/shadow-pop` (sheets), `App/shadow-drawer` (the menu).

## The tokens

Colours, type, radii and spacing are `design/tokens.json`'s, unchanged. One size token is new, for card 12 to add to
`tokens.json` and `src/styles/tokens.css`:

| Token | Value | Use |
|---|---|---|
| `size-touch` | `44px` | The least touch target on a phone: buttons, icon buttons, rows, tabs' hit areas, checkboxes' rows. (= `size-topbar`, by design.) |

The bottom bar's 56 and the sheet's top radius 14 (`radius-xl`) are existing values; the phone uses
`env(safe-area-inset-*)` for the status bar and the home indicator.

## Decisions in this design

1. **Under 600 px** (`@media (max-width: 599px)`) the phone layouts apply; at 600 and wider the PWA shows the desktop
   layouts, as in a computer's browser. Both widths drawn (375, 430) use the same layout: only the island and the full-width
   parts stretch.
2. **Islands stay, smaller.** The background (`canvas-from` → `canvas-to`, opaque: a browser has no glass) holds the top bar
   (44) and the bottom bar (56); the page is one island, `island-gap` (6) from the screen's sides, `island-radius` (10).
   The status bar and the home indicator are on the background too. So the phone reads as the same app, and the bars are
   where the desktop's top bar is: on the background, no fill, no border. Faint text and icons there are `text-2`.
3. **The sidebar becomes a menu and a bottom bar** (3.14). The **bottom bar** has Board, Inbox (its count: `needs` on
   `needs-soft` over `island`, as on the desktop), Chat and the **host chip**; the current one has its icon on
   `canvas-hover` (as the rail's selected tile) and its label in `text`. The **menu** (`menu` in the top bar, or the host
   chip) slides in from the left as a drawer: `float`, `line-2` border, `shadow-drawer`, radius 10, 6 from the screen's
   edges, `min(319px, 100vw − 56px)` wide, over the scrim. At its top **Your computers** is the host switcher (GA-317's
   rail as rows), then the sidebar's rows, the projects, and Settings at the foot.
4. **The host chip** is the host's initials in a 22 px mark (GA-317's tile, smaller), with a 6 px `success` dot while
   connected (GA-317 decision 2: connected is `success`, never teal); dashed `line-2` with `monitor-off` on an `island`
   disc while the host is off. It opens the menu at the switcher. With one host it stays: it says which computer you work on.
5. **The board is one column at a time** (3.14): the columns are tabs in the island's header (`Phone/Column tab`: glyph,
   name in `t-group`, count; selected on `selected`), scrolled sideways, the current one kept in view; swiping the list
   left or right goes to the next or previous column. Cards are full width. **No drag and drop on the phone:** a long press
   on a card (or its page's foot) opens its actions in a sheet (P2): **Open card**, **Run** (or **Continue** / **Stop**, as
   the card allows, `RunPanel`'s rules), and **Move to** with every column (the current one checked).
6. **A card is one column** (P4): head (glyph, column, project), the title in `t-drawer-title`, the working strip while an
   agent works on it; tabs **Details, Comments, Runs, Files** (the desktop's Card, Comments and Runs, and Attachments with
   Screenshots as **Files**; Changes and Activity are left to a computer: see P14); its foot has the card's two main
   actions. **Properties** opens a bottom sheet (P5); each row is 44 high, and a chevron marks what can be changed there.
7. **A run's output is full screen** (P7, P8): no bottom bar; its action bar has **Stop** (danger) while it runs, or
   **Continue** (primary) and **Comment** once it ended where it can go on. Back returns to the card.
8. **Approve and Discard are full width**, stacked, in the waiting call's box (P10): **Approve** primary, **Discard**
   secondary, each 44 high, so the thumb can't hit the wrong one by a hair. A thread is full screen; the text box sits above
   the keyboard (`visualViewport` keeps it there), with **camera**, **photos** and **files** (`paperclip`) and a 44 px
   round send button in `accent`.
9. **The Inbox lists every connected host's Inbox** (P11), each row with its host's chip (`monitor` and the name in
   `t-caption`, a `line-2` border) when you have two hosts or more; a host that is off gets one line saying so. The
   bottom bar's count is the sum.
10. **Run this for me on a phone** (P11) says where to run it: "It runs on PC: use a terminal there." **Done, continue**
    works from the phone (it is the host's own command).
11. **Gold only where you're needed**: the Inbox count, Waiting for your OK, holds, the offline band (P13). The live strip
    and the Live tag stay teal, only for an agent at work. Errors are `danger`; the time-limit stop is `warning`.
12. **The PWA's own screens are one island** with the Hersir mark and **Hersir** in the top bar, a title in
    `t-drawer-title`, the text in `t-small` `text-2`, fields 44 high (`App/Input`), and full-width buttons. Numbered steps
    are 24 px discs on `raised` with a `line-2` border.
13. **Notifications**: the PWA asks from a button in its Settings (S6), never by itself: iOS allows the question only from a
    tap, and only in a Home Screen app (3.14). On iPhone in Safari, S5 shows first.
14. **Places without a phone layout** (Usage, Team, Memory, the Editor, the Terminal, an agent's page, Settings' tabs other
    than the phone's own, a card's Changes and Activity) show P14, not a squeezed desktop page. Clients and Projects open as
    their plain lists.

## Screen by screen: what it adapts and what changes

### P1 The board (card 12: `TasksPage.tsx`, `Board.tsx`, `phone/ColumnTabs.tsx`, `phone/BottomBar.tsx`)

- **Top bar** (desktop `.topbar`): `menu` (opens P3), the page's name **Tasks** and its count, **Filters** (`list-filter`,
  the desktop's popover as a sheet) and **New task** (`square-pen`, the New task drawer full screen). The crumbs collapse
  to the page's name; Board or List, Sort and Group stay on a computer.
- **Island header** (the view toolbar): the column tabs. **What changes:** `.board` columns → one `.board-col` at a time,
  `width: 100%`; the column head becomes its tab.
- **Cards** (`Phone/Board card`): today's card at full width; the PR badge, screenshots sign, labels and avatar as today.
  Live: the `live` border and the working strip. On hold: the hold glyph and its name in `needs`.
- **Empty column**: today's **No tasks** in the island, centred.
- **Keys and drag**: none on the phone; a long press (500 ms) opens P2.

### P2 A card's actions (`phone/Sheet.tsx`, `phone/CardActions.tsx`)

A bottom sheet (`float`, `line-2`, top radius 14, `shadow-pop`, the grabber, 34 under it for the home indicator), over the
scrim: the card's glyph, column and id, its title in `t-section`; **Open card** (secondary) and **Run** (primary); **MOVE
TO** and the board's columns as 44 high rows, the current one with `check` in `accent`. Picking a column moves the card as
a drop does on the desktop (`moveTask`), and closes the sheet with the toast **GA-305 moved to Review** and **Undo**.

### P3 The menu (`phone/Menu.tsx`, `phone/HostSwitcher.tsx`, `Sidebar.tsx`'s items)

- **YOUR COMPUTERS**: a `Phone/Host row` per host: the selected one on `selected` with **Connected · you work on PC now**;
  others **Connected** with their Inbox count, **Connecting…** with `loader-circle`, **Offline since 09:12** with
  `monitor-off` (dashed mark, `text-3` name). Tapping a host switches the whole app to it (`hostSelect`), as the rail does.
  **Pair with a computer** opens S3.
- Then **Search** (the palette, full screen), **Inbox**, **Chat**, **Tasks**, **Clients**, **Projects**, **PROJECTS** with
  their dots (a project opens its board), and **Settings** at the foot. Rows are `Phone/Nav row`, 44 high.
- **What changes:** the sidebar island (248) and the rail (44) are not drawn under 600 px; their items move here.

### P4 A card: Details (`TaskPage.tsx`, `phone/CardTabs.tsx`)

- **Top bar**: back, the card's id (`t-id`), **More** (`ellipsis-vertical`: Copy ID, Archive, Open in a browser tab).
- **Head**: glyph, column, project; the title (`t-drawer-title`, wraps); the working strip (`live-soft`: **Design Agent is
  working · Run 3 · 12 min**).
- **Tabs**: Details, Comments 4, Runs 3, Files 2 (island tabs at touch size: a 32 high pill in a 44 hit area).
- **Details**: Description (`t-prose`), Acceptance criteria with **1 of 3** and 44 high checkbox rows (a tap on the row ticks it), **Properties**.
- **Foot** (inside the island, a `line` above): **Comment** and the run action: **Show output** while live, **Run** when the
  card can run, **Continue** when its last run can go on; in Review: **Comment** and **Move to**.
- **What changes:** `.task-page` two islands → one; Properties island → sheet (P5); Card/Changes/Screenshots island tabs →
  Details/Comments/Runs/Files; the doc's padding `22 36 64` → `14 16 16`.

### P5 Properties (`Properties.tsx` in `phone/Sheet.tsx`)

Rows of 44: **Column**, **Assignee**, **Labels** (chevron: tap to change, with today's pickers as sheets), **Project**,
**Builds on**, **Branch**, **Created**; labels 96 wide in `t-small` `text-3`, values `t-small` `text`, cut with an ellipsis.
A hold adds **Hold** with its badge, reason and **Clear hold**, as on the desktop.

### P6 Runs (`RunHistory.tsx`)

Each run a `raised` box (radius 10, `line`; the live one `live`): **Run 3**, the agent, the outcome (`App/Live tag`, or
`App/Badge` **Done** / **Failed**), the summary's first line, when and how long. A tap opens P7 or P8.

### P7, P8 A run's output (`RunPanel.tsx` output, full screen)

The run's head (the agent's avatar, **Design Agent · Opus · since 14:02**, the live dot and **Working…**), then the
transcript in `t-code` (tool lines `text-3`, the agent's words `text`), scrolling, following the end while live. The action
bar on the background: **Stop** (`Phone/Button` Danger, `square`) → **Stopping…**; ended: the end line (here the limit, on
`warning-soft` with `triangle-alert`), **Comment** and **Continue** (`step-forward`) → **Continuing…**.

### P9, P10 Chat (`ChatPage.tsx`, `ChatThread.tsx`, `PendingActions.tsx`, `ChatEditor.tsx`)

- **Threads**: **PINNED** and **RECENT**, each row the title (`t-row`), the time, **Waiting for your OK** (`App/Badge`
  Needs) when calls wait, and the last line (`t-small` `text-2`, one line). **New chat** in the top bar.
- **A thread**: full screen, back to the list; your messages in a `raised` bubble on the right, the Chief's answers as
  prose under `crown` **Chief** and the time. **A waiting call**: a `raised` box with **Waiting for your OK**, today's
  sentence, the call in `t-code` on `side`, then **Approve** and **Discard** full width. Several calls: one box each, and
  **Approve all** over them as today.
- **The text box**: `raised`, `line-2`, radius 10, **Message the Chief…** (**Queue a message for when this answer is
  done…** while it answers); under it **camera** (`capture`), **photos** and **files**, and send. Files go up first
  (card 11's client) and show as today's chips.
- **What changes:** the chat list island and the side island are not drawn; the side island's tabs open as their own
  pages (a card opens P4).

### P11 The Inbox (`TasksPage.tsx`'s Inbox, `RunForMe.tsx`)

**FROM THE CHIEF**, **RUN THIS FOR ME** and **NEEDS YOU** (the desktop's sections and rows), padding `10 12`; each row with
its host chip when there are two hosts or more. Run this for me keeps its 3 px `needs` bar, the command on `side` with
**Copy**, and **Done, continue** full width. A host that is off: one `side` line with `monitor-off`. Empty: **Nothing needs
you.**

### P12, P13 A host that is off (GA-317's `HostOffline.tsx` and band)

- **P12**: GA-317's offline page in the island: `monitor-off` 20, **MacBook is offline** (`t-drawer-title`), 3.11's text,
  the projects as chips (the opened one on `selected`), **Check again** full width, **Tell me when MacBook is back** as a
  44 high checkbox row. The host chip is the offline one. The page's other states (**Connecting to PC…**, **Remote use is
  off on PC**, **PC runs an older Hersir**, **This device was removed from PC**, **Hersir Cloud's plan ended**) use the
  same layout with GA-317 C3's icons and texts.
- **P13**: the band at the island's top (`needs-soft` over `island`, a 1 px `needs` border, radius 6, `monitor-off` in
  `needs`), the page under it at 50 %, every write disabled (title **Disabled while PC is offline**).

### P14 Open this on a computer (`phone/OpenOnComputer.tsx`, new)

`monitor` 20 in `text-3`, **Open this on a computer** (`t-section`), the line, **Back to the board**. The bottom bar shows
no current place.

### S1, S2 Sign in (card 11: Passport with PKCE at `cloud.hersir.dev`, styled as GA-317's W pages)

Email and password, **Sign in** (`log-in`), then the authenticator's code, **Continue**, **Use a recovery code instead**.
The code's error line is drawn under S2's buttons (`circle-alert` and the text in `danger`); it shows only after a wrong
code. Signing in happens in the PWA's own tab (no browser switch on a phone).

### S3, S4 Pairing (card 11: `src/web/` pairing; 3.13)

- **S3** (no computers yet, and **Pair with a computer** in the menu): three steps and the code field
  (`HP-____-____-____-____-____-__`, 44 high, the hint), **Pair**.
- **S4**: opened by the QR link or after **Pair**: the computer's name in the title, this phone's name (editable, sent with
  the join), **Waiting for PC…** with `loader-circle`, **Cancel**; then **Paired with PC** with `circle-check` in
  `success` and **Open PC's board**. Refused: the sentences of 5.8 under the field in `danger`.

### S5 Add to Home Screen (card 11)

Shown in Safari on iPhone when the PWA isn't on the Home Screen (`navigator.standalone` false), once after pairing and from
Settings' **Allow notifications**: three steps with `share`, `square-plus` and `bell-ring`, **Not now**. Safari's own
toolbar is drawn under it (not ours) with its Share button ringed in `accent`.

### S6, S7 Settings: notifications (card 11; the desktop's Settings → Notifications, per device)

**NOTIFICATIONS**: the line, **Allow notifications** (asks the browser from this tap); then the kinds, each a 44 high
checkbox row, disabled (45 %) until allowed; on: **Notifications are on for this phone.** and **Send a test
notification**. Blocked by the browser: the button is replaced by the line in the texts below. **THIS PHONE**: its name,
the computers it is paired with, the account; **Pair with a computer**, **Sign out**. The kinds' switches are this
device's (`remote_devices.push_on` per kind on each host).

### S8 No connection (card 11's app shell)

`wifi-off` 20, **No connection**, the line, **Try again**. The host chip shows the offline mark: no host can be reached.

### N The notifications (card 8 sends them sealed, card 11's service worker shows them)

Drawn as the phone shows them on the lock screen: **HERSIR**, when, the title in bold and the body. A tap opens the
`url`: the card (P4) or the chat (P10), on that host.

## Every text, word for word

Sample data (names, cards, times, commands, emails) is not text to build. **PC** and **MacBook** stand for a host's name.

**The bars and the menu**

- Bottom bar: **Board**, **Inbox**, **Chat**, the host's name.
- Menu: **YOUR COMPUTERS**, **Connected · you work on PC now**, **Connected**, **Connecting…**, **Offline since 09:12**,
  **Pair with a computer**, **Search**, **Inbox**, **Chat**, **Tasks**, **Clients**, **Projects**, **PROJECTS**,
  **Settings**.
- Top bars: **Tasks**, **Chat**, **Inbox**, **Settings**, the card's id, **Run 3**; tooltips and labels for screen
  readers: **Open the menu**, **Back**, **Filters**, **New task**, **New chat**, **More**.

**The board and a card**

- A card's actions: **Open card**, **Run** / **Continue** / **Stop**, **MOVE TO**; after a move: **GA-305 moved to
  Review** and **Undo**.
- The working strip: **Design Agent is working · Run 3 · 12 min**; on a board card: **Design Agent is working · Run 3**;
  a hold: **On hold:** *the hold's name* (`HOLD_NAMES`, lower case after the colon).
- Card tabs: **Details**, **Comments**, **Runs**, **Files**; **DESCRIPTION**, **ACCEPTANCE CRITERIA** *1 of 3*,
  **Properties**; the foot: **Comment**, **Show output**, **Run**, **Continue**, **Move to**.
- Properties: **Properties**, **Column**, **Assignee**, **Labels**, **Project**, **Builds on**, **Branch**, **Created**
  (*20 Nov* **· by** *the Chief*), **Hold**, **Clear hold**.
- Runs: **Run** *3*, the agent, **Done** / **Failed** / the Live tag, *Today 14:02* **·** *12 min*.
- A run's output: *agent* **·** *model* **· since** *14:02* (ended: *14:02* **to** *14:44*), **Working…**, **Stop**,
  **Stopping…**, **Comment**, **Continue**, **Continuing…**; the end lines are today's (**Stopped at the time or
  tool-call limit** and the rest) followed by **Continue goes on where it stopped.**

**Chat and the Inbox**

- **PINNED**, **RECENT**, **Waiting for your OK**, **Message the Chief…**, **Queue a message for when this answer is
  done…**, **Approve**, **Approving…**, **Discard**, **Discarding…**, **Approve all**; the box's sentence as today
  (**Hersir didn't run this: it gives an agent commands the Chief may only add with your OK.**). Labels: **Take a photo**,
  **Add photos**, **Add files**, **Send**.
- Inbox: **FROM THE CHIEF**, **RUN THIS FOR ME**, **NEEDS YOU**; **The Chief waits for your OK** / *chat* **·** *1 action*;
  **Run what the agent asks for below, then press Done, continue. It runs on PC: use a terminal there.**, **Copy**,
  **Copied**, **Done, continue**; **MacBook is offline: what waits for you there shows when it is back.**; empty:
  **Nothing needs you.**

**Hosts**

- The offline page, its other states, the band and the write's refusal: GA-317's texts, word for word (3.11), with the
  host's name.
- **Open this on a computer** / *Usage* **needs a wider screen than a phone's. Open it in Hersir on PC, or at app.hersir.dev
  in a computer's browser.** / **Back to the board**

**The PWA's own screens**

- Sign in: **Sign in to Hersir Cloud**, **See and work on your computers' projects from this phone: their boards, cards,
  chats and Inbox. Your computers do the work.**, **Email address**, **Password**, **Sign in**, **No account yet? Create one
  at cloud.hersir.dev, with Personal: Cloud Backup and Hersir on your other devices.**; wrong password: **That email and
  password don't match. Check both, or reset your password at cloud.hersir.dev.**
- The code: **Your authenticator's code**, **Type the 6-digit code your authenticator app shows for** *email***.**, **Code
  from your app**, **Continue**, **Use a recovery code instead** (then **Recovery code** and **Continue**), wrong: **That
  code didn't work. Codes change every 30 seconds: type the new one.**
- Pair: **Pair with a computer**, **Hersir on your phone works with the computers that hold your projects. Pair each one
  once; nothing is copied to this phone.**, **On the computer, open Hersir: Settings → Hersir Cloud → Use from other devices
  → Add a device.**, **Scan the code it shows with your phone's camera. The link opens Hersir here.**, **Or type the code
  it shows:**, **Code**, **It works for 5 minutes after the computer shows it.**, **Pair**. (In the app the arrows are
  Lucide's `arrow-right`, as `design/README.md` says; the frames show ›.)
- Waiting: **Pair with PC**, **Accept this phone on PC: it asks 'Add iPhone (Safari) to PC?' and what it may do.**, **This
  phone's name**, **PC and your account page show this name.**, **Waiting for PC… The code works until** *14:07***.**,
  **Cancel**; then **Paired with PC**, **Open PC's board**; refused: **That code has expired: show a new one on PC.**, **PC
  didn't add this phone.**
- Add to Home Screen: **Add Hersir to your Home Screen**, **On iPhone, Hersir can notify you only when it opens from your
  Home Screen. It takes three taps.**, **Tap Share in Safari's toolbar.**, **Choose Add to Home Screen, then Add.**, **Open
  Hersir from your Home Screen, then allow notifications in its Settings.**, **Not now**
- Settings: **NOTIFICATIONS**, **Hersir tells you when something needs you on PC or MacBook, also while this app is closed.
  Each computer sends its own, sealed so only this phone can read them.**, **Allow notifications**, **Notifications are on
  for this phone.**, the kinds **A card goes on hold**, **A card waits for your review**, **The Chief asks you something**,
  **The Chief answered while Hersir was closed**, **A call waits for your OK**, **A computer is back online**, **Send a test
  notification** (its push: **Hersir: notifications work on this phone.**); blocked: **Notifications are blocked for
  Hersir. Turn them on in your phone's settings: Notifications → Hersir.**; **THIS PHONE**, *name* **· paired with**
  *PC and MacBook* **· signed in as** *email*, **Pair with a computer**, **Sign out** (asks: **Sign out of Hersir Cloud on
  this phone? You pair it again with each computer after you sign in.**, **Cancel**, **Sign out**)
- No connection: **No connection**, **This phone is offline, so Hersir can't reach PC or MacBook. It tries again as soon as
  the phone is back online.**, **Try again**

**The notifications, one per kind** (title, then body; the host's own texts from `notifications.rs`, with the host's name
first so you know which computer; at most 2 KB sealed, as 5.8 says)

| Kind (`kind`) | Title | Body | Opens |
|---|---|---|---|
| `hold` | **PC: GA-301 is on hold:** *the reason, or the hold's name* | *the card's title* | `#/task/<id>` |
| `waiting` | **PC: GA-305 waits for your review** (Deploy: **…is merged and waits for deploy**; the Chief's merge: **PC: The Chief merged GA-12's pull request #231**) | *the card's title* | `#/task/<id>` |
| `leadAsks` | **PC: The Chief asks:** *chat* | **Question about** *GA-321* / **Approval for** *GA-321* | `#/chat/<id>` |
| `leadAnswered` | **PC: The Chief answered:** *chat* (failed: **PC: The Chief couldn't answer:** *chat*) | *the answer's start, 160 characters* (failed: why) | `#/chat/<id>` |
| `approval` | **PC: The Chief waits for your OK:** *chat* | **1 action to approve or discard** / *n* **actions to approve or discard** | `#/chat/<id>` |
| a push it can't open | **Hersir: something on PC needs you** | (none) | the app |
| *Tell me when PC is back* (3.11) | **PC is back** | **Its projects open again.** | the host |

## Open points for Jefsev (none blocks building)

1. **The host's name first in every push title** (*PC: …*): it tells two computers apart on the lock screen, even with one
   paired now. Without it, the titles are the desktop's exactly.
2. **The phone's Inbox shows every connected host's Inbox at once** (3.14 says "with the host's name on each row when there
   are several hosts"), while the rest of the app shows the selected host. If Jefsev wants the Inbox per host too, the
   rows lose their chips and the count follows the host chip.
3. **Changes and Activity on a card** are left to a computer (P14), to keep the card's tabs at four; Changes needs wide
   diffs. A later card could add a read-only list of changed files.
4. **Connected stays `success`**, as GA-317 decided (its open point 1).

## Not in this card

- Building the layouts (card 12) and the PWA, its client, install and push (card 11); the host's push sending (card 8).
- Phone layouts for Usage, Team, Memory, Clients' and Projects' pages, the Editor and the Terminal (P14 covers them).
- Tablets: 600 px and wider get the desktop layouts.
