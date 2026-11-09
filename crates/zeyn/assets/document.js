/**
 * Document viewer (`DocumentViewer::of(page_action, page_count)`). Loaded only by pages that
 * hold `[data-zeyn-doc]`.
 *
 * Every page has the size the server gives (`data-width`, `data-height` in CSS pixels), so the
 * scroll area is as tall as the whole document from the start and the pages in view follow from
 * the scroll position alone. Only the pages in view and one on each side are in the document;
 * each is fetched through `POST /_zeyn/data` (key `page`) once scrolling has paused for
 * `SETTLE_MS`, so pages flown past are never fetched, nearest first, at most `MAX_FETCHES` at a
 * time, and a fetch whose page has scrolled away is aborted. When the server's rate limit
 * answers 429, fetching waits for its `Retry-After`. At most `MAX_KEPT` rendered
 * pages are kept, the least recently shown dropped first. The server cleans every page before
 * it is sent; its HTML is inserted only into the page's own sheet, never anything else.
 *
 * Zoom scales the sheets with a CSS transform, so a zoom never fetches a page again: fit width,
 * fit page and 50 to 400 percent. The scroll position stays on the same page. Keys in the
 * viewer: Page Up and Page Down move by page, Home and End go to the first and last page, `+`
 * and `-` zoom. The current page is announced in a polite live region ("Page 3 of 500") once
 * scrolling settles. The outline and the search hits go to their page; the search runs 300 ms
 * after the last key through the key `search`, a newer search aborting the older one.
 * @module document
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const GAP = 16;
const MAX_FETCHES = 3;
const MAX_KEPT = 12;
const ZOOMS = [0.5, 0.75, 1, 1.25, 1.5, 2, 3, 4];
const SEARCH_MS = 300;
const ANNOUNCE_MS = 400;
const SETTLE_MS = 90;

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Replaces `{name}` placeholders. */
function fill(template, values) {
  return (template ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/** Posts to the viewer's data source `key` and returns the JSON answer, or `null`. */
async function call(v, key, input, signal) {
  const screen = v.root.closest('[data-zeyn-screen]');
  let params = null;
  try {
    params = JSON.parse(screen?.dataset.zeynParams ?? 'null');
  } catch {}
  const res = await fetch(BASE + '/_zeyn/data', {
    method: 'POST',
    credentials: 'same-origin',
    headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
    body: JSON.stringify({ screen: screen?.dataset.zeynScreen ?? null, params, node: v.root.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null, key, input }),
    signal,
  });
  if (res.status === 429) return { retry: Number(res.headers.get('retry-after')) || 1 };
  return res.ok ? res.json() : null;
}

/** The scale of the viewer's zoom setting in its current size. */
function scaleOf(v) {
  const w = Math.max(1, v.view.clientWidth - 2 * GAP);
  const h = Math.max(1, v.view.clientHeight - 2 * GAP);
  if (v.zoom === 'width') return Math.min(4, Math.max(0.1, w / v.w));
  if (v.zoom === 'page') return Math.min(4, Math.max(0.1, Math.min(w / v.w, h / v.h)));
  return v.zoom;
}

/** Sets the size of the scroll area and the place of every page in the document. */
function layout(v) {
  const at = v.slot ? (v.view.scrollTop - GAP) / v.slot : null;
  v.scale = scaleOf(v);
  v.pw = v.w * v.scale;
  v.ph = v.h * v.scale;
  v.slot = v.ph + GAP;
  const width = Math.max(v.view.clientWidth, v.pw + 2 * GAP);
  Object.assign(v.pages.style, { height: `${v.count * v.slot + GAP}px`, width: `${width}px` });
  v.left = Math.max(GAP, (width - v.pw) / 2);
  for (const [n, el] of v.shown) place(v, n, el);
  if (at !== null) v.view.scrollTop = GAP + at * v.slot;
  v.zoomSelect.value = typeof v.zoom === 'number' ? String(Math.round(v.zoom * 100)) : v.zoom;
  draw(v);
}

/** Puts page `n` at its place and scale. */
function place(v, n, el) {
  Object.assign(el.style, { top: `${GAP + (n - 1) * v.slot}px`, left: `${v.left}px`, width: `${v.pw}px`, height: `${v.ph}px` });
  const sheet = el.firstElementChild;
  if (sheet) Object.assign(sheet.style, { width: `${v.w}px`, height: `${v.h}px`, transform: `scale(${v.scale})` });
}

/** The element of page `n`, from the kept pages or new. */
function pageEl(v, n) {
  let el = v.kept.get(n);
  if (el) {
    v.kept.delete(n);
    v.kept.set(n, el);
    return el;
  }
  el = document.createElement('div');
  el.className = 'z-doc-page';
  el.setAttribute('role', 'group');
  el.setAttribute('aria-label', fill(v.texts.page, { n }));
  el.setAttribute('aria-busy', 'true');
  el.dataset.page = String(n);
  const sheet = document.createElement('div');
  sheet.className = 'z-doc-sheet';
  el.append(sheet);
  return el;
}

/** Keeps a rendered page, dropping the least recently shown ones beyond `MAX_KEPT`. */
function keep(v, n, el) {
  v.kept.delete(n);
  v.kept.set(n, el);
  for (const k of v.kept.keys()) {
    if (v.kept.size <= Math.max(MAX_KEPT, v.shown.size)) break;
    if (!v.shown.has(k)) v.kept.delete(k);
  }
}

/** Shows the pages in view and one on each side, fetching the missing ones. */
function draw(v) {
  v.frame = 0;
  const top = v.view.scrollTop;
  const first = Math.max(1, Math.floor((top - GAP) / v.slot));
  const last = Math.min(v.count, Math.floor((top + v.view.clientHeight) / v.slot) + 1 + 1);
  for (const [n, el] of v.shown) {
    if (n >= first && n <= last) continue;
    el.remove();
    v.shown.delete(n);
    if (el.dataset.state === 'ready') keep(v, n, el);
    v.failed.delete(n);
  }
  for (const [n, ctl] of v.loading) {
    if (n < first || n > last) {
      ctl.abort();
      v.loading.delete(n);
    }
  }
  for (let n = first; n <= last; n++) {
    if (v.shown.has(n)) continue;
    const el = pageEl(v, n);
    place(v, n, el);
    v.pages.append(el);
    v.shown.set(n, el);
  }
  const end = top > 0 && top + v.view.clientHeight >= v.view.scrollHeight - 1;
  const line = top + Math.min(v.view.clientHeight / 3, v.slot / 2);
  const center = end ? v.count : Math.min(v.count, Math.max(1, Math.floor(line / v.slot) + 1));
  current(v, center);
  const still = performance.now() - v.scrolled;
  if (still < SETTLE_MS || v.paused) {
    clearTimeout(v.settle);
    v.settle = setTimeout(() => draw(v), v.paused ? 1000 : SETTLE_MS - still);
    return;
  }
  const want = [...v.shown.keys()]
    .filter((n) => v.shown.get(n).dataset.state !== 'ready' && !v.loading.has(n) && !v.failed.has(n))
    .sort((a, b) => Math.abs(a - center) - Math.abs(b - center));
  for (const n of want) {
    if (v.loading.size >= MAX_FETCHES) break;
    load(v, n);
  }
}

/** Fetches page `n` and puts its cleaned HTML into its sheet. */
async function load(v, n) {
  const ctl = new AbortController();
  v.loading.set(n, ctl);
  let body = null;
  try {
    body = await call(v, 'page', { page: n }, ctl.signal);
  } catch {}
  if (v.loading.get(n) !== ctl) return;
  v.loading.delete(n);
  if (body?.retry) {
    v.paused = true;
    setTimeout(() => {
      v.paused = false;
      draw(v);
    }, body.retry * 1000);
    return;
  }
  const el = v.shown.get(n);
  if (el) {
    const sheet = el.firstElementChild;
    if (body && typeof body.html === 'string') {
      sheet.innerHTML = body.html;
      el.dataset.state = 'ready';
      el.removeAttribute('aria-busy');
    } else {
      v.failed.add(n);
      sheet.textContent = fill(v.texts.error, { n });
      el.dataset.state = 'error';
      el.removeAttribute('aria-busy');
    }
  }
  draw(v);
}

/** Makes `n` the current page: the page box, the buttons and, once settled, the live region. */
function current(v, n) {
  if (v.at === n) return;
  v.at = n;
  if (document.activeElement !== v.input) v.input.value = String(n);
  v.prev.disabled = n <= 1;
  v.next.disabled = n >= v.count;
  clearTimeout(v.announce);
  v.announce = setTimeout(() => (v.status.textContent = fill(v.texts.position, { n, count: v.count })), ANNOUNCE_MS);
}

/** Scrolls to the top of page `n`. */
function go(v, n) {
  const page = Math.min(v.count, Math.max(1, Math.round(n) || 1));
  v.view.scrollTop = GAP + (page - 1) * v.slot - GAP / 2;
  v.at = 0;
  draw(v);
}

/** Moves the zoom one step in or out from the current scale. */
function step(v, dir) {
  const now = v.scale;
  const next = dir > 0 ? ZOOMS.find((z) => z > now + 0.001) : [...ZOOMS].reverse().find((z) => z < now - 0.001);
  if (next === undefined) return;
  v.zoom = next;
  layout(v);
}

/** Runs the search and lists its hits as buttons. */
async function search(v) {
  const text = v.query.value.trim();
  v.searching?.abort();
  v.hits.replaceChildren();
  if (!text) {
    v.hitsStatus.textContent = '';
    return;
  }
  const ctl = new AbortController();
  v.searching = ctl;
  let body = null;
  try {
    body = await call(v, 'search', { text }, ctl.signal);
  } catch {}
  if (v.searching !== ctl) return;
  v.searching = null;
  const hits = Array.isArray(body?.hits) ? body.hits : [];
  for (const h of hits) {
    const li = document.createElement('li');
    const b = document.createElement('button');
    b.type = 'button';
    b.className = 'z-doc-goto';
    b.dataset.page = String(Number(h.page) || 1);
    const where = document.createElement('span');
    where.className = 'z-doc-hit-page';
    where.textContent = fill(v.texts.hit, { n: b.dataset.page });
    const what = document.createElement('span');
    what.textContent = String(h.text ?? '');
    b.append(where, what);
    li.append(b);
    v.hits.append(li);
  }
  v.hitsStatus.textContent = hits.length ? fill(body.more ? v.texts.more : v.texts.hits, { count: hits.length }) : v.texts.none;
}

/** Handles the viewer's keys; keys typed into its fields are left to them. */
function key(v, e) {
  if (e.altKey || e.ctrlKey || e.metaKey || e.target.closest('input, select, textarea')) return;
  const moves = { PageDown: v.at + 1, PageUp: v.at - 1, Home: 1, End: v.count };
  if (e.key in moves) {
    e.preventDefault();
    go(v, moves[e.key]);
  } else if (e.key === '+' || e.key === '=') {
    e.preventDefault();
    step(v, 1);
  } else if (e.key === '-') {
    e.preventDefault();
    step(v, -1);
  }
}

/** Sets up the viewers inside `root`. */
function init(root) {
  root.querySelectorAll?.('[data-zeyn-doc]:not([data-ready])').forEach((el) => {
    el.dataset.ready = '';
    const q = (name) => el.querySelector(`[data-doc="${name}"]`);
    let texts = {};
    try {
      texts = JSON.parse(el.dataset.texts ?? '{}') ?? {};
    } catch {}
    const v = {
      root: el,
      view: q('view'),
      pages: q('pages'),
      input: q('page'),
      prev: q('prev'),
      next: q('next'),
      zoomSelect: q('zoom'),
      status: q('status'),
      query: q('search'),
      hits: q('hits'),
      hitsStatus: el.querySelector('.z-doc-status'),
      count: Math.max(1, Number(el.dataset.pages) || 1),
      w: Number(el.dataset.width) || 794,
      h: Number(el.dataset.height) || 1123,
      zoom: 'width',
      texts,
      shown: new Map(),
      kept: new Map(),
      loading: new Map(),
      failed: new Set(),
      at: 0,
      scrolled: 0,
      settle: 0,
      paused: false,
      slot: 0,
      frame: 0,
      announce: 0,
      searching: null,
      timer: 0,
    };
    if (!v.view || !v.pages) return;
    v.view.addEventListener(
      'scroll',
      () => {
        v.scrolled = performance.now();
        v.frame ||= requestAnimationFrame(() => draw(v));
      },
      { passive: true },
    );
    el.addEventListener('keydown', (e) => key(v, e));
    el.addEventListener('click', (e) => {
      const b = e.target.closest('button');
      if (!b || !el.contains(b)) return;
      if (b.classList.contains('z-doc-goto')) go(v, Number(b.dataset.page));
      else if (b === v.prev) go(v, v.at - 1);
      else if (b === v.next) go(v, v.at + 1);
      else if (b.dataset.doc === 'in') step(v, 1);
      else if (b.dataset.doc === 'out') step(v, -1);
    });
    v.input.addEventListener('keydown', (e) => {
      if (e.key !== 'Enter') return;
      e.preventDefault();
      go(v, Number(v.input.value));
    });
    v.input.addEventListener('change', () => go(v, Number(v.input.value)));
    v.input.addEventListener('blur', () => (v.input.value = String(v.at)));
    v.zoomSelect.addEventListener('change', () => {
      const z = v.zoomSelect.value;
      v.zoom = z === 'width' || z === 'page' ? z : Number(z) / 100;
      layout(v);
    });
    v.query?.addEventListener('input', () => {
      clearTimeout(v.timer);
      v.timer = setTimeout(() => search(v), SEARCH_MS);
    });
    let width = 0;
    new ResizeObserver(() => {
      if (v.view.clientWidth === width && typeof v.zoom === 'number') return;
      width = v.view.clientWidth;
      layout(v);
    }).observe(v.view);
    layout(v);
  });
}

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
