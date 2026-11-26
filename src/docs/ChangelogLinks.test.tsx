// GA-362 QA: a link in a change opens outside Hersir, never in its window. The changelog's lines (the Changelog section,
// and Recent changes on Home) are drawn through Prose, whose click opens an http(s) link with the host's openExternal and
// keeps every other link (a script, a file, an address of Hersir's) from going anywhere. There is no DOM here: the page
// is drawn to HTML for what it shows, and for the clicks the Changelog section is called as a function with the Docs
// context given to useContext, and each Prose it holds is called too, its click tried on the element it returns.
import { afterEach, describe, expect, it, vi } from "vitest";
import type { MouseEvent, ReactElement, ReactNode } from "react";
import { renderToStaticMarkup } from "react-dom/server";
import { parseDocsPath } from "./route";
import { sampleIndex } from "./sampleIndex.test-util";
import type { DocsHost, Index } from "./types";
import type { DocsCtx } from "./ui";

const h = vi.hoisted(() => ({ ctx: null as unknown }));
// useContext outside a render (a section called as a function) gives the context set here; inside one, React's own
vi.mock("react", async (orig) => {
  const R = await orig<typeof import("react")>();
  const useContext = ((c: unknown) => (h.ctx ?? R.useContext(c as never))) as typeof R.useContext;
  return { ...R, default: { ...R, useContext }, useContext };
});

const { DocsBody } = await import("./DocsView");
const { Changelog } = await import("./Sections");
const { Prose } = await import("./ui");

const opened: string[] = [];
const host: DocsHost = {
  index: () => Promise.resolve(sampleIndex()), source: (path) => Promise.resolve({ path, lines: [] }), commit: () => Promise.resolve(null),
  agentsUrl: (path) => path, openExternal: (url) => { opened.push(url); }, copy: () => Promise.resolve(),
};
const LINKED = 'Exports fixed (<a href="https://github.com/acme/kade/pull/12">#12</a>, <a href="javascript:alert(1)">x</a>, '
  + '<a href="file:///etc/passwd">f</a>, <a href="tauri://localhost/#/board">b</a>, <a href="HTTP://EXAMPLE.COM/A">e</a>).';
const linked = (): Index => {
  const index = sampleIndex();
  index.changelog = [{ version: "1.2.0", date: "2026-11-24", unreleased: false, compareUrl: "", changes: [
    { type: "Fixed", text: "Exports fixed.", html: LINKED }] }, ...index.changelog];
  return index;
};
const draw = (path: string, index: Index) =>
  renderToStaticMarkup(<DocsBody index={index} host={host} route={parseDocsPath(path)} prefix="#/devdocs/p1/" navigate={() => {}} />);

type El = ReactElement<{ children?: ReactNode; html?: string; className?: string; onClick?: (ev: MouseEvent<HTMLDivElement>) => void }>;
/** The elements of `type` in an element tree, without drawing the components in it. */
function find(n: ReactNode, type: unknown, found: El[] = []): El[] {
  if (!n || typeof n !== "object") return found;
  if (Array.isArray(n)) { n.forEach((c: ReactNode) => find(c, type, found)); return found; }
  const el = n as El;
  if (el.type === type) found.push(el);
  find(el.props?.children, type, found);
  return found;
}

/** A click on a link to `href` inside the Prose of the change with `html`, in the Changelog section: whether it was kept
 *  from going anywhere. A click beside the links when `href` is null. */
function click(href: string | null, html = LINKED): boolean {
  const index = linked();
  h.ctx = { index, route: parseDocsPath("changelog"), host, link: () => "#", go: () => {} } satisfies DocsCtx;
  try {
    const prose = find(Changelog(), Prose).find((p) => p.props.html === html);
    expect(prose, "the change is drawn through Prose").toBeTruthy();
    const div = Prose(prose!.props as { html: string; className?: string }) as El;
    const anchor = href == null ? null : { getAttribute: (name: string) => (name === "href" ? href : null) };
    const preventDefault = vi.fn();
    div.props.onClick!({ target: { closest: (sel: string) => (sel === "a" ? anchor : null) }, preventDefault } as unknown as MouseEvent<HTMLDivElement>);
    return preventDefault.mock.calls.length > 0;
  } finally {
    h.ctx = null;
  }
}

afterEach(() => { opened.length = 0; h.ctx = null; });

describe("a link in a change (GA-362)", () => {
  it("is drawn through Prose in the Changelog and in Recent changes on Home", () => {
    expect(draw("changelog", linked())).toContain(`<div class="dd-prose grow">${LINKED}</div>`);
    expect(draw("", linked())).toContain(`<div class="dd-prose grow">${LINKED}</div>`);
    expect(draw("changelog", linked())).not.toContain(`<span class="grow">${LINKED}`);
  });

  it("opens an http(s) link outside, and never lets it go in Hersir's window", () => {
    expect(click("https://github.com/acme/kade/pull/12")).toBe(true);
    expect(click("HTTP://EXAMPLE.COM/A")).toBe(true);
    expect(opened).toEqual(["https://github.com/acme/kade/pull/12", "HTTP://EXAMPLE.COM/A"]);
  });

  it("keeps a script, a file or one of Hersir's addresses from going anywhere, and opens none of them", () => {
    for (const href of ["javascript:alert(1)", "file:///etc/passwd", "tauri://localhost/#/board", "/p/p1/", "evil.example"]) {
      expect(click(href), href).toBe(true);
    }
    expect(opened).toEqual([]);
  });

  it("leaves a click beside the links alone", () => {
    expect(click(null)).toBe(false);
    expect(opened).toEqual([]);
  });
});
