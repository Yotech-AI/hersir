// Hersir's brand artwork (GA-230): the longboat H as mark, logo, favicon and app icon, as SVG files in design/brand/,
// plus the two copies the app ships: src-tauri/icons/gizai.svg (the Linux icon install.sh copies) and
// src/assets/gizai-logo.svg (the onboarding's logo, in one colour). The drawings are Figma's, file "Giz-AI", page
// "Hersir: brand direction" (184:2), taken over number for number: the component Hersir mark (198:258) and its 16 px cut
// (198:263), the wordmark (187:38), the logos (187:39 and 187:50) and the app icon's tile (187:178). Jefsev approved
// them in GW-11. The icons' PNGs, .ico and .icns come from these files: node design/brand/make-icons.mjs.
// Run it with: node design/brand/gen-brand.mjs
import { writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = join(DIR, "../..");
const AMBER = { dark: "#f2a53a", light: "#9e5500" }; // Hersir colour, brand/amber: 9.4:1 and 4.9:1 on the brand's own bg
const TEXT = { dark: "#edebe6", light: "#16191c" }; // Hersir colour, text/text: the name in the two-colour logos
const INKS = {
  "on-dark": { mark: AMBER.dark, word: TEXT.dark },
  "on-light": { mark: AMBER.light, word: TEXT.light },
  white: { mark: "#ffffff", word: "#ffffff" },
  black: { mark: "#000000", word: "#000000" },
};
const TILE = { top: "#1a2128", bottom: "#0b0e11" }; // the icon's iron tile, top to bottom, with a white edge at 8 %
const n = (v) => +v.toFixed(2);

// Maps absolute path data (M, L, C and Z only, so every number pair is a point): x by fx, y by fy.
function mapPoints(d, fx, fy) {
  let i = 0;
  return d.replace(/-?\d*\.?\d+/g, (s) => String(n(i++ % 2 ? fy(+s) : fx(+s))));
}
// Moves and scales them: x by sx, y by sy.
const move = (d, dx, dy, sx = 1, sy = sx) => mapPoints(d, (x) => x * sx + dx, (y) => y * sy + dy);

// The mark on its 24 px grid, one filled outline (GA-239): four strokes of 2 with round caps, merged and mirrored around
// x 12, number for number. The stems (x 6 and 18, y 3 to 21) are the longship's stem and stern; the sheer and the keel
// leave them at y 8 and sag between them, to 11.6 and 15.6, like a hull. As separate strokes the hull's caps touched the
// stems' outer edges at y 8 and browsers painted them out of the line; merged, each outer edge is one straight line, from
// cap to cap. The curves are cut short, the shape staying the same: each hull edge in 8 pieces from stem to middle, each
// quarter of a cap in 4. Chromium's software renderer walks a curve in straight steps on quarter pixels and treats a too
// flat step differently on the left and the right of a span, so long flat curves drew the hole's top tilted at 40 and 48 px.
const MARK = "M5 3C5 2.862 5.028 2.731 5.079 2.611C5.129 2.491 5.203 2.383 5.293 2.293C5.384 2.202 5.491 2.129 5.611 2.079C5.731 2.028 5.862 2 6 2C6.138 2 6.269 2.028 6.389 2.079C6.509 2.129 6.616 2.202 6.707 2.293C6.797 2.383 6.871 2.491 6.921 2.611C6.972 2.731 7 2.862 7 3L7 7.857C7.064 8.07 7.136 8.282 7.217 8.488C7.299 8.694 7.394 8.893 7.505 9.08C7.616 9.267 7.745 9.443 7.896 9.6C8.047 9.757 8.22 9.896 8.421 10.011C8.659 10.147 8.934 10.252 9.231 10.333C9.529 10.413 9.846 10.47 10.167 10.51C10.489 10.549 10.814 10.572 11.126 10.584C11.436 10.597 11.734 10.6 12 10.6C12.266 10.6 12.564 10.597 12.874 10.584C13.186 10.572 13.511 10.549 13.833 10.51C14.154 10.47 14.471 10.413 14.769 10.333C15.066 10.252 15.341 10.147 15.579 10.011C15.78 9.896 15.953 9.757 16.104 9.6C16.255 9.443 16.384 9.267 16.495 9.08C16.606 8.893 16.701 8.694 16.783 8.488C16.864 8.282 16.936 8.07 17 7.857L17 3C17 2.862 17.028 2.731 17.079 2.611C17.129 2.491 17.203 2.383 17.293 2.293C17.384 2.202 17.491 2.129 17.611 2.079C17.731 2.028 17.862 2 18 2C18.138 2 18.269 2.028 18.389 2.079C18.509 2.129 18.616 2.202 18.707 2.293C18.797 2.383 18.871 2.491 18.921 2.611C18.972 2.731 19 2.862 19 3L19 21C19 21.138 18.972 21.269 18.921 21.389C18.871 21.509 18.797 21.617 18.707 21.707C18.616 21.798 18.509 21.871 18.389 21.921C18.269 21.972 18.138 22 18 22C17.862 22 17.731 21.972 17.611 21.921C17.491 21.871 17.384 21.798 17.293 21.707C17.203 21.617 17.129 21.509 17.079 21.389C17.028 21.269 17 21.138 17 21L17 14.593C16.84 14.785 16.672 14.959 16.496 15.119C16.321 15.279 16.138 15.423 15.949 15.553C15.76 15.683 15.565 15.799 15.364 15.901C15.162 16.004 14.954 16.093 14.743 16.171C14.531 16.249 14.314 16.314 14.093 16.369C13.871 16.424 13.645 16.468 13.416 16.502C13.186 16.536 12.953 16.561 12.717 16.577C12.48 16.593 12.241 16.6 12 16.6C11.759 16.6 11.52 16.593 11.283 16.577C11.047 16.561 10.814 16.536 10.584 16.502C10.355 16.468 10.129 16.424 9.907 16.369C9.686 16.314 9.469 16.249 9.257 16.171C9.046 16.093 8.838 16.004 8.636 15.901C8.435 15.799 8.24 15.683 8.051 15.553C7.862 15.423 7.679 15.279 7.504 15.119C7.328 14.959 7.16 14.785 7 14.593L7 21C7 21.138 6.972 21.269 6.921 21.389C6.871 21.509 6.797 21.617 6.707 21.707C6.616 21.798 6.509 21.871 6.389 21.921C6.269 21.972 6.138 22 6 22C5.862 22 5.731 21.972 5.611 21.921C5.491 21.871 5.384 21.798 5.293 21.707C5.203 21.617 5.129 21.509 5.079 21.389C5.028 21.269 5 21.138 5 21ZM7.748 11.914C7.828 12.122 7.917 12.325 8.018 12.521C8.119 12.716 8.231 12.904 8.357 13.081C8.484 13.257 8.624 13.423 8.781 13.575C8.939 13.727 9.112 13.864 9.305 13.984C9.503 14.107 9.713 14.207 9.931 14.288C10.149 14.369 10.376 14.43 10.606 14.477C10.836 14.523 11.07 14.554 11.304 14.573C11.538 14.592 11.771 14.6 12 14.6C12.229 14.6 12.462 14.592 12.696 14.573C12.93 14.554 13.164 14.523 13.394 14.477C13.624 14.43 13.851 14.369 14.069 14.288C14.287 14.207 14.497 14.107 14.695 13.984C14.888 13.864 15.061 13.727 15.219 13.575C15.376 13.423 15.516 13.257 15.643 13.081C15.769 12.904 15.881 12.716 15.982 12.521C16.083 12.325 16.172 12.122 16.252 11.914C16.094 11.989 15.931 12.056 15.763 12.116C15.594 12.176 15.423 12.229 15.248 12.275C15.073 12.321 14.894 12.361 14.714 12.396C14.534 12.43 14.353 12.459 14.169 12.483C13.986 12.507 13.802 12.526 13.619 12.542C13.435 12.557 13.251 12.569 13.069 12.577C12.887 12.586 12.706 12.592 12.527 12.595C12.349 12.599 12.172 12.6 12 12.6C11.828 12.6 11.651 12.599 11.473 12.595C11.294 12.592 11.113 12.586 10.931 12.577C10.749 12.569 10.565 12.557 10.381 12.542C10.198 12.526 10.014 12.507 9.831 12.483C9.647 12.459 9.466 12.43 9.286 12.396C9.106 12.361 8.927 12.321 8.752 12.275C8.577 12.229 8.406 12.176 8.237 12.116C8.069 12.056 7.906 11.989 7.748 11.914Z";
// The mark, its grid scaled by s and moved to (dx, dy).
const mark = (ink, dx, dy, s) => `<path fill="${ink}" d="${move(MARK, dx, dy, s)}"/>`;
// The mark as the 48 px cut draws it: at 1.5, but the space between the stems at 1.6, so the stems are 3 px on whole
// pixels, 19 apart instead of 18.
const stretch48 = (x) => (x <= 7 ? 1.5 * x + 0.5 : x >= 17 ? 1.5 * x + 1.5 : 1.6 * x - 0.2);
const mark48 = (ink, dx, dy) => `<path fill="${ink}" d="${mapPoints(MARK, (x) => stretch48(x) + dx, (y) => 1.5 * y + dy)}"/>`;
// The 16 px cut, on the pixel grid: stems 2 px (x 4 and 12, y 2.5 to 13.5) and the hull filled, so it stays a boat.
const CUT16 = {
  stems: "M4 2.5L4 13.5M12 2.5L12 13.5",
  hull: "M4 5C4.4 9.6 5.6 10.8 8 10.8C10.4 10.8 11.6 9.6 12 5C11.4 6.9 10.4 7.6 8 7.6C5.6 7.6 4.6 6.9 4 5Z",
};
const cut16 = (ink) => `<path fill="none" stroke="${ink}" stroke-width="2" stroke-linecap="round" d="${CUT16.stems}"/>\n`
  + `<path fill="${ink}" stroke="${ink}" stroke-width="0.6" stroke-linejoin="round" d="${CUT16.hull}"/>`;

// The wordmark "Hersir": Schibsted Grotesk Bold, outlined in Figma, the r–s pair closed 6 % and i–r 2.5 %; its H is the
// longboat H at the font's weight (the crossbar sags into a hull). Its box is 113.69 × 31.82, the baseline at 31.31.
const WORD = [
  "M0 31.31L0 3.18L6.03 3.18L6.03 11.4C6.87 14.75 8.32 15.7 12.28 15.7C16.25 15.7 17.7 14.75 18.53 11.4L18.53 3.18L24.57 3.18L24.57 31.31L18.53 31.31L18.53 15.7C18.16 20.38 16.28 21.2 12.28 21.2C8.28 21.2 6.41 20.38 6.03 15.7L6.03 31.31L0 31.31Z",
  "M38.99 31.82C36.92 31.82 35.06 31.37 33.4 30.47C31.76 29.57 30.46 28.29 29.5 26.64C28.55 24.97 28.07 23.01 28.07 20.74C28.07 18.49 28.54 16.54 29.48 14.88C30.43 13.23 31.72 11.95 33.35 11.05C34.99 10.16 36.85 9.71 38.95 9.71C40.11 9.71 41.28 9.88 42.47 10.23C43.66 10.59 44.76 11.21 45.77 12.11C46.77 12.99 47.58 14.24 48.19 15.84C48.8 17.43 49.11 19.46 49.11 21.93L31.8 21.93L31.8 18.61L43.91 18.61L43.31 19.65C43.2 18.24 42.94 17.11 42.53 16.25C42.11 15.39 41.58 14.77 40.94 14.38C40.3 13.98 39.59 13.79 38.79 13.79C37.77 13.79 36.91 14.08 36.22 14.67C35.53 15.25 35.01 16.07 34.65 17.13C34.32 18.17 34.15 19.38 34.15 20.76C34.15 22.85 34.54 24.49 35.32 25.7C36.11 26.91 37.32 27.52 38.95 27.52C40.01 27.52 40.91 27.25 41.67 26.7C42.43 26.15 43.02 25.32 43.42 24.2L48.7 25.55C48.2 27.01 47.45 28.2 46.45 29.12C45.45 30.05 44.3 30.73 42.99 31.17C41.7 31.6 40.37 31.82 38.99 31.82Z",
  "M52.61 31.31L52.61 10.21L58.23 10.21L58.23 14.96L58 14.34C58.66 12.77 59.6 11.61 60.81 10.86C62.02 10.09 63.34 9.71 64.78 9.71C65.18 9.71 65.59 9.74 66.01 9.8C66.42 9.87 66.84 9.98 67.26 10.14L66.81 15.1C65.96 14.86 65.15 14.75 64.37 14.75C63.73 14.75 63.06 14.86 62.35 15.08C61.66 15.3 61.02 15.69 60.42 16.25C59.84 16.8 59.35 17.58 58.98 18.59C58.61 19.6 58.43 20.88 58.43 22.44L58.43 31.31L52.61 31.31Z",
  "M75.97 31.82C74.38 31.82 72.88 31.59 71.48 31.13C70.07 30.66 68.89 29.94 67.92 28.96C66.96 27.99 66.35 26.72 66.09 25.16L71.4 24.1C71.52 25.29 71.93 26.18 72.65 26.8C73.38 27.4 74.41 27.7 75.76 27.7C77.04 27.7 78 27.44 78.63 26.93C79.25 26.41 79.56 25.81 79.56 25.12C79.56 24.51 79.32 23.98 78.82 23.54C78.34 23.09 77.5 22.78 76.3 22.6L74.39 22.29C73.63 22.17 72.83 22.01 71.97 21.82C71.12 21.61 70.32 21.3 69.56 20.9C68.81 20.48 68.19 19.91 67.71 19.18C67.23 18.45 66.99 17.51 66.99 16.37C66.99 14.97 67.36 13.78 68.1 12.79C68.85 11.79 69.9 11.03 71.22 10.51C72.55 9.97 74.07 9.71 75.78 9.71C77.35 9.71 78.79 9.94 80.09 10.41C81.39 10.87 82.48 11.54 83.35 12.44C84.24 13.33 84.81 14.43 85.07 15.74L79.76 16.84C79.67 16.3 79.49 15.8 79.23 15.33C78.98 14.86 78.6 14.49 78.08 14.2C77.56 13.9 76.86 13.75 75.97 13.75C74.97 13.75 74.17 13.95 73.57 14.34C72.98 14.73 72.69 15.26 72.69 15.94C72.69 16.43 72.85 16.84 73.18 17.15C73.52 17.46 73.97 17.71 74.54 17.91C75.13 18.09 75.79 18.25 76.52 18.38L78.7 18.75C79.82 18.93 80.89 19.25 81.89 19.71C82.9 20.15 83.73 20.79 84.37 21.62C85.01 22.45 85.33 23.55 85.33 24.9C85.33 26.4 84.92 27.66 84.1 28.69C83.29 29.72 82.17 30.5 80.76 31.04C79.35 31.56 77.75 31.82 75.97 31.82Z",
  "M88.89 31.31L88.89 10.21L94.71 10.21L94.71 31.31L88.89 31.31ZM91.8 7.7C90.64 7.7 89.71 7.34 89 6.64C88.3 5.94 87.95 5.01 87.95 3.85C87.95 2.68 88.3 1.74 89 1.05C89.71 0.35 90.64 0 91.8 0C92.97 0 93.9 0.35 94.59 1.05C95.29 1.74 95.64 2.68 95.64 3.85C95.64 5.01 95.29 5.94 94.59 6.64C93.9 7.34 92.97 7.7 91.8 7.7Z",
  "M99.04 31.31L99.04 10.21L104.66 10.21L104.66 14.96L104.43 14.34C105.09 12.77 106.03 11.61 107.24 10.86C108.45 10.09 109.77 9.71 111.21 9.71C111.61 9.71 112.02 9.74 112.44 9.8C112.85 9.87 113.27 9.98 113.69 10.14L113.24 15.1C112.39 14.86 111.58 14.75 110.8 14.75C110.16 14.75 109.49 14.86 108.78 15.08C108.09 15.3 107.45 15.69 106.85 16.25C106.26 16.8 105.78 17.58 105.41 18.59C105.04 19.6 104.86 20.88 104.86 22.44L104.86 31.31L99.04 31.31Z",
];
const word = (ink, dx, dy, k = 1) => `<path fill="${ink}" d="${WORD.map((d) => move(d, dx, dy, k)).join("")}"/>`;

// The lockups, as Figma's components lay them out. Horizontal: the mark 48 (its 24 grid at 2), the wordmark at 90 %
// 60 to its right, centred on it. Stacked: the mark 88 over the wordmark at 100 %, 102 down.
const LOGO = {
  horizontal: { w: 162.32, h: 48, body: (i) => `${mark(i.mark, 0, 0, 2)}\n${word(i.word, 60, 9.68, 0.9)}` },
  stacked: { w: 113.69, h: 133.82, body: (i) => `${mark(i.mark, 12.84, 0, 88 / 24)}\n${word(i.word, 0, 102)}` },
};

const svg = (w, h, body, comment = "") =>
  `<svg xmlns="http://www.w3.org/2000/svg" width="${w}" height="${h}" viewBox="0 0 ${w} ${h}" role="img" aria-label="Hersir">\n`
  + `${comment ? `<!-- ${comment} -->\n` : ""}<title>Hersir</title>\n${body}\n</svg>\n`;

const files = {};
for (const [ink, i] of Object.entries(INKS)) {
  files[`mark-${ink}.svg`] = svg(24, 24, mark(i.mark, 0, 0, 1));
  for (const [layout, l] of Object.entries(LOGO)) files[`logo-${layout}-${ink}.svg`] = svg(l.w, l.h, l.body(i));
}

// The tile: the iron gradient, and a white edge at 8 % inside it (1 px on the small tiles).
const gradient = (id) => `<defs><linearGradient id="${id}" x1="0" y1="0" x2="0" y2="1"><stop offset="0" stop-color="${TILE.top}"/><stop offset="1" stop-color="${TILE.bottom}"/></linearGradient></defs>`;
const tile = (s, rx, id) => `${gradient(id)}\n<rect width="${s}" height="${s}" rx="${rx}" fill="url(#${id})"/>\n`
  + `<rect x="0.5" y="0.5" width="${s - 1}" height="${s - 1}" rx="${rx - 0.5}" fill="none" stroke="#ffffff" stroke-opacity="0.08"/>`;
// The small icons on the pixel grid. 16: the 16 px cut on the whole tile. 32: the mark 1:1 on its 24 grid, 4 in. 48 (no
// frame in Figma): the 32's drawing at 1.5, its stems 19 apart instead of 18 so they land on whole pixels.
const SMALL = {
  16: { rx: 3.5, body: () => cut16(AMBER.dark) },
  32: { rx: 7, body: () => mark(AMBER.dark, 4, 4, 1) },
  48: { rx: 10.5, body: () => mark48(AMBER.dark, 5, 6) },
};
const small = (s) => svg(s, s, `${tile(s, SMALL[s].rx, `hs-tile-${s}`)}\n${SMALL[s].body()}`);
for (const s of [16, 32, 48]) files[`favicon-${s}.svg`] = small(s);
files["favicon.svg"] = small(32);
files["app-icon-16.svg"] = small(16);
files["app-icon-32.svg"] = small(32);

// The app icon's 1024 px master for Linux and Windows: Figma's 1024 tile (radius 228, the mark's 24 grid 680 wide at
// 172) scaled to an 896 px tile in the 1024 canvas, as every Linux icon theme leaves a margin.
function bigIcon() {
  const k = 896 / 1024, at = 64, edge = 4 * k, rx = 228 * k;
  return `${gradient("hs-tile")}\n<rect x="${at}" y="${at}" width="896" height="896" rx="${n(rx)}" fill="url(#hs-tile)"/>\n`
    + `<rect x="${n(at + edge / 2)}" y="${n(at + edge / 2)}" width="${n(896 - edge)}" height="${n(896 - edge)}" rx="${n(rx - edge / 2)}" fill="none" stroke="#ffffff" stroke-opacity="0.08" stroke-width="${n(edge)}"/>\n`
    + mark(AMBER.dark, at + 172 * k, at + 172 * k, (680 * k) / 24);
}
files["app-icon-1024.svg"] = svg(1024, 1024, bigIcon());

// macOS: Apple's grid, an 824 px rounded square (continuous corners: Figma's 60 % corner smoothing) in the 1024 px canvas,
// with the soft shadow of Apple's template; the tile, its edge and the mark as Figma's 1024 tile, scaled to 824.
const SQ824 = "M0 296.64C0 192.81 0 140.89 20.21 101.23C37.98 66.34 66.34 37.98 101.23 20.21C140.89 0 192.81 0 296.64 0L527.36 0C631.19 0 683.11 0 722.77 20.21C757.66 37.98 786.02 66.34 803.79 101.23C824 140.89 824 192.81 824 296.64L824 527.36C824 631.19 824 683.11 803.79 722.77C786.02 757.66 757.66 786.02 722.77 803.79C683.11 824 631.19 824 527.36 824L296.64 824C192.81 824 140.89 824 101.23 803.79C66.34 786.02 37.98 757.66 20.21 722.77C0 683.11 0 631.19 0 527.36Z";
function macIcon() {
  const k = 824 / 1024, at = 100, edge = 4 * k, ki = (824 - edge) / 824, inset = at + edge / 2;
  return `<defs><filter id="hs-shadow" x="-10%" y="-10%" width="120%" height="120%"><feGaussianBlur stdDeviation="10"/></filter></defs>\n`
    + `<path fill="#000000" fill-opacity="0.3" transform="translate(0 10)" filter="url(#hs-shadow)" d="${move(SQ824, at, at)}"/>\n`
    + `${gradient("hs-tile-mac")}\n<path fill="url(#hs-tile-mac)" d="${move(SQ824, at, at)}"/>\n`
    + `<path fill="none" stroke="#ffffff" stroke-opacity="0.08" stroke-width="${n(edge)}" d="${move(SQ824, inset, inset, ki)}"/>\n`
    + mark(AMBER.dark, at + 172 * k, at + 172 * k, (680 * k) / 24);
}
files["app-icon-macos-1024.svg"] = svg(1024, 1024, macIcon());

// The macOS menu bar's template image (src-tauri/icons/tray-template.png, 36 px for 18 pt): the mark in black on
// transparent, drawn as the 48 px cut is, its stems on whole pixels; macOS colours it for its look.
files["tray-template.svg"] = svg(36, 36, mark48("#000000", -1, 0));

for (const [name, body] of Object.entries(files)) writeFileSync(join(DIR, name), body);

// The app's copies. gizai.svg keeps its name: install.sh copies it into the icon theme as gizai.svg.
writeFileSync(join(ROOT, "src-tauri/icons/gizai.svg"), svg(1024, 1024, bigIcon(),
  "Hersir's app icon (GA-230): the longboat H on its iron tile, design/brand/app-icon-1024.svg. Made by design/brand/gen-brand.mjs."));
// The onboarding's logo: the horizontal logo in one colour, currentColor. Inside the app the gold means "needs you"
// only, so the logo takes the colour of the text around it (text-2).
writeFileSync(join(ROOT, "src/assets/gizai-logo.svg"), svg(162.32, 48, LOGO.horizontal.body({ mark: "currentColor", word: "currentColor" }),
  "Hersir's horizontal logo (design/brand/logo-horizontal-*.svg, GA-230) in one colour, currentColor: inside the app it is text-2, never gold. Made by design/brand/gen-brand.mjs."));
console.log(`${Object.keys(files).length} files: ${Object.keys(files).join(", ")}; src-tauri/icons/gizai.svg, src/assets/gizai-logo.svg`);
