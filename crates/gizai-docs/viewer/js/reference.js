import { codeBlock, copy, esc, icon, kindBadge, methodBadge, plural, snippet, when } from './ui.js';
import { entryById, filtered, href, isOff, moved, sourceFor, state } from './state.js';

/** The words used for each bucket, in the list, on Home and on an entry. */
const MOVED_LABEL = {
  documented: 'now documented',
  undocumented: 'lost its description',
  added: 'new',
  changed: 'declaration changed',
  removed: 'gone',
};

/**
 * The Reference list and entry pages. YD-062 and YD-064.
 *
 * The entry page is the one the tool produces most of, and most of the
 * time it has no description: 62% on one real Laravel project. So the no-description case is a
 * drawn state rather than a gap, and the page still carries facts, a
 * signature and source whatever else is missing.
 */

const PER_PAGE = 100;

/* ---------- Home ---------- */

/** What Home calls each section a DOCS_ key can switch off. */
const OFF_NAME = {
  guides: 'Guides', reference: 'Reference', api: 'API', changelog: 'Changelog',
  decisions: 'Decisions', mcp: 'MCP', commits: 'Commits',
};

const SECTION_ICON = {
  Guides: 'book',
  Reference: 'code',
  API: 'globe',
  Changelog: 'history',
  Decisions: 'scale',
  MCP: 'plug',
};

export function home() {
  const i = state.index;
  const s = i.stats;
  const pct = s.entries === 0 ? 0 : Math.round((s.described / s.entries) * 100);

  const sections = [
    ['Guides', 'guides', i.guides.length, `${plural(i.guides.length, 'guide', 'guides')} from the repo`],
    ['Reference', 'reference', i.entries.length, i.kinds.slice(0, 4).map((k) => k.name).join(', ')],
    ['API', 'api', i.api ? i.api.endpoints.length : 0, i.api ? `from ${i.api.file}` : 'no spec in this repo'],
    ['Changelog', 'changelog', i.changelog.reduce((n, r) => n + r.changes.length, 0), i.changelog[0] ? `latest ${i.changelog[0].version}` : 'no CHANGELOG.md'],
    ['Decisions', 'decisions', i.decisions.length, 'architecture decision records'],
    ['MCP', 'mcp', i.mcp.length, 'servers agents use on this project'],
  ].filter(([, tab]) => !isOff(tab));

  // Otherwise "this project has none" and "this project left it out" look
  // the same, which is the confusion YD-094 was written to end.
  const off = (i.switchedOff ?? []).map((o) => `${OFF_NAME[o.section] ?? o.section} (${o.key})`);
  const refOff = isOff('reference');

  const problems =
    i.problems.length === 0
      ? ''
      : `<div class="callout warning">
          ${icon('alert')}
          <div>
            <strong>${plural(i.problems.length, 'problem', 'problems')} in this build.</strong>
            ${i.problems
              .slice(0, 2)
              .map((p) => `<div class="mono sm">${esc(p.file)}${p.line ? `:${p.line}` : ''} — ${esc(p.message)}</div>`)
              .join('')}
            ${i.problems.length > 2 ? `<div class="sm">and ${i.problems.length - 2} more, listed in llms.txt.</div>` : ''}
          </div>
        </div>`;

  return `
    <div class="page-head">
      <div class="crumb">Overview</div>
      <h1>${esc(i.project.title)}</h1>
      <p class="lead">Code reference, guides, API, changes, decisions and the MCP servers of this
        project, generated from the repository on every build.</p>
    </div>
    ${problems}
    <div class="home">
      <div class="home-main">
        <div class="cards">
          ${sections
            .map(
              // Every section is listed, zero included. A tab with nothing
              // in it is hidden, so this is the only place someone finds
              // out the feature exists at all.
              ([name, tab, n, note]) => {
                const tag = n === 0 ? 'div' : 'a';
                const attr = n === 0 ? ' class="card empty-card"' : ` class="card" href="#/${tab}"`;
                return `<${tag}${attr}>
                  <span class="card-ico">${icon(SECTION_ICON[name])}</span>
                  <strong>${name} <span class="count">${n}</span></strong>
                  <span class="sm muted">${esc(note)}</span>
                </${tag}>`;
              },
            )
            .join('')}
        </div>
        ${off.length > 0 ? `<p class="sm muted">Switched off in this project: ${esc(off.join(', '))}.</p>` : ''}
        ${refOff ? '' : docChanges()}
        ${recentChanges()}
      </div>
      <aside class="home-side">
        <div class="panel">
          <h3>This build</h3>
          <table class="facts sm">
            <tr><td>Commit</td><td class="mono">${esc(i.project.commit || '—')}</td></tr>
            <tr><td>Branch</td><td class="mono">${esc(i.project.branch || '—')}</td></tr>
            <tr><td>Built</td><td>${esc(when(i.project.builtAt))}</td></tr>
            <tr><td>Took</td><td>${(s.durationMs / 1000).toFixed(1)} s</td></tr>
            ${refOff ? '' : `<tr><td>Files</td><td>${s.files}</td></tr>`}
            ${i.project.probes.length > 0 ? `<tr><td>Probes</td><td>${esc(i.project.probes.join(', '))}</td></tr>` : ''}
          </table>
        </div>
        ${refOff ? '' : `<div class="panel">
          <h3>Descriptions</h3>
          <div class="count-pair">
            <a href="#/reference?described=yes"><b>${s.described}</b><span>have one</span></a>
            <a href="#/reference?described=no"><b>${s.entries - s.described}</b><span>do not</span></a>
          </div>
          <p class="sm muted">Hersir shows what is written above a declaration. It never
            invents one, so this is a count of the comments in the repo.</p>
        </div>`}
        <div class="panel">
          <h3>${icon('robot')} For agents</h3>
          <p class="sm">Every page is also Markdown. Start at llms.txt; everything is in index.json.</p>
          <div class="mono sm inline-code">llms.txt</div>
        </div>
      </aside>
    </div>`;
}

/**
 * What moved in the docs since the last commit this was built from.
 *
 * Not the same thing as the changelog below it: that is a release note
 * someone wrote, this is "somebody documented a constant on Tuesday". The
 * build used to say this once in the terminal and keep no record.
 */
function docChanges() {
  const d = state.index.docChanges;

  // No baseline yet, which is every first build. Saying so beats silence:
  // someone who edits a file and sees nothing here assumes it is broken,
  // when the truth is that this build is the thing being compared against.
  if (d === null || d === undefined) {
    return `<h2>Changed in the docs</h2>
      <p class="sm muted">Hersir doesn't compare the docs between builds yet.
        <a href="#/commits">The commits</a> say what each recent one changed.</p>`;
  }

  if (d.total === 0) {
    return `<h2>Changed in the docs
        <span class="sm muted">since ${d.since.commit === '' ? 'the last build' : `<span class="mono">${esc(d.since.commit)}</span>`}</span></h2>
      <p class="sm muted">Nothing has moved. No entry gained or lost a description, and none
        arrived or went. This list starts again at every commit.
        <a href="#/commits">What the last commits changed ${icon('arrowRight', 14)}</a></p>`;
  }

  const groups = [
    ['documented', d.documented],
    ['undocumented', d.undocumented],
    ['added', d.added],
    ['changed', d.changed],
    ['removed', d.removed],
  ].filter(([, list]) => list.length > 0);

  const LIMIT = 12;
  let shown = 0;
  const rows = [];
  for (const [kind, list] of groups) {
    for (const e of list) {
      if (shown >= LIMIT) break;
      shown += 1;
      const name = `<span class="mono">${esc(e.name)}</span>`;
      rows.push(`<div class="row-item">
          <span class="moved ${kind}">${esc(MOVED_LABEL[kind])}</span>
          <span class="grow">${
            kind === 'removed'
              ? name
              : `<a href="${href({ tab: 'reference', item: e.id, folder: '', kinds: [], described: '' })}">${name}</a>`
          }</span>
          <span class="sm muted">${esc(e.file)}</span>
        </div>`);
    }
  }

  const since =
    d.since.commit === ''
      ? 'the last build'
      : `<span class="mono">${esc(d.since.commit)}</span>`;

  // "removed" has no entry to link to, so the full list only makes sense
  // when something other than a removal moved.
  const listable = d.total - d.removed.length;

  return `<h2>Changed in the docs
      ${
        listable > 0
          ? `<a class="sm" href="${href({ tab: 'reference', item: '', folder: '', kinds: [], described: '', changed: 'any' })}">All ${listable} ${icon('arrowRight', 14)}</a>`
          : ''
      }
      <span class="sm muted">since ${since}</span></h2>
    <div class="rows">${rows.join('')}</div>
    ${d.total > shown ? `<p class="sm muted">${d.total - shown} more. The complete list is also in <a href="llms.txt">llms.txt</a>.</p>` : ''}`;
}

function recentChanges() {
  const i = state.index;
  const recent = i.changelog.flatMap((r) => r.changes.map((c) => ({ ...c, version: r.version, date: r.date }))).slice(0, 6);
  if (recent.length === 0) return '';
  return `<h2>Recent changes <a class="sm" href="#/changelog">All changes ${icon('arrowRight', 14)}</a></h2>
    <div class="rows">
      ${recent
        .map(
          (c) => `<div class="row-item">
            ${kindBadge(c.type.toLowerCase())}
            <span class="grow">${c.html}</span>
            <span class="sm muted">${esc(c.version)}</span>
          </div>`,
        )
        .join('')}
    </div>`;
}

/* ---------- Reference list ---------- */

/**
 * A file that is indexed but declares nothing the parser can name.
 *
 * 327 of 808 files in one Laravel project are like this, mostly config returning a
 * bare array. They were listed in the tree, clickable, and showed "Nothing
 * matches", which reads as a broken page. The file is in the bundle, so
 * show the file.
 */
const MAX_SHOWN_LINES = 500;

function fileView(src) {
  const name = src.path.split('/').pop();
  const crumbs = src.path.split('/');
  const problem = state.index.problems.find((p) => p.file === src.path);
  const shown = Math.min(src.lines.length, MAX_SHOWN_LINES);

  return `<div class="page-head">
      <div class="crumb">${crumbs
        .map((c, n) => (n === crumbs.length - 1 ? `<span class="mono">${esc(c)}</span>` : `${esc(c)} <span class="sep">/</span> `))
        .join('')}</div>
      <h1>${esc(name)}</h1>
      <p class="lead">${
        problem === undefined
          ? 'This file is indexed and its contents are below. It declares no class, function or constant, so it has no Reference entries of its own.'
          : `This file is indexed and its contents are below. ${esc(problem.message)}`
      }</p>
      <div class="actions">
        <button class="btn ghost" data-copy="${esc(src.path)}" data-said="Path copied">${icon('copy')} Copy path</button>
      </div>
    </div>
    <table class="facts">
      <tr><td>Path</td><td class="mono">${esc(src.path)}</td></tr>
      <tr><td>Lines</td><td>${src.lines.length}</td></tr>
    </table>
    <h2>Source</h2>
    ${codeBlock(src.lines, 1, shown)}
    ${
      shown < src.lines.length
        ? `<p class="sm muted">First ${shown} of ${src.lines.length} lines. The whole file is in
             <a href="source/${esc(src.path)}.json">source/${esc(src.path)}.json</a>.</p>`
        : ''
    }`;
}

/** The sidebar's own row for a folder, or null when there is no such folder. */
function folderRow(target) {
  const find = (rows) => {
    for (const row of rows) {
      if (row.target === target) return row;
      const below = Array.isArray(row.children) ? find(row.children) : null;
      if (below !== null) return below;
    }
    return null;
  };
  for (const g of state.index.groups ?? []) {
    const row = g.type === 'tree' ? find(g.rows) : null;
    if (row !== null) return row;
  }
  return null;
}

/**
 * A folder whose files all declare nothing the parser can name.
 *
 * Laravel's routes/ is the usual one: six files of route calls and not one
 * class or function between them. The folder sat in the sidebar, and
 * clicking it said "Nothing matches" while each file inside opened fine.
 * The files are in the bundle, so list the files.
 */
function folderView(row) {
  const files = [];
  const walk = (rows) => {
    for (const r of rows) {
      if (Array.isArray(r.children) && r.children.length > 0) walk(r.children);
      else files.push(r.target);
    }
  };
  walk(row.children);
  const crumbs = row.target.split('/');

  return `<div class="page-head">
      <div class="crumb">${crumbs
        .map((c, n) => (n === crumbs.length - 1 ? `<span class="mono">${esc(c)}</span>` : `${esc(c)} <span class="sep">/</span> `))
        .join('')}</div>
      <h1>${esc(crumbs[crumbs.length - 1])}</h1>
      <p class="lead">${plural(files.length, 'file', 'files')} in this folder, all indexed. None declares a class, function
        or constant, so the folder has no Reference entries. Open a file to read it.</p>
    </div>
    <div class="rows">
      ${files
        .map(
          (path) => `<a class="row-item" href="${href({ folder: path, item: '' })}">
            <span class="glyph">${icon('file')}</span>
            <span class="grow name">${esc(path.slice(row.target.length + 1))}</span>
          </a>`,
        )
        .join('')}
    </div>`;
}

export async function list() {
  const found = filtered();
  const onlyBare = state.described === 'no';
  const pages = Math.max(1, Math.ceil(found.length / PER_PAGE));
  const page = Math.min(state.page, pages);
  const from = (page - 1) * PER_PAGE;
  const shown = found.slice(from, from + PER_PAGE);

  const where = state.folder ? ` in <span class="mono">${esc(state.folder)}</span>` : '';
  const ofKind = state.kinds.length > 0 ? ` of kind ${esc(state.kinds.join(', '))}` : '';
  const bare = onlyBare ? ' with no description' : '';
  const since = state.index.docChanges?.since.commit;
  const movedBy =
    state.changed === ''
      ? ''
      : state.changed === 'any'
        ? ` that moved since ${since || 'the last build'}`
        : ` ${MOVED_LABEL[state.changed] ?? state.changed} since ${since || 'the last build'}`;

  if (found.length === 0) {
    // Before giving up: the filter may be pointing at one real file that
    // happens to declare nothing, or at a folder holding only such files.
    if (state.folder !== '' && state.kinds.length === 0 && state.described === '' && state.changed === '') {
      const src = await sourceFor(state.folder);
      if (src.lines.length > 0) return fileView(src);
      const row = folderRow(state.folder);
      if (row !== null && Array.isArray(row.children) && row.children.length > 0) return folderView(row);
    }
    return `<div class="empty">
        ${icon('search', 24)}
        <h2>Nothing matches</h2>
        <p class="muted">No entry in this build is${where}${ofKind}${bare}${movedBy}.</p>
        <a class="btn" href="${href({ folder: '', kinds: [], item: '', described: '', changed: '' })}">Clear the filters</a>
      </div>`;
  }

  const heading =
    found.length === state.index.entries.length
      ? 'Everything in this build'
      : `${found.length} ${found.length === 1 ? 'entry' : 'entries'}${bare}${movedBy}${where}${ofKind}`;

  return `<div class="page-head">
      <div class="crumb">Reference</div>
      <h1>${heading}</h1>
      ${onlyBare ? '<p class="lead">Nothing is written above these declarations. Hersir shows what the code says; it does not invent.</p>' : ''}
    </div>
    <div class="rows">${shown.map(entryRow).join('')}</div>
    ${pager(page, pages, from, shown.length, found.length)}`;
}

/**
 * Pages, because these lists are long: 1,056 entries have no description in
 * one real project, and cutting at the first hundred with a note made the rest
 * unreachable rather than simply unshown.
 */
function pager(page, pages, from, count, total) {
  if (pages <= 1) return '';
  const link = (n, label, disabled) =>
    disabled
      ? `<span class="page-btn off">${label}</span>`
      : `<a class="page-btn" href="${href({ page: n })}">${label}</a>`;

  const numbers = [];
  const first = Math.max(1, Math.min(page - 2, pages - 4));
  for (let n = first; n < first + 5 && n <= pages; n++) {
    numbers.push(
      n === page
        ? `<span class="page-btn on">${n}</span>`
        : `<a class="page-btn" href="${href({ page: n })}">${n}</a>`,
    );
  }

  return `<nav class="pager">
      <span class="sm muted">${from + 1}–${from + count} of ${total}</span>
      <span class="grow"></span>
      ${link(page - 1, `${icon('arrowLeft', 14)} Previous`, page === 1)}
      ${numbers.join('')}
      ${link(page + 1, `Next ${icon('arrowRight', 14)}`, page === pages)}
    </nav>`;
}

export function entryRow(e) {
  const how = moved().get(e.id);
  return `<a class="row-item entry" href="${href({ tab: 'reference', item: e.id })}">
      <span class="grow">
        <span class="mono name">${esc(e.name)}</span> ${kindBadge(e.kind)}
        ${how === undefined ? '' : `<span class="moved ${how}">${esc(MOVED_LABEL[how])}</span>`}
        <span class="sm muted mono block">${esc(e.file)}:${e.lineStart}</span>
        ${
          e.hasDescription
            ? `<span class="sm block">${esc(snippet(e.description))}</span>`
            : '<span class="nodesc block">no description</span>'
        }
      </span>
    </a>`;
}

/* ---------- Reference entry ---------- */

export async function entry(id) {
  const e = entryById(id);
  if (e === undefined) return notFound(id);

  const src = await sourceFor(e.file);
  const parent = e.parent ? entryById(e.parent) : null;
  const kids = e.children.map(entryById).filter((k) => k !== undefined);
  const routes = e.facts.filter((f) => f.label === 'Route');
  // Say it on the page itself, not only on Home: someone who deep-links
  // straight to an entry never sees Home at all.
  const how = moved().get(e.id);
  const since = state.index.docChanges?.since.commit || 'the last build';
  const facts = e.facts.filter((f) => f.label !== 'Route');
  const crumbs = e.file.split('/');

  const rail = [
    ['summary', 'Summary'],
    ...(facts.length > 0 || e.tags.length > 0 ? [['facts', 'Facts']] : []),
    ...(kids.length > 0 ? [['inside', `Inside (${kids.length})`]] : []),
    ['source', 'Source'],
  ];

  return `
    <div class="entry-page">
      <div class="entry-main">
        <div class="crumb">${crumbs.map((c, n) => (n === crumbs.length - 1 ? `<span class="mono">${esc(c)}</span>` : `${esc(c)} <span class="sep">/</span> `)).join('')}</div>

        <div class="entry-top">
          <div>${kindBadge(e.kind)}${parent ? ` <a class="sm" href="${href({ item: parent.id })}">in ${esc(parent.name)}</a>` : ''}${
            how === undefined
              ? ''
              : ` <a class="moved ${how}" href="${href({ tab: 'reference', item: '', folder: '', kinds: [], described: '', changed: how })}"
                   title="See everything that moved the same way">${esc(MOVED_LABEL[how])} since ${esc(since)}</a>`
          }</div>
          <div class="actions">
            <button class="btn ghost" data-copy="${esc(e.file)}" data-said="Path copied">${icon('copy')} Copy path</button>
            <button class="btn ghost" data-copy-link="${esc(e.id)}" data-said="Link copied">${icon('link')} Copy link</button>
          </div>
        </div>

        <h1 class="mono" id="summary">${esc(e.name)}</h1>
        <div class="sm muted mono">${esc(e.file)} · lines ${e.lineStart}–${e.lineEnd}</div>

        ${
          e.hasDescription
            ? `<div class="prose desc">${e.description.split('\n\n').map((p) => `<p>${esc(p)}</p>`).join('')}</div>`
            : noDescription()
        }

        ${e.signature ? `<pre class="code signature"><code>${esc(e.signature)}</code></pre>` : ''}

        ${
          facts.length > 0 || e.tags.length > 0
            ? `<h2 id="facts">Facts</h2>
               <table class="facts">
                 ${facts.map((f) => `<tr><td>${esc(f.label)}</td><td class="mono">${esc(f.value)}</td></tr>`).join('')}
                 ${e.tags.map((t) => `<tr><td>@${esc(t.name)}</td><td class="mono">${esc(t.text)}</td></tr>`).join('')}
               </table>`
            : ''
        }

        ${
          kids.length > 0
            ? `<h2 id="inside">${plural(kids.length, 'declaration', 'declarations')} inside</h2>
               <div class="rows">${kids.map(entryRow).join('')}</div>`
            : ''
        }

        <h2 id="source">Source</h2>
        <div class="code-head">
          <span class="mono sm">${esc(e.file)}</span>
          <span class="sm muted">lines ${e.lineStart}–${e.lineEnd}</span>
          <button class="btn ghost sm" data-copy-source="${esc(e.id)}" data-said="Source copied">${icon('copy')}</button>
        </div>
        ${codeBlock(src.lines, e.lineStart, e.lineEnd, state.line || e.nameLine)}
      </div>

      <aside class="rail">
        <h3>On this page</h3>
        ${rail.map(([id, label]) => `<a class="rail-link" href="#${id}">${esc(label)}</a>`).join('')}
        ${
          routes.length > 0
            ? `<h3>Used by routes</h3>${routes
                .map((r) => {
                  const [verb = '', ...rest] = r.value.split(' ');
                  return `<div class="rail-route">${methodBadge(verb)}<span class="mono sm">${esc(rest.join(' ').split(' · ')[0])}</span></div>`;
                })
                .join('')}`
            : probeHint()
        }
        <h3>For agents</h3>
        <a class="rail-link mono sm" href="md/reference/${esc(e.id.replace('#', '/'))}.md">Markdown ${icon('external', 14)}</a>
      </aside>
    </div>`;
}

/**
 * The common case, drawn deliberately. 62% of one real project's entries land here, so
 * it has to read as a fact about the code rather than a broken page.
 */
function noDescription() {
  const i = state.index;
  const missing = i.stats.entries - i.stats.described;
  return `<div class="nodesc-block">
      <div class="nodesc-head">No description in the source</div>
      <p class="sm muted">Nothing is written above this declaration, so there is nothing to show.
        Hersir shows what the code says; it does not invent.
        <a href="#/reference?described=no">${missing} others are the same</a>.</p>
    </div>`;
}

/** Said plainly rather than left as an absence. */
function probeHint() {
  if (state.index.project.probes.length > 0) return '';
  return `<h3>Routes</h3>
    <p class="sm muted">None found. Hersir reads them from a Next.js app's route and page files and a
      Laravel app's <span class="mono">routes/</span> files.</p>`;
}

export function notFound(id) {
  const guesses = state.index.entries
    .filter((e) => e.name.toLowerCase().includes(String(id).split('#').pop().toLowerCase().slice(0, 6)))
    .slice(0, 4);
  return `<div class="empty">
      ${icon('search', 24)}
      <h2 class="mono">${esc(String(id).split('#').pop() || id)} is not in this build</h2>
      <p class="muted">The link points to <span class="mono">${esc(id)}</span>, which is not in
        commit ${esc(state.index.project.commit || 'this build')}. It was probably renamed or removed.</p>
      ${guesses.length > 0 ? `<div class="rows">${guesses.map(entryRow).join('')}</div>` : ''}
      <a class="btn" href="#/reference">Search all entries</a>
    </div>`;
}

/** Copy buttons, wired after render because the markup is rebuilt each time. */
export function wireCopy(root) {
  for (const b of root.querySelectorAll('[data-copy]')) {
    b.addEventListener('click', () => copy(b.dataset.copy, b.dataset.said));
  }
  for (const b of root.querySelectorAll('[data-copy-link]')) {
    b.addEventListener('click', () => {
      const url = `${location.origin}${location.pathname}${href({ item: b.dataset.copyLink })}`;
      copy(url, b.dataset.said);
    });
  }
  for (const b of root.querySelectorAll('[data-copy-source]')) {
    b.addEventListener('click', async () => {
      const e = entryById(b.dataset.copySource);
      const src = await sourceFor(e.file);
      copy(src.lines.slice(e.lineStart - 1, e.lineEnd).join('\n'), b.dataset.said);
    });
  }
}
