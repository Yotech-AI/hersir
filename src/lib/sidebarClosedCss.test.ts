// GA-291 QA: how the closed sidebar is drawn (design/islands/README.md → A sidebar that closes). Closed, the sidebar island
// goes and the main column takes its width, island-gap (6px) from the window's left edge; the toolbar sits at the top left
// on the background, 6px from the window's edges, centred in the 44px top bar row, its buttons 2px apart, and the page's
// top bar starts after it, so its crumbs come 12px after the toolbar (its left padding) and never under it. Tool buttons
// are size-tool (28) squares, radius 6, icon in text-2 with no fill until hovered: canvas-hover on the background, hover
// in the sidebar island, the icon then text; keyboard focus is the 2px focus ring outside. The Inbox count is the pill in
// needs on needs-soft over island, at least 18 wide, padding 0 5px, 4px after the icon. Close sidebar shares New task's
// row, New task getting the rest of it, 4px before the button. Files are read from disk (Vitest hands .css back empty);
// node:fs comes in through a computed specifier, as in islands.test.ts.
import { describe, expect, it } from "vitest";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
const css = { app: read("src/styles/app.css"), components: read("src/styles/components.css"), tokens: read("src/styles/tokens.css") };

/** The declarations of every rule for exactly `selector` (each selector of a list on its own), merged in order. */
function decl(text: string, selector: string): Map<string, string> {
  const flat = text.replace(/\/\*[\s\S]*?\*\//g, "").replace(/@media[^{]*\{([\s\S]*?\})\s*\}/g, "$1");
  const out = new Map<string, string>();
  let found = false;
  for (const m of flat.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (!(m[1] ?? "").split(",").map((s) => s.trim().replace(/\s+/g, " ")).includes(selector)) continue;
    found = true;
    for (const d of (m[2] ?? "").split(";")) {
      const at = d.indexOf(":");
      if (at > 0) out.set(d.slice(0, at).trim(), d.slice(at + 1).trim().replace(/\s+/g, " "));
    }
  }
  if (!found) throw new Error(`no ${selector} rule`);
  return out;
}
/** A size token's value in tokens.css's :root. */
const token = (name: string) => new RegExp(`${name}:\\s*([^;]+);`).exec(css.tokens)?.[1]?.trim();

describe("the sizes the closed sidebar is drawn with (GA-291)", () => {
  it("are the notes': island-gap 6, the top bar's row 44, a tool button 28, radius 6", () => {
    expect(token("--island-gap")).toBe("6px");
    expect(token("--size-topbar")).toBe("44px");
    expect(token("--size-tool")).toBe("28px");
    expect(token("--radius")).toBe("6px");
  });
});

describe("closed, the sidebar island goes and the page takes its width (GA-291)", () => {
  it("the window keeps one column, the main one, island-gap from the window's edges", () => {
    const closed = decl(css.app, ".app.side-closed");
    expect(closed.get("grid-template-columns")).toBe("minmax(0, 1fr)");
    expect(decl(css.app, ".app").get("padding")).toBe("var(--island-gap)");
  });
  it("the toolbar is placed in the window, not in the page: the window is its containing block", () => {
    expect(decl(css.app, ".app.side-closed").get("position")).toBe("relative");
    expect(decl(css.app, ".app").get("position")).toBeUndefined();
  });
});

describe("the toolbar at the top left (GA-291)", () => {
  const tools = decl(css.app, ".side-tools");
  it("sits island-gap (6px) from the window's top and left edges, over the page's column", () => {
    expect(tools.get("position")).toBe("absolute");
    expect(tools.get("top")).toBe("var(--island-gap)");
    expect(tools.get("left")).toBe("var(--island-gap)");
    expect(Number(tools.get("z-index"))).toBeGreaterThan(0);
  });
  it("is the top bar's row: as tall (size-topbar), its buttons centred in it, 2px apart", () => {
    expect(tools.get("height")).toBe("var(--size-topbar)");
    expect(tools.get("display")).toBe("flex");
    expect(tools.get("align-items")).toBe("center");
    expect(tools.get("gap")).toBe("2px");
  });
  it("the main column's top bar is in the same row: at its top, as tall, under the window's island-gap", () => {
    expect(decl(css.app, ".page-island .topbar").get("top")).toBe("0");
    expect(decl(css.app, ".main").get("padding-top")).toBe("var(--size-topbar)");
    expect(decl(css.components, ".topbar").get("height")).toBe("var(--size-topbar)");
  });
  it("the page's top bar starts after it while it shows (its width, --side-tools), and at the column's edge when open", () => {
    expect(decl(css.app, ".app.side-closed .page-island .topbar").get("left")).toBe("var(--side-tools, 0px)");
    expect(decl(css.app, ".page-island .topbar").get("left")).toBe("0");
  });
  it("so the crumbs come 12px after the toolbar: the top bar's own left padding", () => {
    expect(decl(css.components, ".topbar").get("padding")).toBe("0 4px 0 12px");
  });
  it("no page moves its top bar elsewhere (left, position or a left padding of its own), so none runs under the toolbar", () => {
    const flat = (css.app + css.components).replace(/\/\*[\s\S]*?\*\//g, "");
    const others = [...flat.matchAll(/([^{}]*\.topbar[^{},]*)\{([^{}]*)\}/g)]
      .filter((m) => /(^|;)\s*(left|position|padding-left|padding|margin-left)\s*:/.test(m[2] ?? ""))
      .map((m) => (m[1] ?? "").trim().replace(/\s+/g, " "))
      .filter((s) => ![".topbar", ".page-island .topbar", ".app.side-closed .page-island .topbar", ".topbar .actions"].includes(s));
    expect(others).toEqual([]);
  });
});

describe("a tool button (GA-291; Figma Islands/Tool button)", () => {
  const tool = decl(css.components, ".tool");
  it("is a size-tool square, radius 6, no border and no fill, its icon in text-2", () => {
    expect(tool.get("min-width")).toBe("var(--size-tool)");
    expect(tool.get("height")).toBe("var(--size-tool)");
    expect(tool.get("border-radius")).toBe("var(--radius)");
    expect(tool.get("border")).toBe("0");
    expect(tool.get("background")).toBe("none");
    expect(tool.get("color")).toBe("var(--text-2)");
    expect(tool.get("justify-content")).toBe("center");
    expect(tool.get("align-items")).toBe("center");
  });
  it("hovered or pressed on the background: canvas-hover, the icon in text", () => {
    for (const s of [".tool:hover", ".tool:active"]) {
      expect(decl(css.components, s).get("background"), s).toBe("var(--canvas-hover)");
      expect(decl(css.components, s).get("color"), s).toBe("var(--text)");
    }
  });
  it("in the sidebar island (Close sidebar) its hover fill is hover, as the sidebar's rows", () => {
    expect(decl(css.components, ".side .tool:hover").get("background")).toBe("var(--hover)");
    expect(decl(css.components, ".side .tool:active").get("background")).toBe("var(--hover)");
    expect(decl(css.components, ".nav-item:hover").get("background")).toBe("var(--hover)");
  });
  it("keyboard focus is the 2px focus ring outside it (every button's)", () => {
    expect(decl(css.components, ":focus-visible").get("outline")).toBe("2px solid var(--focus)");
    expect(decl(css.components, ":focus-visible").get("outline-offset")).toBe("1px");
    expect(tool.get("outline")).toBeUndefined();
  });
  it("is wider by the Inbox count, 4px after the icon", () => {
    expect(tool.get("gap")).toBe("4px");
    expect(tool.get("width")).toBeUndefined();
  });
});

describe("the Inbox count on the background (GA-291)", () => {
  const count = decl(css.components, ".tool .count");
  it("is the sidebar's pill: needs on needs-soft over island, at least 18 wide, padding 0 5px", () => {
    expect(count.get("color")).toBe("var(--needs)");
    expect(count.get("background")).toBe("linear-gradient(var(--needs-soft), var(--needs-soft)), var(--island)");
    expect(count.get("min-width")).toBe("18px");
    expect(count.get("padding")).toBe("0 5px");
    expect(count.get("border-radius")).toBe("var(--radius-pill)");
  });
});

describe("Close sidebar on New task's row (GA-291)", () => {
  it("New task takes the rest of the row, 4px before the button", () => {
    const row = decl(css.components, ".side-top");
    expect(row.get("display")).toBe("flex");
    expect(row.get("gap")).toBe("4px");
    expect(row.get("align-items")).toBe("center");
    expect(decl(css.components, ".side-top > .nav-item").get("flex")).toBe("1");
  });
});
