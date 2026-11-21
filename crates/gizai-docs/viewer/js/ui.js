/**
 * Small shared pieces: escaping, icons, badges.
 *
 * Icons are inline SVG from Lucide at 16px with stroke 1.33, which is what
 * 00 Foundations specifies. They are hand-copied rather than imported so
 * the viewer fetches nothing outside its own folder.
 */

export const esc = (s) =>
  String(s ?? '').replace(
    /[&<>"']/g,
    (c) => ({ '&': '&amp;', '<': '&lt;', '>': '&gt;', '"': '&quot;', "'": '&#39;' })[c],
  );

const PATHS = {
  home: 'M3 9.5 12 2l9 7.5V20a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2ZM9 22V12h6v10',
  book: 'M4 19.5v-15A2.5 2.5 0 0 1 6.5 2H19a1 1 0 0 1 1 1v18a1 1 0 0 1-1 1H6.5a1 1 0 0 1 0-5H20',
  code: 'm16 18 6-6-6-6M8 6l-6 6 6 6',
  globe: 'M12 2a10 10 0 1 0 0 20 10 10 0 0 0 0-20ZM2 12h20M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10Z',
  history: 'M3 12a9 9 0 1 0 2.6-6.4L3 8M3 3v5h5M12 7v5l4 2',
  scale: 'M12 3v18M8 21h8M7 7l-4 7h8ZM17 7l-4 7h8ZM7 7h10',
  plug: 'M12 22v-5M9 7V2M15 7V2M6 7h12v4a6 6 0 0 1-12 0Z',
  layers: 'm12 2 9 5-9 5-9-5 9-5ZM3 12l9 5 9-5M3 17l9 5 9-5',
  folder: 'M20 20a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-7.9a2 2 0 0 1-1.69-.9L9.6 3.9A2 2 0 0 0 7.93 3H4a2 2 0 0 0-2 2v13a2 2 0 0 0 2 2Z',
  folderOpen: 'M6 14l1.45-2.9A2 2 0 0 1 9.24 10H22l-2.72 7.35A2 2 0 0 1 17.4 19H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h3.93a2 2 0 0 1 1.66.9l.82 1.2a2 2 0 0 0 1.66.9H18a2 2 0 0 1 2 2v2',
  file: 'M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7ZM14 2v6h6',
  chevronRight: 'm9 18 6-6-6-6',
  chevronDown: 'm6 9 6 6 6-6',
  chevronsUpDown: 'm7 15 5 5 5-5M7 9l5-5 5 5',
  search: 'M11 19a8 8 0 1 0 0-16 8 8 0 0 0 0 16ZM21 21l-4.3-4.3',
  copy: 'M20 9h-9a2 2 0 0 0-2 2v9a2 2 0 0 0 2 2h9a2 2 0 0 0 2-2v-9a2 2 0 0 0-2-2ZM5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1',
  external: 'M15 3h6v6M10 14 21 3M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6',
  moon: 'M12 3a6 6 0 0 0 9 9 9 9 0 1 1-9-9Z',
  sun: 'M12 17a5 5 0 1 0 0-10 5 5 0 0 0 0 10ZM12 1v2M12 21v2M4.2 4.2l1.4 1.4M18.4 18.4l1.4 1.4M1 12h2M21 12h2M4.2 19.8l1.4-1.4M18.4 5.6l1.4-1.4',
  alert: 'M12 9v4M12 17h.01M10.3 3.9 1.8 18a2 2 0 0 0 1.7 3h17a2 2 0 0 0 1.7-3L13.7 3.9a2 2 0 0 0-3.4 0Z',
  robot: 'M12 8V4H8M4 8h16a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2v-8a2 2 0 0 1 2-2ZM2 14h2M20 14h2M15 13v2M9 13v2',
  filter: 'M3 4h18M7 12h10M11 20h2',
  arrowLeft: 'm12 19-7-7 7-7M19 12H5',
  arrowRight: 'm12 5 7 7-7 7M5 12h14',
  check: 'm20 6-11 11-5-5',
  x: 'M18 6 6 18M6 6l12 12',
  link: 'M10 13a5 5 0 0 0 7.5.5l3-3a5 5 0 0 0-7-7l-1.7 1.7M14 11a5 5 0 0 0-7.5-.5l-3 3a5 5 0 0 0 7 7l1.7-1.7',
  inbox: 'M22 12h-6l-2 3h-4l-2-3H2M5.45 5.1 2 12v6a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2v-6l-3.45-6.9A2 2 0 0 0 16.76 4H7.24a2 2 0 0 0-1.79 1.1Z',
  gitCommit: 'M12 15a3 3 0 1 0 0-6 3 3 0 0 0 0 6ZM3 12h6M15 12h6',
  database: 'M12 8c5 0 9-1.3 9-3s-4-3-9-3-9 1.3-9 3 4 3 9 3ZM3 5v14c0 1.7 4 3 9 3s9-1.3 9-3V5M3 12c0 1.7 4 3 9 3s9-1.3 9-3',
};

/** An inline Lucide glyph. 16px, stroke 1.33, inheriting the current colour. */
export function icon(name, size = 16) {
  const d = PATHS[name];
  if (d === undefined) return '';
  return `<svg class="ico" width="${size}" height="${size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.33" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true"><path d="${d}"/></svg>`;
}

/**
 * Nine kinds have a colour in 00 Foundations. Everything else takes the
 * fallback, because kinds are per project: Dart has widgets, Kotlin has
 * objects, Svelte has stores. YD-083.
 */
const KIND_COLOURS = [
  [/model|twill module/, 'model'],
  [/migration/, 'migration'],
  [/route|endpoint/, 'route'],
  [/resource|repository/, 'resource'],
  [/component|widget|block|view|page/, 'view'],
  [/config|provider|setting|server/, 'config'],
  [/test|factory|seeder/, 'test'],
  [/method|function|hook|property|constant/, 'method'],
  [/class|interface|trait|enum|controller|middleware|job|command|exception/, 'class'],
];

export function kindVar(kind) {
  const found = KIND_COLOURS.find(([re]) => re.test(kind));
  return found ? `var(--yd-kind-${found[1]})` : 'var(--yd-kind-fallback)';
}

export const kindBadge = (kind, extra = '') =>
  `<span class="kind" style="--yd-kind:${kindVar(kind)}"${extra}>${esc(kind)}</span>`;

export const methodBadge = (method) =>
  `<span class="method ${esc(String(method).toLowerCase())}">${esc(method)}</span>`;

/** A toast, for Copy path and friends. Says what happened, then goes. */
let toastTimer = null;
export function toast(message) {
  let el = document.getElementById('toast');
  if (el === null) {
    el = document.createElement('div');
    el.id = 'toast';
    document.body.appendChild(el);
  }
  el.innerHTML = `${icon('check')}<span>${esc(message)}</span>`;
  el.classList.add('on');
  if (toastTimer !== null) clearTimeout(toastTimer);
  toastTimer = setTimeout(() => el.classList.remove('on'), 2200);
}

export async function copy(text, said) {
  try {
    await navigator.clipboard.writeText(text);
    toast(said);
  } catch {
    toast('Could not reach the clipboard');
  }
}

/** Code with line numbers, and the declaration's own lines marked. */
export function codeBlock(lines, from, to, highlight) {
  const rows = [];
  for (let n = from; n <= to; n++) {
    const text = lines[n - 1];
    if (text === undefined) continue;
    const hit = highlight !== undefined && n === highlight ? ' hit' : '';
    rows.push(`<span class="line${hit}"><span class="ln">${n}</span>${esc(text) || ' '}</span>`);
  }
  /*
   * Joined with nothing, not a newline. Each row is already display:block,
   * and a <pre> preserves whitespace, so a newline between the rows was
   * rendering as a second, empty line: every code block in the viewer was
   * double spaced. The copy button reads the source array, not the DOM, so
   * nothing depends on these newlines.
   */
  return `<pre class="code"><code>${rows.join('')}</code></pre>`;
}

export const plural = (n, one, many) => `${n} ${n === 1 ? one : many}`;

/**
 * The first sentence of a description, for a list row.
 *
 * Taking the first line instead cuts wherever the author happened to wrap,
 * which on real code is almost always mid-sentence.
 */
export function snippet(text, limit = 140) {
  const flat = String(text).split('\n\n')[0].replace(/\s+/g, ' ').trim();
  const stop = flat.search(/[.!?](\s|$)/);
  const sentence = stop !== -1 && stop < limit ? flat.slice(0, stop + 1) : flat;
  if (sentence.length <= limit) return sentence;
  const cut = sentence.slice(0, limit);
  return `${cut.slice(0, cut.lastIndexOf(' '))}…`;
}

/** A date a reader cannot misread as the other way round. */
export function when(iso) {
  const d = new Date(iso);
  return Number.isNaN(d.getTime())
    ? ''
    : d.toLocaleString(undefined, {
        day: 'numeric', month: 'short', year: 'numeric',
        hour: '2-digit', minute: '2-digit',
      });
}
