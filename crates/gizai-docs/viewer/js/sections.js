import { esc, icon, kindBadge, methodBadge, plural, when } from './ui.js';
import { href, state } from './state.js';

/**
 * Drop the document's own leading heading when the page already shows it.
 * Nearly every Markdown file starts with its title, and printing it twice
 * reads as a bug rather than as a heading.
 */
function withoutLeadHeading(html, title) {
  const match = /^\s*<h1[^>]*>([\s\S]*?)<\/h1>/.exec(html);
  if (match === null) return html;
  const text = match[1].replace(/<[^>]+>/g, '').trim().toLowerCase();
  return text === title.trim().toLowerCase() ? html.slice(match[0].length) : html;
}

/** Headings minus that same lead, so the rail matches the page. */
function railHeadings(headings, title) {
  const out = headings.filter((h) => h.level <= 3);
  const first = out[0];
  return first !== undefined && first.level === 1 && first.text.trim().toLowerCase() === title.trim().toLowerCase()
    ? out.slice(1)
    : out;
}

/**
 * Guides, API, Changelog, Decisions and MCP connections.
 * YD-065, YD-066, YD-067.
 *
 * Each one is absent from the tabs when the repo has no source for it, so
 * nothing here has to render an empty case for a missing section. What it
 * does have to render is a section whose source exists but is empty, which
 * is a different thing and says so.
 */

/* ---------- Guides ---------- */

export function guides(id) {
  const all = state.index.guides;
  const current = all.find((g) => g.id === id) ?? all[0];
  if (current === undefined) return emptySection('Guides', 'docs/guides/*.md');

  const groups = new Map();
  for (const g of all) {
    const key = g.group || 'Guides';
    if (!groups.has(key)) groups.set(key, []);
    groups.get(key).push(g);
  }

  const at = all.indexOf(current);
  const previous = all[at - 1];
  const next = all[at + 1];

  return `<div class="doc-page">
      <nav class="doc-nav">
        ${[...groups]
          .map(
            ([name, items]) => `<h3>${esc(name)}</h3>${items
              .map(
                (g) => `<a class="doc-link${g.id === current.id ? ' on' : ''}" href="${href({ tab: 'guides', item: g.id })}">${esc(g.title)}</a>`,
              )
              .join('')}`,
          )
          .join('')}
      </nav>
      <article class="doc-main">
        <div class="crumb">Guides${current.group ? ` <span class="sep">/</span> ${esc(current.group)}` : ''}</div>
        <h1>${esc(current.title)}</h1>
        <div class="byline">
          <span class="mono sm">${esc(current.file)}</span>
          <span class="sm muted">updated ${esc(when(current.updated).split(',')[0])}</span>
        </div>
        <div class="prose">${withoutLeadHeading(current.html, current.title)}</div>
        <div class="prev-next">
          ${previous ? `<a href="${href({ tab: 'guides', item: previous.id })}"><span class="sm muted">${icon('arrowLeft', 14)} Previous</span><strong>${esc(previous.title)}</strong></a>` : '<span></span>'}
          ${next ? `<a class="next" href="${href({ tab: 'guides', item: next.id })}"><span class="sm muted">Next ${icon('arrowRight', 14)}</span><strong>${esc(next.title)}</strong></a>` : ''}
        </div>
      </article>
      <aside class="rail">
        <h3>On this page</h3>
        ${railHeadings(current.headings, current.title)
          .map((h) => `<a class="rail-link lvl${h.level}" href="#${esc(h.slug)}">${esc(h.text)}</a>`)
          .join('')}
        <h3>For agents</h3>
        <a class="rail-link mono sm" href="md/${esc(current.id)}.md">Markdown ${icon('external', 14)}</a>
      </aside>
    </div>`;
}

/* ---------- Decisions ---------- */

export function decisions(id) {
  const all = state.index.decisions;
  const current = all.find((d) => d.id === id || d.number === id) ?? all[0];
  if (current === undefined) return emptySection('Decisions', 'docs/decisions/NNNN-title.md');

  return `<div class="doc-page">
      <nav class="doc-nav">
        <h3>${plural(all.length, 'decision', 'decisions')}</h3>
        ${all
          .map(
            (d) => `<a class="doc-link${d.id === current.id ? ' on' : ''}" href="${href({ tab: 'decisions', item: d.id })}">
              <span class="mono sm">${esc(d.number)}</span> ${esc(d.title)}
              <span class="status ${esc(d.status)}">${esc(d.status)}</span>
            </a>`,
          )
          .join('')}
      </nav>
      <article class="doc-main">
        <div class="crumb">Decisions <span class="sep">/</span> ${esc(current.number)}</div>
        <h1>${esc(current.title)}</h1>
        <div class="byline">
          <span class="status ${esc(current.status)}">${esc(current.status)}</span>
          <span class="sm muted">${esc(current.date)}</span>
          <span class="mono sm">${esc(current.file)}</span>
        </div>
        <div class="prose">${withoutLeadHeading(current.html, `${current.number} · ${current.title}`)}</div>
        ${
          current.relatedFiles.length > 0
            ? `<h2>Related files</h2><div class="rows">${current.relatedFiles
                .map((f) => {
                  const e = state.index.entries.find((x) => x.file === f && x.parent === undefined);
                  return e
                    ? `<a class="row-item" href="${href({ tab: 'reference', item: e.id })}"><span class="mono">${esc(f)}</span> ${kindBadge(e.kind)}</a>`
                    : `<div class="row-item"><span class="mono muted">${esc(f)}</span><span class="sm muted">not in this build</span></div>`;
                })
                .join('')}</div>`
            : ''
        }
      </article>
      <aside class="rail">
        <h3>On this page</h3>
        ${railHeadings(current.headings, `${current.number} · ${current.title}`)
          .map((h) => `<a class="rail-link lvl${h.level}" href="#${esc(h.slug)}">${esc(h.text)}</a>`)
          .join('')}
      </aside>
    </div>`;
}

/* ---------- Changelog ---------- */

const CHANGE_TYPES = ['Added', 'Changed', 'Fixed', 'Removed', 'Deprecated', 'Security', 'Other'];

export function changelog() {
  const all = state.index.changelog;
  if (all.length === 0) return emptySection('Changelog', 'CHANGELOG.md');

  const picked = state.kinds;
  const present = CHANGE_TYPES.filter((t) => all.some((r) => r.changes.some((c) => c.type === t)));

  return `<div class="page-head">
      <div class="crumb">Changelog</div>
      <h1>Changelog</h1>
      <p class="lead">From ${esc(state.index.project.changelogFrom ?? 'CHANGELOG.md in Keep a Changelog format')}, newest first.</p>
    </div>
    <div class="chips">
      <a class="chip${picked.length === 0 ? ' on' : ''}" href="${href({ kinds: [] })}">All</a>
      ${present.map((t) => `<a class="chip${picked.includes(t) ? ' on' : ''}" href="${href({ kinds: picked.includes(t) ? picked.filter((p) => p !== t) : [...picked, t] })}">${t}</a>`).join('')}
    </div>
    ${all
      .map((r) => {
        const changes = picked.length === 0 ? r.changes : r.changes.filter((c) => picked.includes(c.type));
        if (changes.length === 0) return '';
        return `<section class="release">
            <h2>${esc(r.version)} ${r.unreleased ? '<span class="status proposed">unreleased</span>' : `<span class="sm muted">${esc(r.date)}</span>`}
              ${r.compareUrl ? `<a class="sm" href="${esc(r.compareUrl)}">Compare ${icon('external', 14)}</a>` : ''}</h2>
            <div class="rows">
              ${changes
                .map(
                  (c) => `<div class="row-item">${kindBadge(c.type.toLowerCase())}<span class="grow">${c.html}</span></div>`,
                )
                .join('')}
            </div>
          </section>`;
      })
      .join('')}`;
}

/* ---------- commits ---------- */

const TOUCHED = { A: 'added', M: 'changed', D: 'removed', R: 'renamed' };

/** Every file the Reference can open, with or without entries of its own. */
function indexedFiles() {
  const files = new Set();
  const walk = (rows) => {
    for (const row of rows) {
      if (Array.isArray(row.children) && row.children.length > 0) walk(row.children);
      else files.add(row.target);
    }
  };
  for (const g of state.index.groups) if (g.type === 'tree') walk(g.rows);
  return files;
}

/**
 * What each recent commit touched.
 *
 * "Changed in the docs" starts again at every commit, so straight after a
 * commit it is empty, and nothing on the page said what that commit did.
 * This is git's own answer, read at build time.
 */
export function commits() {
  const all = state.index.history ?? [];
  if (all.length === 0) {
    return `<div class="empty">
        ${icon('inbox', 24)}
        <h2>No commits to show</h2>
        <p class="muted">Hersir found no commits on this project's main branch. Link its repository on its project page, or commit something, then reload this page.</p>
      </div>`;
  }

  const files = indexedFiles();
  const reference = (path) => href({ tab: 'reference', item: '', folder: path, kinds: [], described: '', changed: '' });

  // A file whose lines are in the bundle folds open on its chevron. One
  // that is not in the docs, or is binary, is only named.
  const fileRow = (f) => {
    const inner = `${kindBadge(TOUCHED[f.status] ?? 'changed')}
      <span class="grow mono sm">${esc(f.path)}${f.from ? ` <span class="muted">from ${esc(f.from)}</span>` : ''}</span>
      ${files.has(f.path) ? `<a class="sm" href="${reference(f.path)}">Open in Reference ${icon('arrowRight', 14)}</a>` : ''}`;
    return f.diff
      ? `<details class="fold">
           <summary class="row-item"><span class="chev">${icon('chevronRight', 14)}</span>${inner}</summary>
           <pre class="diff" data-diff="${esc(f.path)}"><span class="dl hunk">Loading the changed lines\u2026</span></pre>
         </details>`
      : `<div class="row-item"><span class="chev"></span>${inner}</div>`;
  };

  return `<div class="page-head">
      <div class="crumb">Commits</div>
      <h1>Commits</h1>
      <p class="lead">The last ${plural(all.length, 'commit', 'commits')} on <span class="mono">${esc(state.index.project.branch || 'this branch')}</span>,
        newest first. Open a commit to see the files it touched, and a file to see the lines that changed.</p>
    </div>
    <div class="rows">
    ${all
      .map(
        (c) => `<details class="fold commit-fold" data-commit="${esc(c.hash)}">
            <summary>
              <span class="chev">${icon('chevronRight', 16)}</span>
              <span class="mono">${esc(c.hash)}</span>
              <span class="grow">${esc(c.subject)}</span>
              <span class="sm muted">${esc(when(c.date))} · ${esc(c.author)} · ${plural(c.files.length, 'file', 'files')}</span>
            </summary>
            <div class="rows">
              ${
                c.files.length === 0
                  ? '<div class="row-item"><span class="grow sm muted">A merge. Git lists no files of its own for it.</span></div>'
                  : c.files.map(fileRow).join('')
              }
            </div>
          </details>`,
      )
      .join('')}
    </div>`;
}

/**
 * The changed lines of one file, as rows. `patch` is commits/<hash>.json,
 * or null when the bundle has none.
 */
export function diffHtml(patch, path) {
  const say = (text) => `<span class="dl hunk">${esc(text)}</span>`;
  if (patch === null) return say('Hersir couldn\u2019t read this commit\u2019s lines.');
  if (patch.withheld) {
    return say('Withheld. These lines hold something that looks like a credential, so they were left out of the docs.');
  }
  const file = patch.files?.[path];
  if (file === undefined) return say('No lines were recorded for this file.');
  const kind = (line) => (line.startsWith('@@') ? ' hunk' : line.startsWith('+') ? ' add' : line.startsWith('-') ? ' del' : '');
  return (
    file.lines.map((line) => `<span class="dl${kind(line)}">${esc(line) || ' '}</span>`).join('') +
    (file.truncated ? say(`Cut short after ${file.lines.length} lines.`) : '')
  );
}

const patchCache = new Map();

/** Fetch a commit's lines the first time it is opened, and fill every file under it. */
export async function fillCommit(details) {
  if (!details.open || details.dataset.filled === '1') return;
  details.dataset.filled = '1';
  const hash = details.dataset.commit;
  if (!patchCache.has(hash)) {
    const res = await fetch(`commits/${encodeURIComponent(hash)}.json`).catch(() => null);
    patchCache.set(hash, res?.ok ? await res.json() : null);
  }
  for (const pre of details.querySelectorAll('pre[data-diff]')) {
    pre.innerHTML = diffHtml(patchCache.get(hash), pre.dataset.diff);
  }
}

/* ---------- MCP connections ---------- */

export function mcp(id) {
  const servers = state.index.mcp;
  const tools = state.index.mcpTools ?? [];
  if (servers.length === 0 && tools.length === 0) return emptySection('MCP connections', '.mcp.json');

  // A tool id in the URL opens its page; otherwise the connections list.
  const tool = tools.find((t) => t.id === id);
  if (tool !== undefined) return mcpTool(tool, tools);

  // A project can have one without the other. One real project builds 11
  // tools and connects to no servers, so the lead must not promise a
  // .mcp.json that is not there, and the warning about credentials only
  // applies to servers.
  const lead =
    servers.length === 0
      ? 'The tools this project\u2019s own server offers, read from the classes that declare them.'
      : `The servers agents use while they work on this project, read from <span class="mono">.mcp.json</span>${
          tools.length > 0 ? ', and the tools this project\u2019s own server offers' : ''
        }.`;

  return `<div class="page-head">
      <div class="crumb">MCP</div>
      <h1>${servers.length === 0 ? 'MCP tools' : 'MCP connections'}</h1>
      <p class="lead">${lead}</p>
    </div>
    ${
      servers.length === 0
        ? ''
        : `<div class="callout info">
            ${icon('alert')}
            <div><strong>Values are never recorded.</strong> Environment variables and headers are listed
              by name only. A credential written into a URL or an argument is replaced with a placeholder.</div>
          </div>`
    }
    ${servers
      .map(
        (m) => `<section class="panel wide">
          <h2>${icon('plug')} ${esc(m.name)} ${kindBadge(m.transport)}</h2>
          <table class="facts">
            <tr><td>${m.transport === 'http' ? 'URL' : 'Command'}</td><td class="mono">${esc(m.target)}</td></tr>
            ${m.args.length > 0 ? `<tr><td>Arguments</td><td class="mono">${esc(m.args.join(' '))}</td></tr>` : ''}
            ${m.envNames.length > 0 ? `<tr><td>Environment</td><td class="mono">${m.envNames.map((n) => `\${${esc(n)}}`).join(', ')} <span class="muted sm">names only</span></td></tr>` : ''}
            ${m.headerNames.length > 0 ? `<tr><td>Headers</td><td class="mono">${esc(m.headerNames.join(', '))} <span class="muted sm">names only</span></td></tr>` : ''}
          </table>
          ${m.purpose ? `<div class="prose">${m.purpose}</div>` : '<p class="sm muted">No notes. Add a heading named after this server in <span class="mono">docs/mcp.md</span>.</p>'}
        </section>`,
      )
      .join('')}
    ${toolList(tools)}`;
}

/**
 * What this project's own MCP server offers.
 *
 * Read from the source rather than by running the server: the title,
 * description, annotations and input schema are all declared as attributes
 * and a schema method, so they are extracted facts rather than a live
 * answer that might differ from the committed code.
 */
function toolList(tools) {
  if (tools.length === 0) return '';
  const groups = new Map();
  for (const t of tools) {
    if (!groups.has(t.sort)) groups.set(t.sort, []);
    groups.get(t.sort).push(t);
  }
  return [...groups]
    .map(
      ([sort, items]) => `<h2>${plural(items.length, sort, `${sort}s`)} this project offers</h2>
        <div class="rows">${items
          .map(
            (t) => `<a class="row-item" href="${href({ tab: 'mcp', item: t.id })}">
              <span class="grow">
                <span class="mono name">${esc(t.name)}</span>
                ${t.annotations.some((a) => a.name === 'IsReadOnly') ? kindBadge('read only') : ''}
                <span class="sm block">${esc(t.description.split('. ')[0])}${t.description.includes('. ') ? '.' : ''}</span>
              </span>
              <span class="sm muted">${t.inputs.length ? plural(t.inputs.length, 'input', 'inputs') : 'no inputs'}</span>
            </a>`,
          )
          .join('')}</div>`,
    )
    .join('');
}

/** One tool, resource or prompt. The screen drawn as 09 API · MCP tool. */
function mcpTool(tool, all) {
  const entry = state.index.entries.find((e) => e.id === tool.entry);
  return `<div class="doc-page">
      <nav class="doc-nav">
        <h3>${plural(all.length, 'tool', 'tools')}</h3>
        ${all
          .map(
            (t) => `<a class="doc-link${t.id === tool.id ? ' on' : ''}" href="${href({ tab: 'mcp', item: t.id })}">
              <span class="mono sm">${esc(t.name)}</span></a>`,
          )
          .join('')}
        <h3>Connections</h3>
        <a class="doc-link" href="${href({ tab: 'mcp', item: '' })}">All servers</a>
      </nav>
      <article class="doc-main">
        <div class="crumb">MCP <span class="sep">/</span> ${esc(tool.sort)}</div>
        <h1 class="mono">${esc(tool.name)}</h1>
        <div class="byline">
          <span>${esc(tool.title)}</span>
          ${tool.annotations.map((a) => kindBadge(a.value === 'false' ? `not ${a.name.replace(/^Is/, '').toLowerCase()}` : a.name.replace(/^Is/, '').toLowerCase())).join('')}
        </div>
        ${tool.description ? `<div class="prose"><p>${esc(tool.description)}</p></div>` : '<div class="nodesc-block"><div class="nodesc-head">No description on this tool</div><p class="sm muted">An agent calling it is told nothing about what it does. Add a Description attribute.</p></div>'}

        <h2>Input schema</h2>
        ${
          tool.inputs.length === 0
            ? '<p class="sm muted">Takes no input.</p>'
            : `<table class="facts">${tool.inputs
                .map(
                  (i) => `<tr>
                    <td class="mono">${esc(i.name)}${i.required ? ' <span class="req">required</span>' : ''}</td>
                    <td><span class="sm muted mono">${esc(i.type)}</span> ${esc(i.description)}</td>
                  </tr>`,
                )
                .join('')}</table>`
        }

        <h2>Calling it</h2>
        <pre class="code"><code>${esc(JSON.stringify({ name: tool.name, arguments: Object.fromEntries(tool.inputs.map((i) => [i.name, `<${i.type}>`])) }, null, 2))}</code></pre>
        <p class="sm muted">There is no console here. Hersir reads the declaration; it does not call it.</p>
      </article>
      <aside class="rail">
        ${
          tool.file
            ? `<h3>Declared in</h3>
          ${entry ? `<a class="rail-link mono sm" href="${href({ tab: 'reference', item: entry.id, folder: '', kinds: [] })}">${esc(entry.name)}</a>` : ''}
          <p class="sm muted mono">${esc(tool.file)}</p>`
            : ''
        }
        <h3>For agents</h3>
        <a class="rail-link mono sm" href="md/mcp.md">Markdown ${icon('external', 14)}</a>
      </aside>
    </div>`;
}

/* ---------- API ---------- */

export function api(id) {
  const spec = state.index.api;
  if (spec === null) return emptySection('API reference', 'openapi.json');

  const current = spec.endpoints.find((e) => e.id === id) ?? spec.endpoints[0];

  // A derived endpoint has a handler in the Reference. Find it by the Route
  // fact the probe attached, so the page can link to the code.
  const want = `${current.method} ${current.path}`;
  const handler = state.index.entries.find((e) =>
    e.facts.some((f) => f.label === 'Route' && f.value === want),
  );

  const byTag = new Map();
  for (const e of spec.endpoints) {
    const tag = e.tags[0] ?? 'Endpoints';
    if (!byTag.has(tag)) byTag.set(tag, []);
    byTag.get(tag).push(e);
  }

  return `<div class="doc-page api">
      <nav class="doc-nav">
        ${[...byTag]
          .map(
            ([tag, items]) => `<h3>${esc(tag)}</h3>${items
              .map(
                (e) => `<a class="doc-link${e.id === current.id ? ' on' : ''}" href="${href({ tab: 'api', item: e.id })}">
                  ${methodBadge(e.method)}<span class="mono sm">${esc(e.path)}</span></a>`,
              )
              .join('')}`,
          )
          .join('')}
      </nav>
      <article class="doc-main">
        <div class="crumb">API <span class="sep">/</span> ${esc(current.tags[0] ?? 'Endpoints')}</div>
        <h1>${esc(current.summary || current.path)}</h1>
        <div class="endpoint">${methodBadge(current.method)}<span class="mono">${esc(current.path)}</span></div>
        ${current.description ? `<div class="prose"><p>${esc(current.description)}</p></div>` : ''}
        ${current.auth.length > 0 ? `<table class="facts"><tr><td>Auth</td><td class="mono">${esc(current.auth.join(', '))}</td></tr></table>` : ''}
        ${
          current.parameters.length > 0
            ? `<h2>Parameters</h2><table class="facts">${current.parameters
                .map(
                  (p) => `<tr><td class="mono">${esc(p.name)}${p.required ? ' <span class="req">required</span>' : ''}</td>
                    <td><span class="sm muted">${esc(p.in)}</span> ${esc(p.description)}</td></tr>`,
                )
                .join('')}</table>`
            : ''
        }
        ${bodyOf(current)}
        <h2>Responses</h2>
        ${
          current.responses.length > 0
            ? `<table class="facts">${current.responses
                .map((r) => `<tr><td class="mono">${esc(r.status)}</td><td>${esc(r.description)}${fieldTable(fieldsOf(r.schema), true)}</td></tr>`)
                .join('')}</table>`
            : `<p class="sm muted">This endpoint was read from the router, which declares the method and
                 the path and nothing else. Status codes and bodies live in the handler, linked on the right.</p>`
        }
      </article>
      <aside class="rail">
        <h3>Example</h3>
        <pre class="code sm"><code>${esc(current.method)} ${esc(current.path)}</code></pre>
        <p class="sm muted">There is no "try it" console. Copy the example and run it yourself.</p>
        ${
          handler
            ? `<h3>Handler</h3>
               <a class="rail-link mono sm" href="${href({ tab: 'reference', item: handler.id, folder: '', kinds: [] })}">${esc(handler.file)}</a>`
            : ''
        }
      </aside>
    </div>`;
}

/*
 * Hersir: an endpoint's request body and each response's fields, as the Figma's API page draws them (yodocs kept the
 * schemas in the index but drew neither). A $ref is looked up in the spec's own schemas.
 */
function resolved(schema) {
  let s = schema;
  for (let n = 0; s && typeof s === 'object' && typeof s.$ref === 'string' && n < 5; n++) {
    s = state.index.api?.schemas?.[s.$ref.split('/').pop()] ?? null;
  }
  return s && typeof s === 'object' ? s : null;
}

/** An object schema's fields: name, type (and format or allowed values), whether required, description. */
function fieldsOf(schema) {
  const s = resolved(schema);
  if (s === null) return [];
  if (s.type === 'array') return fieldsOf(s.items);
  const required = Array.isArray(s.required) ? s.required : [];
  return Object.entries(s.properties ?? {}).map(([name, raw]) => {
    const f = resolved(raw) ?? {};
    const ref = (x) => (typeof x?.$ref === 'string' ? x.$ref.split('/').pop() : '');
    const type = f.type === 'array' ? `array of ${resolved(f.items)?.type ?? (ref(f.items) || 'items')}` : f.type ?? ref(raw);
    const detail = [type, f.format, Array.isArray(f.enum) ? f.enum.join(' | ') : ''].filter(Boolean).join(' · ');
    return { name, detail, required: required.includes(name), description: f.description ?? '' };
  });
}

function fieldTable(rows, small = false) {
  if (rows.length === 0) return '';
  return `<table class="facts${small ? ' sm' : ''}">${rows
    .map((f) => `<tr><td class="mono">${esc(f.name)} <span class="sm muted">${esc(f.detail)}</span>${f.required ? ' <span class="req">required</span>' : ''}</td>
      <td>${esc(f.description)}</td></tr>`)
    .join('')}</table>`;
}

/** The request body's fields, under its media type, when the spec gives a schema for it. */
function bodyOf(endpoint) {
  const content = endpoint.requestBody?.content;
  const [type, media] = content && typeof content === 'object' ? Object.entries(content)[0] ?? [] : [];
  const rows = media ? fieldsOf(media.schema) : [];
  return rows.length > 0 ? `<h2>Body <span class="inline-code mono sm">${esc(type)}</span></h2>${fieldTable(rows)}` : '';
}

/* ---------- a section whose source exists but is empty ---------- */

function emptySection(name, source) {
  return `<div class="empty">
      ${icon('inbox', 24)}
      <h2>No ${esc(name.toLowerCase())} yet</h2>
      <p class="muted">Nothing was found in <span class="mono">${esc(source)}</span>.
        Add it to the repo, then reload this page.</p>
    </div>`;
}
