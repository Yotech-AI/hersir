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
import { describe, expect, it } from "vitest";

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(`../../${path}`, import.meta.url), "utf8");
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
    expect(own.map((r) => r.selector)).toEqual([".page-island:has(> .split.doc-split)", ".page-island:has(> .split.memory.islands)"]);
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
  it("leaves the Editor's panel and the Chat's terminal tabs as they were (only .term-page > .term-tabs)", () => {
    const added = rules(css["terminal.css"]).filter((r) => r.selector.includes("island") || (r.selector.startsWith(".term-tabs") && r.decl.get("height") === "var(--size-island-head)"));
    expect(added).toEqual([]);
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
