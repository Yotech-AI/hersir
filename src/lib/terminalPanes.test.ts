// GA-348 QA: the Terminal page's pane rules (src/lib/terminalPanes.ts) against the card: which terminals fill a layout,
// where a terminal shown on the page goes (the tab rule), what happens when a terminal in a pane closes, stepping down,
// a layout that couldn't start all its terminals, and the per-folder store kept in memory (never in localStorage).
import { afterEach, describe, expect, it } from "vitest";
import {
  addPane, choose, fitLayout, focused, keepStarted, layoutLabel, LAYOUTS, missingFor, onePane, pagePanes, paneCount, panesOf,
  resetPanes, samePanes, settle, showTerminal, stepDown, swapIn, updatePanes, type Layout, type Panes,
} from "./terminalPanes";
import source from "./terminalPanes.ts?raw";

const panes = (layout: Layout, ids: string[], focus = 0, tabs: string[] = ids): Panes => ({ layout, ids, focus, tabs });

describe("the layouts (the Layout menu, word for word)", () => {
  it("One terminal, 2 side by side, 3 side by side, 4 side by side and 2 × 2, with 1, 2, 3, 4 and 4 panes", () => {
    expect(LAYOUTS.map((l) => [l.id, l.label, l.count])).toEqual([
      ["one", "One terminal", 1], ["two", "2 side by side", 2], ["three", "3 side by side", 3], ["four", "4 side by side", 4], ["grid", "2 × 2", 4],
    ]);
    expect(LAYOUTS.map((l) => paneCount(l.id))).toEqual([1, 2, 3, 4, 4]);
    expect(LAYOUTS.map((l) => layoutLabel(l.id))).toEqual(LAYOUTS.map((l) => l.label));
  });
  it("2 × 2's label uses the multiplication sign, not an x", () => {
    expect(layoutLabel("grid")).toBe("2 × 2");
  });
});

describe("stepping down (a pane goes: 4 → 3 → 2 → One terminal; 2 × 2 → 3 side by side)", () => {
  it("steps one pane down from each layout, and One terminal stays", () => {
    expect(stepDown("four")).toBe("three");
    expect(stepDown("grid")).toBe("three");
    expect(stepDown("three")).toBe("two");
    expect(stepDown("two")).toBe("one");
    expect(stepDown("one")).toBe("one");
  });
  it("fitLayout steps down until the panes that got a terminal fill it", () => {
    expect(fitLayout("three", 3)).toBe("three");
    expect(fitLayout("three", 2)).toBe("two");
    expect(fitLayout("three", 1)).toBe("one");
    expect(fitLayout("grid", 3)).toBe("three");
    expect(fitLayout("grid", 2)).toBe("two");
    expect(fitLayout("four", 1)).toBe("one");
    expect(fitLayout("two", 0)).toBe("one");
    expect(fitLayout("two", 5)).toBe("two");
  });
  it("keepStarted keeps the panes that got a terminal", () => {
    expect(keepStarted(panes("three", ["a", "b"]))).toEqual(panes("two", ["a", "b"]));
    expect(keepStarted(panes("grid", ["a", "b", "c"]))).toMatchObject({ layout: "three", ids: ["a", "b", "c"] });
    expect(keepStarted(panes("four", ["a"]))).toMatchObject({ layout: "one", ids: ["a"] });
    expect(keepStarted(panes("two", ["a", "b"]))).toMatchObject({ layout: "two", ids: ["a", "b"] });
  });
});

describe("choosing a layout (the page's terminal in the first pane, then the folder's next tabs, round to the first)", () => {
  it("two open, 2 side by side: both, the one on the page on the left, the first pane focused; nothing to start", () => {
    expect(choose(["t1", "t2"], "t2", "two")).toEqual({ panes: panes("two", ["t2", "t1"], 0, ["t1", "t2"]), missing: 0 });
    expect(choose(["t1", "t2"], "t1", "two").panes.ids).toEqual(["t1", "t2"]);
  });
  it("one open, 3 side by side: the one there, and two to start", () => {
    expect(choose(["t1"], "t1", "three")).toEqual({ panes: panes("three", ["t1"]), missing: 2 });
    expect(missingFor("three", ["t1"])).toBe(2);
  });
  it("wraps round to the folder's first tabs", () => {
    expect(choose(["a", "b", "c", "d", "e"], "d", "four").panes.ids).toEqual(["d", "e", "a", "b"]);
    expect(choose(["a", "b", "c", "d", "e"], "d", "grid").panes.ids).toEqual(["d", "e", "a", "b"]);
    expect(choose(["a", "b", "c"], "c", "three").panes.ids).toEqual(["c", "a", "b"]);
  });
  it("a smaller layout keeps the page's terminal and drops the others from the panes only (they stay in tabs)", () => {
    const { panes: p, missing } = choose(["a", "b", "c", "d"], "c", "two");
    expect(p.ids).toEqual(["c", "d"]);
    expect(p.tabs).toEqual(["a", "b", "c", "d"]);
    expect(missing).toBe(0);
    expect(choose(["a", "b", "c", "d"], "c", "one").panes).toEqual(panes("one", ["c"], 0, ["a", "b", "c", "d"]));
  });
  it("missingFor counts the terminals the folder lacks for each layout, never below 0", () => {
    expect(LAYOUTS.map((l) => missingFor(l.id, ["a"]))).toEqual([0, 1, 2, 3, 3]);
    expect(LAYOUTS.map((l) => missingFor(l.id, ["a", "b"]))).toEqual([0, 0, 1, 2, 2]);
    expect(LAYOUTS.map((l) => missingFor(l.id, ["a", "b", "c", "d", "e"]))).toEqual([0, 0, 0, 0, 0]);
  });
});

describe("the tab rule (a tab, Ctrl+PgDn and Ctrl+PgUp, an address from elsewhere)", () => {
  const three = panes("three", ["a", "b", "c"], 1, ["a", "b", "c", "d"]);
  it("a terminal in a pane: that pane is focused; the panes stay", () => {
    expect(showTerminal(three, "c")).toEqual({ ...three, focus: 2 });
    expect(showTerminal(three, "b")).toBe(three);
  });
  it("a terminal not in a pane comes into the focused pane; the one there drops to a tab; the layout stays", () => {
    expect(showTerminal(three, "d")).toEqual({ ...three, ids: ["a", "d", "c"] });
  });
  it("a folder with no terminal in a pane gets One terminal with it", () => {
    expect(showTerminal(panes("one", [], 0, []), "x")).toMatchObject({ layout: "one", ids: ["x"], focus: 0 });
  });
});

describe("a layout's missing terminals and Start again", () => {
  it("addPane puts each started terminal in the next pane, until the layout is full", () => {
    let p = panes("three", ["a"]);
    p = addPane(p, "n1");
    p = addPane(p, "n2");
    expect(p.ids).toEqual(["a", "n1", "n2"]);
    expect(addPane(p, "n3")).toBe(p);
    expect(addPane(panes("three", ["a", "n1"]), "n1").ids).toEqual(["a", "n1"]);
  });
  it("swapIn puts Start again's new terminal in the ended one's pane", () => {
    expect(swapIn(panes("three", ["a", "b", "c"], 0), "b", "n")).toMatchObject({ ids: ["a", "n", "c"], focus: 0 });
    const p = panes("two", ["a", "b"]);
    expect(swapIn(p, "x", "n")).toBe(p);
    expect(swapIn(p, "a", "b")).toBe(p);
  });
});

describe("a terminal that closes (×, exit, the sidebar, Start again's old one)", () => {
  it("its pane takes the folder's next terminal that isn't in a pane: the tab after it, else the one before it", () => {
    // tabs a b c d e; panes a b c; b closes: d is the next tab not in a pane
    expect(settle(panes("three", ["a", "b", "c"], 0, ["a", "b", "c", "d", "e"]), ["a", "c", "d", "e"]))
      .toEqual(panes("three", ["a", "d", "c"], 0, ["a", "c", "d", "e"]));
    // tabs a b c d; panes c d; d closes: no tab after it, so b (the one before c, which is in a pane)
    expect(settle(panes("two", ["c", "d"], 0, ["a", "b", "c", "d"]), ["a", "b", "c"]).ids).toEqual(["c", "b"]);
  });
  it("a terminal opened since (a new tab at the end) can take the pane too", () => {
    expect(settle(panes("two", ["a", "b"], 0, ["a", "b"]), ["a", "n"]).ids).toEqual(["a", "n"]);
  });
  it("without one, the pane goes and the layout steps down: 3 → 2, 2 × 2 → 3 side by side, 4 → 3, 2 → One terminal", () => {
    expect(settle(panes("three", ["a", "b", "c"]), ["a", "c"])).toEqual(panes("two", ["a", "c"], 0, ["a", "c"]));
    expect(settle(panes("grid", ["a", "b", "c", "d"]), ["a", "b", "d"])).toEqual(panes("three", ["a", "b", "d"], 0, ["a", "b", "d"]));
    expect(settle(panes("four", ["a", "b", "c", "d"]), ["a", "b", "c"]).layout).toBe("three");
    expect(settle(panes("two", ["a", "b"]), ["a"])).toEqual(panes("one", ["a"], 0, ["a"]));
  });
  it("the focused pane's terminal closes: the pane that takes its place is focused", () => {
    const p = settle(panes("two", ["a", "b"], 1, ["a", "b", "c"]), ["a", "c"]);
    expect(p).toMatchObject({ ids: ["a", "c"], focus: 1 });
    expect(focused(p)).toBe("c");
  });
  it("the focused pane goes: the one left of it is focused; the first one going focuses the one now first", () => {
    expect(focused(settle(panes("three", ["a", "b", "c"], 2), ["a", "b"]))).toBe("b");
    expect(focused(settle(panes("three", ["a", "b", "c"], 1), ["a", "c"]))).toBe("a");
    expect(focused(settle(panes("three", ["a", "b", "c"], 0), ["b", "c"]))).toBe("b");
  });
  it("a pane left of the focused one goes: the focus stays on its terminal", () => {
    const p = settle(panes("three", ["a", "b", "c"], 2), ["b", "c"]);
    expect(p).toMatchObject({ layout: "two", ids: ["b", "c"], focus: 1 });
    expect(focused(p)).toBe("c");
  });
  it("several at once: each pane is settled; all gone leaves no pane and One terminal", () => {
    expect(settle(panes("three", ["a", "b", "c"], 2), ["a"])).toEqual(panes("one", ["a"], 0, ["a"]));
    expect(settle(panes("grid", ["a", "b", "c", "d"], 3), [])).toEqual(panes("one", [], 0, []));
    expect(focused(settle(panes("grid", ["a", "b", "c", "d"], 3), []))).toBeNull();
  });
  it("nothing closed: the panes stay as they are, with the tabs of now", () => {
    expect(settle(panes("two", ["a", "b"], 1, ["a", "b"]), ["a", "b", "c"])).toEqual(panes("two", ["a", "b"], 1, ["a", "b", "c"]));
  });
});

describe("the page's panes (pagePanes)", () => {
  it("a folder's first visit: One terminal, the address's terminal", () => {
    expect(pagePanes(undefined, ["a", "b"], "b")).toEqual(panes("one", ["b"], 0, ["a", "b"]));
    expect(pagePanes(undefined, ["a"], null)).toBeUndefined();
  });
  it("a new address follows the tab rule on the settled panes", () => {
    const p = panes("two", ["a", "b"], 0, ["a", "b", "c"]);
    expect(pagePanes(p, ["a", "b", "c"], "b")).toMatchObject({ ids: ["a", "b"], focus: 1 });
    expect(pagePanes(p, ["a", "b", "c"], "c")).toMatchObject({ ids: ["c", "b"], focus: 0 });
    // an address whose terminal closed is not put back
    expect(pagePanes(p, ["b", "c"], "a")).toMatchObject({ ids: ["c", "b"], focus: 0 });
  });
  it("focused gives the focused pane's terminal, or null", () => {
    expect(focused(undefined)).toBeNull();
    expect(focused(panes("two", ["a", "b"], 1))).toBe("b");
    expect(focused(onePane("x", ["x"]))).toBe("x");
  });
});

describe("the store: per folder, in memory while Hersir runs", () => {
  afterEach(() => resetPanes());
  it("each folder has its own panes; a folder not yet visited has none (it starts with One terminal)", () => {
    updatePanes("/gizai", () => panes("two", ["a", "b"]));
    expect(panesOf("/gizai")?.layout).toBe("two");
    expect(panesOf("/kade")).toBeUndefined();
    expect(pagePanes(panesOf("/kade"), ["k1"], "k1")?.layout).toBe("one");
  });
  it("updatePanes leaves the store as it is when nothing changes, and can forget a folder", () => {
    const first = panes("two", ["a", "b"]);
    updatePanes("/gizai", () => first);
    updatePanes("/gizai", () => ({ ...first, ids: [...first.ids] }));
    expect(panesOf("/gizai")).toBe(first);
    expect(samePanes(first, { ...first, focus: 1 })).toBe(false);
    expect(samePanes(first, { ...first, tabs: ["a", "b", "c"] })).toBe(false);
    updatePanes("/gizai", () => undefined);
    expect(panesOf("/gizai")).toBeUndefined();
  });
  it("nothing goes in localStorage (terminals don't survive a restart)", () => {
    const code = source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
    expect(code).toContain("new Map<string, Panes>()");
    expect(code).not.toMatch(/localStorage|sessionStorage|\bsetItem\s*\(|\bgetItem\s*\(/);
  });
});
