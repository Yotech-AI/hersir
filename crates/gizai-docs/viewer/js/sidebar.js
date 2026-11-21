import { esc, icon, kindVar, methodBadge, when } from './ui.js';
import { href, state, storageWorks } from './state.js';

/**
 * The Reference sidebar. YD-063.
 *
 * Every group comes from the index. No group name appears anywhere in this
 * file: a Flutter project has no routes and a Kotlin project has modules,
 * so the only thing the viewer knows is that there are N groups, each with
 * rows that are either a tree or a flat list.
 *
 * Folders is first because the index puts it first, not because this code
 * says so.
 */

const SHOW_AT_FIRST = 6;

function treeRow(row, depth) {
  const open = state.folder === row.target || state.folder.startsWith(`${row.target}/`);
  const hasKids = Array.isArray(row.children) && row.children.length > 0;
  const current = state.folder === row.target;
  const glyph = hasKids ? icon(open ? 'folderOpen' : 'folder') : icon('file');

  // Always into the Reference. The same sidebar is drawn on Home, and a
  // link that kept the current tab went to "#/?folder=…", which Home ignores.
  const label = `<a class="row${current ? ' on' : ''}" href="${href({ tab: 'reference', folder: current ? '' : row.target, item: '' })}"
      style="padding-left:${8 + depth * 14}px">
      <span class="twist">${hasKids ? icon(open ? 'chevronDown' : 'chevronRight', 14) : ''}</span>
      <span class="glyph">${glyph}</span>
      <span class="label">${esc(row.label)}</span>
      ${row.count > 0 ? `<span class="count">${row.count}</span>` : ''}
    </a>`;

  if (!hasKids) return label;
  return `${label}${open ? `<div class="kids">${row.children.map((c) => treeRow(c, depth + 1)).join('')}</div>` : ''}`;
}

function flatRow(row) {
  const current = state.item === row.target;
  const target = row.target === '' ? '' : href({ tab: 'reference', item: row.target });
  const tag = target === '' ? 'span' : 'a';
  const attr = target === '' ? '' : ` href="${target}"`;
  return `<${tag} class="row flat${current ? ' on' : ''}"${attr}>
      <span class="label">${esc(row.label)}</span>
      ${row.badge ? methodBadge(row.badge) : ''}
    </${tag}>`;
}

function group(g, openGroups) {
  const expanded = openGroups.has(g.name);
  const rows = g.type === 'tree' ? g.rows : g.rows.slice(0, expanded ? g.rows.length : SHOW_AT_FIRST);
  const body =
    g.type === 'tree' ? rows.map((r) => treeRow(r, 0)).join('') : rows.map(flatRow).join('');
  const more =
    g.type === 'flat' && g.rows.length > SHOW_AT_FIRST
      ? `<button class="show-all" data-group="${esc(g.name)}">
           ${expanded ? 'Show fewer' : `Show all ${g.rows.length} ${g.name.toLowerCase()}`}
           ${icon(expanded ? 'chevronDown' : 'chevronRight', 14)}
         </button>`
      : '';

  return `<div class="group">
      <h2 class="group-head">${esc(g.name)}${g.count > 0 ? ` <span class="count">· ${g.count}</span>` : ''}</h2>
      ${body}${more}
    </div>`;
}

function kindFilter() {
  const kinds = state.index.kinds;
  const picked = state.kinds;
  const total = state.index.entries.length;
  const label = picked.length === 0 ? 'All kinds' : picked.join(', ');
  const shown = picked.length === 0 ? total : state.index.entries.filter((e) => picked.includes(e.kind)).length;

  return `<div class="kind-filter">
      <button class="select" id="kind-toggle" aria-expanded="false">
        ${icon('filter')}
        <span class="label">${esc(label)}</span>
        <span class="count">${shown}</span>
        ${icon('chevronsUpDown', 14)}
      </button>
      <div class="kind-menu" id="kind-menu" hidden>
        ${kinds
          .map(
            (k) => `<label class="kind-opt">
              <input type="checkbox" value="${esc(k.name)}"${picked.includes(k.name) ? ' checked' : ''}>
              <span class="dot" style="background:${kindVar(k.name)}"></span>
              <span class="label">${esc(k.name)}</span>
              <span class="count">${k.count}</span>
            </label>`,
          )
          .join('')}
        ${picked.length > 0 ? '<button class="clear" id="kind-clear">Clear</button>' : ''}
      </div>
    </div>`;
}

/**
 * The sidebar footer from 01 Home: which build this is, the files an agent
 * reads, and what the bundle is.
 *
 * The design's last line reads "private, allowlisted IPs only". That is
 * true only when serve was started with --host, so the line goes by the
 * address the page was opened on and says which of the two it is.
 *
 * The design's third agent link is "Markdown". There is no index page for
 * the per-page Markdown, only a folder the server does not list, so this
 * links index.json, which has everything (Hersir builds the search in the page). Each Reference page links its own
 * Markdown.
 */
function footer() {
  const p = state.index.project;
  // The commit in code type, the date in plain type. All of it in code type
  // wrapped the date across two lines in a 288px column.
  const id = p.commit || p.branch;
  const built = when(p.builtAt);

  return `<div class="side-foot">
      ${
        id || built
          ? `<p class="foot-row">${icon('gitCommit', 14)}<span>${id ? `<span class="mono">${esc(id)}</span>` : ''}${id && built ? ' · ' : ''}${built ? `built ${esc(built)}` : ''}</span></p>`
          : ''
      }
      <p class="foot-row">${icon('robot', 14)}<span>For agents:
        <a href="llms.txt">llms.txt</a> ·
        <a href="index.json">index.json</a></span></p>
      ${
        storageWorks()
          ? ''
          : `<p class="foot-row warn">${icon('alert', 14)}<span>This browser is not storing site data for
              <span class="mono">${esc(location.host)}</span>, so the theme resets on every reload.
              Check the shields or cookie settings for this address.</span></p>`
      }
      <p class="foot-note">Hersir ${esc(p.yodocsVersion)} · private. ${
        ['127.0.0.1', 'localhost'].includes(location.hostname)
          ? 'Served to this machine only.'
          : 'Served to allowlisted addresses only.'
      }</p>
    </div>`;
}

/** Groups the reader has opened past the first few rows. */
const openGroups = new Set();

export function renderSidebar(el) {
  // No groups means the Reference is switched off, and an empty column says nothing.
  if (!['reference', 'home'].includes(state.tab) || state.index.groups.length === 0) {
    el.innerHTML = '';
    el.hidden = true;
    return;
  }
  el.hidden = false;
  // Home shows the groups without the kind filter: there is no list on that
  // page for a filter to act on.
  el.innerHTML =
    (state.tab === 'reference' ? kindFilter() : '') +
    state.index.groups.map((g) => group(g, openGroups)).join('') +
    footer();

  const toggle = el.querySelector('#kind-toggle');
  const menu = el.querySelector('#kind-menu');
  toggle?.addEventListener('click', () => {
    menu.hidden = !menu.hidden;
    toggle.setAttribute('aria-expanded', String(!menu.hidden));
  });
  for (const box of el.querySelectorAll('.kind-opt input')) {
    box.addEventListener('change', () => {
      const picked = [...el.querySelectorAll('.kind-opt input:checked')].map((b) => b.value);
      location.hash = href({ kinds: picked, item: '' });
    });
  }
  el.querySelector('#kind-clear')?.addEventListener('click', () => {
    location.hash = href({ kinds: [], item: '' });
  });
  for (const button of el.querySelectorAll('.show-all')) {
    button.addEventListener('click', () => {
      const name = button.dataset.group;
      if (openGroups.has(name)) openGroups.delete(name);
      else openGroups.add(name);
      renderSidebar(el);
    });
  }
}
