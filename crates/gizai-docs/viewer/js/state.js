/**
 * The index, the URL and everything derived from both.
 *
 * Deep links are the contract: every entry and every line has one, and it
 * has to resolve on a cold reload, not just on a click. So the URL is the
 * state, and the page is rebuilt from it.
 *
 *   #/reference/app/Http/Middleware/RestrictToIpAddresses.php#Account.owner
 *   #/reference?kind=class,middleware
 *   #/decisions/0119
 *   #/guides/01-install
 */

import { toast } from './ui.js';

export const state = {
  index: null,
  /** reference · guides · api · changelog · decisions · mcp */
  tab: 'reference',
  /** Entry id, decision id, guide id, endpoint id. */
  item: '',
  /** Folder path filter, Reference only. */
  folder: '',
  /** Kinds selected in the filter, Reference only. */
  kinds: [],
  /** Line to scroll to and mark, from #L51. */
  line: 0,
  /** "no" shows only entries with nothing written above them. */
  described: '',
  /** A bucket from docChanges, or "any". Empty means do not filter. */
  changed: '',
  /** 1-based. The Reference list is long enough to need pages. */
  page: 1,
};

const listeners = new Set();
export const onChange = (fn) => listeners.add(fn);
const announce = () => {
  for (const fn of listeners) fn();
};

export async function load() {
  const res = await fetch('index.json');
  if (!res.ok) throw new Error(`index.json returned ${res.status}`);
  state.index = await res.json();
  return state.index;
}

/**
 * The top bar tabs: id, label, count, icon. A count of null means no count
 * is shown, which is only Home.
 *
 * Every section is listed, including the ones this repo has nothing for.
 * They used to be dropped, which read as "this project has no decisions"
 * and "yodocs cannot show decisions" at the same time, and nobody could
 * tell which. An empty tab is dimmed and still opens, onto a page that says
 * which file to add. It is also how the MCP tab hid a real bug: with the
 * tab absent, a miscounted section looked exactly like an absent one.
 *
 * The order and the icons are the Top bar component in 01 Components. Home
 * is ours: the design reaches it through the logo alone, which is not
 * discoverable.
 */
export function tabs() {
  const i = state.index;
  return [
    ['home', 'Home', null, 'home'],
    ['guides', 'Guides', i.guides.length, 'book'],
    ['reference', 'Reference', i.entries.length, 'code'],
    ['api', 'API', i.api ? i.api.endpoints.length : 0, 'globe'],
    ['changelog', 'Changelog', i.changelog.reduce((n, r) => n + r.changes.length, 0), 'history'],
    ['decisions', 'Decisions', i.decisions.length, 'scale'],
    // Servers plus tools. Counting servers alone hid the tab on a project
    // with no .mcp.json and 11 tools of its own,
    // so the only page that listed them could not be reached.
    ['mcp', 'MCP', i.mcp.length + (i.mcpTools?.length ?? 0), 'plug'],
  ].filter(([id]) => !isOff(id));
}

/**
 * The DOCS_ key that switched a section off, or null when it is on.
 *
 * A switched-off section has no tab at all, where an empty one keeps its
 * tab, dimmed (YD-094). Empty means the repo has none. Off means the
 * project chose to leave it out, and the build wrote nothing for it.
 */
export function offKey(section) {
  return (state.index?.switchedOff ?? []).find((o) => o.section === section)?.key ?? null;
}
export const isOff = (section) => offKey(section) !== null;

/* ---------- the URL ---------- */

export function readUrl() {
  const raw = location.hash.replace(/^#/, '');
  const [path = '', query = ''] = raw.split('?');
  const params = new URLSearchParams(query);
  const parts = path.split('/').filter((p) => p !== '');

  // An empty path is Home, not the Reference list. It used to resolve to
  // 'reference' and main.js painted Home over the top once at startup, so
  // clicking the logo after that landed on the Reference list instead.
  state.tab = parts[0] !== undefined && parts[0] !== '' ? parts[0] : 'home';
  state.item = parts.length > 1 ? decodeURIComponent(parts.slice(1).join('/')) : '';
  state.folder = params.get('folder') ?? '';
  state.kinds = (params.get('kind') ?? '').split(',').filter((k) => k !== '');
  state.described = params.get('described') ?? '';
  state.changed = params.get('changed') ?? '';
  state.page = Math.max(1, Number(params.get('page') ?? 1) || 1);

  // A line reference rides on the item: …Account.owner@L51
  const atLine = /@L(\d+)$/.exec(state.item);
  state.line = atLine === null ? 0 : Number(atLine[1]);
  if (atLine !== null) state.item = state.item.slice(0, atLine.index);
}

export function href(patch = {}) {
  // Changing what is listed resets the page, unless the caller is
  // explicitly paging. Otherwise filtering from page 9 lands on an empty one.
  const paging = Object.prototype.hasOwnProperty.call(patch, 'page');
  const next = {
    tab: state.tab, item: state.item, folder: state.folder,
    kinds: state.kinds, described: state.described, changed: state.changed,
    page: paging ? state.page : 1, ...patch,
  };
  const params = new URLSearchParams();
  if (next.folder) params.set('folder', next.folder);
  if (next.kinds.length > 0) params.set('kind', next.kinds.join(','));
  if (next.described) params.set('described', next.described);
  if (next.changed) params.set('changed', next.changed);
  if (next.page > 1) params.set('page', String(next.page));
  const query = params.toString();
  const item = next.item ? `/${encodeURIComponent(next.item)}` : '';
  // Home is "#/", not "#/home", so the logo's href and the URL agree.
  const tab = next.tab === 'home' ? '' : next.tab;
  return `#/${tab}${item}${query ? `?${query}` : ''}`;
}

export function go(patch) {
  location.hash = href(patch);
}

/**
 * The section a link points at, when it points inside the current page.
 *
 * The whole address after # is the route here, so a plain "#summary" link
 * does not scroll: it replaces the route with one that does not exist, and
 * the reader lands on Home. Routes start "#/", so anything else is a section.
 */
export function sectionOf(link) {
  return /^#[^/]/.test(link ?? '') ? decodeURIComponent(link.slice(1)) : null;
}

window.addEventListener('hashchange', () => {
  readUrl();
  announce();
});

/* ---------- derived ---------- */

export const entryById = (id) => state.index.entries.find((e) => e.id === id);

/**
 * How each entry moved since the baseline, keyed by id.
 *
 * Built once per index rather than per row: the Reference list draws 100
 * rows a page and five array scans each would be 500 passes over a list
 * that is 3,257 long on a large project.
 */
let movedCache = null;
export function moved() {
  if (movedCache !== null && movedCache.index === state.index) return movedCache.map;
  const map = new Map();
  const d = state.index.docChanges;
  if (d !== null && d !== undefined) {
    for (const [bucket, list] of [
      ['documented', d.documented],
      ['undocumented', d.undocumented],
      ['added', d.added],
      ['changed', d.changed],
    ]) {
      for (const e of list) map.set(e.id, bucket);
    }
  }
  movedCache = { index: state.index, map };
  return map;
}

/** Entries after the folder, kind, description and changed filters. */
export function filtered() {
  const { folder, kinds, described, changed } = state;
  const how = changed === '' ? null : moved();
  return state.index.entries.filter(
    (e) =>
      (folder === '' || e.file === folder || e.file.startsWith(`${folder}/`)) &&
      (kinds.length === 0 || kinds.includes(e.kind)) &&
      (described === '' || (described === 'no' ? !e.hasDescription : e.hasDescription)) &&
      (how === null || (changed === 'any' ? how.has(e.id) : how.get(e.id) === changed)),
  );
}

const sourceCache = new Map();
export async function sourceFor(path) {
  if (!sourceCache.has(path)) {
    const res = await fetch(`source/${path}.json`);
    sourceCache.set(path, res.ok ? await res.json() : { path, lines: [] });
  }
  return sourceCache.get(path);
}

/* ---------- theme ---------- */
// ui.js imports nothing, so this cannot cycle back.

/**
 * Can this browser keep anything for this address?
 *
 * Checked once, by writing and reading back, because a browser that blocks
 * site data can accept the write and still hand back null. The answer is
 * shown in the sidebar instead of being swallowed: a theme that silently
 * forgets itself every reload reads as a broken page, and the cause is in
 * browser settings nobody thinks to look at.
 */
let storageAnswer = null;
export function storageWorks() {
  if (storageAnswer !== null) return storageAnswer;
  try {
    localStorage.setItem('yd-probe', '1');
    storageAnswer = localStorage.getItem('yd-probe') === '1';
    localStorage.removeItem('yd-probe');
  } catch {
    storageAnswer = false;
  }
  return storageAnswer;
}

export function theme() {
  return document.documentElement.dataset.theme;
}

/**
 * Switch the theme, and say so when the browser refuses to remember it.
 *
 * This used to swallow the failure. The theme then flipped, the next reload
 * put it back, and nothing anywhere said why: a private window, Brave
 * Shields set to block cookies for the site, or "clear site data on close"
 * all look identical from in here. Writing and reading back is the only way
 * to know, because some browsers accept the write and drop it later.
 */
export function toggleTheme() {
  const next = theme() === 'dark' ? 'light' : 'dark';
  document.documentElement.dataset.theme = next;

  let kept = false;
  try {
    localStorage.setItem('yd-theme', next);
    kept = localStorage.getItem('yd-theme') === next;
  } catch {
    kept = false;
  }
  if (!kept) {
    storageAnswer = false;
    toast('This browser will not store site data for this address, so the theme resets on reload.');
  }
  announce();
}
