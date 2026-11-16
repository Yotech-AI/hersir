# Hersir in islands (1 of 4): the look, the window and a sidebar that closes (GA-284)

The design is in Figma, in the file **Giz-AI**, on the page **Hersir Islands 1: look and window** (node `268:2`):
https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=268-2

These are the notes for the builders. Nothing in the app changes in this card. GA-287 builds the chip's hover card; the
rest is built after cards 2 to 4 (GA-285, GA-286 and GA-288), which draw the rest of the app on this look. What a page
holds and does stays as it is: only how it is laid out and drawn changes.

## Round 3 (Jefsev's review of 16 Nov)

1. **The islands are `#181a1c`** in dark, through the variable `island`, which now has its own value (`#ffffff` in light).
   `bg` follows it (`bg` = `island`), so code, terminals and everything drawn on bg sit on the island.
2. **The background is lighter** than round 2's in dark: `canvas-from` `#26282e`, `canvas-to` `#222b31`, `canvas-glow`
   `#2b3946`: a step of 1.18 from the islands, as round 2's step was from bg.
3. **The background is glass**: the gradient, partly transparent (92% in dark, 86% in light), over the desktop, which the
   system blurs. One shared component, `Islands/Window background`, holds a wallpaper (a stand-in for the desktop) and the
   glass; every frame uses it. The look frames show the glass over a dark and a light wallpaper in both themes. See
   [The glass](#the-glass).
4. **Light** has the same glass; the islands stay white, the background a soft grey a step below them.
5. **What sits on an island steps up from `#181a1c`**: raised, hover, selected, float and the lines are lifted, inputs move
   from bg to raised, and text-3 is lifted so it keeps AA on the island, on raised and on float. See
   [What changed in the GizAi variables](#what-changed-in-the-gizai-variables).
6. **On the background**, the Chief's chip and the Inbox count get `island` beneath them, so their coloured text keeps AA
   on the glass over any wallpaper; `canvas-hover` is translucent, so it shows on any glass.
7. Kept from rounds 1 and 2: no borders on islands, focus without a border, Ctrl+\\ (Cmd+\\ on macOS), the closed
   sidebar's toolbar, the chip's hover card, `island-radius` 10 and `island-gap` 6.

Every frame is redrawn: each window frame now has `Islands/Window background` as its first layer (locked) in place of
round 2's gradient fill and glow ellipse, the sample panels in the look and toolbar frames are glass too, and every
colour follows the new variable values.

## The frames

Open a frame with `https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=<id with - for :>`.

| Frame | Node | Size |
|---|---|---|
| Look · Dark (a sheet, not a screen) | `275:1972` | 1440 × 3264 |
| Look · Light | `305:3659` (new in round 3: round 2's `288:2657` is gone) | 1440 × 3264 |
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

In Look · Dark: The glass `304:3257` (dark glass over the dark wallpaper `304:3263`, over the light wallpaper `304:3378`,
the two wallpapers `304:3492`), The gradient: the glass's tint `275:1977`, An island `275:1990`, Focus `275:1997`, Which
surfaces change `275:2160`, What floats `288:2545`, A tooltip `288:2621`, the token table `275:2184`. Look · Light has the
same layout, with light glass over the same two wallpapers.

The light frames are the dark ones with the GizAi colour collection set to Light on the frame: every colour is a variable,
so a frame switches theme as a whole. Dark frames use the dark wallpaper, light frames the light one.

The frames moved down on the page (y + 1600) to make room for the taller look sheets.

### Components (on the sheet `268:3`)

| Component | Node | Built on |
|---|---|---|
| `Islands/Window background` (new: Wallpaper=Dark `301:3143`, Light `301:3152`; boolean **Glass**, on) | `301:3161` | `wallpaper/*`, `canvas/glass-*`, `island/glass-blur` |
| `Islands/Island` | `269:116` | Island header |
| `Islands/Island header` | `269:67` | Island tab, App/Icon button |
| `Islands/Island tab` (State=Default `269:34`, Hover `269:44`, Selected `269:54`) | `269:64` | Icons, App/* text styles |
| `Islands/Tool button` (State=Default `269:5`, Hover `269:12`, Focus `269:19`) | `269:26` | Icons, the nav count (on `island`) |
| `Islands/Sidebar toolbar` (the closed sidebar's toolbar) | `269:259` | 4 × Tool button (exposed) |
| `Islands/Tooltip` | `269:29` | `float`, App/shadow-pop |
| `Islands/Resize gap` (Orientation × State: Rest, Hover, Dragging, Focus) | `269:209` | |
| `Islands/Scrollbar` (State=Rest `269:212`, Hover `269:214`) | `269:216` | |
| `Islands/Hover card` (Kind=Card `291:2536`, Project `291:2564`, Agent `291:2582`, Client `291:2601`, Person `291:2611`, Doc `291:2624`, Not found `291:2635`; Card has the boolean On hold) | `291:2641` | App/Avatar, Status glyphs, App/shadow-pop |
| New Lucide icons: `panel-left-close` `268:9`, `panel-left-open` `268:14`, `panel-left` `268:18`, `maximize-2` `268:24`, `git-branch` `268:30`, `pin` `268:34`, `chevron-up` `268:37` | `268:4` | 16 px, 1.75 stroke at 24, text-2 |

**`Islands/Window background`** (for cards 2 to 4 too): put an instance first in a window frame, absolute, at 0,0,
stretched to the frame (constraints Stretch), locked, and leave the frame's own fill empty. Wallpaper=Dark in a dark
frame, Light in a light frame. Its layers: `Wallpaper` (a base and five blurred shapes, layer blur 140), `Glass` (a
linear gradient straight down, `glass-from` → `glass-to`, with a background blur bound to `island/glass-blur`, 40) and
`Glow` (an ellipse in `glass-glow`, layer blur 240, from the top left). Turn **Glass** off to show the wallpaper alone.

The page frames use App/Nav item, App/Nav label, App/Button, App/Icon button, App/Kbd, App/Avatar, App/Live tag,
App/Board card and the Status glyphs, and these components. Islands in the page frames are frames drawn exactly as
`Islands/Island` (an instance can't hold a page's own content); their names start with `Island ·`.

## What changed in the GizAi variables

Round 3 (old → new; light unchanged unless named):

| Variable | Dark | Light |
|---|---|---|
| `island/island` | `= bg` (`#0f1013` in tokens.json) → **`#181a1c`**, its own value | `= bg` → `#ffffff`, its own value |
| `surface/bg` | `#0f1013` (tokens.json) → **`= island`** | `#ffffff` → `= island` (the same white) |
| `canvas/canvas-from` | `#1c1e23` → `#26282e` | `#ebedf1` (kept) |
| `canvas/canvas-to` | `#182126` → `#222b31` | `#e2ecf1` (kept) |
| `canvas/canvas-glow` | `#202e3b` → `#2b3946` | `#dce9f4` (kept) |
| `canvas/canvas-hover` | `#282a30` → `rgba(255,255,255,.07)` | `#dddfe6` → `rgba(20,22,27,.07)` |
| `surface/float` | `#191b20` → `#22242a` | `#ffffff` (kept) |
| `surface/overlay` | `#16181d` → `= float` | `#ffffff` → `= float` (the same white) |
| `surface/raised` | `#15171b` → `#1f2125` | `#ffffff` (kept) |
| `surface/hover` | `#1b1d22` → `#292b30` | `#f0f2f5` (kept) |
| `surface/selected` | `#22252c` → `#2e3138` | `#e6e9f0` (kept) |
| `line/line` | `#23262c` → `#31343b` | `#e3e5ea` (kept) |
| `line/line-2` | `#30333a` → `#3f434a` | `#d0d3da` (kept) |
| `surface/side` | `#0b0c0f` → `#141519` | `#f3f4f7` (kept) |
| `text/text-3` | `#7f838d` → `#8a8e98` | `#636772` (kept) |
| `diff/diff-fold` | `#16181e` → `#202228` | `#f1f3f8` (kept) |
| `terminal/term-selection` | `#283350` → `#374260` | `#dae1f9` (kept) |
| `island/scroll-thumb` | `#33353a` → `#414348` | `#caced3` (kept) |
| `island/scroll-thumb-hover` | `#585b61` → `#696c72` | `#9b9fa5` (kept) |

New in round 3: `canvas/glass-from`, `canvas/glass-to`, `canvas/glass-glow` (the glass), `wallpaper/wp-dark-base`,
`wp-dark-1` to `wp-dark-5`, `wp-light-base`, `wp-light-1` to `wp-light-5` (Figma only: the stand-in wallpapers, hidden
from publishing, the same in both modes, not tokens) and, in GizAi size, `island/glass-blur` (40).

Each changed variable's description in Figma names its old value. The steps are measured in OKLCH lightness from the
island (L 0.216): raised +0.031 (as round 2's from bg), float +0.045, hover +0.073, selected +0.097, line +0.109, line-2
+0.166, side −0.020. Hover and the lines step a little more than before, so a hover on float and a border on raised show.

Also in round 3, the components whose fill was bg now use **raised**, so an input steps up from the island: App/Input,
App/Select, App/Checkbox, App/Agent chip (page App: Onboarding), App/Kbd (`136:272`) and App/Seg (`160:108`, page App:
Screenshots); and in the frames the board's Search tasks and Board or List, and the Editor's Files or search.

Round 2 (still true): `island-line` and `island-line-focus` are gone with every stroke that used them; `canvas-glow`,
`canvas-hover` and `float` are new.

Round 1 (still true): the old pink and amber are gone (`status/needs` `#f2a53a` / `#9a5800` and the rest); every colour of
`design/tokens.json` is a variable in Dark and Light (`glyph/pr-urgent`, `diff/*`, `syntax/*`, `git/*`, `terminal/*`,
`band/band-subtle-*`), the ones that follow another as aliases; text styles `App/t-count`, `App/t-avatar` and
`App/t-avatar-sm`; `shadow/shadow-pop` is a variable.

## The tokens

For `design/tokens.json` and `src/styles/tokens.css` when this is built. The colours are in **GizAi colour**, the sizes in
**GizAi size**. Every token the islands add or change:

| Token | Dark | Light | Use |
|---|---|---|---|
| `island` | `#181a1c` | `#ffffff` | Every island's fill. No border, no shadow. |
| `bg` | `var(--island)` (was `#0f1013`) | `var(--island)` | What was bg sits on the island: code (`code.ts`), terminals (`term-bg`, `term-cursor-text`), the main column, what is cut out of a glyph or an avatar. |
| `canvas-from` | `#26282e` | `#ebedf1` | The gradient at the window's top: the glass's tint, and with no glass the background itself. |
| `canvas-to` | `#222b31` | `#e2ecf1` | The gradient at the window's bottom edge (the light of the mountains, hue 237°). |
| `canvas-glow` | `#2b3946` | `#dce9f4` | The glow from the top left, fading to transparent: Hersir's cool blue, low in chroma. |
| `glass-from` | `rgba(38, 40, 46, 0.92)` | `rgba(235, 237, 241, 0.86)` | `canvas-from` as glass. |
| `glass-to` | `rgba(34, 43, 49, 0.92)` | `rgba(226, 236, 241, 0.86)` | `canvas-to` as glass. |
| `glass-glow` | `rgba(43, 57, 70, 0.92)` | `rgba(220, 233, 244, 0.86)` | `canvas-glow` as glass. |
| `canvas-hover` | `rgba(255, 255, 255, 0.07)` | `rgba(20, 22, 27, 0.07)` | Hover fill of a button on the background (the closed sidebar's toolbar, the top bars' buttons). |
| `float` | `#22242a` | `#ffffff` | What floats over the islands: menus, popovers, the palette, tooltips, drawers, toasts, the chip's hover card. Takes `overlay`'s place. |
| `raised` | `#1f2125` | `#ffffff` | Cards, panels, the text box; and now inputs (`.input`, `.select`, `.textarea`), `.seg`, `.kbd`, `.md-editor`, `.ed-goto`, `.agent-chip`. |
| `hover` | `#292b30` | `#f0f2f5` | Hover fill on an island, and of a row on `float`. |
| `selected` | `#2e3138` | `#e6e9f0` | A selected row or tab. |
| `line` | `#31343b` | `#e3e5ea` | Lines inside an island; a card's border. |
| `line-2` | `#3f434a` | `#d0d3da` | The border of inputs, buttons and what floats. |
| `side` | `#141519` | `#f3f4f7` | A recess inside an island: the board's rails, code blocks. No longer a column. |
| `text-3` | `#8a8e98` | `#636772` | Faint text. |
| `diff-fold` | `#202228` | `#f1f3f8` | A diff's folded lines. |
| `term-selection` | `#374260` | `#dae1f9` | A terminal's selection. |
| `scroll-thumb` | `#414348` | `#caced3` | A scrollbar's thumb at rest. |
| `scroll-thumb-hover` | `#696c72` | `#9b9fa5` | The thumb under the pointer and while dragged. |
| `island-radius` | `10px` (= `radius-l`) | | An island's corners. |
| `island-gap` | `6px` (= `size-handle`) | | Between islands, and between an island and the window's edge. |
| `size-island-head` | `40px` | | An island's header. |
| `size-island-tab` | `28px` | | A tab in an island's header. |
| `size-topbar` | `44px` (was `52px`) | | The top bar, on the background with no border. |
| `size-tool` | `28px` | | A button of the closed sidebar's toolbar, and Close sidebar. |
| `size-scroll` | `6px` | | A scrollbar's thumb. |

`glass-blur` (40) is Figma's only: in the app the system blurs. `overlay` goes when this is built: every use becomes
`float`. `island-line` and `island-line-focus` (round 1) are not tokens. `term-bright-black` keeps `#7f838d` (a terminal
colour, not text-3).

### Contrast (WCAG 2)

- **On the island `#181a1c`** (dark): text 14.94, text-2 7.34, **text-3 5.32**, accent 6.27, live 9.35, needs 8.49,
  danger 6.07. On raised: text-3 4.92. On float: text-3 4.73, text-2 6.53, live 8.31. On side: text-3 5.56. On a hovered
  or selected row text-3 is 4.32 / 3.97 (today 4.04 on selected): a row's faint text there is a hint, as today.
  Light is unchanged: on white text-3 5.65.
- **Steps up from the island** (dark): raised 1.08, float 1.13, hover 1.23, selected 1.34, line 1.40, line-2 1.76; hover
  on float 1.10. The background (opaque) 1.18, the glow 1.48.
- **The glass, the worst case.** Dark glass over white (the lightest a wallpaper gets): text-2 4.86 at the top, 4.73 at
  the bottom, 4.97 in the glow; text 9.88; the focus ring 5.14; text-3 3.04 (2.96 at the bottom), so on the background faint text
  and icons are text-2. Light glass over black: text-2 5.09, 4.97, 6.40 in the glow; text 11.25; the focus ring 3.58; text-3 3.51 (3.43 at
  the bottom), so text-2 there too. The Chief's chip ('working' in live) and the Inbox count (needs on needs-soft) sit on `island`: 9.35
  and 6.80 in dark, 5.01 and 5.00 in light, over any wallpaper. Without that, light's live would be 3.11.
- **The islands keep their edge on any wallpaper**: dark glass over black 1.14 from the islands (over white 1.51); light
  glass over white 1.14 from the white islands.
- **The opaque fallback**: dark text-2 6.20 (4.97 in the glow); light text-2 6.99 (6.64 in the glow).

## The look

### The glass

The window is glass, like Apple's frosted glass: the background is the gradient, partly transparent, over the desktop,
which the system blurs and Hersir tints. The islands stay solid.

- **Opacity**: the tint is **92% in dark** and **86% in light** (`glass-from`, `glass-to`, `glass-glow`). That is the
  least that keeps the top bars and the closed sidebar's toolbar at AA in the worst case (dark glass over a white
  wallpaper, light glass over a black one; [Contrast](#contrast-wcag-2)). So the desktop shows as a soft wash of its
  colours: clearest where a wallpaper is far from the theme (a light wallpaper behind dark Hersir).
- **Blur**: Figma draws it with a background blur of **40** (`island/glass-blur`) in both themes. In the app the blur is
  the system's own (below); Hersir sets no blur in CSS: `backdrop-filter` can't reach the desktop behind a window.
- In Figma, `Islands/Window background` holds the wallpaper and the glass; the wallpapers are stand-ins for the desktop
  (abstract blurred shapes, `wallpaper/*`).

```css
/* The window is transparent; the system blurs what is behind it. */
html.glass, html.glass body { background: transparent; }
.app {
  background:
    radial-gradient(ellipse 60% 75% at 0 0, var(--canvas-glow) 45%, transparent),
    linear-gradient(180deg, var(--canvas-from), var(--canvas-to));
}
html.glass .app {
  background:
    radial-gradient(ellipse 60% 75% at 0 0, var(--glass-glow) 45%, transparent),
    linear-gradient(180deg, var(--glass-from), var(--glass-to));
}
```

#### How it is built (a proposal)

- **The window is transparent**: `"transparent": true` on the main window in `src-tauri/tauri.conf.json` (it is set when
  the window is made), with `"macOSPrivateApi": true` under `app` for macOS. The page's own background is transparent
  while the glass is on (`html.glass`), and the gradient above paints the tint.
- **macOS: vibrancy.** `Window::set_effects` (Tauri 2's window effects) with the material `underWindowBackground`, state
  `followsWindowActiveState`. The tint at 92% / 86% goes over it.
- **Windows 11: Mica** (`Effect::Mica`, dark or light with the theme). Mica shows the desktop's wallpaper, blurred, which
  is the look drawn; it costs little and is Microsoft's choice for a long-lived window. **Acrylic** (`Effect::Acrylic`)
  also shows the windows behind, but costs more and lags while a window is dragged on some machines: not the default. On
  Windows 10 there is no Mica: the fallback.
- **Linux: the compositor's blur.** Tauri's window effects do nothing on Linux; a transparent window is blurred by the
  compositor when it does that. **Hyprland** blurs transparent windows (`decoration:blur:enabled`, on by default), and is
  told apart by `HYPRLAND_INSTANCE_SIGNATURE`. KDE's KWin blurs only a window that asks for it, and GNOME doesn't blur:
  there, the fallback.
- **The fallback is the same gradient, opaque** (`canvas-from`, `canvas-to`, `canvas-glow`, as in round 2), with no
  `html.glass` and the window effects cleared (`set_effects(None)`). Hersir uses it:
  - when the system gives no blur (Windows 10, Linux other than Hyprland, a macOS or Windows that won't apply the effect);
  - when **reduce transparency** is on: macOS's Accessibility → Display → Reduce transparency
    (`NSWorkspace.accessibilityDisplayShouldReduceTransparency`), Windows' Personalisation → Colours → Transparency
    effects off (`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize\EnableTransparency` = 0), and
    `@media (prefers-reduced-transparency: reduce)` where the webview knows it. Read at start and when the system says it
    changed (a window focus is enough);
  - when the user turns the glass off in Settings (below).
- The look in the fallback is round 2's: nothing else changes (the islands, the top bars, the toolbar). Text on the
  background keeps AA there too ([Contrast](#contrast-wcag-2)).

#### The switch in Settings → Appearance (a proposal)

It takes the place of the mountains' checkbox (the mountains go, round 1), in the same section:

- Section **Background**, its text: **Behind the islands: a soft gradient, and glass where your system blurs windows.**
- Field **Glass**, a checkbox: **Show the desktop through the background**
- Its hint: **Blurred and tinted, on macOS, Windows 11 and Hyprland. Off gives a plain background.**
- When the system gives no blur, the checkbox is off and disabled, with the hint: **Your system doesn't blur windows
  behind Hersir, so the background stays plain.**
- While reduce transparency is on, the checkbox keeps its value, with the hint: **Plain while your system reduces
  transparency.**
- **Default: on** where the system blurs (macOS, Windows 11, Hyprland); there is nothing to turn on elsewhere. Kept on
  this computer with the other appearance settings (`glass`, as `band` was); Reset to defaults turns it on again.

### On the background

The top bars, the closed sidebar's toolbar and the gaps sit on the background, which is glass:

- **Faint text and icons are `text-2`** on the background (a top bar's count, `.crumbs .sep`, `.topbar .faint`, the
  toolbar's icons): text-3 drops under 3:1 on the glass in the worst case.
- **Buttons on the background have no fill**: a secondary button keeps its `line-2` border, ghost buttons stay ghost.
  Hover: `canvas-hover` (translucent), and the icon or text turns `text`.
- **A chip or pill with coloured text sits on `island`**: the Chief's chip (its `line` border, `island` fill) and the
  Inbox count on the toolbar (`needs-soft` over `island`, as `.org-badge` lays live-soft over bg today). In CSS:
  `background: linear-gradient(var(--needs-soft), var(--needs-soft)), var(--island);`.
- Keyboard focus: the 2px `focus` ring, as everywhere.

### The gradient: the glass's tint

- **The gradient** runs straight down: `canvas-from` at the top (0%), `canvas-to` at the bottom edge (100%).
- **The glow** falls from the top left corner: `canvas-glow`, full for 45% of the way, then fading to transparent at 60%
  of the window's width and 75% of its height. It sits on the background only: the islands cover it.
- **In dark** the background is a clear step lighter than the islands; **in light** the islands are the lightest surface
  and the background a soft grey a step below them, with the glow a soft blue tint.
- One cool blue, low in chroma: the window's own light, as the band was. Never `accent`, `live` or `needs`.
- **The mountains are part of the gradient** (round 1's choice, kept). Their lines go: the islands are solid, so only
  bits of the lines would show in the 6px gaps. So `Band.tsx`, `band.css`, `mountains.svg` and Settings → Appearance →
  Background's "Show mountains along the bottom" go when this is built (`gizai-band` is no longer read); the website keeps
  `mountains-hero.svg`. The first start's onboarding (no islands) keeps its band until the card that draws it.

### An island

- Fill `island` (`#181a1c` in dark, white in light), radius `island-radius` (10), `island-gap` (6) to the next island and
  to the window's edge. `overflow: hidden`, so content keeps to the round corners.
- **No border and no shadow.** The step in fill from the background alone sets an island apart.
- Inside an island, what sits on it steps up from it: cards and panels (`raised`), inputs (now `raised`, with their
  `line-2` border), hover and selected fills; code, terminals and the chat are on the island itself (`bg` = `island`).
  The 1px `line` borders that split the window into regions go: the gaps do that now. Lines inside an island stay (the
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
  Archived cards) at 44px, its controls 28–30px high, the search box and Board or List on `raised`.

### Focus

**No sign on the island.** An island looks the same with the focus or without it: no border, no glow, no change of fill.
What has the focus shows it, as today:

- the composer: its accent border and halo (`.composer-box:focus-within`), kept;
- a field or a button: its 2px `focus` ring;
- a terminal: a filled block cursor with the focus, an outline without it (xterm does this already);
- the code: its caret; a list: its selected row.

That is enough to tell where the keys go when the chat and a terminal sit side by side (the look frame shows both).

### The gap as a resize handle

Between two islands that resize (the Chat's terminal panel and the conversation; the Editor's code and its terminal
panel) the 6px gap is the handle (`size-handle` = `island-gap`; `role="separator"` as now).

- Rest: only the gap (the glass shows through it); the pointer is `col-resize` (or `row-resize`).
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

- Fill **`float`**: in dark a step lighter than the islands and than `raised` (`#22242a`); white in light, as the islands
  are. A row's hover inside it is `hover`, a step lighter still.
- A **1px `line-2` border** inside and **`shadow-pop`** (`shadow-drawer` for a drawer), as `.pop` and `.pal` have today.
  In light the border and the shadow alone set it apart from a white island.
- It is solid: no glass on what floats.
- Each keeps its own size, radius and content: the palette `radius-xl`, a menu `radius-l`, a tooltip `radius-s`.
- The look frame shows the palette over two islands and a tooltip over the background and an island. Cards 2 to 4 draw
  the drawers, menus and dialogs.

### Which surfaces change

| Today | In islands |
|---|---|
| `side`: the sidebar, the chat list, the Editor's tree, the terminal rail (dimmest) | The **background**: the glass (`glass-from` → `glass-to` with `glass-glow`) over the blurred desktop, or the opaque gradient. In dark a step lighter than the islands; in light a soft grey below them. `side` stays for board rails and code blocks inside an island, no longer for a column. |
| `bg`: the canvas, the main column, inputs | **The sidebar island and every work island**: `island` (`#181a1c` / white), and `bg` = `island`. Inputs move to `raised`. |
| `raised`: cards, panels, the text box | `raised`, lifted to step up from the island; inputs join it. |
| `overlay`: drawers, the palette, popovers, toasts | **`float`**, with a `line-2` border and its shadow. |

## The window in islands

- **The window:** `island-gap` (6) padding all round, islands 6 apart, on the glass. Hersir has the system's title bar:
  the frames draw only what is inside it.
- **The sidebar island** (`size-side`, 248) runs the window's full height on the left. Inside it is today's sidebar:
  side-main (padding `6px 10px 14px`, scrolling) and side-foot (Company, its 1px `line` top border).
- **The top bar sits on the background** (all pages): `size-topbar` 44 high, no fill, no border, padding
  `0 4px 0 12px`, over the islands of the main column (right of the sidebar island). Crumbs on the left, the page's
  actions on the right, as today, drawn as in [On the background](#on-the-background). It belongs to all of a page's
  islands (the Chat has three), so it isn't one island's header; and with the sidebar closed the toolbar shares its row.
- **Chat** (`271:136`): the chat list island (260; 220 under 1100px wide), the conversation island, the terminal island
  on the right (`--chat-term-share`, 45% of the room right of the chat list, at least 360; the gap between them is the
  handle). The terminal island's header is its tabs with New terminal, Own page and Hide; under it the folder in mono
  (`t-id`, `text-2`, no fill: the `side` strip goes); then the terminal on `term-bg` (= `island`). Hidden with
  terminals, the rail is a 36px island of its own.
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
  4. **Inbox**, `inbox`, with its count: the sidebar's pill (`App/t-count`, `needs` on `needs-soft` over `island`, at
     least 18 wide, padding `0 5px`, 4px after the icon), the button wider by the pill. With nothing waiting the pill
     goes. Tooltip **Inbox (3 need you)**, or **Inbox** with none. It goes to the Inbox, as the sidebar's row does.
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
in a Chief answer; the component `Islands/Hover card` (`291:2641`) has every kind side by side. GA-287 builds it.

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
- Fill `float` (`#22242a` in dark, round 3), 1px `line-2` border inside, radius `radius-l` (10), `shadow-pop`. Solid, no
  glass. It shows at once (no fade).
- **Heading:** `t-body-strong` in `text`, wrapping, at most 3 lines (then an ellipsis). For a card, its id above it in
  `t-id`, `text-3`, 2px apart.
- **Rows:** 4px apart; each a label in `t-small`, `text-3` (4.73 on float), 56px wide, then 8px, then the value in
  `t-small`, `text`, with its mark 6px before it (the project's colour dot 8px, an `App/Avatar` small, a status glyph
  16px, the live dot 6px). A value that doesn't fit wraps (an email breaks anywhere).
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
- Settings → Appearance → Background (proposed, round 3): the section's text **Behind the islands: a soft gradient, and
  glass where your system blurs windows.**; the field **Glass**; the checkbox **Show the desktop through the background**;
  its hint **Blurred and tinted, on macOS, Windows 11 and Hyprland. Off gives a plain background.**; with no blur from
  the system **Your system doesn't blur windows behind Hersir, so the background stays plain.**; with reduce
  transparency on **Plain while your system reduces transparency.**

Gone: the chip's browser tooltip **Open task** (and Open project, Open client, …); Settings → Appearance → Background's
**A landscape of wireframe mountains along the bottom of the window. It stays behind text and controls.**, **Mountains**,
**Under the sidebar and the page, in the theme's colours. Off gives the plain background back.** and **Show mountains
along the bottom**.

Unchanged: every other text in the frames is today's (the sidebar, the top bars, the chat, the board's toolbar, the
Editor). The chats, cards, files, code and terminal output in the frames are sample data, not texts to build. The texts
in the look frames are notes for the builders, not texts to build.

## Not in this card

- The rest of the app on this look: cards 2 to 4 (GA-285, GA-286 and GA-288), the Editor, the board and cards beside the
  Chief Chat, the other pages, drawers, menus and dialogs, and the first start's onboarding among them.
- Building it: GA-287 builds the hover card; the rest is built after cards 2 to 4.
