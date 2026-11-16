/**
 * Surfaces (`data-zeyn-surface`): items placed freely on an area of a fixed size.
 *
 * A mouse or pen drags an item after it moves 6 px, a touch after a 250 ms hold (`drag.js`).
 * The arrow keys move the focused item one snap step (10 px without a grid). With a single
 * pointer (WCAG 2.5.7), a click picks an item and a click on an empty spot of the surface puts it
 * there. Every move keeps the item inside the surface, on the grid and on its axis, then posts
 * the place to the surface's `move` source; the server checks it again and answers with the place
 * it stored, which the item takes. A refused move puts the item back.
 * @module surface
 */

const search = new URL(import.meta.url).search;
/** The shared request helpers (`net.js`) and the drag core (`drag.js`). */
const { parse, post } = await import(new URL(`net.js${search}`, import.meta.url).href);
const { press, capture, swallowClick, announce } = await import(new URL(`drag.js${search}`, import.meta.url).href);

const fill = (t, v) => (t ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in v ? String(v[k]) : m));

/** Milliseconds the arrow keys wait before the place is saved. */
const KEY_SAVE_MS = 400;

/** Left and top of an item, in pixels from the surface. */
function placeOf(item) {
  return { x: item.offsetLeft, y: item.offsetTop };
}

/** `v` on the grid of `surface` and inside `0..=max`. */
function fit(surface, v, max) {
  const step = Number(surface.dataset.snap) || 0;
  let n = Math.min(Math.max(v, 0), max);
  if (step > 1) {
    n = Math.round(n / step) * step;
    if (n > max) n -= step;
  }
  return Math.min(Math.max(n, 0), max);
}

/** The place `item` takes when asked for `(x, y)`: on its axis, on the grid, inside the bounds. */
function fitPlace(item, x, y, from = placeOf(item)) {
  const surface = item.closest('[data-zeyn-surface]');
  const axis = item.dataset.axis;
  return {
    x: fit(surface, axis === 'y' ? from.x : x, surface.clientWidth - item.offsetWidth),
    y: fit(surface, axis === 'x' ? from.y : y, surface.clientHeight - item.offsetHeight),
  };
}

function put(item, p) {
  item.style.left = `${p.x}px`;
  item.style.top = `${p.y}px`;
}

/** Saves the place of `item`; `back` is where it returns when the server says no. */
async function save(item, back) {
  const surface = item.closest('[data-zeyn-surface]');
  const p = placeOf(item);
  announce(fill(surface.dataset.textMoved, { item: item.getAttribute('aria-label'), x: p.x, y: p.y }));
  if (!surface.hasAttribute('data-save')) return;
  const screen = surface.closest('[data-zeyn-screen]');
  const r = await post('/_zeyn/data', {
    screen: screen?.dataset.zeynScreen ?? null,
    params: parse(screen?.dataset.zeynParams ?? 'null'),
    node: surface.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key: 'move',
    input: { id: item.dataset.id, x: p.x, y: p.y },
  });
  if (r.ok && Number.isFinite(r.body?.x)) return put(item, { x: r.body.x, y: r.body.y });
  if (r.ok) return;
  put(item, back);
  announce(surface.dataset.textRefused);
  document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message: r.body?.message || surface.dataset.textRefused, tone: 'danger' }] } }));
}

/** Follows a pointer drag of `item` pressed at `down`, started by the drag core with `start`. */
function drag(item, down, start) {
  const from = placeOf(item);
  const noPan = (ev) => ev.cancelable && ev.preventDefault();
  if (start.pointerType === 'touch') document.addEventListener('touchmove', noPan, { passive: false });
  item.dataset.dragging = '';
  capture(
    item,
    start,
    (ev) => put(item, fitPlace(item, from.x + ev.clientX - down.clientX, from.y + ev.clientY - down.clientY, from)),
    (ev, cancelled) => {
      document.removeEventListener('touchmove', noPan);
      delete item.dataset.dragging;
      if (ev.type === 'pointerup') swallowClick();
      if (cancelled) return put(item, from);
      const to = placeOf(item);
      if (to.x !== from.x || to.y !== from.y) save(item, from);
    },
  );
}

/** Picks `item` for a click on the surface, or drops the pick. */
function pick(item, on) {
  const surface = item.closest('[data-zeyn-surface]');
  for (const i of surface.querySelectorAll('.z-surface-item[data-picked]')) delete i.dataset.picked;
  if (!on) return;
  item.dataset.picked = '';
  announce(fill(surface.dataset.textPicked, { item: item.getAttribute('aria-label') }));
}

document.addEventListener('pointerdown', (e) => {
  const item = e.target instanceof Element ? e.target.closest('.z-surface-item') : null;
  if (!item || e.button !== 0 || !e.isPrimary || !item.closest('[data-zeyn-surface]')) return;
  press(e, { onStart: (ev) => drag(item, e, ev) });
});

document.addEventListener('click', (e) => {
  if (!(e.target instanceof Element)) return;
  const surface = e.target.closest('[data-zeyn-surface]');
  if (!surface) return;
  const item = e.target.closest('.z-surface-item');
  if (item) {
    if (e.target.closest('a, button, input, select, textarea, [contenteditable]') && !e.target.matches('.z-surface-item')) return;
    pick(item, !item.hasAttribute('data-picked'));
    return;
  }
  const picked = surface.querySelector('.z-surface-item[data-picked]');
  if (!picked) return;
  pick(picked, false);
  const box = surface.getBoundingClientRect();
  const from = placeOf(picked);
  put(picked, fitPlace(picked, e.clientX - box.left - surface.clientLeft, e.clientY - box.top - surface.clientTop));
  const to = placeOf(picked);
  if (to.x !== from.x || to.y !== from.y) save(picked, from);
  picked.focus();
});

document.addEventListener('keydown', (e) => {
  const item = e.target instanceof Element && e.target.matches('.z-surface-item') ? e.target : null;
  if (!item) return;
  if (e.key === 'Escape' && item.hasAttribute('data-picked')) return pick(item, false);
  const keys = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  const dir = keys[e.key];
  if (!dir || e.altKey || e.ctrlKey || e.metaKey) return;
  e.preventDefault();
  const step = Number(item.closest('[data-zeyn-surface]').dataset.snap) || 10;
  const now = placeOf(item);
  item.zeynFrom ??= now;
  put(item, fitPlace(item, now.x + dir[0] * step, now.y + dir[1] * step));
  clearTimeout(item.zeynSave);
  item.zeynSave = setTimeout(() => {
    const back = item.zeynFrom;
    item.zeynFrom = null;
    const to = placeOf(item);
    if (to.x !== back.x || to.y !== back.y) save(item, back);
  }, KEY_SAVE_MS);
});
