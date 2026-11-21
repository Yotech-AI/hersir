import MiniSearch from '../vendor/minisearch.js';
import { esc, icon, kindBadge, methodBadge } from './ui.js';
import { href, state } from './state.js';

/**
 * Cmd+K search. YD-068. Hersir: the search index is built here, from the index the page already has, on the first
 * search (documents() is yodocs' own, from its src/output/search.ts), so there is no search.json.
 *
 * The index loads on the first open, not on page load: it is 1 MB on a
 * mid-sized Laravel project and 2.5 MB on a large one, and most visits never
 * search. Until it arrives the
 * dialog says so rather than looking broken.
 */

const OPTIONS = {
  fields: ['title', 'subtitle', 'body'],
  storeFields: ['section', 'title', 'subtitle', 'kind'],
  searchOptions: { boost: { title: 4, subtitle: 2 }, prefix: true, fuzzy: 0.2 },
};

/** The filters, without the sections this project switched off. */
const sections = () =>
  ['All', 'Reference', 'Guides', 'API', 'Changelog', 'Decisions', 'MCP'].filter(
    (s) => s === 'All' || !(state.index.switchedOff ?? []).some((o) => o.section === s.toLowerCase()),
  );
const RECENT_KEY = 'yd-recent';

let mini = null;
let loading = null;
let results = [];
let cursor = 0;
let filter = 'All';

const el = {};

function recent() {
  try {
    return JSON.parse(localStorage.getItem(RECENT_KEY) ?? '[]');
  } catch {
    return [];
  }
}
function remember(q) {
  if (q.trim() === '') return;
  try {
    localStorage.setItem(RECENT_KEY, JSON.stringify([q, ...recent().filter((r) => r !== q)].slice(0, 5)));
  } catch {
    /* a private window searches without remembering */
  }
}

/** Rendered HTML back to words, so guide bodies are searchable. */
const words = (html) => html.replace(/<[^>]+>/g, ' ').replace(/&[a-z]+;/gi, ' ').replace(/\s+/g, ' ').trim();

/** One search document per entry, guide, decision, change, MCP server and endpoint. */
function documents(index) {
  const docs = [];
  for (const e of index.entries) {
    docs.push({ id: e.id, section: 'reference', title: e.name, subtitle: e.file, kind: e.kind,
      body: `${e.signature} ${e.description} ${e.tags.map((t) => t.text).join(' ')}` });
  }
  for (const g of index.guides) {
    docs.push({ id: g.id, section: 'guides', title: g.title, subtitle: g.file, kind: 'guide',
      body: `${g.headings.map((h) => h.text).join(' ')} ${words(g.html)}` });
  }
  for (const d of index.decisions) {
    docs.push({ id: d.id, section: 'decisions', title: `${d.number} ${d.title}`, subtitle: d.file, kind: d.status, body: words(d.html) });
  }
  for (const r of index.changelog) {
    for (const [i, c] of r.changes.entries()) {
      docs.push({ id: `changelog/${r.version}/${i}`, section: 'changelog', title: c.text, subtitle: `${r.version} ${r.date}`,
        kind: c.type.toLowerCase(), body: '' });
    }
  }
  for (const m of index.mcp) {
    docs.push({ id: `mcp/${m.name}`, section: 'mcp', title: m.name, subtitle: m.target, kind: m.transport,
      body: `${m.args.join(' ')} ${m.envNames.join(' ')} ${words(m.purpose)}` });
  }
  if (index.api !== null) {
    for (const e of index.api.endpoints) {
      docs.push({ id: e.id, section: 'api', title: `${e.method} ${e.path}`, subtitle: e.tags.join(', '), kind: e.method.toLowerCase(),
        body: `${e.summary} ${e.description}` });
    }
  }
  return docs;
}

async function ensureIndex() {
  if (mini !== null) return mini;
  if (loading === null) {
    loading = Promise.resolve().then(() => {
      mini = new MiniSearch(OPTIONS);
      mini.addAll(documents(state.index));
      return mini;
    });
  }
  return loading;
}

const linkFor = (hit) => {
  if (hit.section === 'changelog') return '#/changelog';
  if (hit.section === 'mcp') return '#/mcp';
  return href({ tab: hit.section, item: hit.id, folder: '', kinds: [] });
};

function draw() {
  const q = el.input.value.trim();

  if (q === '') {
    const past = recent();
    el.list.innerHTML = past.length
      ? `<div class="s-head">Recent</div>${past.map((r) => `<button class="s-recent" data-q="${esc(r)}">${icon('history', 14)} ${esc(r)}</button>`).join('')}`
      : `<div class="s-hint">Search entries, guides, decisions, changes and MCP servers.
           <div class="sm muted">${state.index.stats.entries} entries in this build.</div></div>`;
    return;
  }

  if (mini === null) {
    el.list.innerHTML = `<div class="s-hint">${icon('search', 14)} Loading the index…
      <div class="sm muted">It is fetched the first time you search, not on every page load.</div></div>`;
    return;
  }

  const all = mini.search(q);
  results = (filter === 'All' ? all : all.filter((r) => r.section === filter.toLowerCase())).slice(0, 40);
  cursor = 0;

  if (results.length === 0) {
    el.list.innerHTML = `<div class="s-empty">
        <strong>No results for "${esc(q)}"</strong>
        <div class="sm muted">${filter === 'All' ? 'Nothing in this build matches.' : `Nothing in ${filter}. Try All, or a different word.`}</div>
      </div>`;
    return;
  }

  el.list.innerHTML = results
    .map(
      (r, n) => `<a class="s-hit${n === 0 ? ' on' : ''}" href="${linkFor(r)}" data-n="${n}">
        <span class="grow">
          <span class="mono name">${esc(r.title)}</span>
          ${r.section === 'api' ? methodBadge(r.kind.toUpperCase()) : kindBadge(r.kind)}
          <span class="sm muted block mono">${esc(r.subtitle)}</span>
        </span>
        <span class="s-section">${esc(r.section)}</span>
      </a>`,
    )
    .join('');
}

function move(by) {
  const hits = [...el.list.querySelectorAll('.s-hit')];
  if (hits.length === 0) return;
  hits[cursor]?.classList.remove('on');
  cursor = (cursor + by + hits.length) % hits.length;
  hits[cursor].classList.add('on');
  hits[cursor].scrollIntoView({ block: 'nearest' });
}

export function openSearch(prefill = '') {
  el.dialog.hidden = false;
  el.input.value = prefill;
  el.input.focus();
  el.input.select();
  draw();
  void ensureIndex().then(draw);
}

function close() {
  el.dialog.hidden = true;
}

export function mountSearch() {
  el.dialog = document.getElementById('search-dialog');
  el.input = document.getElementById('search-input');
  el.list = document.getElementById('search-list');
  el.tabs = document.getElementById('search-tabs');

  el.tabs.innerHTML = sections().map(
    (s) => `<button class="s-tab${s === filter ? ' on' : ''}" data-section="${s}">${s}</button>`,
  ).join('');

  el.tabs.addEventListener('click', (ev) => {
    const b = ev.target.closest('[data-section]');
    if (b === null) return;
    filter = b.dataset.section;
    for (const t of el.tabs.querySelectorAll('.s-tab')) t.classList.toggle('on', t === b);
    draw();
  });

  el.input.addEventListener('input', draw);

  el.list.addEventListener('click', (ev) => {
    if (ev.target.closest('.s-recent') !== null) {
      el.input.value = ev.target.closest('.s-recent').dataset.q;
      el.input.focus();
      draw();
      return;
    }
    if (ev.target.closest('.s-hit') !== null) {
      remember(el.input.value.trim());
      close();
    }
  });

  el.dialog.addEventListener('click', (ev) => {
    if (ev.target === el.dialog) close();
  });

  document.addEventListener('keydown', (ev) => {
    const typing = /^(INPUT|TEXTAREA)$/.test(ev.target.tagName);

    if ((ev.key === 'k' && (ev.metaKey || ev.ctrlKey)) || (ev.key === '/' && !typing)) {
      ev.preventDefault();
      openSearch();
      return;
    }
    if (el.dialog.hidden) return;

    if (ev.key === 'Escape') {
      ev.preventDefault();
      close();
    } else if (ev.key === 'ArrowDown') {
      ev.preventDefault();
      move(1);
    } else if (ev.key === 'ArrowUp') {
      ev.preventDefault();
      move(-1);
    } else if (ev.key === 'Tab') {
      ev.preventDefault();
      const list = sections();
      const at = list.indexOf(filter);
      filter = list[(at + (ev.shiftKey ? -1 : 1) + list.length) % list.length];
      for (const t of el.tabs.querySelectorAll('.s-tab')) t.classList.toggle('on', t.dataset.section === filter);
      draw();
    } else if (ev.key === 'Enter') {
      const hit = el.list.querySelectorAll('.s-hit')[cursor];
      if (hit !== undefined) {
        ev.preventDefault();
        remember(el.input.value.trim());
        location.hash = hit.getAttribute('href').replace(/^#/, '');
        close();
      }
    }
  });
}
