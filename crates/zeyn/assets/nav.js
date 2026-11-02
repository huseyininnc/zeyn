/**
 * Client side navigation for Zeyn apps.
 *
 * Links inside the app load only the screen (`x-zeyn-partial: 1`) and swap the contents of
 * `#z-main`, the page title and the current menu item, without a full page load. History, the
 * back and forward buttons, new tabs and modified clicks keep working as normal links. Before a
 * screen is swapped, `zeyn:leave` lets other scripts close windows and popups. When the screen
 * opens in the other shell (`x-zeyn-shell`: the side menu app shell or the site shell), the
 * whole frame is swapped instead, still without a page load. Anything
 * the server does not answer with a partial screen (errors, redirects to a login page, other apps)
 * falls back to a normal page load.
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');

let controller = null;

/**
 * Whether a URL is a screen of this app that can be loaded without a full page load. Links to
 * `/_zeyn/route/{screen}` count too: the server redirects them to the screen's own address.
 */
function isScreenUrl(url) {
  if (url.origin !== location.origin) return false;
  const path = url.pathname;
  if (BASE && path !== BASE && !path.startsWith(`${BASE}/`)) return false;
  const local = path.slice(BASE.length) || '/';
  if (local.startsWith('/_zeyn/route/')) return true;
  return !local.startsWith('/_zeyn/') && !local.startsWith('/api/') && !/\.[a-z0-9]+$/i.test(local);
}

/** Marks the menu link of the current page. */
function markCurrent(url) {
  document.querySelectorAll('.z-nav a[href], .z-site-nav a[href]').forEach((a) => {
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
  document.dispatchEvent(new CustomEvent('zeyn:leave'));
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
  const needsIslands = res.headers.get('x-zeyn-islands') === '1' && !document.querySelector('meta[name="zeyn-islands"]');
  if (res.headers.get('x-zeyn-partial') !== '1' || needsIslands) {
    location.assign(res.redirected ? res.url : url.href);
    return;
  }
  const shell = res.headers.get('x-zeyn-shell') ?? 'app';
  if (shell !== (document.querySelector('.z-app')?.dataset.zeynShell ?? 'app')) {
    main.removeAttribute('aria-busy');
    await swapFrame(new URL(res.url || url.href), push, url.hash);
    return;
  }
  if (res.headers.get('x-zeyn-live') === '1') import(`${BASE}/_zeyn/live.js`);
  const html = await res.text();
  const finalUrl = new URL(res.url || url.href);
  if (url.hash && !finalUrl.hash) finalUrl.hash = url.hash;
  if (push) history.pushState({ zeyn: true }, '', finalUrl.href);
  const title = res.headers.get('x-zeyn-title');
  if (title) document.title = decodeURIComponent(title);
  main.innerHTML = html;
  main.removeAttribute('aria-busy');
  markCurrent(finalUrl);
  announce(main);
  const target = finalUrl.hash && document.getElementById(decodeURIComponent(finalUrl.hash.slice(1)));
  if (target) target.scrollIntoView();
  else window.scrollTo(0, 0);
  document.dispatchEvent(new CustomEvent('zeyn:content', { detail: main }));
}

document.addEventListener('click', (e) => {
  if (e.defaultPrevented || e.button !== 0 || e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return;
  const a = e.target.closest('a[href]');
  if (!a || a.target || a.hasAttribute('download') || a.hasAttribute('data-zeyn-reload')) return;
  const url = new URL(a.href, location.href);
  if (!isScreenUrl(url)) return;
  if (url.pathname === location.pathname && url.search === location.search && url.hash) return;
  e.preventDefault();
  visit(url, true);
});

/** Loads the stylesheets and module scripts of `doc`'s head that this page does not have yet. */
async function addAssets(doc) {
  const waits = [];
  const have = (sel, attr, url) => [...document.querySelectorAll(sel)].some((el) => el[attr] === url);
  for (const link of doc.querySelectorAll('head link[rel="stylesheet"][href]')) {
    const href = new URL(link.getAttribute('href'), location.href).href;
    if (have('link[rel="stylesheet"]', 'href', href)) continue;
    const el = document.createElement('link');
    el.rel = 'stylesheet';
    el.href = href;
    waits.push(new Promise((done) => {
      el.onload = done;
      el.onerror = done;
    }));
    document.head.append(el);
  }
  for (const script of doc.querySelectorAll('head script[type="module"][src]')) {
    const src = new URL(script.getAttribute('src'), location.href).href;
    if (have('script[src]', 'src', src)) continue;
    const el = document.createElement('script');
    el.type = 'module';
    el.src = src;
    document.head.append(el);
  }
  await Promise.all(waits);
}

/**
 * Replaces the whole app frame (menu or top bar, screen, footer, title, language, page metadata)
 * with the one of `url` without a page load. Used after something that changes more than the
 * screen, such as the language, and when a link leads to a screen in the other shell (app or
 * site). Only when the page cannot be fetched or read does it fall back to a page load.
 */
async function swapFrame(url, push, hash) {
  let res;
  try {
    res = await fetch(url.href, { credentials: 'same-origin' });
  } catch {
    location.assign(url.href);
    return;
  }
  const doc = new DOMParser().parseFromString(await res.text(), 'text/html');
  const next = doc.querySelector('.z-app');
  const current = document.querySelector('.z-app');
  const needsIslands = doc.querySelector('meta[name="zeyn-islands"]') && !document.querySelector('meta[name="zeyn-islands"]');
  if (!res.ok || !next || !current || needsIslands) {
    location.assign(url.href);
    return;
  }
  if (push) {
    const finalUrl = new URL(res.url || url.href);
    if (hash && !finalUrl.hash) finalUrl.hash = hash;
    history.pushState({ zeyn: true }, '', finalUrl.href);
  }
  document.documentElement.lang = doc.documentElement.lang;
  if (doc.documentElement.dir) document.documentElement.dir = doc.documentElement.dir;
  else document.documentElement.removeAttribute('dir');
  document.title = doc.title;
  for (const meta of doc.querySelectorAll('head meta[name]')) {
    const old = document.head.querySelector(`meta[name="${CSS.escape(meta.name)}"]`);
    if (old) old.content = meta.content;
    else document.head.append(document.importNode(meta, true));
  }
  await addAssets(doc);
  const app = document.importNode(next, true);
  current.replaceWith(app);
  markCurrent(new URL(location.href));
  if (push) {
    const main = document.getElementById('z-main');
    if (main) announce(main);
    window.scrollTo(0, 0);
  }
  document.dispatchEvent(new CustomEvent('zeyn:content', { detail: app }));
}

/** Reloads the whole app frame in place, such as after the language or the time zone changed. */
function refreshPage() {
  return swapFrame(new URL(location.href), false);
}

/**
 * Other scripts ask for navigation with events instead of loading pages themselves:
 * `zeyn:navigate` (`detail.url`, `detail.replace`) loads a screen, `zeyn:refresh` reloads the
 * current one in place and `zeyn:refresh-page` the whole app frame. Each event is canceled when
 * handled, so a sender without this script can fall back to a page load.
 */
document.addEventListener('zeyn:navigate', (e) => {
  const url = new URL(e.detail?.url ?? location.href, location.href);
  if (url.origin !== location.origin) return;
  e.preventDefault();
  visit(url, !e.detail?.replace);
});

document.addEventListener('zeyn:refresh', (e) => {
  e.preventDefault();
  visit(new URL(location.href), false);
});

document.addEventListener('zeyn:refresh-page', (e) => {
  e.preventDefault();
  refreshPage();
});

window.addEventListener('popstate', () => {
  const url = new URL(location.href);
  if (isScreenUrl(url)) visit(url, false);
});

history.replaceState({ zeyn: true }, '', location.href);
