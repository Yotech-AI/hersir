// GA-294 QA: a card's page in two islands (design/islands-pages). The card island's header holds Card, Changes and
// Screenshots as tabs (their tablist, links and aria-label as before); the error banner and the open view sit in the card
// island under that header; Properties, while it shows, is its own island beside the card island, with Properties and Close
// properties in its header. On Changes and Screenshots (Properties closed) the card island is alone in the row and Show
// properties is in the top bar. Rendered to HTML on the server with the data hooks answering at once, as in
// TaskPageChanges.test.tsx.
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import { renderToStaticMarkup } from "react-dom/server";
import type { Task, Team } from "../types";

const answers: Record<string, (...a: unknown[]) => unknown> = {};
vi.mock("../api", async (orig) => {
  const real = await orig<Record<string, unknown>>();
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(real)) out[k] = typeof v !== "function" ? v : (...a: unknown[]) => (answers[k] ? answers[k](...a) : new Promise(() => {}));
  return out;
});
vi.mock("../lib/useData", () => ({
  useData: (fetch: () => unknown) => {
    let data: unknown = null;
    try { const v = fetch(); if (!(v instanceof Promise)) data = v; } catch { /* no data */ }
    return { data, error: null, reload: () => {}, setData: () => {} };
  },
}));
vi.mock("../lib/useLiveRuns", () => ({ useLiveRuns: () => [] }));

const { TaskPage } = await import("./TaskPage");

const team: Team = { id: "team", name: "Software", members: [], labels: [], states: [{ id: "s-prog", name: "In progress", category: "started", sortKey: "a2" }] } as unknown as Team;
const task = {
  id: "t1", identifier: "GA-12", projectId: "p1", projectName: "Shop", projectColor: "#36f", title: "Export invoices", descriptionMd: "Write the CSV export.",
  acceptanceMd: "- [ ] it works", stateId: "s-prog", stateName: "In progress", stateCategory: "started", priority: 0, labels: [], bounceCount: 0,
  failCount: 0, sortKey: "a0", testing: true, createdAt: 0, updatedAt: 0, branch: "gizai/ga-12-export-invoices", screenshots: true,
} as unknown as Task;

let store: Map<string, string>;
beforeEach(() => {
  store = new Map();
  vi.stubGlobal("localStorage", { getItem: (k: string) => store.get(k) ?? null, setItem: (k: string, v: string) => void store.set(k, v), removeItem: (k: string) => void store.delete(k) });
  Object.assign(answers, { getTask: () => task, getTeam: () => team, listUsers: () => [], listComments: () => [], taskActivity: () => [], listRuns: () => [] });
});
afterEach(() => vi.unstubAllGlobals());

/** The element that starts at `open` (an exact start tag) with its whole content, by counting the same tag. */
function element(html: string, open: string): string {
  const at = html.indexOf(open);
  if (at < 0) return "";
  const tag = /^<(\w+)/.exec(open)?.[1] ?? "div";
  const re = new RegExp(`<${tag}[\\s>]|</${tag}>`, "g");
  re.lastIndex = at;
  let depth = 0;
  for (let m = re.exec(html); m; m = re.exec(html)) {
    depth += m[0].startsWith("</") ? -1 : 1;
    if (depth === 0) return html.slice(at, m.index + m[0].length);
  }
  return "";
}
const TABLIST = '<div class="tabs task-tabs" role="tablist" aria-label="The card, its changes and its screenshots">';

describe("a card's page in islands", () => {
  it("Card: the page is the card island and the Properties island side by side, in that order", () => {
    const html = renderToStaticMarkup(<TaskPage id="t1" />);
    const row = element(html, '<div class="task-page">');
    expect(row).not.toBe("");
    const card = element(row, '<div class="card-island">');
    const props = element(row, '<aside class="props" aria-label="Properties">');
    expect(card).not.toBe("");
    expect(props).not.toBe("");
    // the row holds exactly the two islands: the card's, then Properties'
    expect(row).toBe(`<div class="task-page">${card}${props}</div>`);
    // Properties isn't inside the card island
    expect(card).not.toContain('aria-label="Properties"');
  });

  it("Card: the tabs are the first thing in the card island, in its header; the card's doc is under it", () => {
    const html = renderToStaticMarkup(<TaskPage id="t1" />);
    const card = element(html, '<div class="card-island">');
    const head = element(card, '<div class="island-head">');
    expect(card.startsWith(`<div class="card-island">${head}`)).toBe(true);
    expect(head.startsWith(`<div class="island-head">${TABLIST}`)).toBe(true);
    expect(head).toMatch(/<a class="tab" role="tab" aria-selected="true" href="#\/task\/t1">.*Card<\/a>/);
    expect(head).toMatch(/<a class="tab" role="tab" aria-selected="false" href="#\/task\/t1\/changes"[^>]*>.*Changes/);
    // a card that takes screenshots has the Screenshots tab, in the same header
    expect(head).toMatch(/<a class="tab" role="tab" aria-selected="false" href="#\/task\/t1\/screenshots"[^>]*>.*Screenshots/);
    expect(card).toContain('<div class="split"><div class="doc"><div class="doc-in">');
    expect(card).toContain("Write the CSV export.");
    // the old strip under the top bar is gone: only one tablist with these tabs on the page
    expect(html.split(TABLIST).length).toBe(2);
  });

  it("Properties' header says Properties with Close properties (]); its rows are in props-body under it", () => {
    const html = renderToStaticMarkup(<TaskPage id="t1" />);
    const props = element(html, '<aside class="props" aria-label="Properties">');
    expect(props).toMatch(/^<aside class="props" aria-label="Properties"><div class="props-head">Properties<button class="btn ghost sm icon-only" aria-label="Close properties" title="Close properties \(\]\)">/);
    expect(props).toContain('<div class="props-body">');
    // Show properties isn't in the top bar while it shows
    expect(html).not.toContain('aria-label="Show properties"');
  });

  it("Changes: the Changes view inside the card island under the same header; no Properties island; Show properties in the top bar", () => {
    const html = renderToStaticMarkup(<TaskPage id="t1" view="changes" />);
    const row = element(html, '<div class="task-page">');
    const card = element(row, '<div class="card-island">');
    expect(row).toBe(`<div class="task-page">${card}</div>`);
    const head = element(card, '<div class="island-head">');
    expect(head.startsWith(`<div class="island-head">${TABLIST}`)).toBe(true);
    expect(head).toMatch(/aria-selected="true" href="#\/task\/t1\/changes"/);
    expect(card).toContain('<div class="split"><div class="chg">');
    expect(element(html, '<div class="topbar">')).toContain('aria-label="Show properties" title="Show properties (])"');
  });

  it("Screenshots: the sets inside the card island under the same header, Screenshots selected", () => {
    const html = renderToStaticMarkup(<TaskPage id="t1" view="screenshots" />);
    const card = element(html, '<div class="card-island">');
    const head = element(card, '<div class="island-head">');
    expect(head).toMatch(/aria-selected="true" href="#\/task\/t1\/screenshots"/);
    expect(head).toMatch(/aria-selected="false" href="#\/task\/t1"/);
    expect(card.indexOf(head)).toBe('<div class="card-island">'.length);
    expect(card).toContain('<div class="split">');
    expect(card).not.toContain('<div class="doc">');
  });

  it("with Properties closed on Card the card island is alone in the row", () => {
    store.set("gizai-props", "closed");
    const html = renderToStaticMarkup(<TaskPage id="t1" />);
    const row = element(html, '<div class="task-page">');
    expect(row).toBe(`<div class="task-page">${element(row, '<div class="card-island">')}</div>`);
    expect(html).toContain('aria-label="Show properties" title="Show properties (])"');
  });
});
