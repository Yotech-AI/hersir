// The Terminal page's panes (GA-348): a folder's terminals side by side, 2, 3 or 4 in a row, or 2 × 2, with one of them
// focused (the page's terminal: its tab is selected, the address is its id, the top bar is about it). Kept per folder in
// memory while Hersir runs: terminals don't survive a restart, so nothing goes in localStorage. Each folder starts with One
// terminal.
// - The rules are pure functions (tested in Node): which terminals fill a layout, which pane a terminal shown on the page
//   goes to (the tab rule), what happens when a terminal in a pane closes, and stepping down.
// - The store is a map from a folder (an EditorFolder id) to its panes; the page reads it with useFolderPanes.
import { useSyncExternalStore } from "react";

export type Layout = "one" | "two" | "three" | "four" | "grid";

/** The layouts in the Layout menu, word for word, with how many panes each shows. */
export const LAYOUTS: readonly { id: Layout; label: string; count: number }[] = [
  { id: "one", label: "One terminal", count: 1 },
  { id: "two", label: "2 side by side", count: 2 },
  { id: "three", label: "3 side by side", count: 3 },
  { id: "four", label: "4 side by side", count: 4 },
  { id: "grid", label: "2 × 2", count: 4 },
];

export const paneCount = (l: Layout): number => LAYOUTS.find((x) => x.id === l)?.count ?? 1;
export const layoutLabel = (l: Layout): string => LAYOUTS.find((x) => x.id === l)?.label ?? "One terminal";

/** A folder's panes.
 *  - `ids`: the terminal in each pane, left to right (2 × 2: the first row, then the second). Shorter than the layout's
 *    count only while the missing terminals start; the panes without one wait for theirs.
 *  - `focus`: the focused pane (an index in `ids`).
 *  - `tabs`: the folder's terminals as last seen, so a pane whose terminal closed takes the next one (nextAfterClose's
 *    order). */
export type Panes = { layout: Layout; ids: string[]; focus: number; tabs: string[] };

/** A folder's start: One terminal, the one on the page. */
export const onePane = (id: string, tabs: string[]): Panes => ({ layout: "one", ids: [id], focus: 0, tabs });

/** The layout one pane smaller, when a pane goes: 4 → 3 → 2 → One terminal; 2 × 2 → 3 side by side. */
export function stepDown(l: Layout): Layout {
  return l === "four" || l === "grid" ? "three" : l === "three" ? "two" : "one";
}

/** The layout that keeps `n` panes of `l`, when not all of its terminals could start: `l` stepped down until it fits. */
export function fitLayout(l: Layout, n: number): Layout {
  let at = l;
  while (at !== "one" && paneCount(at) > n) at = stepDown(at);
  return at;
}

/** How many terminals the folder lacks for the layout: the ones choosing it starts. */
export const missingFor = (l: Layout, tabs: string[]): number => Math.max(0, paneCount(l) - tabs.length);

/** Choosing a layout: the terminal on the page (`current`) in the first pane, then the folder's next tabs, round to the
 *  first, as many as the layout has panes; the first pane is focused. The terminals that drop out stay open as tabs.
 *  `missing`: how many the folder lacks, to start one after the other (each in the next pane, addPane). */
export function choose(tabs: string[], current: string, l: Layout): { panes: Panes; missing: number } {
  const i = tabs.indexOf(current);
  const round = i < 0 ? [current, ...tabs] : [...tabs.slice(i), ...tabs.slice(0, i)];
  const ids = round.slice(0, paneCount(l));
  return { panes: { layout: l, ids, focus: 0, tabs }, missing: paneCount(l) - ids.length };
}

/** A terminal shown on the page (its address, a tab, Ctrl+PgDn and Ctrl+PgUp): in a pane, that pane is focused; else it
 *  comes into the focused pane, and the one that was there stays open as a tab. */
export function showTerminal(p: Panes, id: string): Panes {
  const i = p.ids.indexOf(id);
  if (i >= 0) return i === p.focus ? p : { ...p, focus: i };
  if (!p.ids.length) return { ...p, layout: "one", ids: [id], focus: 0 };
  const focus = Math.min(Math.max(p.focus, 0), p.ids.length - 1);
  return { ...p, ids: p.ids.map((x, k) => (k === focus ? id : x)), focus };
}

/** A terminal the layout started: in the next pane, while the layout has one without a terminal. */
export function addPane(p: Panes, id: string): Panes {
  if (p.ids.includes(id) || p.ids.length >= paneCount(p.layout)) return p;
  return { ...p, ids: [...p.ids, id] };
}

/** Start again: the new terminal in the ended one's pane. */
export function swapIn(p: Panes, old: string, id: string): Panes {
  if (!p.ids.includes(old) || p.ids.includes(id)) return p;
  return { ...p, ids: p.ids.map((x) => (x === old ? id : x)) };
}

/** The layout couldn't get all its terminals: it keeps the panes that got one. */
export function keepStarted(p: Panes): Panes {
  return { ...p, layout: fitLayout(p.layout, p.ids.length) };
}

/** The folder's tab after `id` among `spare`, in nextAfterClose's order on `was`: the next tab, else the one before it, and
 *  so on (src/lib/terminals.ts). Tabs that opened since `was` come after it, as they do in the tab strip. */
function nextSpare(was: string[], id: string, spare: string[]): string | null {
  const all = [...was, ...spare.filter((x) => !was.includes(x))];
  const i = all.indexOf(id);
  const order = i < 0 ? all : [...all.slice(i + 1), ...all.slice(0, i).reverse()];
  return order.find((x) => spare.includes(x)) ?? null;
}

/** The panes with the folder's terminals open now (`now`, in their order). A pane whose terminal closed (×, exit, the
 *  sidebar) takes the folder's next terminal that isn't in a pane; with none, the pane goes and the layout steps down.
 *  The focused pane stays on its terminal; when that closed, the pane that took its place is focused, or the one left
 *  of it when it went. */
export function settle(p: Panes, now: string[]): Panes {
  let { layout, focus } = p;
  const ids = [...p.ids];
  for (const gone of p.ids.filter((x) => !now.includes(x))) {
    const i = ids.indexOf(gone);
    const take = nextSpare(p.tabs, gone, now.filter((x) => !ids.includes(x)));
    if (take) { ids[i] = take; continue; }
    ids.splice(i, 1);
    layout = stepDown(layout);
    if (focus > i || (focus === i && i > 0)) focus -= 1;
  }
  focus = ids.length ? Math.min(Math.max(focus, 0), ids.length - 1) : 0;
  if (!ids.length) layout = "one";
  return { layout, ids, focus, tabs: now };
}

export const samePanes = (a: Panes | undefined, b: Panes | undefined): boolean =>
  a === b || (!!a && !!b && a.layout === b.layout && a.focus === b.focus && a.ids.join("\n") === b.ids.join("\n") && a.tabs.join("\n") === b.tabs.join("\n"));

/** The panes the page shows: the stored ones (or One terminal), settled on the folder's terminals open now, with the
 *  terminal of a new address (`show`) placed by the tab rule. Undefined when there is nothing to show. */
export function pagePanes(stored: Panes | undefined, now: string[], show: string | null): Panes | undefined {
  let p = stored ? settle(stored, now) : show ? onePane(show, now) : undefined;
  if (p && show && now.includes(show)) p = showTerminal(p, show);
  return p;
}

/** The terminal of the focused pane, or null when the folder has none in a pane. */
export const focused = (p: Panes | undefined): string | null => (p ? p.ids[p.focus] ?? null : null);

// ---------- the store: per folder, while Hersir runs ----------

const byFolder = new Map<string, Panes>();
const subs = new Set<() => void>();
const subscribe = (f: () => void) => { subs.add(f); return () => { subs.delete(f); }; };

/** The folder's panes now, or undefined before its first visit. */
export const panesOf = (folder: string): Panes | undefined => byFolder.get(folder);

/** Changes the folder's panes; nothing happens (and nobody hears of it) when they stay the same. */
export function updatePanes(folder: string, f: (p: Panes | undefined) => Panes | undefined): void {
  const was = byFolder.get(folder);
  const next = f(was);
  if (samePanes(was, next)) return;
  if (next) byFolder.set(folder, next);
  else byFolder.delete(folder);
  subs.forEach((g) => g());
}

/** The folder's panes, for a component (undefined for none, or before its first visit). */
export function useFolderPanes(folder: string | null): Panes | undefined {
  const get = () => (folder ? byFolder.get(folder) : undefined);
  return useSyncExternalStore(subscribe, get, get);
}

/** Forgets every folder's panes (the tests). */
export function resetPanes(): void {
  byFolder.clear();
  subs.forEach((g) => g());
}
