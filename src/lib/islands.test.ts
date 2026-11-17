// GA-290 QA: the islands build 1 of 9 (design/islands/README.md, round 3). The README's token table is in design/tokens.json
// and tokens.css (made from it) in both themes; the window paints the README's opaque gradient and holds the sidebar and the
// page as islands (island fill, island-radius, island-gap from the edges and each other, no border); each page's top bar
// sits on the background (size-topbar 44, no fill, no border, its buttons without fill until hovered, faint text text-2);
// scrollbars are a 6px thumb in scroll-thumb; inputs, selects, checkboxes, kbd and segmented controls fill with raised; what
// floats is float (no overlay left in the app's styles); the mountains are gone from the app (and, since GA-295, from the
// onboarding too); and design/README.md's Visual foundations describes the islands. Files are read from disk (Vitest
// hands .css back empty); node:fs comes in through a computed specifier, as in terminalTokens.test.ts.
import { describe, expect, it } from "vitest";
import { tokensCss, type Tokens } from "./tokensCss";
import { THEME_TOKENS } from "./terminals";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const exists = (path: string): boolean => fs.existsSync(new URL(`../../${path}`, import.meta.url));
type Token = { name: string; value: unknown; usage: string };
const tokensJson = JSON.parse(read("design/tokens.json")) as Tokens & { color: { tokens: Token[] }; size: { tokens: Token[] } };
const tokensFile = read("src/styles/tokens.css");
const islandsMd = read("design/islands/README.md");
const STYLES = ["app.css", "components.css", "terminal.css", "changes.css", "appearance.css", "screenshots.css", "onboarding.css"];
const css = Object.fromEntries(STYLES.map((f) => [f, read(`src/styles/${f}`)]));
const APP_STYLES = STYLES.filter((f) => f !== "onboarding.css");

/** `--name: value;` declarations of the first block whose selector is exactly `selector`. */
function block(text: string, selector: string): Map<string, string> {
  const at = text.indexOf(`${selector} {`);
  if (at < 0) throw new Error(`no ${selector} block`);
  const body = text.slice(text.indexOf("{", at) + 1, text.indexOf("}", at)).replace(/\/\*[\s\S]*?\*\//g, "");
  return new Map([...body.matchAll(/(--[\w-]+)\s*:\s*([^;]+);/g)].map((m) => [m[1] ?? "", (m[2] ?? "").trim()]));
}
const dark = block(tokensFile, ':root, [data-theme="dark"]');
const light = block(tokensFile, '[data-theme="light"]');
const plain = block(tokensFile, ":root");
/** A token's value as tokens.css writes it in a theme (light falls back to dark's; sizes are in the plain :root). */
const raw = (theme: "dark" | "light", name: string) => (theme === "light" ? light.get(name) : undefined) ?? dark.get(name) ?? plain.get(name);
/** A colour's value in a theme, aliases followed. */
function colour(theme: "dark" | "light", name: string): string {
  const value = raw(theme, name);
  if (value === undefined) throw new Error(`${name} isn't in tokens.css`);
  const alias = /^var\((--[\w-]+)\)$/.exec(value);
  return alias ? colour(theme, alias[1] ?? "") : value;
}

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
const all = APP_STYLES.flatMap((f) => rules(css[f] ?? "").map((r) => ({ ...r, file: f })));
/** The declarations of the rules for exactly `selector` in `file`, merged in order. */
function decl(file: string, selector: string): Map<string, string> {
  const found = rules(css[file] ?? "").filter((r) => r.selector === selector);
  if (!found.length) throw new Error(`no ${selector} in ${file}`);
  return new Map(found.flatMap((r) => [...r.decl]));
}
const borders = (d: Map<string, string>) => [...d.keys()].filter((k) => /^border(-(top|right|bottom|left))?(-width|-style|-color)?$/.test(k));

// WCAG 2's relative luminance, for the steps the README draws between the background and the islands.
function lum(hex: string): number {
  const [r, g, b] = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255).map((c) => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4));
  return 0.2126 * (r ?? 0) + 0.7152 * (g ?? 0) + 0.0722 * (b ?? 0);
}
const contrast = (a: string, b: string) => { const [x, y] = [lum(a), lum(b)].sort((p, q) => q - p); return ((x ?? 0) + 0.05) / ((y ?? 0) + 0.05); };

/** design/islands/README.md → The tokens: each row's token, its dark value and its light value (the first `code` of each cell). */
function readmeTokens(): { name: string; dark: string; light: string | null }[] {
  const at = islandsMd.indexOf("## The tokens");
  const table = islandsMd.slice(at, islandsMd.indexOf("`glass-blur` (40)", at));
  return table.split("\n").filter((l) => /^\| `[\w-]+` \|/.test(l)).map((l) => {
    const cells = l.split(" | ");
    const code = (c: string | undefined) => /`([^`]+)`/.exec(c ?? "")?.[1] ?? null;
    return { name: code(cells[0]) ?? "", dark: code(cells[1]) ?? "", light: code(cells[2]) };
  });
}

describe("the design notes (GA-284 to GA-288) are on main", () => {
  it("has the four READMEs, and the look's is round 3 with its token table", () => {
    for (const f of ["design/islands/README.md", "design/islands-chat/README.md", "design/islands-pages/README.md", "design/islands-settings/README.md"]) expect(exists(f), f).toBe(true);
    expect(islandsMd).toContain("## Round 3 (Jefsev's review of 16 Nov)");
    expect(readmeTokens().length).toBe(28);
  });
});

describe("the tokens (design/islands, round 3)", () => {
  it("tokens.css is made from design/tokens.json", () => {
    expect(tokensFile).toBe(tokensCss(tokensJson));
  });

  it("are every row of the README's token table, in dark and in light, in tokens.css", () => {
    const rows = readmeTokens();
    expect(rows.map((r) => r.name)).toEqual(expect.arrayContaining(["island", "bg", "canvas-from", "canvas-to", "canvas-glow", "glass-from", "glass-to", "glass-glow",
      "canvas-hover", "float", "raised", "hover", "selected", "line", "line-2", "side", "text-3", "diff-fold", "term-selection", "scroll-thumb", "scroll-thumb-hover",
      "island-radius", "island-gap", "size-island-head", "size-island-tab", "size-topbar", "size-tool", "size-scroll"]));
    for (const row of rows) {
      for (const theme of ["dark", "light"] as const) {
        const want = theme === "dark" ? row.dark : row.light;
        if (want === null) continue; // a size: one value for both
        const got = raw(theme, `--${row.name}`);
        expect([row.name, theme, got]).toEqual([row.name, theme, want]);
      }
    }
  });

  it("have the card's values in dark: island and bg #181a1c, float and overlay #22242a, raised, side and text-3 lifted", () => {
    const want: Record<string, string> = { island: "#181a1c", bg: "#181a1c", float: "#22242a", overlay: "#22242a", raised: "#1f2125", hover: "#292b30",
      selected: "#2e3138", line: "#31343b", "line-2": "#3f434a", side: "#141519", "text-3": "#8a8e98", "scroll-thumb": "#414348", "scroll-thumb-hover": "#696c72" };
    for (const [name, value] of Object.entries(want)) expect([name, colour("dark", `--${name}`)]).toEqual([name, value]);
    expect(raw("dark", "--bg")).toBe("var(--island)");
    expect(raw("dark", "--overlay")).toBe("var(--float)");
    expect(raw("dark", "--canvas-hover")).toBe("rgba(255, 255, 255, 0.07)");
    const sizes: Record<string, string> = { "island-radius": "10px", "island-gap": "6px", "size-island-head": "40px", "size-island-tab": "28px", "size-topbar": "44px", "size-tool": "28px", "size-scroll": "6px" };
    for (const [name, value] of Object.entries(sizes)) expect([name, plain.get(`--${name}`)]).toEqual([name, value]);
    // the README's equalities: island-radius = radius-l, island-gap = size-handle
    expect(plain.get("--island-radius")).toBe(plain.get("--radius-l"));
    expect(plain.get("--island-gap")).toBe(plain.get("--size-handle"));
  });

  it("in light, white islands on the soft grey: the background a step below them, the glow a soft blue", () => {
    for (const name of ["island", "bg", "float", "raised"]) expect([name, colour("light", `--${name}`)]).toEqual([name, "#ffffff"]);
    expect([colour("light", "--canvas-from"), colour("light", "--canvas-to"), colour("light", "--canvas-glow")]).toEqual(["#ebedf1", "#e2ecf1", "#dce9f4"]);
    for (const c of ["--canvas-from", "--canvas-to", "--canvas-glow"]) expect(lum(colour("light", c)), c).toBeLessThan(lum(colour("light", "--island")));
    const glow = colour("light", "--canvas-glow");
    expect(parseInt(glow.slice(5, 7), 16)).toBeGreaterThan(parseInt(glow.slice(1, 3), 16)); // more blue than red
  });

  it("in dark, the background is a clear step lighter than the islands (1.18 at the top, as the README measures)", () => {
    const island = colour("dark", "--island");
    for (const c of ["--canvas-from", "--canvas-to", "--canvas-glow"]) expect(lum(colour("dark", c)), c).toBeGreaterThan(lum(island));
    expect(contrast(colour("dark", "--canvas-from"), island)).toBeCloseTo(1.18, 1);
    expect(contrast(colour("dark", "--canvas-glow"), island)).toBeCloseTo(1.48, 1);
    // text-3 keeps AA on the island, on raised and on float; on the background faint text is text-2
    for (const s of ["--island", "--raised", "--float"]) expect(contrast(colour("dark", "--text-3"), colour("dark", s)), s).toBeGreaterThanOrEqual(4.5);
    expect(contrast(colour("dark", "--text-2"), colour("dark", "--canvas-glow"))).toBeGreaterThanOrEqual(4.5);
  });
});

describe("the window: the background and the islands", () => {
  it("paints the README's opaque gradient on .app, with the glow from the top left", () => {
    const want = /\.app \{\s*background:\s*([^;]+);/.exec(islandsMd)?.[1]?.replace(/\s+/g, " ").trim();
    expect(want).toBe("radial-gradient(ellipse 60% 75% at 0 0, var(--canvas-glow) 45%, transparent), linear-gradient(180deg, var(--canvas-from), var(--canvas-to))");
    const app = decl("app.css", ".app");
    expect(app.get("background")).toBe(want);
    expect(app.get("padding")).toBe("var(--island-gap)");
    expect(app.get("gap")).toBe("var(--island-gap)");
    expect(app.get("height")).toBe("100vh");
    // the glass (card 3, GA-292) is only under html.glass: glassCss.test.ts
    for (const r of all.filter((x) => /--glass-(from|to|glow)\)/.test([...x.decl.values()].join(";")))) expect(r.selector).toMatch(/^html\.glass\b/);
  });

  it("draws the sidebar as an island: island fill, island-radius, no border, no shadow", () => {
    const side = decl("components.css", ".side");
    expect(side.get("background")).toBe("var(--island)");
    expect(side.get("border-radius")).toBe("var(--island-radius)");
    expect(side.get("overflow")).toBe("hidden");
    expect(borders(side)).toEqual([]);
    // no other rule gives the sidebar a border, a shadow or the old side fill
    for (const r of all.filter((x) => x.selector === ".side")) {
      expect([r.file, borders(r.decl)]).toEqual([r.file, []]);
      expect(r.decl.has("box-shadow")).toBe(false);
      expect(r.decl.get("background")).not.toBe("var(--side)");
    }
    // side-main starts 6px in, as the README says
    expect(decl("components.css", ".side-main").get("padding")).toBe("6px 10px 14px");
  });

  it("draws the page as one island in .main, under the top bar's row", () => {
    const page = decl("app.css", ".page-island");
    expect(page.get("background")).toBe("var(--island)");
    expect(page.get("border-radius")).toBe("var(--island-radius)");
    expect(page.get("overflow")).toBe("hidden");
    expect(page.has("position")).toBe(false); // so the top bar is placed on .main, outside the island's clip
    expect(borders(page)).toEqual([]);
    const main = decl("app.css", ".main");
    expect(main.get("padding-top")).toBe("var(--size-topbar)");
    expect(main.get("position")).toBe("relative");
    expect(main.has("background")).toBe(false); // the background shows around the island
    expect(borders(main)).toEqual([]);
    const bar = decl("app.css", ".page-island .topbar");
    expect([bar.get("position"), bar.get("top"), bar.get("left"), bar.get("right")]).toEqual(["absolute", "0", "0", "0"]);
    // the page's own wrappers stay unpositioned, or the top bar would sit inside the island
    for (const sel of [".ed-page", ".term-page", ".page-island"]) for (const r of all.filter((x) => x.selector === sel)) expect([sel, r.decl.get("position")]).toEqual([sel, undefined]);
  });

  it("keeps the top bar on the background: 44px, no fill, no border, the README's padding; buttons without fill until hovered", () => {
    const bar = decl("components.css", ".topbar");
    expect(bar.get("height")).toBe("var(--size-topbar)");
    expect(bar.get("padding")).toBe("0 4px 0 12px");
    expect(borders(bar)).toEqual([]);
    expect(bar.has("background")).toBe(false);
    // no rule anywhere gives a top bar a border or a fill back
    for (const r of all.filter((x) => /(^|\s)\.topbar$/.test(x.selector))) {
      expect([r.selector, borders(r.decl)]).toEqual([r.selector, []]);
      expect([r.selector, r.decl.get("background")]).toEqual([r.selector, undefined]);
    }
    expect(decl("components.css", ".topbar .btn:not(.primary):not(.danger)").get("background-color")).toBe("transparent");
    expect(decl("components.css", ".topbar .btn:not(.primary):not(.danger):hover").get("background-color")).toBe("var(--canvas-hover)");
    expect(decl("components.css", ".topbar select.chip:hover").get("background-color")).toBe("var(--canvas-hover)");
    // faint text on the background is text-2
    expect(decl("components.css", ".crumbs .sep").get("color")).toBe("var(--text-2)");
    for (const sel of [".main .topbar .faint", ".main .topbar .sep", ".main .topbar .id"]) expect([sel, decl("app.css", sel).get("color")]).toEqual([sel, "var(--text-2)"]);
    // the Chief's chip, coloured text on the background, sits on island
    expect(decl("components.css", ".lead-chip").get("background")).toBe("var(--island)");
    // and grows with the interface from 44px
    expect(block(css["appearance.css"] ?? "", ":root").get("--size-topbar")).toBe("calc(44px + var(--ui-box, 0px))");
  });
});

describe("scrollbars inside the islands", () => {
  it("are a size-scroll (6px) thumb in scroll-thumb, scroll-thumb-hover under the pointer, no track, 2px from the edge", () => {
    const bar = decl("components.css", "::-webkit-scrollbar");
    const thumb = decl("components.css", "::-webkit-scrollbar-thumb");
    const edge = parseFloat(/^(\d+)px solid transparent$/.exec(thumb.get("border") ?? "")?.[1] ?? "NaN");
    expect(edge).toBe(2);
    expect(thumb.get("background-clip")).toBe("padding-box");
    for (const side of ["width", "height"]) expect(parseFloat(bar.get(side) ?? "") - 2 * edge).toBe(parseFloat(plain.get("--size-scroll") ?? ""));
    expect(thumb.get("background-color")).toBe("var(--scroll-thumb)");
    expect(thumb.get("border-radius")).toBe("var(--radius-pill)");
    expect(decl("components.css", "::-webkit-scrollbar-thumb:hover").get("background-color")).toBe("var(--scroll-thumb-hover)");
    expect(decl("components.css", "::-webkit-scrollbar-thumb:active").get("background-color")).toBe("var(--scroll-thumb-hover)");
    expect(decl("components.css", "::-webkit-scrollbar-track").get("background")).toBe("transparent");
    // at least 4px from its ends: the track's 2px margin and the thumb's 2px border
    expect(parseFloat(decl("components.css", "::-webkit-scrollbar-track").get("margin") ?? "") + edge).toBeGreaterThanOrEqual(4);
  });

  it("in a terminal (xterm.js draws its own) the same thumb and colours", () => {
    const slider = Object.fromEntries(THEME_TOKENS.filter(([k]) => String(k).startsWith("scrollbarSlider")).map(([k, t]) => [k, t]));
    expect(slider).toEqual({ scrollbarSliderBackground: "scroll-thumb", scrollbarSliderHoverBackground: "scroll-thumb-hover", scrollbarSliderActiveBackground: "scroll-thumb-hover" });
    const s = decl("terminal.css", ".term-host .xterm .xterm-scrollable-element > .scrollbar.vertical > .slider");
    expect(s.get("width")).toBe("var(--size-scroll) !important");
    expect(s.get("right")).toBe("2px");
  });
});

describe("controls on an island fill with raised", () => {
  it.each([".input", ".select", ".textarea", ".seg", ".kbd", ".md-editor", ".ed-goto", ".agent-chip", 'input[type="checkbox"]'])("%s", (sel) => {
    const file = [".agent-chip"].includes(sel) ? "app.css" : "components.css";
    expect(decl(file, sel).get("background")).toBe("var(--raised)");
    // and no rule puts one back on bg
    for (const r of all.filter((x) => x.selector === sel || x.selector.endsWith(` ${sel}`))) expect([r.file, r.selector, r.decl.get("background")]).not.toEqual([r.file, r.selector, "var(--bg)"]);
  });

  it("checkboxes are drawn: raised with a line-2 border, accent with the tick when on, dimmed when disabled", () => {
    const box = decl("components.css", 'input[type="checkbox"]');
    expect(box.get("appearance")).toBe("none");
    expect(box.get("border")).toBe("1px solid var(--line-2)");
    const on = decl("components.css", 'input[type="checkbox"]:checked');
    expect([on.get("background"), on.get("border-color")]).toEqual(["var(--accent)", "var(--accent)"]);
    expect(decl("components.css", 'input[type="checkbox"]:checked::before').get("visibility")).toBe("visible");
    expect(decl("components.css", 'input[type="checkbox"]::before').get("visibility")).toBe("hidden");
    expect(box.get("color")).toBe("var(--on-accent)");
    expect(decl("components.css", 'input[type="checkbox"]:disabled').get("opacity")).toBe(".45");
  });
});

describe("what floats", () => {
  it("is float: no var(--overlay) is left in the app's styles or code", () => {
    for (const f of STYLES) expect([f, (css[f] ?? "").includes("var(--overlay)")]).toEqual([f, false]);
    expect(read("src/lib/codeTheme.ts")).not.toContain("--overlay");
    for (const sel of [".pop", ".pal", ".drawer", ".toast", ".ask"]) {
      const d = all.filter((r) => r.selector === sel).map((r) => r.decl.get("background")).filter(Boolean);
      expect([sel, d]).toEqual([sel, ["var(--float)"]]);
    }
  });
});

describe("the mountains are gone from the window (the gradient took their place)", () => {
  it("has no Band component, no band.css and no rule for the app's band", () => {
    expect(exists("src/components/Band.tsx")).toBe(false);
    expect(exists("src/styles/band.css")).toBe(false);
    expect(read("src/main.tsx")).not.toMatch(/band/i);
    expect(read("src/App.tsx")).not.toMatch(/\bBand\b|"band"/);
    for (const f of APP_STYLES) expect([f, /\.band\b|data-band/.test((css[f] ?? "").replace(/\/\*[\s\S]*?\*\//g, ""))]).toEqual([f, false]);
    const code = (path: string) => read(path).replace(/\/\*[\s\S]*?\*\/|\/\/.*$/gm, "");
    // its Background section holds the glass now (GA-292), with no word of the mountains
    expect(code("src/components/AppearanceSettings.tsx")).not.toMatch(/mountains|band/i);
    expect(read("src/lib/appearance.ts")).not.toMatch(/dataset\.band/);
  });

  // GA-290 kept the onboarding's band "until the card that draws it"; GA-295 is that card (design/islands-settings: 9.
  // Onboarding, "The band goes here as well"), so the band, its rules and mountains.svg are gone from the onboarding too
  it("and from the first start's onboarding since GA-295, the card that draws it in islands", () => {
    expect(islandsMd).toContain("The first start's onboarding (no islands) keeps its band until the card that draws it.");
    const onb = (css["onboarding.css"] ?? "").replace(/\/\*[\s\S]*?\*\//g, "");
    expect(onb).not.toMatch(/\.band\b|--onb-band|band-line|band-glow|size-band/);
    const page = read("src/pages/OnboardingPage.tsx").replace(/\/\*[\s\S]*?\*\/|\/\/.*$/gm, "");
    expect(page).not.toMatch(/mountains|className="band"/);
    expect(exists("src/assets/mountains.svg")).toBe(false);
    // nothing else in the app still loads it
    for (const f of ["src/App.tsx", "src/main.tsx", "src/components/AppearanceSettings.tsx"]) expect([f, read(f).includes("mountains.svg")]).toEqual([f, false]);
  });
});

describe("design/README.md's Visual foundations", () => {
  const md = read("design/README.md");
  const vf = md.slice(md.indexOf("## Visual foundations"), md.indexOf("\n## ", md.indexOf("## Visual foundations") + 5));
  it("describes the background, the islands and what floats, as design/islands/README.md says", () => {
    expect(vf).toContain("design/islands/README.md");
    for (const s of ["**The background**", "**The islands**", "**On the background**", "**On an island**", "**What floats**",
      "`canvas-from`", "`canvas-to`", "`canvas-glow`", "`island`", "`island-radius` (10)", "`island-gap` (6)", "`size-topbar`, 44", "`canvas-hover`",
      "`float`", "`raised`", "`size-scroll` (6px)", "`scroll-thumb`", "No border and no shadow"]) expect(vf).toContain(s);
  });
  it("no longer says the sidebar is dimmer than the canvas, nor that 1px borders separate regions", () => {
    expect(vf).not.toContain("The sidebar is dimmer than the canvas");
    expect(vf).not.toContain("Separate regions with 1px borders");
    expect(vf).not.toMatch(/mountains along the bottom of the window \(`band-\*`/);
  });
  it("and docs/settings.md's Appearance no longer lists the mountains (it has the glass, GA-292)", () => {
    const settings = read("docs/settings.md");
    expect(settings).toContain("| Appearance | The font, text sizes, theme and density, and the glass behind the panels |");
    expect(settings).not.toMatch(/Mountains/);
  });
});
