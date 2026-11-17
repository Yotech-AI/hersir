// GA-294 QA: the islands build 5 of 9 (design/islands-pages/README.md, GA-286). The Inbox's and the task list's, Projects'
// and Clients' toolbars are their island's header (44px, padding 0 8, no line under it); a project and a client stay one
// island without a header. A card's page is two islands: the card, with Card, Changes and Screenshots as island tabs in its
// header (28px pills, the selected one in `selected`), and Properties as an island of its own, size-props (320) wide, with an
// island header. The Changes file list keeps its line on the right without the side fill; Screenshots has 18px at the sides.
// Everything that floats (the palette, popovers and menus, the close question, toasts) is float with a 1px line-2 border and
// shadow-pop; a drawer is an island of its own, island-gap in from the window's top, right and bottom edges, island-radius
// all round, float, a line-2 border all round and shadow-drawer, over the scrim, sliding in from the right, and nothing
// under it moves. Files are read from disk (Vitest hands .css back empty), as in islands.test.ts.
import { describe, expect, it } from "vitest";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const STYLES = ["app.css", "components.css", "terminal.css", "changes.css", "appearance.css", "screenshots.css"];
const css = Object.fromEntries(STYLES.map((f) => [f, read(`src/styles/${f}`)]));
const tokens = read("src/styles/tokens.css");
const notes = read("design/islands-pages/README.md");

/** The style rules of a stylesheet: each selector of a list on its own, with its declarations. With `media`, the rules
 *  inside media queries count too (flattened); without, only the top-level ones. */
function rules(text: string, media = true): { selector: string; decl: Map<string, string> }[] {
  const out: { selector: string; decl: Map<string, string> }[] = [];
  const flat = text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/@media[^{]*\{([\s\S]*?\})\s*\}/g, media ? "$1" : "").replace(/@keyframes[^{]*\{[\s\S]*?\}\s*\}/g, "");
  for (const m of flat.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const decl = new Map((m[2] ?? "").split(";").map((d) => d.split(":")).filter((d) => d.length > 1)
      .map(([k, ...v]) => [(k ?? "").trim(), v.join(":").trim().replace(/\s+/g, " ")] as [string, string]));
    for (const s of (m[1] ?? "").split(",")) out.push({ selector: s.trim().replace(/\s+/g, " "), decl });
  }
  return out;
}
const all = STYLES.flatMap((f) => rules(css[f] ?? "").map((r) => ({ ...r, file: f })));
/** The declarations of the top-level rules for exactly `selector` in `file`, merged in order. */
function decl(file: string, selector: string): Map<string, string> {
  const found = rules(css[file] ?? "", false).filter((r) => r.selector === selector);
  if (!found.length) throw new Error(`no ${selector} in ${file}`);
  return new Map(found.flatMap((r) => [...r.decl]));
}
const borders = (d: Map<string, string>) => [...d].filter(([k]) => /^border(-(top|right|bottom|left))?(-width|-style|-color)?$/.test(k));
/** A size token's value in tokens.css (the plain :root). */
const token = (name: string) => new RegExp(`${name}:\\s*([^;]+);`).exec(tokens)?.[1]?.trim();
/** Source without its comments, so a comment naming a class doesn't count as using it. */
const code = (path: string) => read(path).replace(/\{\/\*[\s\S]*?\*\/\}|\/\*[\s\S]*?\*\/|\/\/.*$/gm, "");

describe("a page's toolbar is its island's header", () => {
  it.each(["src/pages/TasksPage.tsx", "src/pages/ProjectsPage.tsx", "src/pages/ClientsPage.tsx"])("%s: its one toolbar is an island-bar", (path) => {
    const src = code(path);
    // the Tasks page's also carries GA-293's island-toolbar (its board's rules and probe)
    expect(src.match(/className="toolbar island-bar( island-toolbar)?"/g)?.length).toBe(1);
    expect(src).not.toMatch(/className="toolbar"/);
  });

  it("a project and a client are one island without a header (no toolbar, as the notes say)", () => {
    expect(notes).toContain("**A project: one island without a header**");
    expect(notes).toContain("**A client: one island without a header**");
    for (const path of ["src/pages/ProjectPage.tsx", "src/pages/ClientPage.tsx"]) expect([path, /className="toolbar/.test(code(path))]).toEqual([path, false]);
  });

  it("44px high (growing with the interface), padding 0 8px, no line under it and no fill of its own", () => {
    const bar = decl("components.css", ".toolbar.island-bar");
    expect(bar.get("min-height")).toBe("calc(44px + var(--ui-box, 0px))");
    expect(bar.get("padding")).toBe("0 8px");
    expect(bar.get("flex")).toBe("none");
    // no rule gives a toolbar a border or a fill: the island shows through, and the gaps split the column
    for (const r of all.filter((x) => /\.toolbar(\.island-bar)?$/.test(x.selector))) {
      expect([r.selector, borders(r.decl)]).toEqual([r.selector, []]);
      expect([r.selector, r.decl.get("background") ?? r.decl.get("background-color")]).toEqual([r.selector, undefined]);
    }
    // its controls 28 to 30px high: the search box (on raised, as every input since GA-290)
    expect(decl("app.css", ".toolbar .search-input").get("height")).toBe("calc(28px + var(--ui-box, 0px))");
  });

  it("the Inbox's list starts 14px under Run this for me (8px of its own and a group head's 6px), only in the Inbox", () => {
    expect(notes).toContain("then the list (padding `14 12 24`)");
    expect(decl("app.css", ".inbox-list .task-list").get("padding-top")).toBe("8px");
    expect(code("src/pages/TasksPage.tsx")).toContain('className={`content${inbox ? " inbox-list" : ""}`}');
  });
});

describe("a card's page is two islands", () => {
  it("the page's own island gives way to the card's island and Properties', island-gap apart", () => {
    const page = decl("app.css", ".page-island:has(> .task-page)");
    expect([page.get("background"), page.get("border-radius")]).toEqual(["none", "0"]);
    const row = decl("app.css", ".task-page");
    expect([row.get("display"), row.get("gap"), row.get("flex"), row.get("min-height")]).toEqual(["flex", "var(--island-gap)", "1", "0"]);
  });

  it("the card island: the island's fill and radius, no border, its content kept to the corners", () => {
    const card = decl("app.css", ".card-island");
    expect(card.get("background")).toBe("var(--island)");
    expect(card.get("border-radius")).toBe("var(--island-radius)");
    expect(card.get("overflow")).toBe("hidden");
    expect([card.get("display"), card.get("flex-direction"), card.get("flex")]).toEqual(["flex", "column", "1"]);
    expect(borders(card)).toEqual([]);
    expect([token("--island-radius"), token("--island-gap")]).toEqual(["10px", "6px"]);
  });

  it("its header: size-island-head (40) high, growing with the interface, padding 0 6px, no line under it", () => {
    expect(token("--size-island-head")).toBe("40px");
    const head = decl("components.css", ".island-head");
    expect([head.get("display"), head.get("align-items"), head.get("padding"), head.get("gap")]).toEqual(["flex", "center", "0 6px", "2px"]);
    expect(borders(head)).toEqual([]);
    expect(decl("app.css", ".card-island > .island-head").get("height")).toBe("calc(var(--size-island-head) + var(--ui-box, 0px))");
  });

  it("Card, Changes and Screenshots are island tabs: 28px pills, radius 6, no underline, the selected one in selected", () => {
    expect(token("--size-island-tab")).toBe("28px");
    expect(token("--radius")).toBe("6px");
    const strip = decl("changes.css", ".island-head > .task-tabs");
    expect([strip.get("border-bottom"), strip.get("padding"), strip.get("flex-wrap")]).toEqual(["0", "0", "nowrap"]);
    const tab = decl("changes.css", ".task-tabs .tab");
    expect(tab.get("height")).toBe("calc(var(--size-island-tab) + var(--ui-box, 0px))");
    expect([tab.get("border"), tab.get("border-radius"), tab.get("padding"), tab.get("gap")]).toEqual(["0", "var(--radius)", "0 6px 0 10px", "6px"]);
    expect(tab.has("background")).toBe(false); // no fill at rest
    expect(decl("changes.css", '.task-tabs .tab[aria-selected="true"]').get("background")).toBe("var(--selected)");
    expect(decl("changes.css", ".task-tabs .tab:hover").get("background")).toBe("var(--hover)");
    // the underlined tab look (.tabs .tab's border-bottom) doesn't come back on these tabs
    for (const r of all.filter((x) => /\.task-tabs .tab(\[aria-selected="true"\])?$/.test(x.selector))) {
      expect([r.selector, r.decl.get("border-bottom"), r.decl.get("border-bottom-color")]).toEqual([r.selector, undefined, undefined]);
    }
    // Changes keeps its diffstat in diff-add and diff-del
    expect(decl("changes.css", ".diffstat .add").get("color")).toBe("var(--diff-add)");
    expect(decl("changes.css", ".diffstat .del").get("color")).toBe("var(--diff-del)");
  });

  it("Properties is an island of its own: size-props (320) wide, the island's fill and radius, no line on its left", () => {
    expect(token("--size-props")).toBe("320px");
    expect(decl("components.css", ".props").get("width")).toBe("var(--size-props)");
    const props = decl("app.css", ".task-page > .props");
    expect(props.get("background")).toBe("var(--island)");
    expect(props.get("border-radius")).toBe("var(--island-radius)");
    expect(props.get("border-left")).toBe("0");
    expect([props.get("display"), props.get("flex-direction"), props.get("overflow")]).toEqual(["flex", "column", "hidden"]);
  });

  it("Properties' header is an island header (40px, no line under it); its rows scroll under it with padding 4 12 24 14", () => {
    const head = decl("app.css", ".task-page > .props > .props-head");
    expect(head.get("height")).toBe("calc(var(--size-island-head) + var(--ui-box, 0px))");
    expect(head.get("border-bottom")).toBe("0");
    expect(head.get("flex")).toBe("none");
    const body = decl("app.css", ".task-page > .props > .props-body");
    expect([body.get("padding"), body.get("overflow-y"), body.get("flex")]).toEqual(["4px 12px 24px 14px", "auto", "1"]);
    const src = read("src/components/Properties.tsx");
    expect(src).toContain('<div className="props-head">Properties<button className="btn ghost sm icon-only" aria-label="Close properties" title="Close properties (])"');
  });

  it("in a narrow window Properties hides as before, the island version too", () => {
    expect(css["app.css"]).toMatch(/@media \(max-width: 1100px\) \{ \.props, \.task-page > \.props \{ display: none; \} \}/);
  });

  it("the Changes file list keeps its line on the right and loses the side fill (the island is its surface)", () => {
    const files = decl("changes.css", ".chg-files");
    expect(files.get("border-right")).toBe("1px solid var(--line)");
    expect(files.has("background")).toBe(false);
    for (const r of all.filter((x) => /\.chg-files$/.test(x.selector))) expect([r.selector, r.decl.get("background")]).toEqual([r.selector, undefined]);
  });

  it("the Screenshots tab has 18px at the sides (padding 20 18 48)", () => {
    expect(decl("screenshots.css", ".shots-in").get("padding")).toBe("20px 18px 48px");
  });
});

describe("what floats is float, with a thin line-2 border and its shadow", () => {
  // the notes' table: what, its size and radius, its padding
  const FLOATS: [string, string, Record<string, string>][] = [
    ["the palette", ".pal", { "border-radius": "var(--radius-xl)", padding: "6px 6px 10px" }],
    ["a popover or a menu", ".pop", { "border-radius": "var(--radius-l)", padding: "6px", "min-width": "220px", "max-height": "360px" }],
    ["the close question", ".ask", { "border-radius": "var(--radius-xl)", padding: "20px 22px 16px", width: "min(460px, 100%)" }],
    ["a toast", ".toast", { "border-radius": "var(--radius-l)", padding: "10px 14px", right: "20px", bottom: "20px" }],
  ];
  const fileOf = (sel: string) => (sel === ".pal" || sel === ".pop" ? "app.css" : "components.css");

  it.each(FLOATS)("%s (%s): float, 1px solid line-2, shadow-pop, its own size, radius and padding", (_what, sel, own) => {
    const d = decl(fileOf(sel), sel);
    expect([d.get("background"), d.get("border"), d.get("box-shadow")]).toEqual(["var(--float)", "1px solid var(--line-2)", "var(--shadow-pop)"]);
    for (const [k, v] of Object.entries(own)) expect([sel, k, d.get(k)]).toEqual([sel, k, v]);
  });

  it("no other rule gives one of them another fill, border or shadow", () => {
    for (const r of all.filter((x) => /(^|[\s>])\.(pal|pop|ask|toast|drawer)([.:[][^\s>]*)?$/.test(x.selector))) {
      const bg = r.decl.get("background") ?? r.decl.get("background-color");
      if (bg !== undefined) expect([r.file, r.selector, bg]).toEqual([r.file, r.selector, "var(--float)"]);
      for (const [k, v] of borders(r.decl)) if (!/radius/.test(k)) expect([r.file, r.selector, k, v]).toEqual([r.file, r.selector, k, "1px solid var(--line-2)"]);
      const shadow = r.decl.get("box-shadow");
      if (shadow !== undefined) expect([r.file, r.selector, shadow]).toEqual([r.file, r.selector, r.selector.includes("drawer") ? "var(--shadow-drawer)" : "var(--shadow-pop)"]);
    }
  });

  it("a menu (the editor's right-click menu, a popover's) is a .pop", () => {
    expect(read("src/components/editor/Menus.tsx")).toMatch(/className="pop" role="menu"/);
    expect(read("src/components/Popover.tsx")).toMatch(/className=\{`pop\$\{/);
  });

  it("the radii and the dark and light float and line-2 are the design's (#22242a / white; shadows in both themes)", () => {
    expect([token("--radius-l"), token("--radius-xl")]).toEqual(["10px", "14px"]);
    const darkBlock = tokens.slice(tokens.indexOf(':root, [data-theme="dark"]'), tokens.indexOf('[data-theme="light"]'));
    const lightBlock = tokens.slice(tokens.indexOf('[data-theme="light"]'));
    expect(/--float:\s*#22242a;/.test(darkBlock)).toBe(true);
    expect(/--float:\s*#ffffff;/i.test(lightBlock)).toBe(true);
    for (const b of [darkBlock, lightBlock]) for (const n of ["--line-2", "--shadow-pop", "--shadow-drawer", "--scrim"]) expect([n, new RegExp(`${n}:`).test(b)]).toEqual([n, true]);
  });

  it("the palette's, the drawer's and the close question's scrims cover the whole window", () => {
    for (const [file, sel] of [["app.css", ".pal-backdrop"], ["components.css", ".scrim"], ["components.css", ".ask-scrim"]] as const) {
      const d = decl(file, sel);
      expect([sel, d.get("position"), d.get("inset"), d.get("background")]).toEqual([sel, "fixed", "0", "var(--scrim)"]);
    }
  });
});

describe("a drawer floats as an island of its own", () => {
  const drawer = decl("components.css", ".drawer");
  it("island-gap in from the window's top, right and bottom edges, not from its left", () => {
    expect(drawer.get("position")).toBe("fixed");
    expect([drawer.get("top"), drawer.get("right"), drawer.get("bottom")]).toEqual(["var(--island-gap)", "var(--island-gap)", "var(--island-gap)"]);
    expect(drawer.has("left")).toBe(false);
    expect(drawer.has("height")).toBe(false); // top and bottom give it 100vh - 12px
  });

  it("rounded like the islands on all four corners, its content kept to them, a line-2 border all round, float and shadow-drawer", () => {
    expect(drawer.get("border-radius")).toBe("var(--island-radius)");
    expect(drawer.get("overflow")).toBe("hidden");
    expect(drawer.get("border")).toBe("1px solid var(--line-2)");
    expect(drawer.has("border-left")).toBe(false);
    expect(drawer.get("background")).toBe("var(--float)");
    expect(drawer.get("box-shadow")).toBe("var(--shadow-drawer)");
  });

  it("today's width, at most size-drawer (1024px); the wide one at most 1280px", () => {
    expect(token("--size-drawer")).toBe("1024px");
    expect(drawer.get("width")).toBe("min(var(--size-drawer), calc(100vw - 48px))");
    expect(decl("components.css", ".drawer.wide").get("width")).toBe("min(1280px, calc(100vw - 48px))");
  });

  it("slides in from the right (32px and a fade, .2s) over the dimmed backdrop, which is under it", () => {
    expect(drawer.get("animation")).toBe("gz-slide .2s cubic-bezier(.2, .8, .2, 1)");
    expect(css["components.css"]).toContain("@keyframes gz-slide { from { transform: translateX(32px); opacity: 0; } to { transform: none; opacity: 1; } }");
    const scrim = decl("components.css", ".scrim");
    expect(Number(scrim.get("z-index"))).toBeLessThan(Number(drawer.get("z-index")));
  });

  it("the islands under it don't move: no rule reacts to an open drawer", () => {
    for (const r of all) expect([r.file, r.selector, /:has\([^)]*(drawer|scrim)/.test(r.selector)]).toEqual([r.file, r.selector, false]);
    // every drawer is Drawer.tsx's: fixed, on top of the page, not in its flow
    expect(read("src/components/Drawer.tsx")).toContain('<aside className={`drawer${wide ? " wide" : ""}`} role="dialog" aria-modal="true" aria-label={title}>');
  });
});

describe("design/README.md says how the pages and the drawer sit now", () => {
  const md = read("design/README.md");
  it("list toolbars are island headers, a card's page is two islands, a drawer floats as an island", () => {
    expect(md).toContain("view toolbar as their island's header (44 high, padding `0 8`, no line under it");
    expect(md).toContain("A card's page is two islands (`design/islands-pages/README.md`)");
    expect(md).toContain("The drawer is an island of its own floating over the work islands: `island-gap` in from");
    expect(md).toContain("the islands under it don't move");
  });
});
