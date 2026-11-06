# Hersir brand (GA-230)

Hersir's mark, logo, favicon and app icon as SVG files, made by `gen-brand.mjs`, and the app's icons made from them by
`make-icons.mjs`. The drawings are the direction Jefsev approved in Figma (GW-11): the file "Giz-AI", page **Hersir:
brand direction** ([open](https://www.figma.com/design/Q2dp2xiv8MMm90Xwj3ToCI/Giz-AI?node-id=184-2)). `gen-brand.mjs`
takes them over number for number (the component Hersir mark 198:258 and its 16 px cut 198:263, the wordmark 187:38,
the logos 187:39 and 187:50, the app icon's tile 187:178) and draws the files itself rather than keeping Figma's exports,
so every logo, tile, cut and copy comes from one place. They replace the old G (GA-52); the website (GW-12) made its
icons from the same frames.

Run `node design/brand/gen-brand.mjs` to write the SVGs again, with the app's two copies (`src-tauri/icons/gizai.svg` and
`src/assets/gizai-logo.svg`), then `node design/brand/make-icons.mjs` for `src-tauri/icons`. If a drawing changes in
Figma, change its numbers in `gen-brand.mjs`.

## The files

| File | What it is |
|---|---|
| `mark-*.svg` | The mark alone on its 24 px grid, 24 × 24. |
| `logo-horizontal-*.svg` | The mark (48 high) and the wordmark side by side, 162.32 × 48: Figma's Hersir logo · Horizontal. |
| `logo-stacked-*.svg` | The mark (88 high) over the wordmark, 113.69 × 133.82: Hersir logo · Stacked. |
| `favicon.svg` | The SVG favicon: the 32 px drawing, which browsers scale. |
| `favicon-16.svg`, `-32.svg`, `-48.svg` | The favicon drawn on the pixel grid: 16 is the 16 px cut, 32 the mark 1:1 on its grid. Figma has no 48: it is the 32's drawing at 1.5, stems 3 px, the space between them at 1.6, so they land on whole pixels, 19 apart instead of 18. |
| `app-icon-1024.svg` | The app icon's master for Linux and Windows: Figma's 1024 tile as an 896 px tile in the 1024 canvas, the margin icon themes leave. `src-tauri/icons/gizai.svg` is the same drawing. |
| `app-icon-macos-1024.svg` | macOS: Apple's 824 px rounded square (continuous corners) in the 1024 canvas, with its shadow. |
| `app-icon-16.svg`, `-32.svg` | The app icon at 16 and 32 px, drawn on the pixel grid: the favicon's drawings. |
| `tray-template.svg` | The macOS menu bar's template image, 36 px for 18 pt: the mark in black on transparent, drawn as the 48 px cut is. |
| `gen-brand.mjs` | Writes every SVG above, `src-tauri/icons/gizai.svg` and `src/assets/gizai-logo.svg`. |
| `make-icons.mjs` | Writes `src-tauri/icons`: renders with `npx tauri icon`, puts the 16 and 32 px cuts into `32x32.png`, `icon.ico` and `icon.icns` (macOS's from its own master), and makes `tray-template.png`. File names stay. |

The mark and each logo come in four colours: `-on-dark` (the mark #f2a53a, the name #edebe6), `-on-light` (#9e5500 and
#16191c), `-white` and `-black` (one colour each, for prints, photos and colour). The backgrounds are transparent.

## What the design decides

- **The mark** is the longboat H (take B, the hull): the H of Hersir drawn as a longship. The stems are its stem and
  stern posts; the sheer and the keel leave them high (y 8 on the 24 grid) and sag between them (to 11.6 and 15.6) like
  a hull. Four strokes of one weight, 2 on the 24 grid, round caps, merged into one filled outline, so each stem's outer
  edge is one straight line from cap to cap, in every renderer and at every size (GA-239). The strokes it is merged from:
  `M6 3L6 21M18 3L18 21M6 8C6.8 10.8 8.2 11.6 12 11.6C15.8 11.6 17.2 10.8 18 8M6 8C6.4 13.6 8.2 15.6 12 15.6C15.8 15.6 17.6 13.6 18 8`.
  Use it from 24 px up; under 24 px use the 16 px cut, whose hull is filled so it stays an H with a boat in it.
- **The wordmark** reads Hersir: Schibsted Grotesk Bold, outlined, the r–s pair closed 6 % and the i–r pair 2.5 % so
  the eye runs through it as one word. Its H is the longboat H at the font's weight. Say it HER-seer.
- **Colour**: on dark and on light the longboat H is amber, Hersir colour's `brand/amber` (#f2a53a, 9.4:1 on the brand's
  dark page; #9e5500, 4.9:1 on its light page), and the name is text colour; white or black for one colour.
- **The gold and "needs you"** share the hue and keep apart by place and shape, as the old pink did. The brand gold goes
  on the logo, the app icon and the favicon. Inside the app's window gold means only "needs you" (`needs`: Review,
  Deploy, holds, the Inbox count), so the app shows its logo in one colour (`text-2`), and the accent that buttons,
  links and focus use stays blue. `needs` is the brand's amber in dark; in light it is #9a5800, a hair yellower than
  #9e5500 (ΔE2000 2), so that On hold's red (`danger`, #c93530) keeps 35° of hue from it. In progress and `warning`
  moved to a yellow for the same rule (`design/README.md`). The status glyph is a thin 16 px ring with a dot, beside its
  word; the mark is an H and never stands in a list or on a card.
- **Clear space**: half the mark's height on every side. **Smallest**: the horizontal logo 20 px high (the onboarding
  shows it 24 high, so its mark is on its own grid); the mark 24 px, under that the 16 px cut.
- **The icon** is the amber mark on an iron tile, the same in both themes: a gradient from #1a2128 at the top to #0b0e11,
  with a white edge at 8 % inside it, and Figma's corner radius (228 of 1024; on macOS Apple's continuous corners). The
  mark's 24 grid is 680 of the 1024 tile, 172 in. At 32 px the mark is 1:1 on its grid, at 16 px the 16 px cut.
- **Small sizes are the cuts**: in `src-tauri/icons`, `32x32.png` and the 16 and 32 px images in `icon.ico` and
  `icon.icns` are the pixel-grid cuts, filling their square as small icons do; every other size is the master scaled.
- **The tray**: macOS's menu bar takes `tray-template.png` (black; macOS colours it for its look); Linux and Windows
  take `64x64.png`, the app icon.
- **Type**: Schibsted Grotesk for the wordmark and the website's headings; text stays Atkinson Hyperlegible Next, and
  labels and code Atkinson Hyperlegible Mono. The website's scale is Figma's text styles `Hersir/*` (frame 5 Type).

## In the Figma file

- The page **Hersir: brand direction** (184:2) holds the brand: the mark, the logo, the app icon and favicon, colour
  (the variables `Hersir colour`, the website's), type, line art and voice.
- The app's colours stay `design/tokens.json` (the variables `GizAi colour`, which still hold the old pink and amber
  until Jefsev has seen the change). The frame **Hersir · 8 App statuses: before and after (GA-230)** (236:384) on the
  same page shows the app's status glyphs, badges and the Inbox count with the old pink and amber and with the gold and
  yellow, in dark and light, with the numbers and the memory graph's new colours, for Jefsev's review.
- The frame **Hersir mark: before and after (GA-239)** (246:528) on the same page shows the four strokes and the one
  outline at 800 %, what a browser paints with each, and the logo, the favicon and the app icon at real size, in dark and
  light.
- The old brandbook (page Brandbook, GA-52) stays as it was.
