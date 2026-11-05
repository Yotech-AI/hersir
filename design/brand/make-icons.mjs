// Hersir's app icons (GA-230): every file in src-tauri/icons again, from design/brand/'s SVGs (node design/brand/gen-brand.mjs
// first). `npx tauri icon` renders them: the Linux and Windows master (app-icon-1024.svg) gives the PNGs and icon.ico,
// the macOS master (app-icon-macos-1024.svg) icon.icns. Then the 16 and 32 px images in all of them are the pixel-grid
// cuts (app-icon-16.svg and -32.svg) instead of the master scaled down, and tray-template.png is tray-template.svg at
// 36 px. The file names stay: tauri.conf.json's bundle list, install.sh and src-tauri/src/lib.rs name them.
// Run it with: node design/brand/make-icons.mjs
import { execFileSync } from "node:child_process";
import { copyFileSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { inflateSync } from "node:zlib";

const DIR = dirname(fileURLToPath(import.meta.url));
const ROOT = join(DIR, "../..");
const ICONS = join(ROOT, "src-tauri/icons");
const TMP = mkdtempSync(join(tmpdir(), "hersir-icons-"));
const render = (svg, out, sizes = []) =>
  execFileSync("npx", ["tauri", "icon", join(DIR, svg), "-o", join(TMP, out), ...sizes.flatMap((s) => ["-p", String(s)])], { cwd: ROOT, stdio: "ignore" });
render("app-icon-1024.svg", "all");
render("app-icon-macos-1024.svg", "mac");
render("app-icon-16.svg", "cut16", [16]);
render("app-icon-32.svg", "cut32", [32]);
render("tray-template.svg", "tray", [36]);
const cut = { 16: readFileSync(join(TMP, "cut16/16x16.png")), 32: readFileSync(join(TMP, "cut32/32x32.png")) };

// The PNGs, from the master, but 32x32.png is the 32 px cut.
const PNGS = ["128x128.png", "128x128@2x.png", "64x64.png", "icon.png", "StoreLogo.png", "Square30x30Logo.png", "Square44x44Logo.png",
  "Square71x71Logo.png", "Square89x89Logo.png", "Square107x107Logo.png", "Square142x142Logo.png", "Square150x150Logo.png",
  "Square284x284Logo.png", "Square310x310Logo.png"];
for (const name of PNGS) copyFileSync(join(TMP, "all", name), join(ICONS, name));
writeFileSync(join(ICONS, "32x32.png"), cut[32]);
copyFileSync(join(TMP, "tray/36x36.png"), join(ICONS, "tray-template.png"));

// icon.ico: Tauri's (16, 24, 32, 48, 64 and 256 px, each a PNG), with the cuts at 16 and 32.
function ico(file) {
  const b = readFileSync(file), count = b.readUInt16LE(4);
  const entries = Array.from({ length: count }, (_, i) => {
    const o = 6 + 16 * i, size = b[o] || 256, at = b.readUInt32LE(o + 12);
    return { size, png: cut[size] ?? b.subarray(at, at + b.readUInt32LE(o + 8)) };
  });
  const head = Buffer.alloc(6 + 16 * count);
  head.writeUInt16LE(1, 2);
  head.writeUInt16LE(count, 4);
  let at = head.length;
  entries.forEach((e, i) => {
    const o = 6 + 16 * i;
    head[o] = e.size % 256;
    head[o + 1] = e.size % 256;
    head.writeUInt16LE(1, o + 4);
    head.writeUInt16LE(32, o + 6);
    head.writeUInt32LE(e.png.length, o + 8);
    head.writeUInt32LE(at, o + 12);
    at += e.png.length;
  });
  return Buffer.concat([head, ...entries.map((e) => e.png)]);
}
writeFileSync(join(ICONS, "icon.ico"), ico(join(TMP, "all/icon.ico")));

// icon.icns: Tauri's from the macOS master. Its 16 px (is32 and its mask s8mk) and 32 px (il32 and l8mk, and ic11, the
// 16 pt @2x PNG) become the cuts. is32 and il32 are RGB, each channel run-length coded; the masks are the alpha, raw.
function rgba(png) {
  let p = 8, w = 0, h = 0;
  const idat = [];
  while (p < png.length) {
    const len = png.readUInt32BE(p), type = png.toString("ascii", p + 4, p + 8);
    if (type === "IHDR") {
      w = png.readUInt32BE(p + 8);
      h = png.readUInt32BE(p + 12);
      if (png[p + 16] !== 8 || png[p + 17] !== 6 || png[p + 20] !== 0) throw new Error("expected an 8-bit RGBA PNG, not interlaced");
    }
    if (type === "IDAT") idat.push(png.subarray(p + 8, p + 8 + len));
    p += 12 + len;
  }
  const raw = inflateSync(Buffer.concat(idat)), stride = w * 4, out = Buffer.alloc(h * stride);
  for (let y = 0; y < h; y++) {
    const filter = raw[y * (stride + 1)];
    for (let x = 0; x < stride; x++) {
      const v = raw[y * (stride + 1) + 1 + x], a = x >= 4 ? out[y * stride + x - 4] : 0, b = y ? out[(y - 1) * stride + x] : 0;
      const c = x >= 4 && y ? out[(y - 1) * stride + x - 4] : 0, pa = Math.abs(b - c), pb = Math.abs(a - c), pc = Math.abs(a + b - 2 * c);
      out[y * stride + x] = (v + [0, a, b, (a + b) >> 1, pa <= pb && pa <= pc ? a : pb <= pc ? b : c][filter]) & 255;
    }
  }
  return { w, h, px: out };
}
function packBits(bytes) {
  const out = [];
  for (let i = 0; i < bytes.length;) {
    let run = 1;
    while (i + run < bytes.length && run < 130 && bytes[i + run] === bytes[i]) run++;
    if (run >= 3) { out.push(0x80 + run - 3, bytes[i]); i += run; continue; }
    let j = i;
    const lit = [];
    while (j < bytes.length && lit.length < 128 && !(j + 2 < bytes.length && bytes[j] === bytes[j + 1] && bytes[j] === bytes[j + 2])) lit.push(bytes[j++]);
    out.push(lit.length - 1, ...lit);
    i = j;
  }
  return Buffer.from(out);
}
function legacy(png) {
  const { w, h, px } = rgba(png), n = w * h;
  const channel = (k) => Array.from({ length: n }, (_, i) => px[i * 4 + k]);
  return { rgb: Buffer.concat([0, 1, 2].map((k) => packBits(channel(k)))), mask: Buffer.from(channel(3)) };
}
function icns(file) {
  const b = readFileSync(file), small = legacy(cut[16]), large = legacy(cut[32]);
  const swap = { is32: small.rgb, s8mk: small.mask, il32: large.rgb, l8mk: large.mask, ic11: cut[32] };
  const parts = [];
  for (let p = 8; p < b.length;) {
    const type = b.toString("ascii", p, p + 4), len = b.readUInt32BE(p + 4);
    const data = swap[type] ?? b.subarray(p + 8, p + len), head = Buffer.alloc(8);
    head.write(type, 0, "ascii");
    head.writeUInt32BE(data.length + 8, 4);
    parts.push(head, data);
    p += len;
  }
  const head = Buffer.alloc(8);
  head.write("icns", 0, "ascii");
  head.writeUInt32BE(8 + parts.reduce((s, x) => s + x.length, 0), 4);
  return Buffer.concat([head, ...parts]);
}
writeFileSync(join(ICONS, "icon.icns"), icns(join(TMP, "mac/icon.icns")));
rmSync(TMP, { recursive: true, force: true });
console.log(`wrote src-tauri/icons: ${PNGS.length + 2} PNGs, icon.ico and icon.icns (16 and 32 px: the pixel-grid cuts)`);
