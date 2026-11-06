// GA-230 QA: Hersir's look in the app. design/brand holds the longboat H (the mark, the logos in four colours, the
// favicons, the app icons and the tray template) and its README names them; src-tauri/icons are made from them, with the
// pixel-grid cuts at 16 and 32 px in 32x32.png, icon.ico and icon.icns, the same file names and tauri.conf.json's list
// as before; gizai.svg is the app icon and tray-template.png the mark in black on transparent; the onboarding's logo is
// the horizontal logo in one colour. needs is the brand gold, readable as text, every status keeps 35° of hue and a
// ΔE2000 of 15 from it, the accent stays blue, and the terminal's cyan and magenta keep away from live and needs.
// GA-239 QA: the mark is one filled outline, the outline of the same four strokes: in every drawing each stem's outer edge
// is one straight line from cap to cap, left and right mirror each other, and the icons show it pixel by pixel.
// Files are read from disk; node:fs and node:zlib come in through a computed specifier, as in terminalTokens.test.ts.
import { describe, expect, it } from "vitest";
import { tokensCss, type Tokens } from "./lib/tokensCss";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const zlib = await import(/* @vite-ignore */ ["node", "zlib"].join(":"));
const url = (path: string) => new URL(`../${path}`, import.meta.url);
const read = (path: string): string => fs.readFileSync(url(path), "utf8");
// a Node Buffer (readUInt32BE, subarray, equals…): the build has no Node types
type Bytes = any;
const bytes = (path: string): Bytes => fs.readFileSync(url(path));
const withoutComments = (svg: string) => svg.replace(/<!--[\s\S]*?-->\n?/g, "");

// ---- colours: the same sums as design/terminal/gen-terminal.mjs (GA-132) ----
const rgbOf = (hex: string) => { const n = parseInt(hex.slice(1), 16); return [n >> 16, (n >> 8) & 255, n & 255] as const; };
const lin = (v: number) => { v /= 255; return v <= 0.04045 ? v / 12.92 : ((v + 0.055) / 1.055) ** 2.4; };
const lum = (hex: string) => { const [r, g, b] = rgbOf(hex); return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b); };
const contrast = (a: string, b: string) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p) as [number, number]; return (x + 0.05) / (y + 0.05); };
/** OKLCH lightness, chroma and hue in degrees (Björn Ottosson's OKLab). */
function oklch(hex: string) {
  const [r, g, b] = rgbOf(hex).map(lin) as [number, number, number];
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b);
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b);
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b);
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s, B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s;
  return { L: 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s, C: Math.hypot(A, B), H: ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360 };
}
const hueGap = (a: string, b: string) => { const d = Math.abs(oklch(a).H - oklch(b).H) % 360; return d > 180 ? 360 - d : d; };
function lab(hex: string) {
  const [r, g, b] = rgbOf(hex).map(lin) as [number, number, number];
  const f = (x: number) => (x > 216 / 24389 ? Math.cbrt(x) : ((24389 / 27) * x + 16) / 116);
  const [X, Y, Z] = [f((0.4124 * r + 0.3576 * g + 0.1805 * b) / 0.95047), f(0.2126 * r + 0.7152 * g + 0.0722 * b), f((0.0193 * r + 0.1192 * g + 0.9505 * b) / 1.08883)];
  return [116 * Y - 16, 500 * (X - Y), 200 * (Y - Z)] as const;
}
/** CIEDE2000 colour difference. */
function deltaE(h1: string, h2: string) {
  const [L1, a1, b1] = lab(h1), [L2, a2, b2] = lab(h2), rad = Math.PI / 180, deg = 180 / Math.PI;
  const Cb = (Math.hypot(a1, b1) + Math.hypot(a2, b2)) / 2, G = 0.5 * (1 - Math.sqrt(Cb ** 7 / (Cb ** 7 + 25 ** 7)));
  const a1p = (1 + G) * a1, a2p = (1 + G) * a2, C1 = Math.hypot(a1p, b1), C2 = Math.hypot(a2p, b2);
  const hp1 = (Math.atan2(b1, a1p) * deg + 360) % 360, hp2 = (Math.atan2(b2, a2p) * deg + 360) % 360;
  let dh = hp2 - hp1; if (C1 * C2 === 0) dh = 0; else if (dh > 180) dh -= 360; else if (dh < -180) dh += 360;
  const dH = 2 * Math.sqrt(C1 * C2) * Math.sin((dh / 2) * rad), Lb = (L1 + L2) / 2, Cbp = (C1 + C2) / 2;
  let hb = hp1 + hp2; if (C1 * C2 !== 0) { if (Math.abs(hp1 - hp2) > 180) hb += hp1 + hp2 < 360 ? 360 : -360; hb /= 2; }
  const T = 1 - 0.17 * Math.cos((hb - 30) * rad) + 0.24 * Math.cos(2 * hb * rad) + 0.32 * Math.cos((3 * hb + 6) * rad) - 0.2 * Math.cos((4 * hb - 63) * rad);
  const Rc = 2 * Math.sqrt(Cbp ** 7 / (Cbp ** 7 + 25 ** 7)), dTh = 30 * Math.exp(-(((hb - 275) / 25) ** 2));
  const Sl = 1 + (0.015 * (Lb - 50) ** 2) / Math.sqrt(20 + (Lb - 50) ** 2), Sc = 1 + 0.045 * Cbp, Sh = 1 + 0.015 * Cbp * T;
  const dC = C2 - C1, dL = L2 - L1;
  return Math.sqrt((dL / Sl) ** 2 + (dC / Sc) ** 2 + (dH / Sh) ** 2 - Math.sin(2 * dTh * rad) * Rc * (dC / Sc) * (dH / Sh));
}

// ---- the tokens: tokens.css's two theme blocks, aliases followed to the hex ----
type Token = { name: string; value: string | { dark: string; light: string }; usage: string };
const tokensJson = JSON.parse(read("design/tokens.json")) as Tokens & { color: { tokens: Token[] } };
const tokensFile = read("src/styles/tokens.css");
function block(selector: string): Map<string, string> {
  const at = tokensFile.indexOf(`${selector} {`);
  const body = tokensFile.slice(tokensFile.indexOf("{", at) + 1, tokensFile.indexOf("}", at)).replace(/\/\*[\s\S]*?\*\//g, "");
  return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1] ?? "", (m[2] ?? "").trim()]));
}
const THEMES = { dark: block(':root, [data-theme="dark"]'), light: block('[data-theme="light"]') };
type Theme = keyof typeof THEMES;
function colour(theme: Theme, name: string): string {
  const value = (theme === "light" ? THEMES.light.get(`--${name}`) : undefined) ?? THEMES.dark.get(`--${name}`);
  if (value === undefined) throw new Error(`--${name} isn't in tokens.css`);
  const alias = /^var\(--([\w-]+)\)$/.exec(value);
  return alias ? colour(theme, alias[1] ?? "") : value;
}
const rgbaOf = (v: string) => /^rgba\((\d+), (\d+), (\d+), ([\d.]+)\)$/.exec(v)?.slice(1).map(Number);

// ---- the icons: PNGs (8-bit RGBA), icon.ico's PNG entries and icon.icns's parts ----
function png(b: Bytes) {
  let p = 8, w = 0, h = 0;
  const idat: Uint8Array[] = [];
  while (p < b.length) {
    const len = b.readUInt32BE(p), type = b.toString("ascii", p + 4, p + 8);
    if (type === "IHDR") {
      w = b.readUInt32BE(p + 8);
      h = b.readUInt32BE(p + 12);
      if (b[p + 16] !== 8 || b[p + 17] !== 6) throw new Error("not an 8-bit RGBA PNG");
    }
    if (type === "IDAT") idat.push(b.subarray(p + 8, p + 8 + len));
    p += 12 + len;
  }
  const all = new Uint8Array(idat.reduce((n, d) => n + d.length, 0));
  idat.reduce((at, d) => { all.set(d, at); return at + d.length; }, 0);
  const raw = zlib.inflateSync(all) as Uint8Array, stride = w * 4, px = new Uint8Array(h * stride);
  for (let y = 0; y < h; y++) {
    const filter = raw[y * (stride + 1)] ?? 0;
    for (let x = 0; x < stride; x++) {
      const v = raw[y * (stride + 1) + 1 + x] ?? 0, a = x >= 4 ? px[y * stride + x - 4] ?? 0 : 0, up = y ? px[(y - 1) * stride + x] ?? 0 : 0;
      const c = x >= 4 && y ? px[(y - 1) * stride + x - 4] ?? 0 : 0, pa = Math.abs(up - c), pb = Math.abs(a - c), pc = Math.abs(a + up - 2 * c);
      px[y * stride + x] = (v + [0, a, up, (a + up) >> 1, pa <= pb && pa <= pc ? a : pb <= pc ? up : c][filter]!) & 255;
    }
  }
  return { w, h, px, at: (x: number, y: number) => Array.from(px.subarray((y * w + x) * 4, (y * w + x) * 4 + 4)) };
}
function icoEntries(b: Bytes): Map<number, Bytes> {
  const entries = new Map();
  for (let i = 0; i < b.readUInt16LE(4); i++) {
    const o = 6 + 16 * i;
    entries.set(b[o] || 256, b.subarray(b.readUInt32LE(o + 12), b.readUInt32LE(o + 12) + b.readUInt32LE(o + 8)));
  }
  return entries;
}
function icnsParts(b: Bytes): Map<string, Bytes> {
  const parts = new Map();
  for (let p = 8; p < b.length; p += b.readUInt32BE(p + 4)) parts.set(b.toString("ascii", p, p + 4), b.subarray(p + 8, p + b.readUInt32BE(p + 4)));
  return parts;
}
/** is32 / il32: each of R, G and B run-length coded in turn (Apple's PackBits variant). */
function unpackRgb(data: Uint8Array, n: number): number[][] {
  const out: number[] = [];
  for (let i = 0; i < data.length;) {
    const k = data[i++] ?? 0;
    if (k >= 0x80) { const v = data[i++] ?? 0; for (let j = 0; j < k - 0x80 + 3; j++) out.push(v); }
    else for (let j = 0; j <= k; j++) out.push(data[i++] ?? 0);
  }
  return [out.slice(0, n), out.slice(n, 2 * n), out.slice(2 * n, 3 * n)];
}

// ---- path data: absolute M, L, C and Z only, as gen-brand.mjs writes it ----
type Point = readonly [number, number];
/** One subpath: where it starts, and each step with its points (L one, C two handles and its end, Z none). */
type Sub = { start: Point; steps: { op: string; points: Point[] }[] };
function subpaths(d: string): Sub[] {
  const subs: Sub[] = [];
  for (const [, op = "", args = ""] of d.matchAll(/([MLCZ])([^MLCZ]*)/g)) {
    const nums = args.trim() ? args.trim().split(/\s+/).map(Number) : [];
    const points: Point[] = [];
    for (let i = 0; i + 1 < nums.length; i += 2) points.push([nums[i]!, nums[i + 1]!]);
    if (op === "M") subs.push({ start: points[0]!, steps: [] });
    else subs[subs.length - 1]!.steps.push({ op, points });
  }
  return subs;
}
const pointsOf = (d: string): Point[] => subpaths(d).flatMap((s) => [s.start, ...s.steps.flatMap((step) => step.points)]);
/** A subpath as short straight pieces, n to each step. */
function flatten(sub: Sub, n = 40): Point[] {
  const out: Point[] = [sub.start];
  let p = sub.start;
  for (const { op, points } of sub.steps) {
    const to = op === "Z" ? sub.start : points[points.length - 1]!;
    for (let i = 1; i <= n; i++) {
      const t = i / n, u = 1 - t, at = p;
      if (op === "C") {
        const [a, b] = points as [Point, Point, Point];
        out.push([u ** 3 * at[0] + 3 * u * u * t * a[0] + 3 * u * t * t * b[0] + t ** 3 * to[0], u ** 3 * at[1] + 3 * u * u * t * a[1] + 3 * u * t * t * b[1] + t ** 3 * to[1]]);
      } else out.push([at[0] + (to[0] - at[0]) * t, at[1] + (to[1] - at[1]) * t]);
    }
    p = to;
  }
  return out;
}
function distanceTo(p: Point, lines: Point[][]): number {
  let best = Infinity;
  for (const line of lines) for (let i = 1; i < line.length; i++) {
    const a = line[i - 1]!, b = line[i]!, dx = b[0] - a[0], dy = b[1] - a[1], len = dx * dx + dy * dy;
    const t = len ? Math.max(0, Math.min(1, ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / len)) : 0;
    best = Math.min(best, Math.hypot(p[0] - a[0] - t * dx, p[1] - a[1] - t * dy));
  }
  return best;
}
const signedArea = (line: Point[]) => line.reduce((sum, p, i) => { const q = line[(i + 1) % line.length]!; return sum + p[0] * q[1] - q[0] * p[1]; }, 0) / 2;
/** The mark on its 24 grid, moved and scaled as gen-brand.mjs does: its numbers rounded to two decimals. */
function placed(d: string, dx: number, dy: number, s: number): string {
  let i = 0;
  return d.replace(/-?\d*\.?\d+/g, (v) => String(+(i++ % 2 ? +v * s + dy : +v * s + dx).toFixed(2)));
}
/** The outer contour's stems: its outermost x on each side, and how many straight steps run along each (one, when each edge is one line). */
function stemEdges(d: string) {
  const outer = subpaths(d)[0]!;
  const ends: Point[] = [outer.start, ...outer.steps.map((s) => (s.op === "Z" ? outer.start : s.points[s.points.length - 1]!))];
  const ops = outer.steps.map((s) => s.op);
  const all = pointsOf(d), left = Math.min(...all.map((p) => p[0])), right = Math.max(...all.map((p) => p[0]));
  const lines = (x: number) => ops.flatMap((op, i) => (op !== "C" && ends[i]![0] === x && ends[i + 1]![0] === x ? [[ends[i]!, ends[i + 1]!] as const] : []));
  const span = (x: number) => { const ys = lines(x).flat().map((p) => p[1]); return { x, top: Math.min(...ys), bottom: Math.max(...ys) }; };
  return { left: span(left), right: span(right), leftLines: lines(left).length, rightLines: lines(right).length };
}
/**
 * How much a stem's outer edge wobbles in an image, from cap to cap: for each row, the coverage (0 to 1) of the column that
 * holds the edge, against the middle row's value, and the coverage of the column outside it. `size` is the drawing's own width.
 */
function edgeWobble(im: ReturnType<typeof png>, e: ReturnType<typeof stemEdges>, size: number, cover: (x: number, y: number, bgx: number) => number) {
  const sc = im.w / size, top = Math.ceil(e.left.top * sc), bottom = Math.floor(e.left.bottom * sc) - 1;
  const side = (col: number, out: number) => {
    const values: number[] = [];
    let outside = 0;
    for (let y = top; y <= bottom; y++) {
      values.push(cover(col, y, col + 3 * out));
      outside = Math.max(outside, Math.abs(cover(col + out, y, col + 3 * out)));
    }
    const mid = [...values].sort((a, b) => a - b)[values.length >> 1] ?? 0;
    return { wobble: Math.max(...values.map((v) => Math.abs(v - mid))), outside };
  };
  return { left: side(Math.floor(e.left.x * sc + 1e-6), -1), right: side(Math.ceil(e.right.x * sc - 1e-6) - 1, 1) };
}

const BRAND = "design/brand";
// gen-brand.mjs's MARK, on the 24 grid. Change it with that file's numbers (GA-239).
const MARK_D = "M5 3C5 2.448 5.448 2 6 2C6.552 2 7 2.448 7 3L7 7.857C7.256 8.708 7.617 9.55 8.421 10.011C9.371 10.555 10.935 10.6 12 10.6C13.065 10.6 14.629 10.555 15.579 10.011C16.383 9.55 16.744 8.708 17 7.857L17 3C17 2.448 17.448 2 18 2C18.552 2 19 2.448 19 3L19 21C19 21.552 18.552 22 18 22C17.448 22 17 21.552 17 21L17 14.593C15.718 16.125 13.931 16.6 12 16.6C10.069 16.6 8.282 16.125 7 14.593L7 21C7 21.552 6.552 22 6 22C5.448 22 5 21.552 5 21ZM7.748 11.914C8.068 12.745 8.534 13.504 9.305 13.984C10.096 14.476 11.085 14.6 12 14.6C12.915 14.6 13.904 14.476 14.695 13.984C15.466 13.504 15.932 12.745 16.252 11.914C14.988 12.516 13.38 12.6 12 12.6C10.62 12.6 9.012 12.516 7.748 11.914Z";
// The four strokes of 2 it is merged from, the drawing Jefsev approved in GW-11: the stems, the sheer and the keel.
const MARK_STROKES = "M6 3L6 21M18 3L18 21M6 8C6.8 10.8 8.2 11.6 12 11.6C15.8 11.6 17.2 10.8 18 8M6 8C6.4 13.6 8.2 15.6 12 15.6C15.8 15.6 17.6 13.6 18 8";
const AMBER = "#f2a53a", AMBER_LIGHT = "#9e5500";
// GizAi's G (GA-52), the drawing these files replace
const OLD_G = "M75.11 11.56A45 45 0 1 0";

describe("design/brand holds Hersir's mark, logo, favicon and app icon (GA-230)", () => {
  const COLOURS = ["on-dark", "on-light", "white", "black"];
  const FILES = [
    ...COLOURS.flatMap((c) => [`mark-${c}.svg`, `logo-horizontal-${c}.svg`, `logo-stacked-${c}.svg`]),
    "favicon.svg", "favicon-16.svg", "favicon-32.svg", "favicon-48.svg",
    "app-icon-1024.svg", "app-icon-macos-1024.svg", "app-icon-16.svg", "app-icon-32.svg", "tray-template.svg",
  ];

  it("has every drawing the card asks for, each labelled Hersir, none of them GizAi's G", () => {
    for (const f of FILES) {
      const svg = read(`${BRAND}/${f}`);
      expect([f, svg.startsWith("<svg "), svg.includes('aria-label="Hersir"'), svg.includes("<title>Hersir</title>")]).toEqual([f, true, true, true]);
      expect([f, svg.includes(OLD_G), /giz\s*ai/i.test(withoutComments(svg))]).toEqual([f, false, false]);
    }
  });

  it("draws the mark on its 24 grid in the four colours: the brand amber on dark and on light, white and black", () => {
    const ink = { "on-dark": AMBER, "on-light": AMBER_LIGHT, white: "#ffffff", black: "#000000" } as const;
    for (const c of COLOURS) {
      const svg = read(`${BRAND}/mark-${c}.svg`);
      expect([c, svg]).toEqual([c, expect.stringContaining('viewBox="0 0 24 24"')]);
      expect([c, svg]).toEqual([c, expect.stringContaining(`<path fill="${ink[c as keyof typeof ink]}" d="${placed(MARK_D, 0, 0, 1)}"/>`)]);
      // one colour each: no other colour in the file
      expect([c, [...new Set(svg.match(/#[0-9a-f]{6}/gi))]]).toEqual([c, [ink[c as keyof typeof ink]]]);
    }
    // the logos: the mark amber with the name in text colour on dark and on light; one colour for white and black
    for (const kind of ["horizontal", "stacked"]) {
      expect(read(`${BRAND}/logo-${kind}-on-dark.svg`)).toContain(`<path fill="${AMBER}"`);
      expect(read(`${BRAND}/logo-${kind}-on-light.svg`)).toContain(`<path fill="${AMBER_LIGHT}"`);
      for (const [c, hex] of [["white", "#ffffff"], ["black", "#000000"]]) {
        expect([kind, c, [...new Set(read(`${BRAND}/logo-${kind}-${c}.svg`).match(/#[0-9a-f]{6}/gi))]]).toEqual([kind, c, [hex]]);
      }
    }
  });

  it("cuts the favicon at 16, 32 and 48 px and the app icon at 16 and 32 px, the 16 px cut with its filled hull", () => {
    for (const s of [16, 32, 48]) expect(read(`${BRAND}/favicon-${s}.svg`)).toContain(`width="${s}" height="${s}" viewBox="0 0 ${s} ${s}"`);
    for (const s of [16, 32]) expect(read(`${BRAND}/app-icon-${s}.svg`)).toContain(`width="${s}" height="${s}" viewBox="0 0 ${s} ${s}"`);
    // the 16 px cut fills the hull between sheer and keel, so it stays an H with a boat in it: its stems are strokes and
    // its hull one closed shape, where the 32 px drawing is the mark with its hole, one outline without a stroke (GA-239)
    for (const f of ["favicon-16.svg", "app-icon-16.svg"]) {
      const cut = read(`${BRAND}/${f}`);
      expect([f, cut]).toEqual([f, expect.stringContaining(`stroke="${AMBER}" stroke-width="2" stroke-linecap="round" d="M4 2.5L4 13.5M12 2.5L12 13.5"`)]);
      expect([f, cut]).toEqual([f, expect.stringContaining(`<path fill="${AMBER}" stroke="${AMBER}" stroke-width="0.6" stroke-linejoin="round" d="M4 5C4.4 9.6 5.6 10.8 8 10.8C10.4 10.8 11.6 9.6 12 5C11.4 6.9 10.4 7.6 8 7.6C5.6 7.6 4.6 6.9 4 5Z"/>`)]);
    }
    expect(read(`${BRAND}/favicon-32.svg`)).toContain(`<path fill="${AMBER}" d="${placed(MARK_D, 4, 4, 1)}"/>`);
    expect(read(`${BRAND}/favicon-32.svg`)).not.toContain(`stroke="${AMBER}"`);
  });

  it("makes the 1024 app icon for Linux and Windows and macOS's 824 px rounded square with its shadow", () => {
    expect(read(`${BRAND}/app-icon-1024.svg`)).toContain('viewBox="0 0 1024 1024"');
    const mac = read(`${BRAND}/app-icon-macos-1024.svg`);
    expect(mac).toContain('viewBox="0 0 1024 1024"');
    expect(mac).toContain('filter="url(#hs-shadow)"');
    // Apple's grid: the square runs from 100 to 924, 824 wide
    expect(mac).toMatch(/d="M100 [\d.]+C100 [\s\S]*L924 [\d.]+/);
    expect(mac).toContain(`<path fill="${AMBER}"`);
  });

  it("README describes every file, the colours and the cuts, and names the Figma frames", () => {
    const readme = read(`${BRAND}/README.md`);
    expect(readme).toMatch(/^# Hersir brand \(GA-230\)/);
    for (const name of ["mark-*.svg", "logo-horizontal-*.svg", "logo-stacked-*.svg", "favicon.svg", "favicon-16.svg", "app-icon-1024.svg", "app-icon-macos-1024.svg", "app-icon-16.svg", "tray-template.svg", "gen-brand.mjs", "make-icons.mjs"]) {
      expect([name, readme.includes(`\`${name}\``)]).toEqual([name, true]);
    }
    for (const id of ["198:258", "198:263", "184:2", "236:384", "246:528"]) expect([id, readme.includes(id)]).toEqual([id, true]);
    expect(readme).toContain(AMBER);
    expect(readme).toContain(AMBER_LIGHT);
    // GA-239: it says the mark is one outline, and keeps the strokes it is merged from
    expect(readme).toContain("merged into one filled outline");
    expect(readme).toContain(MARK_STROKES);
  });
});

describe("the longboat H is one filled outline, so each stem is one straight line (GA-239)", () => {
  const big = (at: number, k: number) => ({ dx: at + 172 * k, dy: at + 172 * k, s: (680 * k) / 24 });
  const LOGOS = ["on-dark", "on-light", "white", "black"].flatMap((c) => [`${BRAND}/logo-horizontal-${c}.svg`, `${BRAND}/logo-stacked-${c}.svg`]);
  // Each drawing of the mark and where its 24 grid is. The logos' first path is the mark; in the others it is the last.
  const DRAWINGS = new Map<string, { dx: number; dy: number; s: number }>([
    ...["on-dark", "on-light", "white", "black"].map((c) => [`${BRAND}/mark-${c}.svg`, { dx: 0, dy: 0, s: 1 }] as const),
    ...LOGOS.map((f) => [f, f.includes("stacked") ? { dx: 12.84, dy: 0, s: 88 / 24 } : { dx: 0, dy: 0, s: 2 }] as const),
    ["src/assets/gizai-logo.svg", { dx: 0, dy: 0, s: 2 }],
    ...["favicon.svg", "favicon-32.svg", "app-icon-32.svg"].map((f) => [`${BRAND}/${f}`, { dx: 4, dy: 4, s: 1 }] as const),
    [`${BRAND}/app-icon-1024.svg`, big(64, 896 / 1024)],
    ["src-tauri/icons/gizai.svg", big(64, 896 / 1024)],
    [`${BRAND}/app-icon-macos-1024.svg`, big(100, 824 / 1024)],
  ]);
  // The 48 px cut and the tray: the mark at 1.5 with the stems on whole pixels, so its grid isn't scaled evenly.
  const CUTS = new Map([[`${BRAND}/favicon-48.svg`, { left: 13, right: 35, top: 10.5, bottom: 37.5 }], [`${BRAND}/tray-template.svg`, { left: 7, right: 29, top: 4.5, bottom: 31.5 }]]);
  const ALL = [...DRAWINGS.keys(), ...CUTS.keys()];
  const markOf = (f: string) => {
    const paths = [...read(f).matchAll(/<path [^>]*\/>/g)].map((m) => m[0]);
    const path = f.includes("logo") ? paths[0] : paths[paths.length - 1];
    return { path: path ?? "", d: / d="([^"]+)"/.exec(path ?? "")?.[1] ?? "" };
  };

  it("gen-brand.mjs's MARK is the outline these tests pin", () => {
    expect(read(`${BRAND}/gen-brand.mjs`)).toContain(`const MARK = "${MARK_D}";`);
  });

  it("is the outline of the same four strokes: every point of it lies 1 from their centre lines, and none of them is nearer than 1 to its edge", () => {
    const outline = subpaths(MARK_D).map((s) => flatten(s));
    const strokes = subpaths(MARK_STROKES).map((s) => flatten(s));
    for (const sub of outline) for (const p of sub) expect([p, Math.abs(distanceTo(p, strokes) - 1) < 0.005]).toEqual([p, true]);
    // so the weight is 2 all along, and the stems (x 6 and 18, y 3 to 21) and the sag (11.6 and 15.6) are where they were
    for (const line of strokes) for (const p of line) expect([p, distanceTo(p, outline) > 0.995]).toEqual([p, true]);
    const all = pointsOf(MARK_D);
    expect([Math.min(...all.map((p) => p[0])), Math.max(...all.map((p) => p[0])), Math.min(...all.map((p) => p[1])), Math.max(...all.map((p) => p[1]))]).toEqual([5, 19, 2, 22]);
  });

  it("keeps the hole between the sheer and the keel: two closed shapes wound opposite ways, as a nonzero fill needs", () => {
    const subs = subpaths(MARK_D);
    expect(subs).toHaveLength(2);
    const [outer, hole] = subs.map((s) => signedArea(flatten(s))) as [number, number];
    expect(outer * hole).toBeLessThan(0);
    // the stems' 2 × 20 and the hull's sag between them, less the hole
    expect([Math.round(Math.abs(outer)), Math.round(Math.abs(hole))]).toEqual([138, 13]);
  });

  it("every drawing is that outline at its place, filled, with no stroke left on the mark", () => {
    for (const [f, at] of DRAWINGS) {
      const { path, d } = markOf(f);
      expect([f, d]).toEqual([f, placed(MARK_D, at.dx, at.dy, at.s)]);
      expect([f, path.includes("stroke")]).toEqual([f, false]);
    }
    for (const f of CUTS.keys()) expect([f, markOf(f).path.includes("stroke")]).toEqual([f, false]);
    expect(markOf("src/assets/gizai-logo.svg").path).toContain('fill="currentColor"');
    expect(markOf(`${BRAND}/tray-template.svg`).path).toContain('fill="#000000"');
  });

  it("in every drawing each stem's outer edge is one straight line from cap to cap, the same height on both sides", () => {
    for (const f of ALL) {
      const e = stemEdges(markOf(f).d);
      expect([f, e.leftLines]).toEqual([f, 1]);
      expect([f, e.rightLines]).toEqual([f, 1]);
      expect([f, e.left.top, e.left.bottom]).toEqual([f, e.right.top, e.right.bottom]);
      const at = DRAWINGS.get(f);
      // 18 of the grid long, rounded to the files' two decimals
      if (at) expect([f, Math.abs(e.left.bottom - e.left.top - 18 * at.s) < 0.011]).toEqual([f, true]);
    }
    // the 48 px cut and the tray keep their stems on whole pixels, 3 wide and 19 apart
    for (const [f, want] of CUTS) {
      const e = stemEdges(markOf(f).d);
      expect([f, e.left.x, e.right.x, e.left.top, e.left.bottom]).toEqual([f, want.left, want.right, want.top, want.bottom]);
      expect([f, pointsOf(markOf(f).d).some(([x]) => x === want.left + 3), pointsOf(markOf(f).d).some(([x]) => x === want.right - 3)]).toEqual([f, true, true]);
    }
  });

  it("in every drawing left and right mirror each other, point by point", () => {
    for (const f of ALL) {
      const { d } = markOf(f), points = pointsOf(d), e = stemEdges(d), twice = e.left.x + e.right.x;
      // within the files' rounding to two decimals
      for (const [x, y] of points) expect([f, x, y, points.some(([u, v]) => Math.abs(u - (twice - x)) < 0.0101 && Math.abs(v - y) < 0.0101)]).toEqual([f, x, y, true]);
    }
  });

  describe("pixel by pixel, as resvg made the icons", () => {
    const ICONS = "src-tauri/icons";
    const master = stemEdges(markOf("src-tauri/icons/gizai.svg").d), mac = stemEdges(markOf(`${BRAND}/app-icon-macos-1024.svg`).d);
    const cut32 = stemEdges(markOf(`${BRAND}/app-icon-32.svg`).d), tray = stemEdges(markOf(`${BRAND}/tray-template.svg`).d);
    const PNGS: [string, number][] = [["icon.png", 512], ["128x128@2x.png", 256], ["128x128.png", 128], ["64x64.png", 64], ["StoreLogo.png", 50], ["Square30x30Logo.png", 30], ["Square44x44Logo.png", 44], ["Square71x71Logo.png", 71], ["Square89x89Logo.png", 89], ["Square107x107Logo.png", 107], ["Square142x142Logo.png", 142], ["Square150x150Logo.png", 150], ["Square284x284Logo.png", 284], ["Square310x310Logo.png", 310]];
    type Image = [string, ReturnType<typeof png>, ReturnType<typeof stemEdges>, number];
    // Decoded once: the two tests below read the same 26 images, the largest 1024 px.
    let decoded: Image[] | undefined;
    const IMAGES = (): Image[] => {
      if (decoded) return decoded;
      const ico = icoEntries(bytes(`${ICONS}/icon.ico`)), icns = icnsParts(bytes(`${ICONS}/icon.icns`));
      decoded = [
        ...PNGS.map(([f, size]): Image => {
          const im = png(bytes(`${ICONS}/${f}`));
          expect([f, im.w, im.h]).toEqual([f, size, size]);
          return [f, im, master, 1024];
        }),
        ["32x32.png", png(bytes(`${ICONS}/32x32.png`)), cut32, 32],
        ...[24, 48, 64, 256].map((s): Image => [`icon.ico ${s}`, png(ico.get(s)), master, 1024]),
        ...["ic07", "ic08", "ic09", "ic10", "ic12", "ic13", "ic14"].map((t): Image => [`icon.icns ${t}`, png(icns.get(t)), mac, 1024]),
      ];
      return decoded;
    };

    it("no row of a stem's outer edge differs from the others, and nothing lies outside it, in any PNG, icon.ico or icon.icns", () => {
      for (const [f, im, e, size] of IMAGES()) {
        const got = edgeWobble(im, e, size, (x, y, bgx) => { const bg = im.at(bgx, y)[0]!; return (im.at(x, y)[0]! - bg) / (0xf2 - bg); });
        expect([f, got.left.wobble < 0.015, got.left.outside < 0.015, got.right.wobble < 0.015, got.right.outside < 0.015]).toEqual([f, true, true, true, true]);
      }
    }, 30_000);

    it("the tray's template too, by its see-through", () => {
      const im = png(bytes(`${ICONS}/tray-template.png`));
      const got = edgeWobble(im, tray, 36, (x, y) => im.at(x, y)[3]! / 255);
      // its edges are on whole pixels: the column inside is solid, the one outside empty, all the way down
      expect(got).toEqual({ left: { wobble: 0, outside: 0 }, right: { wobble: 0, outside: 0 } });
    });

    it("left and right mirror each other in each of them, within resvg's steps of coverage", () => {
      const bg = rgbOf("#0b0e11");
      for (const [f, { w, h, px }] of [...IMAGES().map(([f, im]) => [f, im] as const), ["tray-template.png", png(bytes(`${ICONS}/tray-template.png`))] as const]) {
        let worst = 0;
        for (let y = 0; y < h; y++) for (let x = 0; x < w >> 1; x++) {
          const p = (y * w + x) * 4, q = (y * w + w - 1 - x) * 4, a = px[p + 3]! / 255, b = px[q + 3]! / 255;
          // on the tile's bottom colour, so see-through pixels' own colours don't count
          for (let k = 0; k < 3; k++) worst = Math.max(worst, Math.abs(px[p + k]! * a + bg[k as 0 | 1 | 2] * (1 - a) - px[q + k]! * b - bg[k as 0 | 1 | 2] * (1 - b)));
        }
        expect([f, worst <= 64]).toEqual([f, true]);
      }
    }, 30_000);
  });
});

describe("src-tauri/icons show the longboat H, with the pixel-grid cuts at 16 and 32 px (GA-230)", () => {
  const ICONS = "src-tauri/icons";
  const icon = (f: string) => bytes(`${ICONS}/${f}`);

  it("keeps every file name and tauri.conf.json's bundle list", () => {
    const names = (fs.readdirSync(url(ICONS)) as string[]).sort();
    expect(names).toEqual([
      "128x128.png", "128x128@2x.png", "32x32.png", "64x64.png", "Square107x107Logo.png", "Square142x142Logo.png", "Square150x150Logo.png",
      "Square284x284Logo.png", "Square30x30Logo.png", "Square310x310Logo.png", "Square44x44Logo.png", "Square71x71Logo.png", "Square89x89Logo.png",
      "StoreLogo.png", "gizai.svg", "icon.icns", "icon.ico", "icon.png", "tray-template.png",
    ]);
    const conf = JSON.parse(read("src-tauri/tauri.conf.json")) as { bundle: { icon: string[] } };
    expect(conf.bundle.icon).toEqual(["icons/32x32.png", "icons/128x128.png", "icons/128x128@2x.png", "icons/icon.icns", "icons/icon.ico"]);
  });

  it("gizai.svg is the app icon's master (design/brand/app-icon-1024.svg), the file install.sh copies", () => {
    expect(withoutComments(read(`${ICONS}/gizai.svg`))).toBe(withoutComments(read(`${BRAND}/app-icon-1024.svg`)));
    expect(read("install.sh")).toContain('"$SRC/src-tauri/icons/gizai.svg"');
  });

  it("draws the amber mark on the iron tile in every PNG, with icon theme's margin around the scaled master", () => {
    for (const [f, size] of [["icon.png", 512], ["128x128@2x.png", 256], ["128x128.png", 128], ["64x64.png", 64], ["Square310x310Logo.png", 310], ["Square44x44Logo.png", 44]] as const) {
      const im = png(icon(f)), mid = size >> 1;
      expect([f, im.w, im.h]).toEqual([f, size, size]);
      // the middle of the H's crossbar is the brand amber, the tile's top edge iron, the corner see-through
      let amber = 0;
      for (let i = 0; i < size * size; i++) if (im.px[i * 4] === 0xf2 && im.px[i * 4 + 1] === 0xa5 && im.px[i * 4 + 2] === 0x3a && im.px[i * 4 + 3] === 255) amber++;
      expect([f, amber > size * size * 0.03]).toEqual([f, true]);
      expect([f, im.at(0, 0)[3]]).toEqual([f, 0]);
      const [r, g, b, a] = im.at(mid, Math.round(size * 0.1));
      expect([f, a, Math.max(r!, g!, b!) < 60]).toEqual([f, 255, true]);
    }
    // the master leaves a margin (an 896 tile in 1024): the top middle of 64x64.png is see-through
    expect(png(icon("64x64.png")).at(32, 0)[3]).toBe(0);
  });

  it("uses the 32 px cut in 32x32.png, icon.ico and icon.icns, and the 16 px cut in both", () => {
    const cut32 = icon("32x32.png"), im32 = png(cut32);
    expect([im32.w, im32.h]).toEqual([32, 32]);
    // a cut fills its square, as small icons do: its top middle is the tile, where the scaled master has its margin
    expect(im32.at(16, 0)[3]).toBe(255);
    const ico = icoEntries(icon("icon.ico"));
    expect([...ico.keys()].sort((a, b) => a - b)).toEqual([16, 24, 32, 48, 64, 256]);
    expect(ico.get(32).equals(cut32)).toBe(true);
    const ico16 = png(ico.get(16));
    expect([ico16.w, ico16.h, ico16.at(8, 0)[3]]).toEqual([16, 16, 255]);
    // the 24 px entry is the master scaled, with its margin
    expect(png(ico.get(24)).at(12, 0)[3]).toBe(0);

    const icns = icnsParts(icon("icon.icns"));
    expect(icns.get("ic11").equals(cut32)).toBe(true);
    // il32 + l8mk (32 px) and is32 + s8mk (16 px), RGB run-length coded and the alpha raw, are the same cuts
    for (const [rgb, mask, im] of [["il32", "l8mk", im32], ["is32", "s8mk", ico16]] as const) {
      const n = im.w * im.h, [R, G, B] = unpackRgb(icns.get(rgb), n), alpha = Array.from(icns.get(mask) as Uint8Array);
      const want = (k: number) => Array.from({ length: n }, (_, i) => im.px[i * 4 + k]);
      expect([rgb, R, G, B]).toEqual([rgb, want(0), want(1), want(2)]);
      expect([mask, alpha]).toEqual([mask, want(3)]);
    }
  });

  it("tray-template.png is the mark for macOS's menu bar: 36 px, black on transparent", () => {
    const t = png(icon("tray-template.png"));
    expect([t.w, t.h]).toEqual([36, 36]);
    let drawn = 0, clear = 0;
    for (let i = 0; i < t.w * t.h; i++) {
      const [r, g, b, a] = Array.from(t.px.subarray(i * 4, i * 4 + 4));
      if (a) expect([i, r, g, b]).toEqual([i, 0, 0, 0]);
      if (a! > 200) drawn++;
      if (!a) clear++;
    }
    expect(drawn).toBeGreaterThan(150);
    expect(clear).toBeGreaterThan(t.w * t.h * 0.6);
    // the H's two stems (x 8.5 and 27.5 at 36 px) and the gap between them
    expect([t.at(8, 18)[3], t.at(27, 18)[3], t.at(18, 30)[3]]).toEqual([255, 255, 0]);
  });
});

describe("the onboarding shows the Hersir logo (GA-230)", () => {
  it("is the horizontal logo's drawing in one colour, currentColor, at 24 px high", () => {
    const logo = read("src/assets/gizai-logo.svg"), brand = read(`${BRAND}/logo-horizontal-black.svg`);
    expect(logo).not.toContain(OLD_G);
    expect(logo).toContain('width="162.32" height="48" viewBox="0 0 162.32 48"');
    expect(logo.match(/#[0-9a-f]{3,6}\b/gi)).toBeNull();
    // the same paths as the brand's black logo, coloured currentColor
    const paths = (svg: string) => [...svg.matchAll(/ d="([^"]+)"/g)].map((m) => m[1]);
    expect(paths(logo)).toEqual(paths(brand));
    expect(logo.match(/(stroke|fill)="currentColor"/g)).toHaveLength(2);
    expect(read("src/styles/onboarding.css")).toContain(".onb-logo svg { display: block; width: 81.16px; height: 24px; }");
    expect(read("src/pages/OnboardingPage.tsx")).toContain('import logo from "../assets/gizai-logo.svg?raw";');
  });
});

describe("gold is needs you (GA-230)", () => {
  const STATUSES = ["st-backlog", "st-todo", "st-progress", "st-testing", "st-done", "st-blocked", "st-cancelled", "success", "warning", "danger", "live"];

  it("tokens.css is made from tokens.json", () => {
    expect(tokensFile).toBe(tokensCss(tokensJson));
  });

  it("needs is Hersir colour's amber: the brand's in dark, a ΔE2000 under 3 from it in light", () => {
    expect(colour("dark", "needs")).toBe(AMBER);
    expect(deltaE(colour("light", "needs"), AMBER_LIGHT)).toBeLessThan(3);
    for (const theme of ["dark", "light"] as const) {
      const h = oklch(colour(theme, "needs")).H;
      expect([theme, h > 55 && h < 80]).toEqual([theme, true]);
    }
    expect(colour("dark", "st-review")).toBe(colour("dark", "needs"));
    expect(colour("light", "st-review")).toBe(colour("light", "needs"));
  });

  it("needs-soft is needs' colour at its alpha (0.12 in dark, 0.08 in light)", () => {
    for (const [theme, alpha] of [["dark", 0.12], ["light", 0.08]] as const) {
      expect([theme, rgbaOf(colour(theme, "needs-soft"))]).toEqual([theme, [...rgbOf(colour(theme, "needs")), alpha]]);
    }
  });

  it("needs reads as text at 4.5:1 or more on bg and raised, in dark and light", () => {
    for (const theme of ["dark", "light"] as const) for (const surface of ["bg", "raised"]) {
      const c = contrast(colour(theme, "needs"), colour(theme, surface));
      expect([theme, surface, c >= 4.5]).toEqual([theme, surface, true]);
    }
  });

  it("every status glyph and badge colour is at least 35° of OKLCH hue and a ΔE2000 of 15 from needs", () => {
    for (const theme of ["dark", "light"] as const) {
      const needs = colour(theme, "needs");
      for (const name of STATUSES) {
        const c = colour(theme, name);
        expect([theme, name, c, hueGap(c, needs) >= 35, deltaE(c, needs) >= 15]).toEqual([theme, name, c, true, true]);
      }
    }
  });

  it("In progress and warning moved off the old amber to one yellow, readable as text", () => {
    for (const theme of ["dark", "light"] as const) {
      expect(colour(theme, "st-progress")).toBe(colour(theme, "warning"));
      expect([theme, rgbaOf(colour(theme, "warning-soft"))?.slice(0, 3)]).toEqual([theme, [...rgbOf(colour(theme, "warning"))]]);
      for (const surface of ["bg", "raised"]) expect([theme, surface, contrast(colour(theme, "warning"), colour(theme, surface)) >= 4.5]).toEqual([theme, surface, true]);
    }
    expect([colour("dark", "warning"), colour("light", "warning")]).not.toEqual(["#f0b429", "#a15c07"]);
    expect([colour("dark", "st-progress"), colour("light", "st-progress")]).not.toEqual(["#f0b429", "#b45309"]);
  });

  it("the accent is still blue: buttons, links and focus keep their colours", () => {
    const BEFORE: Record<string, [string, string]> = {
      accent: ["#6f97ff", "#2f5bdb"], "accent-hover": ["#8aabff", "#2449b8"], "accent-soft": ["rgba(111, 151, 255, 0.14)", "rgba(47, 91, 219, 0.10)"],
      "on-accent": ["#0b1636", "#ffffff"], focus: ["#8aabff", "#2f5bdb"],
    };
    for (const [name, [dark, light]] of Object.entries(BEFORE)) expect([name, colour("dark", name), colour("light", name)]).toEqual([name, dark, light]);
    for (const theme of ["dark", "light"] as const) {
      const h = oklch(colour(theme, "accent")).H;
      expect([theme, h > 240 && h < 280]).toEqual([theme, true]);
      expect([theme, hueGap(colour(theme, "accent"), colour(theme, "needs")) >= 35]).toEqual([theme, true]);
    }
  });

  it("the terminal's cyan and magenta keep 35° of hue and a ΔE2000 of 15 from live and the gold needs, as GA-132 set", () => {
    for (const theme of ["dark", "light"] as const) for (const name of ["term-cyan", "term-bright-cyan", "term-magenta", "term-bright-magenta"]) {
      for (const state of ["live", "needs"]) {
        const a = colour(theme, name), b = colour(theme, state);
        expect([theme, name, state, hueGap(a, b) >= 35, deltaE(a, b) >= 15]).toEqual([theme, name, state, true, true]);
      }
    }
    expect(read("design/terminal/gen-terminal.mjs")).not.toMatch(/needs \(magenta/);
  });
});
