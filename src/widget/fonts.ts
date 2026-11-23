// GA-343: the board view's fonts, Atkinson Hyperlegible Next and Mono, Latin only: one woff2 file each (a variable font,
// every weight in it), inlined in the view's one file. They are added with the FontFace API from their bytes, not with
// an @font-face url(): a host's Content Security Policy for a view may leave out font-src, which would refuse a data:
// URL, and bytes are no URL at all. Nothing is fetched.
import nextFont from "../assets/fonts/AtkinsonHyperlegibleNext-400-latin.woff2?inline";
import monoFont from "../assets/fonts/AtkinsonHyperlegibleMono-400-latin.woff2?inline";

/** The Latin range of the app's own @font-face rules (src/styles/fonts.css). */
const LATIN = "U+0000-00FF, U+0131, U+0152-0153, U+02BB-02BC, U+02C6, U+02DA, U+02DC, U+0304, U+0308, U+0329, U+2000-206F, U+20AC, U+2122, U+2191, U+2193, U+2212, U+2215, U+FEFF, U+FFFD";

/** A data: URL's bytes. */
function bytes(dataUrl: string): ArrayBuffer {
  const text = atob(dataUrl.slice(dataUrl.indexOf(",") + 1));
  const out = new Uint8Array(text.length);
  for (let i = 0; i < text.length; i++) out[i] = text.charCodeAt(i);
  return out.buffer;
}

/** Adds the two fonts to the document; a font that can't be added leaves the system's sans-serif in its place. */
export function loadFonts(doc: Document = document) {
  for (const [family, url, weight] of [["Atkinson Hyperlegible Next", nextFont, "200 800"], ["Atkinson Hyperlegible Mono", monoFont, "200 800"]] as const) {
    try {
      const face = new FontFace(family, bytes(url), { weight, style: "normal", unicodeRange: LATIN });
      doc.fonts.add(face);
      face.load().catch(() => {});
    } catch {
      // no FontFace here, or bytes it can't read: the fallback font draws the board
    }
  }
}
