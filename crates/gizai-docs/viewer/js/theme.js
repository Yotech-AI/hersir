/*
 * Runs before first paint, so the page never flashes the wrong theme.
 * Follows the system until someone toggles it, then is remembered. Hersir: ?theme=dark or ?theme=light, which the
 * app passes to match its own appearance, comes first.
 *
 * It has a file of its own because serve sends
 * Content-Security-Policy: default-src 'self', which refuses every inline
 * script. Inline in index.html it never ran, so each load came up light.
 * It is a plain script and not a module, because a module waits for the
 * page to be parsed and would run after that first paint.
 */
(function () {
  var system = false;
  try { system = matchMedia('(prefers-color-scheme: dark)').matches; } catch (e) {}
  var saved = null;
  // Reading storage throws outright when a browser blocks site data, and
  // one catch around both lines fell back to light on a dark desktop.
  try { saved = localStorage.getItem('yd-theme'); } catch (e) {}
  var asked = null;
  try { asked = new URLSearchParams(location.search).get('theme'); } catch (e) {}
  var dark = asked === 'dark' || asked === 'light' ? asked === 'dark' : saved === null || saved === '' ? system : saved === 'dark';
  document.documentElement.dataset.theme = dark ? 'dark' : 'light';
})();
