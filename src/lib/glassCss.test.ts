// GA-292 QA: the glass behind the islands, in the window's settings and the styles (design/islands/README.md → The glass, and
// How it is built). The main window is transparent and asks for vibrancy (macOS) and Mica (Windows 11) when it is made, with
// macOSPrivateApi and the tauri feature it needs, and the right to change its effects; the UI asks glass_support which blur the
// system gives. With html.glass the page is see-through and the body paints the notes' gradient at 92% in dark and 86% in
// light; without it, the background is GA-290's opaque gradient. The islands and what floats stay opaque, and text on the
// glass keeps AA over a white and a black desktop (the notes' worst cases). Files are read from disk (Vitest hands .css back
// empty); node:fs comes in through a computed specifier, as in islands.test.ts.
import { describe, expect, it } from "vitest";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const islandsMd = read("design/islands/README.md");
const tokensFile = read("src/styles/tokens.css");
const STYLES = ["app.css", "components.css", "terminal.css", "changes.css", "appearance.css", "screenshots.css", "onboarding.css"];
const css = Object.fromEntries(STYLES.map((f) => [f, read(`src/styles/${f}`)]));

/** The style rules of a stylesheet: each selector of a list on its own, with its declarations. */
function rules(text: string): { selector: string; decl: Map<string, string> }[] {
  const out: { selector: string; decl: Map<string, string> }[] = [];
  const flat = text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/@media[^{]*\{([\s\S]*?\})\s*\}/g, "$1");
  for (const m of flat.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const decl = new Map((m[2] ?? "").split(";").map((d) => d.split(":")).filter((d) => d.length > 1)
      .map(([k, ...v]) => [(k ?? "").trim(), v.join(":").trim().replace(/\s+/g, " ")] as [string, string]));
    for (const s of (m[1] ?? "").split(",")) out.push({ selector: s.trim().replace(/\s+/g, " "), decl });
  }
  return out;
}
const all = STYLES.flatMap((f) => rules(css[f] ?? "").map((r) => ({ ...r, file: f })));
/** The declarations of the rules for exactly `selector` in `file`, merged in order. */
function decl(file: string, selector: string): Map<string, string> {
  const found = rules(css[file] ?? "").filter((r) => r.selector === selector);
  if (!found.length) throw new Error(`no ${selector} in ${file}`);
  return new Map(found.flatMap((r) => [...r.decl]));
}

/** `--name: value;` declarations of the first block whose selector is exactly `selector`. */
function block(text: string, selector: string): Map<string, string> {
  const at = text.indexOf(`${selector} {`);
  if (at < 0) throw new Error(`no ${selector} block`);
  const body = text.slice(text.indexOf("{", at) + 1, text.indexOf("}", at)).replace(/\/\*[\s\S]*?\*\//g, "");
  return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1] ?? "", (m[2] ?? "").trim()]));
}
const themes = { dark: block(tokensFile, ':root, [data-theme="dark"]'), light: block(tokensFile, '[data-theme="light"]') };
type Theme = keyof typeof themes;
/** A colour's value in a theme, aliases followed (light falls back to dark's). */
function colour(theme: Theme, name: string): string {
  const value = (theme === "light" ? themes.light.get(name) : undefined) ?? themes.dark.get(name);
  if (value === undefined) throw new Error(`${name} isn't in tokens.css`);
  const alias = /^var\((--[\w-]+)\)$/.exec(value);
  return alias ? colour(theme, alias[1] ?? "") : value;
}
type RGBA = [number, number, number, number];
function rgba(value: string): RGBA {
  const hex = /^#([0-9a-f]{6})$/i.exec(value);
  if (hex) return [0, 2, 4].map((i) => parseInt((hex[1] ?? "").slice(i, i + 2), 16)).concat(1) as RGBA;
  const fn = /^rgba\((\d+),\s*(\d+),\s*(\d+),\s*([\d.]+)\)$/.exec(value);
  if (!fn) throw new Error(`not a colour: ${value}`);
  return [Number(fn[1]), Number(fn[2]), Number(fn[3]), Number(fn[4])];
}
/** `top` painted over the opaque `under`. */
const over = (top: RGBA, under: RGBA): RGBA => [0, 1, 2].map((i) => (top[i] ?? 0) * top[3] + (under[i] ?? 0) * (1 - top[3])).concat(1) as RGBA;
// WCAG 2's contrast
const lum = (c: RGBA) => 0.2126 * ch(c[0]) + 0.7152 * ch(c[1]) + 0.0722 * ch(c[2]);
function ch(v: number) { const c = v / 255; return c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4; }
const contrast = (a: RGBA, b: RGBA) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return ((x ?? 0) + 0.05) / ((y ?? 0) + 0.05); };
const WHITE: RGBA = [255, 255, 255, 1];
const BLACK: RGBA = [0, 0, 0, 1];

describe("the window (src-tauri)", () => {
  const conf = JSON.parse(read("src-tauri/tauri.conf.json")) as { app: { windows: Record<string, unknown>[]; macOSPrivateApi?: boolean } };
  const main = conf.app.windows.find((w) => w.label === "main") ?? {};
  it("is transparent, with macOSPrivateApi and tauri's macos-private-api feature it needs", () => {
    expect(conf.app.windows).toHaveLength(1);
    expect(main.transparent).toBe(true);
    expect(conf.app.macOSPrivateApi).toBe(true);
    expect(read("src-tauri/Cargo.toml")).toMatch(/^tauri = \{ version = "2\.12", features = \[[^\]]*"macos-private-api"[^\]]*\] \}$/m);
    // the rest of the window as GA-290 left it
    expect(main).toMatchObject({ title: "Hersir", width: 1440, height: 900, minWidth: 900, minHeight: 600 });
  });
  it("asks for vibrancy under the window's background (macOS) and Mica (Windows 11) when it is made, active with the window", () => {
    expect(main.windowEffects).toEqual({ effects: ["underWindowBackground", "mica"], state: "followsWindowActiveState" });
  });
  it("may change its effects (setEffects, clearEffects), and the UI asks glass_support which blur the system gives", () => {
    expect(JSON.parse(read("src-tauri/capabilities/default.json")).permissions).toContain("core:window:allow-set-effects");
    expect(read("src-tauri/src/lib.rs")).toMatch(/generate_handler!\[[^\]]*\bglass::glass_support\b/);
    const api = read("src/api.ts");
    expect(api).toContain('invoke<"vibrancy" | "mica" | "compositor" | "none">("glass_support")');
    expect(api).toContain("getCurrentWindow().setEffects(effects) : getCurrentWindow().clearEffects()");
  });
  it("glass.rs answers vibrancy on macOS, Mica from Windows 11's build 22000, Hyprland's compositor by HYPRLAND_INSTANCE_SIGNATURE", () => {
    const rs = read("src-tauri/src/glass.rs");
    expect(rs).toMatch(/#\[cfg\(target_os = "macos"\)\]\s*return "vibrancy";/);
    expect(rs).toMatch(/#\[cfg\(windows\)\]\s*return if windows_version::OsVersion::current\(\)\.build >= 22000 \{ "mica" \} else \{ "none" \};/);
    expect(rs).toContain('std::env::var_os("HYPRLAND_INSTANCE_SIGNATURE")');
  });
  it("starts the glass right after the appearance, before anything is drawn (main.tsx)", () => {
    const main = read("src/main.tsx");
    expect(main.indexOf("startGlass();")).toBeGreaterThan(main.indexOf("applyAppearance(document.documentElement, getAppearance());"));
    expect(main.indexOf("startGlass();")).toBeLessThan(main.indexOf("onboardingPending()"));
  });
});

describe("the background", () => {
  const gradient = (glow: string, from: string, to: string) =>
    `radial-gradient(ellipse 60% 75% at 0 0, var(--${glow}) 45%, transparent), linear-gradient(180deg, var(--${from}), var(--${to}))`;
  it("without the glass: GA-290's opaque gradient on .app, over the page's opaque bg", () => {
    expect(decl("app.css", ".app").get("background")).toBe(gradient("canvas-glow", "canvas-from", "canvas-to"));
    expect(decl("components.css", "html").get("background")).toBe("var(--bg)");
    expect(decl("components.css", "body").get("background")).toBe("var(--bg)");
    for (const t of ["dark", "light"] as const) {
      for (const name of ["--bg", "--canvas-from", "--canvas-to", "--canvas-glow"]) expect([t, name, rgba(colour(t, name))[3]]).toEqual([t, name, 1]);
    }
  });
  it("with the glass (html.glass): the page is see-through and the body paints the notes' gradient as glass; .app paints nothing over it", () => {
    expect(decl("app.css", "html.glass").get("background")).toBe("transparent");
    expect(decl("app.css", "html.glass body").get("background")).toBe(gradient("glass-glow", "glass-from", "glass-to"));
    expect(decl("app.css", "html.glass .app").get("background")).toBe("none");
    // the notes' gradient, with the glass tokens
    const notes = /html\.glass \.app \{\s*background:\s*([^;]+);/.exec(islandsMd)?.[1]?.replace(/\s+/g, " ").trim();
    expect(notes).toBe(gradient("glass-glow", "glass-from", "glass-to"));
    // no blur in CSS: backdrop-filter can't reach the desktop behind a window
    for (const r of all.filter((x) => x.selector.startsWith("html.glass"))) expect([r.selector, r.decl.has("backdrop-filter")]).toEqual([r.selector, false]);
  });
  it("the glass is the gradient at 92% in dark and 86% in light: canvas-from, canvas-to and canvas-glow as glass", () => {
    for (const [t, alpha] of [["dark", 0.92], ["light", 0.86]] as const) {
      for (const n of ["from", "to", "glow"]) {
        const glass = rgba(colour(t, `--glass-${n}`));
        expect([t, n, glass[3]]).toEqual([t, n, alpha]);
        expect([t, n, glass.slice(0, 3)]).toEqual([t, n, rgba(colour(t, `--canvas-${n}`)).slice(0, 3)]);
      }
    }
  });
  it("html.glass touches only html, body and .app: the islands, menus, popovers, the palette and drawers keep their solid fills", () => {
    const glassRules = all.filter((r) => r.selector.startsWith("html.glass"));
    expect(glassRules.map((r) => r.selector).sort()).toEqual(["html.glass", "html.glass .app", "html.glass body"]);
    expect(decl("components.css", ".side").get("background")).toBe("var(--island)");
    expect(decl("app.css", ".page-island").get("background")).toBe("var(--island)");
    for (const sel of [".pop", ".pal", ".drawer", ".toast", ".ask"]) {
      const fills = all.filter((r) => r.selector === sel).map((r) => r.decl.get("background")).filter(Boolean);
      expect([sel, fills]).toEqual([sel, ["var(--float)"]]);
    }
    for (const t of ["dark", "light"] as const) {
      for (const name of ["--island", "--float", "--raised", "--bg"]) expect([t, name, rgba(colour(t, name))[3]]).toEqual([t, name, 1]);
    }
  });
  it("the first start's onboarding keeps its own opaque background with the glass on", () => {
    // GA-295 (design/islands-settings: Onboarding) gave it the window's background, the islands' opaque canvas gradient as
    // .app paints it without the glass, in place of var(--bg); never the glass-* colours, so the desktop doesn't show through
    expect(decl("onboarding.css", ".onb").get("background")).toBe(gradient("canvas-glow", "canvas-from", "canvas-to"));
    for (const t of ["dark", "light"] as const) {
      for (const n of ["--canvas-from", "--canvas-to", "--canvas-glow"]) expect([t, n, rgba(colour(t, n))[3]]).toEqual([t, n, 1]);
    }
    expect(all.filter((r) => r.selector.startsWith("html.glass") && r.selector.includes(".onb"))).toEqual([]);
  });
});

describe("text on the glass stays readable over a light and a dark desktop (the notes' worst cases)", () => {
  /** text-2 and text at the top, the bottom and in the glow of the glass over a desktop of `desktop`. */
  function on(t: Theme, desktop: RGBA) {
    const top = over(rgba(colour(t, "--glass-from")), desktop);
    const bottom = over(rgba(colour(t, "--glass-to")), desktop);
    const glow = over(rgba(colour(t, "--glass-glow")), top);
    const text2 = rgba(colour(t, "--text-2"));
    const text = rgba(colour(t, "--text"));
    return { text2: [contrast(text2, top), contrast(text2, bottom), contrast(text2, glow)], text: [contrast(text, top), contrast(text, bottom)] };
  }
  it("keeps text and text-2 at AA (4.5) in both themes over white and black", () => {
    for (const t of ["dark", "light"] as const) {
      for (const [name, desktop] of [["white", WHITE], ["black", BLACK]] as const) {
        const c = on(t, desktop);
        for (const v of [...c.text2, ...c.text]) expect([t, name, v >= 4.5]).toEqual([t, name, true]);
      }
    }
  });
  it("measures what the notes measure: dark glass over white, light glass over black", () => {
    const dark = on("dark", WHITE);
    expect(dark.text2[0]).toBeCloseTo(4.86, 1);
    expect(dark.text2[1]).toBeCloseTo(4.73, 1);
    expect(dark.text2[2]).toBeCloseTo(4.97, 1);
    expect(dark.text[0]).toBeCloseTo(9.88, 1);
    const light = on("light", BLACK);
    expect(light.text2[0]).toBeCloseTo(5.09, 1);
    expect(light.text2[1]).toBeCloseTo(4.97, 1);
    expect(light.text2[2]).toBeCloseTo(6.40, 1);
    expect(light.text[0]).toBeCloseTo(11.25, 1);
  });
});

describe("docs/settings.md", () => {
  it("describes Background → Glass: on by default on macOS and Windows 11, off on Linux, and when it stays plain", () => {
    const md = read("docs/settings.md");
    expect(md).toContain("| Appearance | The font, text sizes, theme and density, and the glass behind the islands |");
    const glass = md.slice(md.indexOf("- **Background → Glass:**"), md.indexOf("\n", md.indexOf("- **Background → Glass:**")));
    for (const s of ["Show the desktop through the background", "On by default on macOS (vibrancy) and Windows 11 (Mica)", "Off by default on Linux",
      "Hyprland", "reduced transparency"]) expect(glass).toContain(s);
  });
});
