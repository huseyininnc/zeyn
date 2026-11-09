/**
 * Virtual lists (`List::of(...).paging(Paging::virtual_scroll())`). Loaded only by pages that
 * hold `[data-zeyn-vlist]`.
 *
 * The scroll area is as tall as every row at the list's fixed height; only the rows in view
 * (and a few around them) are in the page, each placed at `index * height`. Blocks of rows are
 * fetched by offset through `POST /_zeyn/grid`, where the server sets the block size, and at most
 * `MAX_BLOCKS` are kept. Every row carries `aria-posinset` and `aria-setsize` from the server, so
 * screen readers hear its place in the whole list. A row that holds focus stays in the page while
 * it is scrolled out of view.
 * @module vlist
 */

const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const OVERSCAN = 6;
const MAX_BLOCKS = 10;
/** @type {WeakMap<Element, object>} State per list. */
const lists = new WeakMap();

/** Returns the CSRF token of the page, or asks `zeyn.js` for one when the page came without it. */
async function csrf() {
  const token = document.querySelector('meta[name="zeyn-csrf"]')?.content;
  return token || (window.zeynCsrf ? window.zeynCsrf() : '');
}

/** Screen name, params and node path of an element, as the endpoints expect them. */
function address(el) {
  const screen = el.closest('[data-zeyn-screen]');
  let params = null;
  try {
    params = JSON.parse(screen?.dataset.zeynParams ?? 'null');
  } catch {}
  return { screen: screen?.dataset.zeynScreen ?? null, params, node: el.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null };
}

/** Keeps the rows of a parsed block by their index in the whole list. */
function keep(s, b, items) {
  s.blocks.set(b, items);
  while (s.blocks.size > MAX_BLOCKS) s.blocks.delete(s.blocks.keys().next().value);
}

/** Rows of `html` with their index, read from `aria-posinset`. */
function parse(html) {
  const t = document.createElement('template');
  t.innerHTML = html;
  return [...t.content.children].map((li) => [Number(li.getAttribute('aria-posinset')) - 1, li]);
}

/** Fetches block `b` of a list, then draws again. */
async function load(section, b) {
  const s = lists.get(section);
  if (s.loading.has(b)) return;
  s.loading.add(b);
  let body = null;
  try {
    const res = await fetch(BASE + '/_zeyn/grid', {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': await csrf() },
      body: JSON.stringify({ ...address(section), query: { page: { type: 'offset', offset: b * s.block, limit: s.block }, with_total: true } }),
    });
    body = res.ok ? await res.json() : null;
  } catch {}
  s.loading.delete(b);
  if (!body) {
    const status = section.querySelector('[data-zeyn-cards-status]');
    if (status) status.textContent = section.dataset.textError ?? '';
    return;
  }
  keep(s, b, parse(body.rows ?? ''));
  draw(section);
}

/** Places the rows in view, asking for the blocks that are missing. */
function draw(section) {
  const s = lists.get(section);
  if (!s) return;
  s.frame = 0;
  const top = s.view.scrollTop;
  const from = Math.max(0, Math.floor(top / s.h) - OVERSCAN);
  const to = Math.min(s.total, Math.ceil((top + (s.view.clientHeight || 400)) / s.h) + OVERSCAN);
  const want = new Map();
  for (let b = Math.floor(from / s.block); b * s.block < to; b++) {
    const items = s.blocks.get(b);
    if (!items) {
      load(section, b);
      continue;
    }
    for (const [i, li] of items) if (i >= from && i < to) want.set(li, i);
  }
  const active = document.activeElement?.closest?.('[data-zeyn-rows] > li');
  for (const li of [...s.list.children]) if (!want.has(li) && li !== active) li.remove();
  for (const [li, i] of want) {
    if (li.parentElement !== s.list) {
      Object.assign(li.style, { position: 'absolute', insetInline: '0', top: `${i * s.h}px`, height: `${s.h}px`, overflow: 'hidden', boxSizing: 'border-box' });
      s.list.append(li);
    }
  }
}

/** Sets up the virtual lists inside `root`. */
function init(root) {
  root.querySelectorAll?.('[data-zeyn-vlist]:not([data-ready])').forEach((section) => {
    const d = section.dataset;
    d.ready = '';
    const view = section.querySelector('[data-zeyn-vlist-view]');
    const list = section.querySelector('[data-zeyn-rows]');
    if (!view || !list) return;
    const s = { h: Number(d.zeynRowHeight) || 40, block: Number(d.zeynBlock) || 200, total: Number(d.zeynTotal) || 0, blocks: new Map(), loading: new Set(), view, list, frame: 0 };
    lists.set(section, s);
    keep(s, 0, [...list.children].map((li) => [Number(li.getAttribute('aria-posinset')) - 1, li]));
    Object.assign(view.style, { overflowY: 'auto', maxHeight: `min(70vh, ${s.total * s.h}px)` });
    Object.assign(list.style, { position: 'relative', height: `${s.total * s.h}px`, margin: '0', padding: '0' });
    view.addEventListener('scroll', () => (s.frame ||= requestAnimationFrame(() => draw(section))), { passive: true });
    draw(section);
  });
}

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
