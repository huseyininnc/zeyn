/**
 * Client side navigation for Zeyn apps.
 *
 * Links inside the app load only the screen (`x-zeyn-partial: 1`) and swap the contents of
 * `#z-main`, the page title and the current menu item, without a full page load. History, the
 * back and forward buttons, new tabs and modified clicks keep working as normal links. Anything
 * the server does not answer with a partial screen (errors, redirects to a login page, other apps)
 * falls back to a normal page load.
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');

let controller = null;

/** Whether a URL is a screen of this app that can be loaded without a full page load. */
function isScreenUrl(url) {
  if (url.origin !== location.origin) return false;
  const path = url.pathname;
  if (BASE && path !== BASE && !path.startsWith(`${BASE}/`)) return false;
  const local = path.slice(BASE.length) || '/';
  return !local.startsWith('/_zeyn/') && !local.startsWith('/api/') && !/\.[a-z0-9]+$/i.test(local);
}

/** Marks the menu link of the current page. */
function markCurrent(url) {
  document.querySelectorAll('.z-nav a[href]').forEach((a) => {
    const here = new URL(a.href, location.href).pathname === url.pathname;
    if (here) a.setAttribute('aria-current', 'page');
    else a.removeAttribute('aria-current');
  });
}

/** Moves focus to the new screen's heading and tells screen readers that the page changed. */
function announce(main) {
  const heading = main.querySelector('h1');
  if (heading) {
    heading.setAttribute('tabindex', '-1');
    heading.focus({ preventScroll: true });
  }
}

/** Loads a screen into `#z-main`. Falls back to a full page load whenever that is safer. */
async function visit(url, push) {
  const main = document.getElementById('z-main');
  if (!main) {
    location.assign(url.href);
    return;
  }
  controller?.abort();
  controller = new AbortController();
  main.setAttribute('aria-busy', 'true');
  let res;
  try {
    res = await fetch(url.href, {
      headers: { 'x-zeyn-partial': '1' },
      credentials: 'same-origin',
      signal: controller.signal,
    });
  } catch (err) {
    if (err?.name === 'AbortError') return;
    location.assign(url.href);
    return;
  }
  if (res.headers.get('x-zeyn-partial') !== '1') {
    location.assign(res.redirected ? res.url : url.href);
    return;
  }
  const html = await res.text();
  const finalUrl = new URL(res.url || url.href);
  if (push) history.pushState({ zeyn: true }, '', finalUrl.href);
  const title = res.headers.get('x-zeyn-title');
  if (title) document.title = decodeURIComponent(title);
  main.innerHTML = html;
  main.removeAttribute('aria-busy');
  markCurrent(finalUrl);
  window.scrollTo(0, 0);
  announce(main);
  document.dispatchEvent(new CustomEvent('zeyn:content', { detail: main }));
}

document.addEventListener('click', (e) => {
  if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  const a = e.target.closest('a[href]');
  if (!a || a.target || a.hasAttribute('download') || a.hasAttribute('data-zeyn-reload')) return;
  if (a.closest('dialog')) return;
  const url = new URL(a.href, location.href);
  if (!isScreenUrl(url)) return;
  if (url.pathname === location.pathname && url.search === location.search && url.hash) return;
  e.preventDefault();
  visit(url, true);
});

window.addEventListener('popstate', () => {
  const url = new URL(location.href);
  if (isScreenUrl(url)) visit(url, false);
});

history.replaceState({ zeyn: true }, '', location.href);
