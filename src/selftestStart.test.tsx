// GA-300 QA: the self-test's start check (App.tsx, before any probe) looked only for the sidebar's rows (.side .nav-item), so
// a start with the sidebar closed (GA-291, kept on this computer) reported "sidebar did not render". It now takes the closed
// sidebar's toolbar too. Here: the check's selector has both, each half matches what its component draws (the toolbar
// rendered on the server, the sidebar's classes from its source), and a page with neither still fails the check.
import { describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";

vi.mock("./lib/useData", () => ({ useData: () => ({ data: [], error: null, reload: () => {}, setData: () => {} }) }));
vi.mock("./api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  return Object.fromEntries(Object.entries(real).map(([k, v]) => [k, typeof v === "function" ? () => new Promise(() => {}) : v]));
});

const fs = await import(/* @vite-ignore */ ["node", "fs"].join(":"));
const read = (path: string): string => fs.readFileSync(new URL(path, import.meta.url), "utf8");
const app = read("./App.tsx");
const { SidebarToolbar } = await import("./components/SidebarToolbar");

/** The selector the start check asks document.querySelector for. */
const startCheck = /if \(!document\.querySelector\("([^"]+)"\)\) errors\.push\("sidebar did not render"\);/.exec(app)?.[1];

/** Whether markup has an element with class `outer` that holds one with class `inner` (a descendant selector, on the
 *  server's markup: no DOM library in the project). */
function has(html: string, outer: string, inner: string): boolean {
  const open = new RegExp(`<(\\w+) [^>]*class="(?:[^"]* )?${outer}(?: [^"]*)?"`).exec(html);
  if (!open) return false;
  return new RegExp(`class="(?:[^"]* )?${inner}(?: [^"]*)?"`).test(html.slice(open.index + open[0].length));
}

describe("the self-test's start check (GA-300)", () => {
  it("is there once, and takes the open sidebar's rows or the closed sidebar's toolbar", () => {
    expect(app.match(/sidebar did not render/g)).toHaveLength(1);
    expect(startCheck?.split(",").map((s) => s.trim())).toEqual([".side .nav-item", ".side-tools .tool"]);
  });

  it("the closed sidebar's toolbar matches it: .side-tools with its four .tool buttons", () => {
    const html = renderToStaticMarkup(<SidebarToolbar youId="you" onShow={() => {}} onNewTask={() => {}} onSearch={() => {}} />);
    expect(html).toMatch(/^<div class="side-tools" role="group" aria-label="Sidebar actions">/);
    expect(has(html, "side-tools", "tool")).toBe(true);
    expect(html.match(/class="tool"/g)).toHaveLength(4);
    // and not the open sidebar's half: a start with the sidebar closed passed only by the toolbar
    expect(has(html, "side", "nav-item")).toBe(false);
  });

  it("the open sidebar still matches it: an aside .side with .nav-item rows", () => {
    const side = read("./components/Sidebar.tsx");
    expect(side).toMatch(/className=\{?[`"]side[\s`"$]/);
    expect(side).toMatch(/className=\{?[`"]nav-item/);
  });

  it("App draws one or the other: the toolbar when the sidebar is closed, else the sidebar", () => {
    expect(app).toMatch(/\{sideClosed\s*\? <SidebarToolbar [^\n]*\n\s*: <Sidebar /);
  });

  it("a page with neither still fails it", () => {
    expect(has('<div class="app"><main class="main"><div class="page-island"></div></main></div>', "side", "nav-item")).toBe(false);
    expect(has('<div class="app"><main class="main"><div class="page-island"></div></main></div>', "side-tools", "tool")).toBe(false);
  });
});
