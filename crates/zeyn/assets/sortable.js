/**
 * Sortable lists and card grids (`CardGrid::sortable`, `List::of(..).sortable(..)`), marked
 * `[data-zeyn-sortable]`. Loaded only by pages that hold one.
 *
 * Every row (`li[data-zeyn-key]`) has a handle. A mouse or pen drags it after 6 px, a touch
 * after a 250 ms hold (`drag.js`); the row follows the pointer and a line shows where it lands.
 * Row boxes are measured once when the drag starts, so a move only does arithmetic and writes
 * transforms. On the focused handle Space lifts the row, the arrow keys (and Home, End) move it,
 * Space or Enter drops it, Escape and Tab put it back, and Alt with Up or Down moves it at
 * once. A click or Enter opens the move menu (APG menu button), the single pointer way to move.
 *
 * A move shows at once and sends `POST /_zeyn/data` (key `reorder`) with the row's key and the
 * keys of its new neighbors (`after`, `before`), one request after the other per list. When the
 * server refuses, the row goes back and the reason is shown. While a list has more rows to load,
 * no row can become the last one, because its next neighbor is not known yet.
 * @module sortable
 */

const search = new URL(import.meta.url).search;
const { press, capture, swallowClick, announce } = await import(new URL(`drag.js${search}`, import.meta.url).href);
const { parse, post } = await import(new URL(`net.js${search}`, import.meta.url).href);

/** Replaces `{name}` placeholders. */
const fill = (t, v) => (t ?? '').replace(/\{(\w+)\}/g, (m, k) => (k in v ? String(v[k]) : m));

/** The texts of a sortable section. */
function texts(section) {
  section.zeynTexts ??= parse(section.dataset.texts ?? '{}') ?? {};
  return section.zeynTexts;
}

const rowsOf = (list) => [...list.querySelectorAll(':scope > li[data-zeyn-key]')];
const sepOf = (li) => (li.previousElementSibling?.classList.contains('z-separator') ? li.previousElementSibling : null);
const nameOf = (li) => (li.querySelector('.z-list-title, .z-poster-title, h2, h3, h4, strong') ?? li).textContent.trim().slice(0, 80);

/** Whether every row of the list is loaded, so a row may become the last one. */
const complete = (section) => !section.dataset.zeynNext;

/** Where a row of `rows` may go: slots `0..=max`, slot `s` meaning before `rows[s]`. */
const maxSlot = (section, rows) => (complete(section) ? rows.length : Math.max(0, rows.length - 1));

/** Names the handles after their rows and shows them. */
function init(root) {
  const scope = root instanceof Element ? root.closest('[data-zeyn-sortable]') ?? root : document;
  const sections = scope.matches?.('[data-zeyn-sortable]') ? [scope] : [...scope.querySelectorAll('[data-zeyn-sortable]')];
  for (const section of sections) {
    section.classList.add('z-sort-on');
    const t = texts(section);
    for (const handle of section.querySelectorAll('.z-sort-handle:not([data-named])')) {
      handle.dataset.named = '';
      handle.setAttribute('aria-label', fill(t.handle, { item: nameOf(handle.closest('li')) }));
    }
  }
}

/** Puts `li` (with its separator) before the row `before`, or at the end of the list. */
function place(list, li, before) {
  const sep = sepOf(li);
  sep?.remove();
  li.remove();
  const anchor = before ? sepOf(before) ?? before : null;
  if (sep) list.insertBefore(sep, anchor);
  list.insertBefore(li, anchor);
}

/** Position words of a row: `{ item, pos, count }`. */
function where(section, li) {
  const rows = rowsOf(li.parentElement);
  const count = Number(section.dataset.zeynTotal) || rows.length;
  return { item: nameOf(li), pos: rows.indexOf(li) + 1, count };
}

/** Sends a finished move, one at a time per list; puts the row back when it is refused. */
function commit(section, li, back) {
  const rows = rowsOf(li.parentElement);
  const i = rows.indexOf(li);
  const input = { key: li.dataset.zeynKey, after: rows[i - 1]?.dataset.zeynKey ?? null, before: rows[i + 1]?.dataset.zeynKey ?? null };
  const screen = section.closest('[data-zeyn-screen]');
  const body = {
    screen: screen?.dataset.zeynScreen ?? null,
    params: parse(screen?.dataset.zeynParams ?? 'null'),
    node: section.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key: 'reorder',
    input,
  };
  const t = texts(section);
  announce(fill(t.dropped, where(section, li)));
  section.zeynQueue = (section.zeynQueue ?? Promise.resolve()).then(async () => {
    const r = await post('/_zeyn/data', body);
    if (r.ok) return;
    const focused = li.contains(document.activeElement);
    if (back === null || back.parentElement === li.parentElement) place(li.parentElement, li, back);
    if (focused) li.querySelector('.z-sort-handle')?.focus();
    announce(fill(t.refused, where(section, li)));
    document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message: r.body?.message || fill(t.refused, where(section, li)), tone: 'danger' }] } }));
  });
}

/** Moves `li` to slot `slot` of its list and keeps its handle focused. */
function moveTo(li, slot) {
  const list = li.parentElement;
  const rows = rowsOf(list);
  place(list, li, rows[slot] ?? null);
  li.querySelector('.z-sort-handle')?.focus();
}

/** Moves a row by `step` rows (or to an end) and sends it. */
function step(li, to) {
  const section = li.closest('[data-zeyn-sortable]');
  const rows = rowsOf(li.parentElement);
  const i = rows.indexOf(li);
  const slot = to === 'top' ? 0 : to === 'bottom' ? rows.length : i + (to > 0 ? 2 : -1);
  if (slot < 0 || slot > maxSlot(section, rows) || slot === i || slot === i + 1) return;
  const back = rows[i + 1] ?? null;
  moveTo(li, slot);
  commit(section, li, back);
}

/* Keyboard ----------------------------------------------------------------------------------- */

let lifted = null;

/** Ends a keyboard move: sends it when `drop` and the row moved, else puts it back. */
function settle(drop) {
  if (!lifted) return;
  const { li, back, section } = lifted;
  lifted = null;
  li.classList.remove('z-sort-lifted');
  li.querySelector('.z-sort-handle')?.setAttribute('aria-pressed', 'false');
  const moved = rowsOf(li.parentElement)[rowsOf(li.parentElement).indexOf(li) + 1] ?? null;
  if (moved === back) {
    announce(fill(drop ? texts(section).dropped : texts(section).cancelled, where(section, li)));
    return;
  }
  if (drop) {
    commit(section, li, back);
    return;
  }
  place(li.parentElement, li, back);
  li.querySelector('.z-sort-handle')?.focus();
  announce(fill(texts(section).cancelled, where(section, li)));
}

/** Keys of a lifted row. */
function liftedKey(e, horizontal) {
  const { li, section } = lifted;
  const rows = rowsOf(li.parentElement);
  const i = rows.indexOf(li);
  const rtl = getComputedStyle(li).direction === 'rtl';
  const back = rtl ? 'ArrowRight' : 'ArrowLeft';
  const fwd = rtl ? 'ArrowLeft' : 'ArrowRight';
  let slot = null;
  if (e.key === 'ArrowUp' || (horizontal && e.key === back)) slot = i - 1;
  else if (e.key === 'ArrowDown' || (horizontal && e.key === fwd)) slot = i + 2;
  else if (e.key === 'Home') slot = 0;
  else if (e.key === 'End') slot = rows.length;
  else if (e.key === ' ' || e.key === 'Enter') settle(true);
  else if (e.key === 'Escape') settle(false);
  else if (e.key === 'Tab') return settle(false);
  else return;
  e.preventDefault();
  e.stopPropagation();
  if (slot === null || slot < 0 || slot > maxSlot(section, rows) || slot === i || slot === i + 1) return;
  moveTo(li, slot);
  announce(fill(texts(section).moved, where(section, li)));
}

document.addEventListener('keydown', (e) => {
  if (menu && menu.el.contains(e.target)) return menuKey(e);
  const handle = e.target instanceof Element ? e.target.closest('[data-zeyn-sortable] .z-sort-handle') : null;
  if (!handle) return;
  const li = handle.closest('li');
  const section = handle.closest('[data-zeyn-sortable]');
  const horizontal = !section.querySelector('.z-list');
  if (lifted?.li === li) return liftedKey(e, horizontal);
  if (e.key === ' ') {
    e.preventDefault();
    if (e.repeat) return;
    closeMenu(false);
    const rows = rowsOf(li.parentElement);
    lifted = { li, section, back: rows[rows.indexOf(li) + 1] ?? null };
    li.classList.add('z-sort-lifted');
    handle.setAttribute('aria-pressed', 'true');
    announce(fill(texts(section).lifted, where(section, li)));
    return;
  }
  if (e.altKey && (e.key === 'ArrowUp' || e.key === 'ArrowDown')) {
    e.preventDefault();
    step(li, e.key === 'ArrowUp' ? -1 : 1);
    return;
  }
  if (e.key === 'ArrowUp' || e.key === 'ArrowDown') {
    const rows = rowsOf(li.parentElement);
    const next = rows[rows.indexOf(li) + (e.key === 'ArrowUp' ? -1 : 1)];
    if (next) {
      e.preventDefault();
      next.querySelector('.z-sort-handle')?.focus();
    }
  }
});

document.addEventListener('keyup', (e) => {
  if (e.key === ' ' && e.target instanceof Element && e.target.closest('[data-zeyn-sortable] .z-sort-handle')) e.preventDefault();
});

document.addEventListener('focusout', (e) => {
  if (lifted && e.target === lifted.li.querySelector('.z-sort-handle') && !lifted.li.contains(e.relatedTarget)) {
    queueMicrotask(() => {
      if (lifted && !lifted.li.contains(document.activeElement)) settle(false);
    });
  }
});

/* Move menu ---------------------------------------------------------------------------------- */

let menu = null;

/** Closes the move menu, focus back on its handle. */
function closeMenu(refocus = true) {
  if (!menu) return;
  const { el, handle } = menu;
  menu = null;
  el.remove();
  handle.setAttribute('aria-expanded', 'false');
  if (refocus) handle.focus();
}

/** Opens the move menu of a row with the moves it can make. */
function openMenu(handle) {
  closeMenu(false);
  const li = handle.closest('li');
  const section = handle.closest('[data-zeyn-sortable]');
  const t = texts(section);
  const rows = rowsOf(li.parentElement);
  const i = rows.indexOf(li);
  const last = maxSlot(section, rows);
  const el = document.createElement('ul');
  el.className = 'z-sort-menu';
  el.id = `z-sort-menu-${Date.now()}`;
  el.setAttribute('role', 'menu');
  el.setAttribute('aria-label', fill(t.menu, { item: nameOf(li) }));
  const options = [
    ['top', i > 0],
    ['up', i > 0],
    ['down', i + 2 <= last],
    ['bottom', complete(section) && i < rows.length - 1],
  ];
  for (const [to, ok] of options) {
    if (!ok) continue;
    const item = document.createElement('li');
    item.setAttribute('role', 'menuitem');
    item.tabIndex = -1;
    item.dataset.to = to;
    item.textContent = t[to];
    el.append(item);
  }
  if (!el.children.length) return;
  li.append(el);
  handle.setAttribute('aria-expanded', 'true');
  handle.setAttribute('aria-controls', el.id);
  menu = { el, handle, li };
  el.querySelector('[role="menuitem"]').focus();
}

/** Runs a menu choice. */
function choose(item) {
  const { li } = menu;
  const to = item.dataset.to;
  closeMenu();
  step(li, to === 'up' ? -1 : to === 'down' ? 1 : to);
}

/** Keys inside the move menu. */
function menuKey(e) {
  const items = [...menu.el.querySelectorAll('[role="menuitem"]')];
  const i = items.indexOf(document.activeElement);
  const go = (n) => items[(n + items.length) % items.length]?.focus();
  if (e.key === 'ArrowDown') go(i + 1);
  else if (e.key === 'ArrowUp') go(i - 1);
  else if (e.key === 'Home') go(0);
  else if (e.key === 'End') go(items.length - 1);
  else if (e.key === 'Escape') closeMenu();
  else if (e.key === 'Tab') return closeMenu(false);
  else if ((e.key === 'Enter' || e.key === ' ') && items[i]) choose(items[i]);
  else return;
  e.preventDefault();
  e.stopPropagation();
}

document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  const handle = t?.closest('[data-zeyn-sortable] .z-sort-handle');
  if (handle) {
    if (lifted) return;
    if (menu?.handle === handle) closeMenu();
    else openMenu(handle);
    return;
  }
  const item = t?.closest('.z-sort-menu [role="menuitem"]');
  if (item && menu) return choose(item);
  if (menu && !menu.el.contains(t)) closeMenu(false);
});

/* Pointer ------------------------------------------------------------------------------------ */

/** Follows a pointer drag of the row of `handle`, started by `drag.js`. */
function drag(handle, start) {
  closeMenu(false);
  settle(false);
  const li = handle.closest('li');
  const list = li.parentElement;
  const section = handle.closest('[data-zeyn-sortable]');
  const rows = rowsOf(list);
  const from = rows.indexOf(li);
  const last = maxSlot(section, rows);
  const vertical = list.classList.contains('z-list');
  const sx = scrollX;
  const sy = scrollY;
  const box = list.getBoundingClientRect();
  const boxes = rows.map((row) => {
    const r = row.getBoundingClientRect();
    return { l: r.left + sx, t: r.top + sy, r: r.right + sx, b: r.bottom + sy };
  });
  const origin = { x: box.left + sx, y: box.top + sy };
  const line = document.createElement('div');
  line.className = vertical ? 'z-sort-line' : 'z-sort-line z-sort-line-v';
  line.setAttribute('aria-hidden', 'true');
  if (vertical) line.style.inlineSize = `${box.width}px`;
  list.append(line);
  li.classList.add('z-sort-dragging');
  let slot = from;
  const rtl = getComputedStyle(list).direction === 'rtl';
  const slotAt = (x, y) => {
    if (vertical) {
      let lo = 0;
      let hi = boxes.length;
      while (lo < hi) {
        const mid = (lo + hi) >> 1;
        if (y < (boxes[mid].t + boxes[mid].b) / 2) hi = mid;
        else lo = mid + 1;
      }
      return lo;
    }
    let best = 0;
    let dist = Infinity;
    for (let k = 0; k < boxes.length; k++) {
      const b = boxes[k];
      const dx = x < b.l ? b.l - x : x > b.r ? x - b.r : 0;
      const dy = y < b.t ? b.t - y : y > b.b ? y - b.b : 0;
      const d = dx * dx + dy * dy;
      if (d < dist) {
        dist = d;
        best = k;
      }
    }
    const b = boxes[best];
    return best + ((x > (b.l + b.r) / 2) !== rtl ? 1 : 0);
  };
  const paintLine = () => {
    const show = slot !== from && slot !== from + 1;
    line.hidden = !show;
    if (!show) return;
    if (vertical) {
      const y = slot < boxes.length ? boxes[slot].t : boxes[boxes.length - 1].b;
      line.style.transform = `translate(0, ${y - origin.y - 1}px)`;
      return;
    }
    const b = boxes[Math.min(slot, boxes.length - 1)];
    const atEnd = slot >= boxes.length;
    const x = atEnd !== rtl ? b.r : b.l;
    line.style.blockSize = `${b.b - b.t}px`;
    line.style.transform = `translate(${x - origin.x - 1}px, ${b.t - origin.y}px)`;
  };
  paintLine();
  const x0 = start.clientX;
  const y0 = start.clientY;
  const scrolled = { x: sx, y: sy };
  const onScroll = () => {
    scrolled.x = scrollX;
    scrolled.y = scrollY;
  };
  addEventListener('scroll', onScroll, { passive: true });
  const move = (ev) => {
    const dx = ev.clientX - x0 + scrolled.x - sx;
    const dy = ev.clientY - y0 + scrolled.y - sy;
    li.style.transform = vertical ? `translate(0, ${dy}px)` : `translate(${dx}px, ${dy}px)`;
    const next = Math.min(last, slotAt(ev.clientX + scrolled.x, ev.clientY + scrolled.y));
    if (next === slot) return;
    slot = next;
    paintLine();
    const at = slot > from ? slot - 1 : slot;
    announce(fill(texts(section).moved, { item: nameOf(li), pos: at + 1, count: Number(section.dataset.zeynTotal) || rows.length }));
  };
  const end = (ev, cancelled) => {
    removeEventListener('scroll', onScroll);
    line.remove();
    li.classList.remove('z-sort-dragging');
    li.style.transform = '';
    if (!cancelled) swallowClick();
    if (cancelled || slot === from || slot === from + 1) {
      if (cancelled) announce(fill(texts(section).cancelled, where(section, li)));
      return;
    }
    const back = rows[from + 1] ?? null;
    moveTo(li, slot);
    commit(section, li, back);
  };
  capture(handle, start, move, end);
}

document.addEventListener('pointerdown', (e) => {
  if (e.button !== 0 || !(e.target instanceof Element)) return;
  const handle = e.target.closest('[data-zeyn-sortable] .z-sort-handle');
  if (!handle) return;
  press(e, { onStart: (ev) => drag(handle, ev) });
});

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail : document));
init(document);
