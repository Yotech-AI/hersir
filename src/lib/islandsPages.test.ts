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

// GA-294 and GA-295 both added this file: GA-295's checks below keep their own helpers in a block of their own.
{
  // GA-295 QA: the islands build 6 of 9 (design/islands-settings/README.md, GA-288; design/islands/README.md for the shared
  // parts). The styles that lay Memory, a doc, Users, Usage, Settings, Team, a terminal's own page and the first start's
  // onboarding out in islands: the shared island header (40 high, padding 0 6px, 2px between items, no line under it) and
  // Island tabs (28 high, radius 6, the open one a pill in selected, no underline); a doc's History and Memory's Notes, note
  // and About this note as islands of their own (island fill, island-radius, island-gap apart, no border), the page's own
  // island giving way to them; the note bar as the note island's header that stays while the note scrolls; Users' toolbar
  // as a 44px header; the terminal's tabs as Island tabs with the terminal 6px under them and 14px in; Team's picker on
  // island; and the onboarding as one 760px island centred under a 44px top row, with no mountains. The pages' markup is
  // tested beside each page (SettingsTabs, UsagePage, DocPage, OnboardingPage); this reads the stylesheets from disk, as
  // islands.test.ts does (Vitest hands .css back empty).

  const FILES = ["app.css", "components.css", "terminal.css", "onboarding.css"] as const;
  const css = Object.fromEntries(FILES.map((f) => [f, read(`src/styles/${f}`)])) as Record<(typeof FILES)[number], string>;
  const tokens = read("src/styles/tokens.css");
  const token = (name: string) => new RegExp(`--${name}:\\s*([^;]+);`).exec(tokens)?.[1]?.trim();

  /** A selector list split at its top-level commas (not those inside :is(…) or :has(…)). */
  function selectors(list: string): string[] {
    const out: string[] = [];
    let depth = 0, from = 0;
    for (let i = 0; i < list.length; i++) {
      if (list[i] === "(") depth++;
      else if (list[i] === ")") depth--;
      else if (list[i] === "," && depth === 0) { out.push(list.slice(from, i)); from = i + 1; }
    }
    return [...out, list.slice(from)];
  }
  /** The style rules of a stylesheet outside @media blocks (a wide window's), each selector of a list on its own, with its
   *  declarations. */
  function rules(text: string): { selector: string; decl: Map<string, string> }[] {
    const out: { selector: string; decl: Map<string, string> }[] = [];
    const flat = text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/@media[^{]*\{[\s\S]*?\}\s*\}/g, "");
    for (const m of flat.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
      const decl = new Map((m[2] ?? "").split(";").map((d) => d.split(":")).filter((d) => d.length > 1)
        .map(([k, ...v]) => [(k ?? "").trim(), v.join(":").trim().replace(/\s+/g, " ")] as [string, string]));
      for (const s of selectors(m[1] ?? "")) out.push({ selector: s.trim().replace(/\s+/g, " "), decl });
    }
    return out;
  }
  /** The declarations of every rule for exactly `selector` in `file`, merged in order (a later rule wins, as in CSS). */
  function decl(file: (typeof FILES)[number], selector: string): Map<string, string> {
    const found = rules(css[file]).filter((r) => r.selector === selector);
    if (!found.length) throw new Error(`no ${selector} in ${file}`);
    return new Map(found.flatMap((r) => [...r.decl]));
  }

  describe("the tokens the islands use (design/islands)", () => {
    it("island header 40, Island tab 28, History 320, top bar 44, island-gap 6, island-radius 10", () => {
      expect([token("size-island-head"), token("size-island-tab"), token("size-props"), token("size-topbar"), token("island-gap"), token("island-radius")])
        .toEqual(["40px", "28px", "320px", "44px", "6px", "10px"]);
    });
  });

  describe("an island's header and Island tabs (components.css)", () => {
    it("the header: size-island-head high, padding 0 6px, 2px between items, no line under it", () => {
      const head = decl("components.css", ".island-head");
      expect([head.get("height"), head.get("padding"), head.get("gap"), head.get("display"), head.get("flex")])
        .toEqual(["var(--size-island-head)", "0 6px", "2px", "flex", "none"]);
      expect(head.has("border-bottom")).toBe(false);
      // its title: t-body-strong in text, 6px in, cut off with an ellipsis; its actions 26 × 26 on the right
      const title = decl("components.css", ".island-head > .island-title");
      expect([title.get("color"), title.get("font-weight"), title.get("padding-left"), title.get("text-overflow")]).toEqual(["var(--text)", "600", "6px", "ellipsis"]);
      expect(decl("components.css", ".island-head > .acts").get("margin-left")).toBe("auto");
      const icon = decl("components.css", ".island-head > .acts .btn.icon-only");
      expect([icon.get("width"), icon.get("height")]).toEqual(["26px", "26px"]);
    });

    it("a page's toolbar as the header (Users) is 44 high, as the Tasks board's", () => {
      expect(decl("components.css", ".island-head.bar").get("height")).toBe("44px");
    });

    it("Island tabs: 28 high, radius 6, padding 0 6px 0 10px, no underline; the open one a pill in selected, hover fills with hover", () => {
      const row = decl("components.css", ".tabs.island-tabs");
      expect(row.get("border-bottom")).toBe("0");
      const tab = decl("components.css", ".island-tabs > .tab");
      expect([tab.get("height"), tab.get("border-radius"), tab.get("padding"), tab.get("border"), tab.get("color"), tab.get("gap")])
        .toEqual(["var(--size-island-tab)", "var(--radius)", "0 6px 0 10px", "0", "var(--text-2)", "6px"]);
      expect(token("radius")).toBe("6px");
      const open = decl("components.css", '.island-tabs > .tab[aria-selected="true"]');
      expect([open.get("background"), open.get("color")]).toEqual(["var(--selected)", "var(--text)"]);
      expect(decl("components.css", ".island-tabs > .tab:hover").get("background")).toBe("var(--hover)");
      // the icon 14px in text-3, text-2 when open or hovered
      const icon = decl("components.css", ".island-tabs > .tab > .icon");
      expect([icon.get("width"), icon.get("height"), icon.get("color")]).toEqual(["14px", "14px", "var(--text-3)"]);
      expect(decl("components.css", '.island-tabs > .tab[aria-selected="true"] > .icon').get("color")).toBe("var(--text-2)");
      // the keyboard focus still shows
      expect(decl("components.css", ".island-tabs > .tab:focus-visible").get("outline")).toMatch(/^2px solid var\(--/);
    });
  });

  describe("Settings and Usage: the forms start 16px under the header", () => {
    it("Settings' page keeps max 1100 and the page's padding at the sides, its top 16 (the tabs went to the header)", () => {
      expect(decl("app.css", ".settings-page").get("padding-top")).toBe("16px");
      expect(decl("components.css", ".page").get("padding")).toBe("24px 28px 48px");
      expect(read("src/pages/SettingsPage.tsx")).toContain('className="page settings-page" style={{ maxWidth: 1100 }}');
    });
    it("Usage's toolbar line fills the rest of the header, right of the tabs", () => {
      expect(decl("app.css", ".usage-page").get("padding-top")).toBe("16px");
      const tools = decl("app.css", ".usage-tools");
      expect([tools.get("flex"), tools.get("justify-content"), tools.get("min-width")]).toEqual(["1", "flex-end", "0"]);
    });
  });

  describe("panes as islands of their own: a doc and Memory with its notes (app.css)", () => {
    it("the page's island gives way (no fill, no radius) and the panes sit island-gap apart", () => {
      const own = rules(css["app.css"]).filter((r) => r.selector.startsWith(".page-island:has("));
      // a doc and Memory (GA-295); the card's page (GA-297) and the Chat (GA-296) give way the same since
      expect(own.map((r) => r.selector)).toEqual([".page-island:has(> .split.doc-split)", ".page-island:has(> .split.memory.islands)",
        ".page-island:has(> .task-page)", ".page-island:has(> .chat)"]);
      for (const r of own) expect([r.selector, r.decl.get("background"), r.decl.get("border-radius")]).toEqual([r.selector, "none", "0"]);
      expect(decl("app.css", ".split.doc-split").get("gap")).toBe("var(--island-gap)");
      expect(decl("app.css", ".split.memory.islands").get("gap")).toBe("var(--island-gap)");
    });

    it("each pane is an island: its fill, island-radius, and it scrolls inside; a drawer under the split keeps its own look", () => {
      for (const sel of [".doc-split > :is(.doc, .props)", ".memory.islands > :is(.mem-files, .mem-note, .mem-side, .mem-local)"]) {
        const d = decl("app.css", sel);
        expect([sel, d.get("background"), d.get("border-radius"), d.get("overflow-y")]).toEqual([sel, "var(--island)", "var(--island-radius)", "auto"]);
      }
      // only the panes by name: no rule for every child of the split
      expect(rules(css["app.css"]).some((r) => /^\.(doc-split|memory\.islands) > \*$/.test(r.selector))).toBe(false);
    });

    it("History is 320 (size-props) with no border on its left; Notes 260 and About this note 300, without their old borders and fills", () => {
      expect(decl("components.css", ".props").get("width")).toBe("var(--size-props)");
      expect(decl("app.css", ".doc-split > .props").get("border-left")).toBe("0");
      const files = decl("app.css", ".mem-files");
      expect([files.get("width"), files.has("border-right"), files.has("background")]).toEqual(["260px", false, false]);
      const side = decl("app.css", ".mem-side");
      expect([side.get("width"), side.has("border-left"), side.has("background")]).toEqual(["300px", false, false]);
      const local = decl("app.css", ".mem-local");
      expect([local.get("width"), local.has("border-left"), local.has("background")]).toEqual(["360px", false, false]);
      expect(decl("app.css", ".mem-local-head").get("height")).toBe("var(--size-island-head)");
      expect(decl("app.css", ".mem-local-head").has("border-bottom")).toBe(false);
    });

    it("the note bar is the note island's header: 40 high, no line, not sticky; the note scrolls under it", () => {
      const bar = decl("app.css", ".mem-note-bar");
      expect([bar.get("height"), bar.get("flex"), bar.has("border-bottom"), bar.has("position")]).toEqual(["var(--size-island-head)", "none", false, false]);
      const open = decl("app.css", ".mem-note.open");
      expect([open.get("display"), open.get("flex-direction"), open.get("overflow")]).toEqual(["flex", "column", "hidden"]);
      const body = decl("app.css", ".mem-note-body");
      expect([body.get("flex"), body.get("min-height"), body.get("overflow-y")]).toEqual(["1", "0", "auto"]);
      // errors at the top of the note island, under its header
      expect(decl("app.css", ".mem-note-body > .error-banner").get("margin")).toBe("6px 20px 0");
    });

    it("the open note's row: selected, its icon in text; the panel toggle shows pressed in the top bar", () => {
      expect(decl("app.css", ".mem-item.on").get("background")).toBe("var(--selected)");
      expect(decl("app.css", ".mem-item.on > .icon").get("color")).toBe("var(--text)");
      expect(decl("app.css", ".main .topbar .btn.mem-toggle.on").get("background-color")).toBe("var(--selected)");
    });

    it("the Notes header's old 11px uppercase head is gone", () => {
      expect(rules(css["app.css"]).some((r) => r.selector === ".mem-files-head" && r.decl.get("text-transform") === "uppercase")).toBe(false);
    });
  });

  describe("Team's picker sits on island (components.css)", () => {
    it("island under it, its line border, the name in text", () => {
      const chip = decl("components.css", ".topbar select.chip.on-island");
      expect([chip.get("background-color"), chip.get("border-color"), chip.get("color")]).toEqual(["var(--island)", "var(--line)", "var(--text)"]);
    });
  });

  describe("a terminal's own page: its tabs are the island's header (terminal.css)", () => {
    it("the header: size-island-head, padding 0 6px, 2px apart, no fill, no line", () => {
      const head = decl("terminal.css", ".term-page > .term-tabs");
      expect([head.get("height"), head.get("padding"), head.get("gap"), head.get("background"), head.get("box-shadow")])
        .toEqual(["var(--size-island-head)", "0 6px", "2px", "none", "none"]);
    });
    it("its tabs are Island tabs, the open one a pill in selected; New terminal (+) on the right, 26 × 26", () => {
      const tab = decl("terminal.css", ".term-page > .term-tabs > .ed-tab");
      expect([tab.get("height"), tab.get("padding"), tab.get("border-radius"), tab.get("border"), tab.get("color")])
        .toEqual(["var(--size-island-tab)", "0 6px 0 10px", "var(--radius)", "0", "var(--text-2)"]);
      expect(decl("terminal.css", '.term-page > .term-tabs > .ed-tab[aria-selected="true"]').get("background")).toBe("var(--selected)");
      const plus = decl("terminal.css", ".term-page > .term-tabs > .btn.icon-only");
      expect([plus.get("width"), plus.get("height"), plus.get("margin")]).toEqual(["26px", "26px", "0 0 0 auto"]);
    });
    it("the terminal 6px under the header and 14px in; only the open or hovered tab shows its close", () => {
      expect(decl("terminal.css", ".term-page > .term-place").get("padding")).toBe("6px 14px 8px");
      expect(decl("terminal.css", ".term-page > .term-tabs > .ed-tab .x").get("display")).toBe("none");
      expect(decl("terminal.css", '.term-page > .term-tabs > .ed-tab[aria-selected="true"] .x').get("display")).toBe("grid");
    });
    it("no rule outside .term-page gives .term-tabs the island header's height", () => {
      // GA-296 and GA-298 put the side island's .island-head, .island-tabs and .ed-code-island rules in terminal.css; what
      // this checks is that the Terminal page's header doesn't reach the Editor's panel or the Chat's terminal tabs
      const tall = rules(css["terminal.css"]).filter((r) => r.selector.includes(".term-tabs") && r.decl.get("height") === "var(--size-island-head)");
      expect(tall.map((r) => r.selector)).toEqual([".term-page > .term-tabs"]);
    });
  });

  describe("the first start's onboarding in one island (onboarding.css)", () => {
    it("the window's background with a top row of size-topbar, and island-gap round the island but at the top", () => {
      const onb = decl("onboarding.css", ".onb");
      expect(onb.get("grid-template-rows")).toBe("calc(var(--size-topbar) + var(--ui-box, 0px)) minmax(0, 1fr)");
      expect(onb.get("padding")).toBe("0 var(--island-gap) var(--island-gap)");
      expect(onb.get("height")).toBe("100vh");
      expect(onb.get("background")).toContain("var(--canvas-from)");
      expect(onb.get("background")).toContain("var(--canvas-to)");
      expect(decl("onboarding.css", ".onb-top").has("border-bottom")).toBe(false);
    });
    it("one island, 760 wide (the 640 column and 60 each side), centred, its fill and island-radius", () => {
      expect(decl("onboarding.css", ".onb").get("--onb-w")).toMatch(/^min\(calc\(640px \+ 20 \* var\(--ui-grow, 0px\)\),/);
      const island = decl("onboarding.css", ".onb-island");
      expect([island.get("width"), island.get("margin"), island.get("background"), island.get("border-radius"), island.get("overflow")])
        .toEqual(["calc(var(--onb-w) + 120px)", "0 auto", "var(--island)", "var(--island-radius)", "hidden"]);
      expect(island.has("box-shadow")).toBe(false);
      expect(island.has("border")).toBe(false);
    });
    it("the step scrolls inside the island; the actions stay at its foot, 24 from its edge, above a line", () => {
      expect(decl("onboarding.css", ".onb-scroll").get("overflow-y")).toBe("auto");
      expect(decl("onboarding.css", ".onb-scroll").get("flex")).toBe("1");
      const actions = decl("onboarding.css", ".onb-actions");
      expect([actions.get("flex"), actions.get("margin"), actions.get("border-top")]).toEqual(["none", "0 auto 24px", "1px solid var(--line)"]);
    });
    it("the steps: the current number an accent ring on island, the others text-2 (text-3 would drop under AA on the background)", () => {
      const on = decl("onboarding.css", ".onb-step.on .num");
      expect([on.get("border-color"), on.get("background")]).toEqual(["var(--accent)", "var(--island)"]);
      expect(decl("onboarding.css", ".onb-step").get("color")).toBe("var(--text-2)");
      expect(decl("onboarding.css", ".onb-eyebrow").get("color")).toBe("var(--text-2)");
    });
  });
}
