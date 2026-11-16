// GA-293 QA: the islands build 4 of 9, the Tasks board and the Editor in islands, against design/islands/README.md (An
// island, An island's header, The gap as a resize handle, The window in islands: Tasks as a board and Editor). The rules
// are read from the stylesheets on disk (Vitest hands .css back empty), as islands.test.ts (GA-290) does; the cage's
// self-test (scripts/ui-test.sh: the board's drag probe and the terminal panel probe) measures the same on screen.
import { describe, expect, it } from "vitest";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const STYLES = ["app.css", "components.css", "terminal.css"];
const css = Object.fromEntries(STYLES.map((f) => [f, read(`src/styles/${f}`)]));
const tokens = read("src/styles/tokens.css");
const islandsMd = read("design/islands/README.md");

/** The style rules of a stylesheet: each selector of a list on its own, with its declarations. */
function rules(text: string): { selector: string; decl: Map<string, string> }[] {
  const out: { selector: string; decl: Map<string, string> }[] = [];
  const flat = text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/@media[^{]*\{([\s\S]*?\})\s*\}/g, "$1");
  for (const m of flat.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    const decl = new Map((m[2] ?? "").split(";").map((d) => d.split(":")).filter((d) => d.length > 1)
      .map(([k, ...v]) => [(k ?? "").trim(), v.join(":").trim().replace(/\s+/g, " ")] as [string, string]));
    for (const s of splitSelectors(m[1] ?? "")) out.push({ selector: s.trim().replace(/\s+/g, " "), decl });
  }
  return out;
}
/** A selector list split at its top-level commas (not those inside :has(), :not() or :is()). */
function splitSelectors(list: string): string[] {
  const out: string[] = [];
  let depth = 0, at = 0;
  for (let i = 0; i < list.length; i++) {
    const c = list[i];
    if (c === "(") depth++;
    else if (c === ")") depth--;
    else if (c === "," && depth === 0) { out.push(list.slice(at, i)); at = i + 1; }
  }
  out.push(list.slice(at));
  return out;
}
const all = STYLES.flatMap((f) => rules(css[f] ?? "").map((r) => ({ ...r, file: f })));
/** The declarations of every rule for exactly `selector` in `file`, merged in order. */
function decl(file: string, selector: string): Map<string, string> {
  const found = rules(css[file] ?? "").filter((r) => r.selector === selector);
  if (!found.length) throw new Error(`no ${selector} in ${file}`);
  return new Map(found.flatMap((r) => [...r.decl]));
}
const borders = (d: Map<string, string>) => [...d.keys()].filter((k) => /^border(-(top|right|bottom|left))?(-width|-style|-color)?$/.test(k));
const token = (name: string) => new RegExp(`--${name}:\\s*([^;]+);`).exec(tokens)?.[1]?.trim();

describe("the tokens the card uses (design/islands: The tokens)", () => {
  it("island-gap 6, island-radius 10, size-island-head 40, size-island-tab 28, and the handle as wide as the gap", () => {
    expect([token("island-gap"), token("island-radius"), token("size-island-head"), token("size-island-tab"), token("size-handle")]).toEqual(["6px", "10px", "40px", "28px", "6px"]);
  });
});

describe("the Tasks board: its view toolbar is the island's header (frame 273:1172)", () => {
  it("is 44px high (growing with the interface), padding 0 8, with no line under it and no fill of its own", () => {
    const bar = decl("app.css", ".toolbar.island-toolbar");
    expect(bar.get("height")).toBe("calc(44px + var(--ui-box, 0px))");
    expect(bar.get("padding")).toBe("0 8px");
    expect(bar.get("flex")).toBe("none");
    // neither the toolbar's own rule nor the header's draws a line or a fill
    for (const r of all.filter((x) => x.selector === ".toolbar" || x.selector === ".toolbar.island-toolbar")) {
      expect([r.selector, borders(r.decl), r.decl.get("background"), r.decl.get("box-shadow")]).toEqual([r.selector, [], undefined, undefined]);
    }
    expect(islandsMd).toContain("The board's header holds today's toolbar (New task, Search ID or title, Board or List, Filters, Sort,\n  Archived cards) at 44px");
  });

  it("puts the columns 12px in and 4px under the header, and leaves other pages' toolbars as they are", () => {
    expect(decl("app.css", ".island-toolbar ~ .board-wrap > .board").get("padding")).toBe("4px 12px 12px");
    // the plain .toolbar (other pages, cards 5 and 6) keeps its padding
    expect(decl("components.css", ".toolbar").get("padding")).toBe("12px 20px");
    expect(decl("components.css", ".board").get("padding")).toBe("4px 20px 20px");
  });
});

describe("the Editor in islands (frames 274:1564 and 274:2270)", () => {
  it("with a folder open the page's island gives way: three islands on the background, island-gap apart", () => {
    const page = decl("components.css", ".page-island:has(> .ed-page > .ed)");
    expect([page.get("background"), page.get("border-radius")]).toEqual(["none", "0"]);
    expect(decl("components.css", ".ed").get("gap")).toBe("var(--island-gap)");
    // the code column holds the code island and the terminal panel; it has no fill of its own, so the gap between them shows
    // the background
    const main = decl("components.css", ".ed-main");
    expect(main.has("background")).toBe(false);
    expect(decl("terminal.css", ".term-panel").has("background")).toBe(false);
  });

  it("the files panel, the code and the terminal panel are each an island: island fill, island-radius, overflow hidden, no border", () => {
    for (const [file, sel] of [["components.css", ".ed-side"], ["components.css", ".ed-code-island"], ["terminal.css", ".term-panel-in"]] as const) {
      const d = decl(file, sel);
      expect([sel, d.get("background"), d.get("border-radius"), d.get("overflow")]).toEqual([sel, "var(--island)", "var(--island-radius)", "hidden"]);
      for (const r of all.filter((x) => x.selector === sel)) expect([sel, borders(r.decl), r.decl.get("box-shadow")]).toEqual([sel, [], undefined]);
    }
    // the files panel keeps its width (size-tree, 260): the notes make only the code and terminal gap a handle
    expect(decl("components.css", ".ed-side").get("width")).toBe("var(--size-tree)");
    expect(decl("components.css", ".ed-side").get("flex")).toBe("none");
  });

  it("each island's header is size-island-head high with padding 0 6: the files panel's, the code island's tabs and the terminal panel's", () => {
    const head = decl("components.css", ".island-head");
    expect([head.get("height"), head.get("padding"), head.get("gap"), head.get("align-items")]).toEqual(["var(--size-island-head)", "0 6px", "2px", "center"]);
    expect(borders(head)).toEqual([]);
    const side = decl("components.css", ".ed-side-top");
    expect([side.get("height"), side.get("padding")]).toEqual(["var(--size-island-head)", "0 6px"]);
    const tabs = decl("components.css", ".ed-code-island > .ed-tabs");
    expect([tabs.get("height"), tabs.get("padding"), tabs.get("background"), tabs.get("box-shadow")]).toEqual(["var(--size-island-head)", "0 6px", "none", "none"]);
    // the header's actions are 26 × 26 icon buttons
    const act = decl("components.css", ".island-head > .acts .btn.icon-only");
    expect([act.get("width"), act.get("height")]).toEqual(["26px", "26px"]);
  });

  it("the tabs are Island tabs: 28 high, radius 6, padding 0 6 0 10, 6 apart inside; hover in hover; the open one a pill in selected, no accent line", () => {
    const t = decl("components.css", ".island-tabs > .ed-tab");
    expect([t.get("height"), t.get("border-radius"), t.get("padding"), t.get("gap"), t.get("border"), t.get("background"), t.get("box-shadow"), t.get("color")])
      .toEqual(["var(--size-island-tab)", "var(--radius)", "0 6px 0 10px", "6px", "0", "none", "none", "var(--text-2)"]);
    expect(token("radius")).toBe("6px");
    expect(decl("components.css", ".island-tabs").get("gap")).toBe("2px");
    const hover = decl("components.css", ".island-tabs > .ed-tab:hover");
    expect([hover.get("background"), hover.get("color")]).toEqual(["var(--hover)", "var(--text)"]);
    const on = decl("components.css", '.island-tabs > .ed-tab[aria-selected="true"]');
    expect([on.get("background"), on.get("color"), on.get("box-shadow")]).toEqual(["var(--selected)", "var(--text)", "none"]);
    // close: 12px in an 18px box, radius 4; shown on the open, the hovered and an unsaved tab, the others keep no room for it
    const x = decl("components.css", ".island-tabs > .ed-tab .x");
    expect([x.get("display"), x.get("width"), x.get("height"), x.get("border-radius")]).toEqual(["none", "18px", "18px", "var(--radius-s)"]);
    expect(token("radius-s")).toBe("4px");
    for (const s of [".island-tabs > .ed-tab:hover .x", '.island-tabs > .ed-tab[aria-selected="true"] .x', ".island-tabs > .ed-tab.unsaved .x"]) expect([s, decl("components.css", s).get("display")]).toEqual([s, "grid"]);
    expect(decl("components.css", ".island-tabs > .ed-tab .x .icon").get("width")).toBe("12px");
  });

  it("the status bar sits in the code island, on its fill, with its line above it (the notes keep the status bar's line)", () => {
    const s = decl("components.css", ".ed-status");
    expect([s.get("background"), s.get("border-top"), s.get("height")]).toEqual(["none", "1px solid var(--line)", "var(--size-statusbar)"]);
  });

  it("with the terminal panel, the code island keeps room for its header, the code's least height and the status bar", () => {
    expect(decl("terminal.css", ".ed-main.has-term > .ed-code-island").get("min-height")).toBe("calc(var(--size-island-head) + var(--size-term-code-min) + var(--size-statusbar))");
    expect(decl("terminal.css", ".term-panel").get("min-height")).toBe("var(--size-term-panel-min)");
    expect(decl("terminal.css", ".term-panel").get("flex")).toBe("0 1 var(--term-share, var(--size-term-panel))");
  });
});

describe("the gap above the terminal panel is its resize handle (design/islands: The gap as a resize handle)", () => {
  const sel = ".term-panel > .term-handle";
  it("at rest: the whole 6px gap, see-through, with the row-resize pointer", () => {
    const h = decl("terminal.css", sel);
    expect([h.get("height"), h.get("background"), h.get("box-shadow"), h.get("border-radius")]).toEqual(["var(--size-handle)", "transparent", "none", "4px"]);
    expect(decl("terminal.css", ".term-handle").get("cursor")).toBe("row-resize");
    expect(decl("terminal.css", `${sel}::before`).get("height")).toBe("100%");
    // the line: 2px across the gap's middle, the islands' full width, round ends; drawn only on hover, drag and focus
    const line = decl("terminal.css", `${sel}::after`);
    expect([line.get("left"), line.get("right"), line.get("top"), line.get("height"), line.get("border-radius"), line.has("background")])
      .toEqual(["0", "0", "calc(50% - 1px)", "2px", "1px", false]);
  });

  it("hover: the 2px accent line, and not the old 1px line's fill", () => {
    expect(decl("terminal.css", `${sel}:hover::after`).get("background")).toBe("var(--accent)");
    const hv = decl("terminal.css", `${sel}:hover`);
    expect([hv.get("background"), hv.get("box-shadow")]).toEqual(["transparent", "none"]);
  });

  it("dragging: the line and accent-soft across the whole gap, and the pointer kept everywhere", () => {
    expect(decl("terminal.css", `${sel}.is-drag::after`).get("background")).toBe("var(--accent)");
    const d = decl("terminal.css", `${sel}.is-drag`);
    expect([d.get("background"), d.get("box-shadow")]).toEqual(["var(--accent-soft)", "none"]);
    // the drag rule comes after the hover rule, so a gap both hovered and dragged is accent-soft
    const order = rules(css["terminal.css"] ?? "").map((r) => r.selector);
    expect(order.lastIndexOf(`${sel}.is-drag`)).toBeGreaterThan(order.lastIndexOf(`${sel}:hover`));
    expect(decl("terminal.css", ".ed-main.is-dragging").get("cursor")).toBe("row-resize");
  });

  it("keyboard focus: the line in focus", () => {
    expect(decl("terminal.css", `${sel}:focus-visible::after`).get("background")).toBe("var(--focus)");
    const f = decl("terminal.css", `${sel}:focus-visible`);
    expect([f.get("background"), f.get("box-shadow")]).toEqual(["transparent", "none"]);
  });

  it("the Chat's handle (GA-278, on its panel's left edge) keeps its upright line", () => {
    expect(decl("terminal.css", '.term-handle[aria-orientation="vertical"]').get("cursor")).toBe("col-resize");
  });
});
