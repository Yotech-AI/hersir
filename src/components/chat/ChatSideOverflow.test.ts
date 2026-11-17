// GA-300 QA: the side island's leftovers from GA-296 (design/islands-chat → Its header, The tool strip). (1) Tabs that don't fit
// go in an overflow button after the last that fits ("2 more"): fitTabs measures the header with every tab in it and marks
// the others data-off, keeping the shown tab first, then the ones picked from the overflow's menu, then the rest in the order
// they opened while they fit; room is kept for the one × that shows on hover. Here the header, its tabs and its actions are
// small stand-ins with the sizes the browser would give. (2) The tool strip's tooltip is drawn as Islands/Tooltip (.strip-tip)
// instead of the browser's title, and the hidden tabs and the overflow button have their CSS. Also the Tasks toolbar's fold
// (src/lib/useFold.ts: fitLevel and the CSS of each level) and design/tokens.json's GA-300 changes.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  return Object.fromEntries(Object.entries(real).map(([k, v]) => [k, typeof v === "function" ? () => new Promise(() => {}) : v]));
});

const { fitTabs } = await import("./ChatSide");
const { fitLevel } = await import("../../lib/useFold");
const { tokensCss } = await import("../../lib/tokensCss");

// ---------- stand-ins for the header ----------
type Tab = { key: string; w: number; shown?: boolean; hover?: boolean; xShows?: boolean };
type El = Record<string, unknown> & { dataset: Record<string, string> };
const STYLE = new Map<unknown, Record<string, string>>();
const HEAD = { padding: 6, gap: 8, tabGap: 2, acts: 86 };

/** A header `width` wide with these tabs (the shown one's × showing, as the CSS has it), and the actions on the right. */
function header(width: number, tabs: Tab[]) {
  const els: El[] = tabs.map((t) => {
    const x = { offsetWidth: t.shown || t.hover || t.xShows ? 18 : 0 };
    const terminal = t.key.startsWith("terminal:");
    return {
      dataset: (terminal ? { term: t.key.slice("terminal:".length) } : { tab: t.key }) as Record<string, string>,
      querySelector: (s: string) => (s === ".x" ? x : null),
      getBoundingClientRect: () => ({ width: t.w }),
      getAttribute: (a: string) => (a === "aria-selected" ? String(!!t.shown) : null),
      matches: (s: string) => (s === ":hover" ? !!t.hover : false),
    };
  });
  const list = { children: els };
  const acts = { getBoundingClientRect: () => ({ width: HEAD.acts }) };
  const head = { clientWidth: width, querySelector: (s: string) => (s === ":scope > .island-tabs" ? list : s === ":scope > .acts" ? acts : null) };
  STYLE.set(head, { columnGap: `${HEAD.gap}px`, paddingLeft: `${HEAD.padding}px`, paddingRight: `${HEAD.padding}px` });
  STYLE.set(list, { columnGap: `${HEAD.tabGap}px` });
  return { head: head as unknown as HTMLElement, els, off: () => els.filter((e) => "off" in e.dataset).map((e) => e.dataset.tab ?? `terminal:${e.dataset.term}`) };
}
const more = (w: number) => ({ getBoundingClientRect: () => ({ width: w }) }) as unknown as HTMLElement;

beforeEach(() => { vi.stubGlobal("getComputedStyle", (el: unknown) => STYLE.get(el) ?? {}); });
afterEach(() => { vi.unstubAllGlobals(); STYLE.clear(); });

describe("tabs that don't fit go in the overflow button (fitTabs)", () => {
  const T = (key: string, w: number, more: Partial<Tab> = {}): Tab => ({ key, w, ...more });

  it("all fit: none hidden, no overflow button (an empty list)", () => {
    // room: 480 - 12 padding - 86 actions - 8 gap - 24 for a hover × = 350; tabs 100 + 110 + 100 and 2 gaps = 314
    const h = header(480, [T("terminal:t1", 100, { shown: true }), T("card:GA-1", 110), T("editor:src/a.ts", 100)]);
    expect(fitTabs(h.head, null, "terminal:t1", [])).toEqual([]);
    expect(h.off()).toEqual([]);
  });

  it("too many: the ones after the last that fits go, in the order they opened, with room for the overflow button", () => {
    // room 350 - the button (72 before it is drawn) - 8 = 270: t1 (100) and GA-1 (110 + 2) fit, the rest don't
    const h = header(480, [T("terminal:t1", 100, { shown: true }), T("card:GA-1", 110), T("editor:src/a.ts", 100), T("board:hersir", 90)]);
    expect(fitTabs(h.head, null, "terminal:t1", [])).toEqual(["editor:src/a.ts", "board:hersir"]);
    expect(h.off()).toEqual(["editor:src/a.ts", "board:hersir"]);
    // the button as drawn, wider: one fewer fits
    expect(fitTabs(h.head, more(140), "terminal:t1", [])).toEqual(["card:GA-1", "editor:src/a.ts", "board:hersir"]);
  });

  it("the shown tab always stays in the row, first, even the last one opened", () => {
    const h = header(480, [T("terminal:t1", 100), T("card:GA-1", 110), T("editor:src/a.ts", 100), T("board:hersir", 90, { shown: true })]);
    expect(fitTabs(h.head, null, "board:hersir", [])).toEqual(["card:GA-1", "editor:src/a.ts"]);
  });

  it("a shown tab wider than the room still shows; every other one goes", () => {
    const h = header(300, [T("card:GA-1", 80), T("editor:a-very-long-file-name.ts", 400, { shown: true }), T("board:hersir", 80)]);
    expect(fitTabs(h.head, null, "editor:a-very-long-file-name.ts", [])).toEqual(["card:GA-1", "board:hersir"]);
  });

  it("a tab picked from the menu comes into the row after the shown one, in place of the last that fit", () => {
    const tabs = [T("terminal:t1", 100, { shown: true }), T("card:GA-1", 80), T("editor:src/a.ts", 80), T("board:hersir", 80), T("card:GA-2", 80)];
    // room 270: t1 100, then two of 80 (+2 each) = 264
    expect(fitTabs(header(480, tabs).head, null, "terminal:t1", [])).toEqual(["board:hersir", "card:GA-2"]);
    expect(fitTabs(header(480, tabs).head, null, "terminal:t1", ["card:GA-2"])).toEqual(["editor:src/a.ts", "board:hersir"]);
    // the latest pick first: two picks take both places after the shown tab
    expect(fitTabs(header(480, tabs).head, null, "terminal:t1", ["board:hersir", "card:GA-2"])).toEqual(["card:GA-1", "editor:src/a.ts"]);
  });

  it("hidden tabs show again when there is room again (the island dragged wider)", () => {
    const tabs = [T("terminal:t1", 100, { shown: true }), T("card:GA-1", 110), T("editor:src/a.ts", 100), T("board:hersir", 90)];
    const narrow = header(480, tabs);
    expect(fitTabs(narrow.head, null, "terminal:t1", [])).toHaveLength(2);
    const wide = header(700, tabs);
    for (const e of wide.els) e.dataset.off = "";
    expect(fitTabs(wide.head, null, "terminal:t1", [])).toEqual([]);
    expect(wide.off()).toEqual([]);
  });

  it("a hovered tab is measured without its ×, so hovering doesn't change the row; room is kept for one × only", () => {
    // 4 tabs at rest 100 + 3 × 80 + 3 gaps = 346 ≤ 350 (with room for one × only: one per tab would leave 302); the hovered
    // GA-1 is 80 + 24 wide while hovered, which counted as such would be 370
    const rest = [T("terminal:t1", 100, { shown: true }), T("card:GA-1", 80), T("card:GA-2", 80), T("card:GA-3", 80)];
    expect(fitTabs(header(480, rest).head, null, "terminal:t1", [])).toEqual([]);
    const hovered = rest.map((t) => (t.key === "card:GA-1" ? { ...t, w: 104, hover: true } : t));
    expect(fitTabs(header(480, hovered).head, null, "terminal:t1", [])).toEqual([]);
  });

  it("names a terminal tab by its data-term, the others by data-tab; null without a header", () => {
    const h = header(200, [T("card:GA-1", 120, { shown: true }), T("terminal:t9", 120)]);
    expect(fitTabs(h.head, null, "card:GA-1", [])).toEqual(["terminal:t9"]);
    expect(fitTabs(null, null, "card:GA-1", [])).toBeNull();
  });
});

// ---------- the styles ----------
const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../../${path}`, import.meta.url), "utf8");
/** The declarations of the rules whose selector list has exactly `selector`, merged in order (comments and @media left out). */
function decl(sheet: string, selector: string): Map<string, string> {
  const flat = sheet.replace(/\/\*[\s\S]*?\*\//g, "");
  const out = new Map<string, string>();
  for (const m of flat.matchAll(/([^{}]+)\{([^{}]*)\}/g)) {
    if (!(m[1] ?? "").split(",").map((s) => s.trim().replace(/\s+/g, " ")).includes(selector)) continue;
    for (const d of (m[2] ?? "").split(";")) {
      const at = d.indexOf(":");
      if (at > 0) out.set(d.slice(0, at).trim(), d.slice(at + 1).trim().replace(/\s+/g, " "));
    }
  }
  if (!out.size) throw new Error(`no ${selector}`);
  return out;
}
const terminalCss = read("src/styles/terminal.css");
const appCss = read("src/styles/app.css");
const chatSide = read("src/components/chat/ChatSide.tsx");

describe("the side island's header and strip in CSS", () => {
  it("a tab that doesn't fit is hidden; the overflow button sits after the last that fits, the actions on the right", () => {
    expect(decl(terminalCss, ".island-tabs > [data-off]").get("display")).toBe("none");
    expect(decl(terminalCss, ".island-head > .tabs-more").get("flex")).toBe("none");
    expect(decl(terminalCss, ".island-head > .tabs-more + .acts").get("margin-left")).toBe("auto");
    expect(decl(terminalCss, ".island-head > .tabs-more > .icon").get("width")).toBe("14px");
  });

  it("the overflow button says how many (\"2 more\") with chevron-down after it, and its menu has each tab's icon", () => {
    expect(chatSide).toMatch(/\{off\.length\} more<ChevronDown className="icon" \/>/);
    expect(chatSide).toMatch(/aria-haspopup="menu" aria-expanded=\{!!moreMenu\}/);
    expect(chatSide).toMatch(/icon: <Icon className="icon kind-icon" \/>/);
    expect(decl(terminalCss, ".pop .opt > .icon.kind-icon").get("width")).toBe("14px");
  });

  it("the strip's tooltip has the notes' look (Islands/Tooltip): float, a 1px line-2 border, radius 4, shadow-pop; its key in text-3", () => {
    const tip = decl(terminalCss, ".strip-tip");
    expect([tip.get("position"), tip.get("background"), tip.get("border"), tip.get("border-radius"), tip.get("box-shadow"), tip.get("pointer-events"), tip.get("white-space")])
      .toEqual(["fixed", "var(--float)", "1px solid var(--line-2)", "var(--radius-s)", "var(--shadow-pop)", "none", "nowrap"]);
    expect(read("src/styles/tokens.css")).toMatch(/--radius-s: 4px;/);
    expect(decl(terminalCss, ".strip-tip > .key").get("color")).toBe("var(--text-3)");
  });

  it("the strip's buttons have no title (the browser's tooltip), but a drawn one: role tooltip, after a rest or at once from the keyboard", () => {
    const strip = chatSide.slice(chatSide.indexOf('className="strip-btn"'), chatSide.indexOf("</button>", chatSide.indexOf('className="strip-btn"')));
    expect(strip).not.toMatch(/\btitle=/);
    expect(strip).toMatch(/onMouseEnter=\{\(e\) => showTip\(kind, e\.currentTarget, false\)\}/);
    expect(strip).toMatch(/onFocus=\{\(e\) => \{ if \(e\.currentTarget\.matches\(":focus-visible"\)\) showTip\(kind, e\.currentTarget, true\); \}\}/);
    expect(chatSide).toMatch(/<div id=\{tipId\} className="strip-tip" role="tooltip"/);
    // 6px left of the icon, centred on it
    expect(chatSide).toMatch(/right: window\.innerWidth - r\.left \+ 6, y: r\.top \+ r\.height \/ 2/);
    expect(decl(terminalCss, ".strip-tip").get("transform")).toBe("translateY(-50%)");
  });
});

// ---------- the Tasks toolbar's fold ----------
describe("the Tasks toolbar folds level by level to the least that fits (fitLevel)", () => {
  /** A toolbar `have` wide that needs need[level] at each level of data-fold. */
  const bar = (have: number, need: number[]) => {
    const el = { dataset: {} as Record<string, string>, clientWidth: have, get scrollWidth() { return need[Number(el.dataset.fold ?? 0)] ?? 0; } };
    return el;
  };

  it("takes level 0 (no data-fold) while it fits, and folds no further than it must", () => {
    const wide = bar(900, [708, 640, 600]);
    expect(fitLevel(wide as unknown as HTMLElement, 2)).toBe(0);
    expect("fold" in wide.dataset).toBe(false);
    const short = bar(660, [708, 640, 600]);
    expect(fitLevel(short as unknown as HTMLElement, 2)).toBe(1);
    expect(short.dataset.fold).toBe("1");
    const at900 = bar(634, [708, 680, 600]);
    expect(fitLevel(at900 as unknown as HTMLElement, 2)).toBe(2);
    expect(at900.dataset.fold).toBe("2");
  });

  it("stops at the last level when even that doesn't fit, and goes back to 0 when it fits again", () => {
    const el = bar(500, [708, 680, 600]);
    expect(fitLevel(el as unknown as HTMLElement, 2)).toBe(2);
    el.clientWidth = 708;
    expect(fitLevel(el as unknown as HTMLElement, 2)).toBe(0);
    expect("fold" in el.dataset).toBe(false);
  });

  it("level 1 makes Filters, Sort and Group 30px icon buttons (Filters keeps its count); level 2 New task too, and the search and Team box give width", () => {
    const icon = decl(appCss, ".island-toolbar[data-fold] .btn.fold");
    expect([icon.get("width"), icon.get("padding"), icon.get("font-size"), icon.get("justify-content")]).toEqual(["calc(30px + var(--ui-box, 0px))", "0", "0", "center"]);
    expect([...decl(appCss, '.island-toolbar[data-fold="2"] .btn.fold-2')]).toEqual([...icon]);
    expect(decl(appCss, ".island-toolbar[data-fold] .btn.fold[data-count]").get("width")).toBe("auto");
    expect(decl(appCss, ".island-toolbar[data-fold] .btn.fold[data-count]::after").get("content")).toBe("attr(data-count)");
    expect(decl(appCss, '.island-toolbar[data-fold="2"] .search-input').get("min-width")).toBe("120px");
    expect(decl(appCss, '.island-toolbar[data-fold="2"] select.chip').get("min-width")).toBe("80px");
    // as drawn (no data-fold), the search and the Team box keep their width
    expect(decl(appCss, ".island-toolbar .search-input").get("flex")).toBe("none");
  });
});

// ---------- design/tokens.json ----------
describe("design/tokens.json after GA-300", () => {
  type Token = { name: string; value: unknown; usage: string };
  const json = JSON.parse(read("design/tokens.json")) as { color: { tokens: Token[] }; size: { tokens: Token[] } };
  const tokensFile = read("src/styles/tokens.css");
  const size = (n: string) => json.size.tokens.find((t) => t.name === n)!;

  it("size-term-panel-min's usage says a 40px header and 3 lines; size-handle's the 6px gap itself; the values stay", () => {
    expect(size("size-term-panel-min").value).toBe("120px");
    expect(size("size-term-panel-min").usage).toContain("40px header");
    expect(size("size-term-panel-min").usage).toContain("3 lines");
    expect(size("size-term-panel-min").usage).not.toMatch(/32px|4 lines/);
    expect(size("size-handle").value).toBe("6px");
    expect(size("size-handle").usage).toContain("6px gap");
    expect(size("size-handle").usage).not.toMatch(/1px/);
  });

  it("the band's four colours are gone from tokens.json and tokens.css, and nothing in the app uses them", () => {
    const gone = ["band-line", "band-glow", "band-subtle-line", "band-subtle-glow"];
    expect(json.color.tokens.filter((t) => gone.includes(t.name))).toEqual([]);
    expect(tokensFile).not.toMatch(/--band-(subtle-)?(line|glow)/);
    // the band's sizes stay (size-band, size-band-min)
    expect(json.size.tokens.map((t) => t.name)).toEqual(expect.arrayContaining(["size-band", "size-band-min"]));
    // the design page under design/branding/ brings its own (band-tokens.css), loaded before tokens.css
    expect(read("design/branding/band-tokens.css")).toMatch(/--band-line:/);
    expect(read("design/branding/bottom-band.html").indexOf("band-tokens.css")).toBeLessThan(read("design/branding/bottom-band.html").indexOf("src/styles/tokens.css"));
  });

  it("tokens.css is made from it (scripts/tokens-css.ts)", () => {
    expect(tokensFile).toBe(tokensCss(json as never));
  });
});
