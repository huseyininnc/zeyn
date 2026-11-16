/**
 * Surfaces (`data-zeyn-surface`): items placed freely on an area of a fixed size.
 *
 * A mouse or pen drags an item after it moves 6 px, a touch after a 250 ms hold (`drag.js`).
 * The arrow keys move the focused item one snap step (10 px without a grid); Shift and the arrow
 * keys resize a resizable one (`data-min`, `data-max`, `data-aspect`), as does a drag of its
 * corner handle. With a single pointer (WCAG 2.5.7), a click picks an item and a click on an
 * empty spot of the surface puts it there; a click on the handle picks the size and a click on
 * the surface puts the corner there. Every change keeps the item inside the surface, on the grid,
 * on its axis and inside its size limits, then posts the place and size to the surface's `move`
 * source; the server checks them again and answers with what it stored, which the item takes. A
 * refused change puts the item back.
 * @module surface
 */

const search = new URL(import.meta.url).search;
/** The shared request helpers (`net.js`) and the drag core (`drag.js`). */
const { parse, post } = await import(new URL(`net.js${search}`, import.meta.url).href);
const { press, capture, swallowClick, announce } = await import(new URL(`drag.js${search}`, import.meta.url).href);

const fill = (t, v) => (t ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in v ? String(v[k]) : m));

/** Milliseconds the arrow keys wait before the place is saved. */
const KEY_SAVE_MS = 400;

/** Left, top, width and height of an item, in pixels from the surface. */
function boxOf(item) {
  return { x: item.offsetLeft, y: item.offsetTop, w: item.offsetWidth, h: item.offsetHeight };
}

const stepOf = (surface) => Number(surface.dataset.snap) || 0;

/** `v` on the grid of `surface` and inside `0..=max`. */
function fit(surface, v, max) {
  const step = stepOf(surface);
  let n = Math.min(Math.max(v, 0), max);
  if (step > 1) {
    n = Math.round(n / step) * step;
    if (n > max) n -= step;
  }
  return Math.min(Math.max(n, 0), max);
}

/** `v` on the grid of `surface` and inside `min..=max`. */
function fitLen(surface, v, min, max) {
  const lo = Math.min(min, max);
  const step = stepOf(surface);
  let n = Math.min(Math.max(v, lo), max);
  if (step > 1) n = Math.round(n / step) * step;
  return Math.min(Math.max(n, lo), max);
}

/** The place `item` takes when asked for `(x, y)`: on its axis, on the grid, inside the bounds. */
function fitPlace(item, x, y, from = boxOf(item)) {
  const surface = item.closest('[data-zeyn-surface]');
  const axis = item.dataset.axis;
  return {
    x: fit(surface, axis === 'y' ? from.x : x, surface.clientWidth - item.offsetWidth),
    y: fit(surface, axis === 'x' ? from.y : y, surface.clientHeight - item.offsetHeight),
  };
}

const pair = (v) => (v ?? '').split(',').map(Number);

/** The size `item` takes when asked for `(w, h)` with its corner growing away from `at`. */
function fitSize(item, w, h, at = boxOf(item)) {
  const surface = item.closest('[data-zeyn-surface]');
  const [minW, minH] = pair(item.dataset.min);
  const [maxW, maxH] = pair(item.dataset.max);
  const capW = Math.min(maxW, surface.clientWidth - at.x);
  const capH = Math.min(maxH, surface.clientHeight - at.y);
  const width = fitLen(surface, w, minW, capW);
  if (!item.dataset.aspect) return { w: width, h: fitLen(surface, h, minH, capH) };
  const [aw, ah] = pair(item.dataset.aspect);
  const height = Math.min(Math.round((width * ah) / aw), capH);
  return { w: Math.min(width, Math.round((height * aw) / ah)), h: height };
}

function put(item, p) {
  if (Number.isFinite(p.x)) {
    item.style.left = `${p.x}px`;
    item.style.top = `${p.y}px`;
  }
  if (Number.isFinite(p.w)) {
    item.style.width = `${p.w}px`;
    item.style.height = `${p.h}px`;
  }
}

/** Saves the place and size of `item`; `back` is where it returns when the server says no. */
async function save(item, back) {
  const surface = item.closest('[data-zeyn-surface]');
  const p = boxOf(item);
  const resized = p.w !== back.w || p.h !== back.h;
  const text = resized ? surface.dataset.textResized : surface.dataset.textMoved;
  announce(fill(text, { item: item.getAttribute('aria-label'), ...p }));
  if (!surface.hasAttribute('data-save')) return;
  const screen = surface.closest('[data-zeyn-screen]');
  const r = await post('/_zeyn/data', {
    screen: screen?.dataset.zeynScreen ?? null,
    params: parse(screen?.dataset.zeynParams ?? 'null'),
    node: surface.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key: 'move',
    input: { id: item.dataset.id, ...p },
  });
  if (r.ok && Number.isFinite(r.body?.x)) return put(item, r.body);
  if (r.ok) return;
  put(item, back);
  announce(surface.dataset.textRefused);
  document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message: r.body?.message || surface.dataset.textRefused, tone: 'danger' }] } }));
}

const changed = (a, b) => a.x !== b.x || a.y !== b.y || a.w !== b.w || a.h !== b.h;

/** Follows a pointer drag of `item` pressed at `down`, started by the drag core with `start`. */
function drag(item, down, start, resize) {
  const from = boxOf(item);
  const noPan = (ev) => ev.cancelable && ev.preventDefault();
  if (start.pointerType === 'touch') document.addEventListener('touchmove', noPan, { passive: false });
  item.dataset.dragging = '';
  const [aw, ah] = pair(item.dataset.aspect);
  const grow = (dx, dy) => (aw && Math.abs((dy * aw) / ah) > Math.abs(dx) ? (dy * aw) / ah : dx);
  const follow = resize
    ? (ev) => put(item, fitSize(item, from.w + grow(ev.clientX - down.clientX, ev.clientY - down.clientY), from.h + ev.clientY - down.clientY, from))
    : (ev) => put(item, fitPlace(item, from.x + ev.clientX - down.clientX, from.y + ev.clientY - down.clientY, from));
  capture(item, start, follow, (ev, cancelled) => {
    document.removeEventListener('touchmove', noPan);
    delete item.dataset.dragging;
    if (ev.type === 'pointerup') swallowClick();
    if (cancelled) return put(item, from);
    if (changed(boxOf(item), from)) save(item, from);
  });
}

/** Picks `item` (its place, or its size when `mode` is `size`) for a click on the surface. */
function pick(item, mode) {
  const surface = item.closest('[data-zeyn-surface]');
  for (const i of surface.querySelectorAll('.z-surface-item[data-picked]')) delete i.dataset.picked;
  if (!mode) return;
  item.dataset.picked = mode;
  const text = mode === 'size' ? surface.dataset.textPickedSize : surface.dataset.textPicked;
  announce(fill(text, { item: item.getAttribute('aria-label') }));
}

document.addEventListener('pointerdown', (e) => {
  const item = e.target instanceof Element ? e.target.closest('.z-surface-item') : null;
  if (!item || e.button !== 0 || !e.isPrimary || !item.closest('[data-zeyn-surface]')) return;
  const resize = Boolean(e.target.closest('.z-surface-resize'));
  press(e, { onStart: (ev) => drag(item, e, ev, resize) });
});

document.addEventListener('click', (e) => {
  if (!(e.target instanceof Element)) return;
  const surface = e.target.closest('[data-zeyn-surface]');
  if (!surface) return;
  const item = e.target.closest('.z-surface-item');
  if (item) {
    if (e.target.closest('.z-surface-resize')) return pick(item, item.dataset.picked === 'size' ? null : 'size');
    if (e.target.closest('a, button, input, select, textarea, [contenteditable]') && !e.target.matches('.z-surface-item')) return;
    pick(item, item.dataset.picked === 'place' ? null : 'place');
    return;
  }
  const picked = surface.querySelector('.z-surface-item[data-picked]');
  if (!picked) return;
  const mode = picked.dataset.picked;
  pick(picked, null);
  const box = surface.getBoundingClientRect();
  const from = boxOf(picked);
  const px = e.clientX - box.left - surface.clientLeft;
  const py = e.clientY - box.top - surface.clientTop;
  put(picked, mode === 'size' ? fitSize(picked, px - from.x, py - from.y) : fitPlace(picked, px, py));
  if (changed(boxOf(picked), from)) save(picked, from);
  picked.focus();
});

document.addEventListener('keydown', (e) => {
  const item = e.target instanceof Element && e.target.matches('.z-surface-item') ? e.target : null;
  if (!item) return;
  if (e.key === 'Escape' && item.hasAttribute('data-picked')) return pick(item, null);
  const keys = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] };
  const dir = keys[e.key];
  if (!dir || e.altKey || e.ctrlKey || e.metaKey) return;
  const resize = e.shiftKey;
  if (resize && !item.dataset.max) return;
  e.preventDefault();
  const step = stepOf(item.closest('[data-zeyn-surface]')) || 10;
  const now = boxOf(item);
  item.zeynFrom ??= now;
  const dw = item.dataset.aspect ? dir[0] + dir[1] : dir[0];
  put(item, resize ? fitSize(item, now.w + dw * step, now.h + dir[1] * step) : fitPlace(item, now.x + dir[0] * step, now.y + dir[1] * step));
  clearTimeout(item.zeynSave);
  item.zeynSave = setTimeout(() => {
    const back = item.zeynFrom;
    item.zeynFrom = null;
    if (changed(boxOf(item), back)) save(item, back);
  }, KEY_SAVE_MS);
});
