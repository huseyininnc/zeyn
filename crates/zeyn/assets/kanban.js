/**
 * Kanban boards (`zeyn::ui::Kanban`). Loaded only by pages that hold `[data-zeyn-kanban]`.
 *
 * Every column is a virtual list (`vlist.js`). The board is one tab stop: the arrow keys move
 * between cards (Left and Right swap under `dir="rtl"`), Home and End go to the first and last
 * card in view, Enter or Space opens the move menu (APG menu button: Up, Down, Home, End,
 * Enter, Escape) and Shift with Left or Right moves the card to the column next to it; on a
 * sortable board Alt with Up or Down moves it inside its column. Cards
 * drag on the drag core (`drag.js`): a mouse or pen after 6 px, a touch after a 250 ms hold, so
 * a swipe still scrolls the column. The board scrolls sideways while a card is held near its
 * edge. On a board with `Kanban::sortable` a card dropped on another card lands before or after
 * it (a line shows where), and inside a column the handle of each card reorders it
 * (`sortable.js`, with its own keys and move menu).
 *
 * A move asks first when its transition has a confirmation (`zeyn:confirm`), then shows the card
 * in the target column at once (optimistic), sends `POST /_zeyn/data` (key `move`) and renders
 * both columns again with one `POST /_zeyn/nodes` when the server accepts. A drop on a card of a
 * sortable column then sends the column's `reorder` with the card's new neighbors (the contract of
 * `sortable.js`), so the card lands where it was dropped. When the server refuses, the
 * card goes back where it was, the counts too, and the reason is shown. Only moves of
 * `data-moves`, the ones the user may make, are offered; the server checks every move again.
 * `performance.measure('zeyn-kanban-move')` spans a move from the key or the drop to the
 * optimistic paint, `zeyn-kanban-saved` to the new columns.
 * @module kanban
 */

const search = new URL(import.meta.url).search;
/** The shared request helpers (`net.js`) and the drag core (`drag.js`). */
const { parse, post } = await import(new URL(`net.js${search}`, import.meta.url).href);
const { press, capture, swallowClick, scrollerOf, autoScroll } = await import(new URL(`drag.js${search}`, import.meta.url).href);

/** Screen name and params of an element. */
function screenOf(el) {
  const screen = el.closest('[data-zeyn-screen]');
  return { screen: screen?.dataset.zeynScreen ?? null, params: parse(screen?.dataset.zeynParams ?? 'null') };
}

/** Board state: moves, texts. */
function boardOf(el) {
  const board = el.closest('[data-zeyn-kanban]');
  if (!board) return null;
  board.zeynMoves ??= parse(board.dataset.moves ?? '{}') ?? {};
  board.zeynTexts ??= parse(board.dataset.texts ?? '{}') ?? {};
  return board;
}

const cols = (board) => [...board.querySelectorAll(':scope .z-kanban-col')];
const cardsOf = (col) => [...col.querySelectorAll('[data-zeyn-kanban-card]')].filter((c) => !c.closest('[hidden]'));
const titleOf = (card) => (card.querySelector('h1, h2, h3, h4, strong, .z-list-title, p') ?? card).textContent.trim().slice(0, 80);

/** Makes `card` the one tab stop of its board and focuses it. */
function focusCard(card, focus = true) {
  const board = card?.closest('[data-zeyn-kanban]');
  if (!board) return;
  for (const c of board.querySelectorAll('[data-zeyn-kanban-card][tabindex="0"]')) c.tabIndex = -1;
  card.tabIndex = 0;
  if (focus) card.focus({ preventScroll: false });
}

/** Gives every board one tab stop and shows the move buttons. */
function init(root) {
  const boards = root.matches?.('[data-zeyn-kanban]') ? [root] : [...(root.querySelectorAll?.('[data-zeyn-kanban]') ?? [])];
  boards.forEach((board) => {
    boardOf(board);
    for (const b of board.querySelectorAll('.z-kanban-move[hidden]')) b.hidden = false;
    for (const h of board.querySelectorAll('.z-sort-handle')) h.tabIndex = -1;
    if (!board.querySelector('[data-zeyn-kanban-card][tabindex="0"]')) {
      const first = board.querySelector('[data-zeyn-kanban-card]');
      if (first) first.tabIndex = 0;
    }
  });
}

/** Writes the count of a column. */
function setCount(col, n) {
  col.dataset.count = String(n);
  const wip = Number(col.dataset.wip || 0);
  const el = col.querySelector('[data-zeyn-kanban-count]');
  if (el) el.textContent = wip ? `${n} / ${wip}` : String(n);
  col.classList.toggle('z-kanban-over', wip > 0 && n > wip);
}

/** Says something in the board's live region. */
function say(board, text) {
  const status = board.querySelector('[data-zeyn-kanban-status]');
  if (status) {
    status.textContent = '';
    requestAnimationFrame(() => (status.textContent = text));
  }
}

/** Asks the user through the shared confirm dialog; `true` when no dialog is there to ask. */
function ask(board, move) {
  if (!move.confirm) return Promise.resolve(true);
  return new Promise((resolve) => {
    const t = board.zeynTexts;
    const handled = !document.dispatchEvent(
      new CustomEvent('zeyn:confirm', { cancelable: true, detail: { title: t.confirm_title, message: move.confirm, ok: move.label, cancel: t.cancel, danger: move.danger, resolve } }),
    );
    if (!handled) resolve(false);
  });
}

/** Renders nodes of the board's screen again and swaps them in. */
async function rerender(board, nodes) {
  const r = await post('/_zeyn/nodes', { ...screenOf(board), nodes: nodes.map((n) => n.dataset.zeynNode) });
  const html = r.body?.nodes ?? {};
  for (const node of nodes) {
    const fresh = html[node.dataset.zeynNode];
    if (typeof fresh !== 'string' || !node.isConnected) continue;
    const t = document.createElement('template');
    t.innerHTML = fresh;
    const el = t.content.firstElementChild;
    if (!el) continue;
    node.replaceWith(el);
    document.dispatchEvent(new CustomEvent('zeyn:content', { detail: el }));
  }
}

/**
 * Moves a card to the column of state `to`; `spot` (`{after, before}`, the keys of its new
 * neighbors) places it inside a sortable column.
 */
async function move(card, to, spot = null) {
  const board = boardOf(card);
  const fromCol = card.closest('.z-kanban-col');
  const toCol = cols(board).find((c) => c.dataset.state === to);
  if (!board || !fromCol || !toCol || toCol === fromCol) return;
  const m = (board.zeynMoves[fromCol.dataset.state] ?? []).find((x) => x.to === to);
  if (!m) {
    say(board, board.zeynTexts.none);
    return;
  }
  performance.mark('zeyn-kanban-start');
  if (!(await ask(board, m))) {
    card.focus();
    return;
  }
  const li = card.closest('li');
  const key = li?.dataset.zeynKey;
  if (!key) return;
  const view = toCol.querySelector('[data-zeyn-vlist-view], .z-cards');
  const pending = document.createElement('div');
  pending.className = 'z-kanban-pending';
  const ghost = card.cloneNode(true);
  ghost.removeAttribute('data-zeyn-kanban-card');
  ghost.tabIndex = -1;
  ghost.setAttribute('aria-busy', 'true');
  pending.append(ghost);
  (view ?? toCol).before(pending);
  li.hidden = true;
  const counts = [Number(fromCol.dataset.count || 0), Number(toCol.dataset.count || 0)];
  setCount(fromCol, Math.max(0, counts[0] - 1));
  setCount(toCol, counts[1] + 1);
  const name = titleOf(card);
  requestAnimationFrame(() => performance.measure('zeyn-kanban-move', 'zeyn-kanban-start'));
  const r = await post('/_zeyn/data', {
    ...screenOf(board),
    node: board.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key: 'move',
    input: { key, from: fromCol.dataset.state, to },
  });
  if (!r.ok) {
    pending.remove();
    li.hidden = false;
    setCount(fromCol, counts[0]);
    setCount(toCol, counts[1]);
    focusCard(card);
    say(board, board.zeynTexts.refused);
    document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message: r.body?.message || board.zeynTexts.refused, tone: 'danger' }] } }));
    return;
  }
  const sortable = spot && toCol.querySelector('[data-zeyn-sortable]');
  if (sortable) {
    await post('/_zeyn/data', {
      ...screenOf(board),
      node: sortable.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
      key: 'reorder',
      input: { key, after: spot.after, before: spot.before },
    });
  }
  const nodes = [fromCol, toCol].map((c) => c.querySelector('[data-zeyn-node]')).filter(Boolean);
  await rerender(board, nodes);
  pending.remove();
  const moved = toCol.querySelector(`li[data-zeyn-key="${CSS.escape(key)}"] [data-zeyn-kanban-card]`);
  focusCard(moved ?? cardsOf(fromCol)[0] ?? cardsOf(toCol)[0]);
  performance.measure('zeyn-kanban-saved', 'zeyn-kanban-start');
  say(board, board.zeynTexts.moved.replace('{card}', name).replace('{column}', m.label));
}

/* Move menu ---------------------------------------------------------------------------------- */

let menu = null;

/** Closes the move menu, focus back on its card. */
function closeMenu(refocus = true) {
  if (!menu) return;
  const { el, card, button } = menu;
  menu = null;
  el.remove();
  button?.setAttribute('aria-expanded', 'false');
  if (refocus) card.focus();
}

/** Opens the move menu of a card. */
function openMenu(card) {
  closeMenu(false);
  const board = boardOf(card);
  const state = card.closest('.z-kanban-col')?.dataset.state;
  const options = board.zeynMoves[state] ?? [];
  const button = card.querySelector('.z-kanban-move');
  const el = document.createElement('ul');
  el.className = 'z-kanban-menu';
  el.setAttribute('role', 'menu');
  el.setAttribute('aria-label', board.zeynTexts.move_to);
  el.id = `z-kanban-menu-${Date.now()}`;
  if (!options.length) {
    const li = document.createElement('li');
    li.setAttribute('role', 'none');
    li.className = 'z-kanban-menu-none';
    li.textContent = board.zeynTexts.none;
    el.append(li);
  }
  for (const o of options) {
    const li = document.createElement('li');
    li.setAttribute('role', 'menuitem');
    li.tabIndex = -1;
    li.dataset.to = o.to;
    li.textContent = o.label;
    el.append(li);
  }
  card.append(el);
  button?.setAttribute('aria-expanded', 'true');
  button?.setAttribute('aria-controls', el.id);
  menu = { el, card, button };
  (el.querySelector('[role="menuitem"]') ?? card).focus();
}

/** Keys inside the move menu. */
function menuKey(e) {
  const items = [...menu.el.querySelectorAll('[role="menuitem"]')];
  const i = items.indexOf(document.activeElement);
  const go = (n) => items[(n + items.length) % items.length]?.focus();
  switch (e.key) {
    case 'ArrowDown':
      go(i + 1);
      break;
    case 'ArrowUp':
      go(i - 1);
      break;
    case 'Home':
      go(0);
      break;
    case 'End':
      go(items.length - 1);
      break;
    case 'Escape':
    case 'Tab':
      closeMenu(e.key === 'Escape');
      if (e.key === 'Tab') return;
      break;
    case 'Enter':
    case ' ': {
      const to = document.activeElement?.dataset.to;
      const card = menu.card;
      closeMenu();
      if (to) move(card, to);
      break;
    }
    default:
      return;
  }
  e.preventDefault();
  e.stopPropagation();
}

/* Keyboard ----------------------------------------------------------------------------------- */

/** The reading direction of an element's key: Left and Right swap under `dir="rtl"`. */
function reading(key, el) {
  if (getComputedStyle(el).direction !== 'rtl') return key;
  return key === 'ArrowLeft' ? 'ArrowRight' : key === 'ArrowRight' ? 'ArrowLeft' : key;
}

document.addEventListener('keydown', (e) => {
  if (menu && menu.el.contains(e.target)) return menuKey(e);
  const card = e.target instanceof Element ? e.target.closest('[data-zeyn-kanban-card]') : null;
  if (!card || e.target !== card) return;
  const board = boardOf(card);
  const columns = cols(board);
  const col = card.closest('.z-kanban-col');
  const ci = columns.indexOf(col);
  const list = cardsOf(col);
  const i = list.indexOf(card);
  const key = reading(e.key, card);
  if (e.altKey && (key === 'ArrowUp' || key === 'ArrowDown') && col.querySelector('[data-zeyn-sortable]')) {
    const li = card.closest('li');
    const n = Number(li.getAttribute('aria-posinset'));
    const at = (k) => li.parentElement.querySelector(`:scope > li[aria-posinset="${k}"]`)?.dataset.zeynKey ?? null;
    const total = Number(col.querySelector('[data-zeyn-total]')?.dataset.zeynTotal) || n;
    const up = key === 'ArrowUp';
    const spot = up ? { after: n > 2 ? at(n - 2) : null, before: at(n - 1) } : { after: at(n + 1), before: n + 1 < total ? at(n + 2) : null };
    const known = up ? n > 1 && spot.before && (n <= 2 || spot.after) : n < total && spot.after && (n + 1 >= total || spot.before);
    if (known) reorder(card, spot);
    e.preventDefault();
    return;
  }
  if (e.shiftKey && (key === 'ArrowLeft' || key === 'ArrowRight')) {
    const target = columns[ci + (key === 'ArrowRight' ? 1 : -1)];
    if (target) move(card, target.dataset.state);
    e.preventDefault();
    return;
  }
  let next = null;
  if (key === 'ArrowDown') next = list[i + 1];
  else if (key === 'ArrowUp') next = list[i - 1];
  else if (key === 'Home') next = list[0];
  else if (key === 'End') next = list.at(-1);
  else if (key === 'ArrowRight' || key === 'ArrowLeft') {
    const step = key === 'ArrowRight' ? 1 : -1;
    for (let k = ci + step; k >= 0 && k < columns.length && !next; k += step) {
      const other = cardsOf(columns[k]);
      next = other[Math.min(i, other.length - 1)] ?? null;
    }
  } else if (e.key === 'Enter' || e.key === ' ') {
    openMenu(card);
    e.preventDefault();
    return;
  } else return;
  e.preventDefault();
  if (next) focusCard(next);
});

document.addEventListener('focusin', (e) => {
  const card = e.target instanceof Element ? e.target.closest('[data-zeyn-kanban-card]') : null;
  if (card && e.target === card) focusCard(card, false);
});

/* Pointer ------------------------------------------------------------------------------------ */

document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  const button = t?.closest('.z-kanban-move');
  if (button) {
    const card = button.closest('[data-zeyn-kanban-card]');
    if (menu?.card === card) closeMenu();
    else openMenu(card);
    return;
  }
  const item = t?.closest('.z-kanban-menu [role="menuitem"]');
  if (item && menu) {
    const card = menu.card;
    closeMenu();
    move(card, item.dataset.to);
    return;
  }
  if (menu && !menu.el.contains(t)) closeMenu(false);
});

/**
 * Where a drop at `y` over `li` (a card of a sortable column) puts the dragged card: the keys of
 * its new neighbors, or `null` when a neighbor is not loaded yet. Rows of the virtual list are
 * found by `aria-posinset`, so this needs no layout beyond the one row under the pointer.
 */
function spotAt(li, y) {
  const list = li.parentElement;
  const n = Number(li.getAttribute('aria-posinset'));
  const total = Number(li.closest('[data-zeyn-total]')?.dataset.zeynTotal) || n;
  const r = li.getBoundingClientRect();
  const above = y < r.top + r.height / 2;
  const at = (i) => list.querySelector(`:scope > li[aria-posinset="${i}"]`)?.dataset.zeynKey ?? null;
  const key = li.dataset.zeynKey;
  const after = above ? (n > 1 ? at(n - 1) : null) : key;
  const before = above ? key : n < total ? at(n + 1) : null;
  if ((above && n > 1 && !after) || (!above && n < total && !before)) return null;
  return { after, before, li, above };
}

/** Sends the `reorder` of the sortable column of `card` and draws the column again. */
async function reorder(card, spot) {
  const board = boardOf(card);
  const col = card.closest('.z-kanban-col');
  const section = col.querySelector('[data-zeyn-sortable]');
  const key = card.closest('li').dataset.zeynKey;
  const r = await post('/_zeyn/data', {
    ...screenOf(board),
    node: section.closest('[data-zeyn-node]')?.dataset.zeynNode ?? null,
    key: 'reorder',
    input: { key, after: spot.after, before: spot.before },
  });
  if (!r.ok) {
    say(board, board.zeynTexts.refused);
    document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message: r.body?.message || board.zeynTexts.refused, tone: 'danger' }] } }));
    return;
  }
  await rerender(board, [section.closest('[data-zeyn-node]')]);
  focusCard(col.querySelector(`li[data-zeyn-key="${CSS.escape(key)}"] [data-zeyn-kanban-card]`) ?? cardsOf(col)[0]);
}

/** Drags `card` from the pointer press `start`, on the drag core. */
function drag(card, start) {
  closeMenu(false);
  const board = boardOf(card);
  const from = card.closest('.z-kanban-col');
  const allowed = new Set((board.zeynMoves[from.dataset.state] ?? []).map((m) => m.to));
  for (const c of cols(board)) c.dataset.drop = c === from ? 'from' : allowed.has(c.dataset.state) ? 'ok' : 'no';
  const r = card.getBoundingClientRect();
  const ghost = card.cloneNode(true);
  ghost.className = 'z-kanban-card z-kanban-ghost';
  ghost.removeAttribute('data-zeyn-kanban-card');
  ghost.setAttribute('aria-hidden', 'true');
  Object.assign(ghost.style, { width: `${r.width}px`, height: `${r.height}px`, left: `${r.left}px`, top: `${r.top}px` });
  document.body.append(ghost);
  card.classList.add('z-kanban-dragging');
  const scroll = autoScroll(scrollerOf(card.closest('.z-kanban-board'), 'x') ?? card.closest('.z-kanban-board'));
  const noPan = (e) => e.cancelable && e.preventDefault();
  if (start.pointerType === 'touch') document.addEventListener('touchmove', noPan, { passive: false });
  const own = card.closest('li');
  const sorts = !!from.querySelector('[data-zeyn-sortable]');
  let over = null;
  let spot = null;
  const mark = (next) => {
    spot?.li.classList.remove('z-kanban-before', 'z-kanban-after');
    spot = next;
    spot?.li.classList.add(spot.above ? 'z-kanban-before' : 'z-kanban-after');
  };
  const onMove = (e) => {
    ghost.style.transform = `translate(${e.clientX - start.clientX}px, ${e.clientY - start.clientY}px)`;
    scroll.at(e.clientX, e.clientY);
    const hit = document.elementFromPoint(e.clientX, e.clientY);
    const col = hit?.closest('.z-kanban-col');
    if (col !== over) {
      over?.classList.remove('z-kanban-target');
      over = col?.dataset.drop === 'ok' || (sorts && col === from) ? col : null;
      over?.classList.add('z-kanban-target');
    }
    const li = over?.querySelector('[data-zeyn-sortable]') ? hit.closest('li[data-zeyn-key]') : null;
    mark(li && li !== own && li.closest('.z-kanban-col') === over ? spotAt(li, e.clientY) : null);
  };
  const onEnd = (e, cancelled) => {
    document.removeEventListener('touchmove', noPan);
    scroll.stop();
    ghost.remove();
    card.classList.remove('z-kanban-dragging');
    const target = over;
    const place = spot && { after: spot.after, before: spot.before };
    mark(null);
    for (const c of cols(board)) {
      delete c.dataset.drop;
      c.classList.remove('z-kanban-target');
    }
    if (cancelled) return;
    swallowClick();
    if (target === from) {
      if (place && place.after !== own.dataset.zeynKey && place.before !== own.dataset.zeynKey) reorder(card, place);
    } else if (target) move(card, target.dataset.state, place);
  };
  capture(card, start, onMove, onEnd);
}

document.addEventListener('pointerdown', (e) => {
  if (e.button !== 0) return;
  const t = e.target instanceof Element ? e.target : null;
  const card = t?.closest('[data-zeyn-kanban-card]');
  if (!card || t.closest('button, a, input, select, textarea, .z-kanban-menu')) return;
  press(e, { onStart: (ev) => drag(card, ev) });
});

document.addEventListener('contextmenu', (e) => {
  if (e.target instanceof Element && e.target.closest('.z-kanban-dragging')) e.preventDefault();
});

document.addEventListener('zeyn:content', (e) => init(e.detail instanceof Element ? e.detail.closest('[data-zeyn-kanban]') ?? e.detail : document));
init(document);
