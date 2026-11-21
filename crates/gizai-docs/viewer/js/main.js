import { esc, icon } from './ui.js';
import { go, href, isOff, load, offKey, onChange, readUrl, sectionOf, state, tabs, theme, toggleTheme } from './state.js';
import { renderSidebar } from './sidebar.js';
import { entry, home, list, wireCopy } from './reference.js';
import { api, changelog, commits, decisions, fillCommit, guides, mcp } from './sections.js';
import { mountSearch, openSearch } from './search.js';

/**
 * The shell and the router. YD-060, and the states in YD-069.
 *
 * Layout from 00 Foundations: top bar 60, sidebar 288, content 880 for
 * Reference and 760 for guides, right rail 232. Below 1024 the sidebar
 * becomes a drawer and the rail hides, which the design does not draw.
 */

const el = {};

/* ---------- top bar ---------- */

function topBar() {
  const i = state.index;

  // No fallback to 'reference': on Home no tab is current, which is what the
  // design shows, and the logo is the way back.
  el.tabs.innerHTML = tabs()
    .map(
      // No counts in the bar, as in Figma. Home shows them, and the tooltip
      // says how many. An empty section stays, dimmed (YD-094).
      ([id, label, n, glyph]) => `<a class="tab${id === state.tab ? ' on' : ''}${n === 0 ? ' quiet' : ''}"
        href="${href({ tab: id, item: '', folder: '', kinds: [], described: '' })}"
        title="${esc(label)}${n === null ? '' : n === 0 ? ', nothing here yet' : `, ${n}`}">
        ${icon(glyph)}<span class="label">${esc(label)}</span></a>`,
    )
    .join('');

  el.brand.innerHTML = `<a href="#/" class="brand">
      <span class="mark">${icon('book')}</span>
      <span class="name">${esc(i.project.title.replace(/ dev docs$/i, ''))}</span>
      <span class="suffix">dev docs</span>
    </a>`;

  const commit = i.project.commit || i.project.branch || '';
  // The chip is the way in to the commit list. It has no tab of its own,
  // because seven tabs already do not fit a laptop (YD-094).
  const chip = `${icon('gitCommit', 12)}<span>${esc(commit)}</span>`;
  el.commit.innerHTML =
    commit === '' ? '' : isOff('commits') ? chip : `<a href="#/commits" title="What the last commits changed">${chip}</a>`;
  el.commit.hidden = commit === '';
  el.themeBtn.innerHTML = icon(theme() === 'dark' ? 'sun' : 'moon');
  fitTabs();
}

/**
 * Show as much of each tab as the bar has room for.
 *
 * The first fit that does not overflow wins: everything, then without the
 * "dev docs" after the name, then words without icons, then icons alone. It
 * measures, because the room left depends on the project's name: a short
 * one and a long one leave very different amounts, and no fixed width is
 * right for both.
 */
function fitTabs() {
  const bar = el.tabs.closest('header');
  for (const fit of ['full', 'tight', 'text', 'icons']) {
    bar.dataset.fit = fit;
    if (el.tabs.scrollWidth <= el.tabs.clientWidth) break;
  }
}

/* ---------- states ---------- */

const loading = `<div class="skeleton">
    ${'<div class="sk-line"></div>'.repeat(3)}
    <div class="sk-block"></div>
    ${'<div class="sk-line"></div>'.repeat(5)}
  </div>`;

function indexError(message) {
  return `<div class="empty">
      <span class="empty-ico danger">${icon('alert', 24)}</span>
      <h2>The docs index could not be loaded</h2>
      <p class="muted">Hersir couldn't build this project's docs, so there is nothing to show. Hersir said why
        below: fix that, then reload this page.</p>
      <p class="sm muted mono">${esc(message)}</p>
    </div>`;
}

/* ---------- routing ---------- */

async function view() {
  const { tab, item } = state;
  if (tab === 'home') return home();
  // An old link into a section that is now switched off says why it is empty.
  if (isOff(tab)) {
    return `<div class="empty">
        ${icon('inbox', 24)}
        <h2>Switched off in this project</h2>
        <p class="muted">This project left this section out of its docs in Hersir:
          <span class="mono">${esc(offKey(tab))}</span>.</p>
      </div>`;
  }
  if (tab === 'reference') return item === '' ? list() : entry(item);
  if (tab === 'guides') return guides(item);
  if (tab === 'decisions') return decisions(item);
  if (tab === 'changelog') return changelog();
  if (tab === 'commits') return commits();
  if (tab === 'mcp') return mcp(item);
  if (tab === 'api') return api(item);
  return home();
}

let drawing = 0;

async function render() {
  const mine = ++drawing;
  topBar();
  renderSidebar(el.side);

  el.body.innerHTML = loading;
  let html;
  try {
    html = await view();
  } catch (error) {
    html = `<div class="empty"><h2>That page could not be built</h2>
      <p class="sm muted mono">${esc(error.message)}</p></div>`;
  }
  if (mine !== drawing) return; // a newer navigation won

  el.body.innerHTML = html;
  el.body.classList.toggle('narrow', ['home', 'guides', 'decisions'].includes(state.tab));
  wireCopy(el.body);

  // A deep link lands on its anchor, and #L51 marks the line.
  const anchor = location.hash.split('#')[2];
  if (anchor !== undefined && anchor !== '') {
    el.body.querySelector(`#${CSS.escape(anchor)}`)?.scrollIntoView({ block: 'start' });
  } else {
    window.scrollTo(0, 0);
  }
  el.shell.classList.remove('drawer-open');
}

/* ---------- start ---------- */

async function start() {
  el.shell = document.getElementById('shell');
  el.brand = document.getElementById('brand');
  el.tabs = document.getElementById('tabs');
  el.side = document.getElementById('side');
  el.body = document.getElementById('body');
  el.commit = document.getElementById('commit');
  el.themeBtn = document.getElementById('theme');

  try {
    await load();
  } catch (error) {
    document.getElementById('body').innerHTML = indexError(error.message);
    return;
  }

  document.title = state.index.project.title;
  el.themeBtn.addEventListener('click', toggleTheme);
  document.getElementById('search-open').addEventListener('click', () => openSearch());
  document.getElementById('menu').addEventListener('click', () => el.shell.classList.toggle('drawer-open'));

  mountSearch();
  window.addEventListener('resize', fitTabs);
  onChange(render);
  readUrl();
  await render();
}

document.addEventListener('click', (ev) => {
  // "On this page" links scroll, and leave the address alone.
  const section = sectionOf(ev.target.closest('a[href^="#"]')?.getAttribute('href'));
  if (section !== null) {
    ev.preventDefault();
    document.getElementById(section)?.scrollIntoView({ block: 'start' });
    return;
  }

  const a = ev.target.closest('a[href^="#/"]');
  if (a !== null && a.getAttribute('href') === location.hash) {
    ev.preventDefault();
    render();
  }
});

// toggle does not bubble, so it is caught on the way down. A commit's lines
// are fetched the first time its chevron is opened, not with the page.
document.addEventListener(
  'toggle',
  (ev) => {
    if (ev.target.matches?.('details[data-commit]')) void fillCommit(ev.target);
  },
  true,
);

void start();
export { go };
