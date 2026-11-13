/**
 * Dragging leaves of a draggable tree (`data-zeyn-tree-drag`) onto a drop target, such as an
 * island. A drag carries `application/x-zeyn-tree` with the JSON `{ id, path, label }` and the
 * label as `text/plain`. Without dragging, Control+Enter (or Command+Enter) on a focused leaf and
 * the Insert button on the focused leaf dispatch the cancelable `zeyn:tree-insert` with the same
 * object as `detail`; a target accepts it with `preventDefault()`, and only then is the insert
 * announced in the tree's live region. One delegated listener serves every tree, however many
 * leaves it has, and each tree has one Insert button that moves to the focused leaf. Loaded only
 * by pages that hold a draggable tree.
 *
 * Items of a movable tree (`data-zeyn-tree-move`, `Tree::movable`) move on the drag core
 * (`drag.js`): a mouse or pen drags an item onto the top, the middle (a branch) or the bottom of
 * another row, and a mark shows where it lands. The Move button on the focused item opens a menu
 * (APG menu button: up, down, out of its branch, into the branch above), the single pointer way
 * to move and the one touch uses; Alt with an arrow key moves at once. A move is sent as
 * `POST /_zeyn/data` (key `move`, `{ id, target, place }`); the server checks it and refuses
 * cycles, and the tree loads the changed levels again (`zeyn:tree-reload`).
 * @module treedrag
 */
const search = new URL(import.meta.url).search;
const K = await import(new URL(`components.js${search}`, import.meta.url).href);
const { address, clickHandlers, fill, keyHandlers, make, parse, post, readingKey, texts } = K;
const { press, capture, swallowClick, announce, scrollerOf, autoScroll } = await import(new URL(`drag.js${search}`, import.meta.url).href);

const MIME = 'application/x-zeyn-tree';

/** The leaf under `t` in a draggable tree, or `null`. */
function leafOf(t) {
  const li = t.closest('[data-zeyn-tree-drag] [role="treeitem"]');
  return li && !li.hasAttribute('aria-expanded') ? li : null;
}

/** What a drag or an insert carries: the leaf's id, the ids from the top level down, its label. */
function payload(li) {
  const path = [];
  for (let at = li; at?.getAttribute('role') === 'treeitem'; at = at.parentElement.closest('[role="treeitem"], [role="tree"]')) {
    path.unshift(at.dataset.id);
  }
  return { id: li.dataset.id, path, label: li.querySelector(':scope > .z-tree-row > .z-tree-label').textContent };
}

/** Offers a leaf to the page; announces the insert when a target accepted it. */
function insert(li) {
  const wrap = li.closest('[data-zeyn-tree-drag]');
  const detail = payload(li);
  const event = new CustomEvent('zeyn:tree-insert', { bubbles: true, cancelable: true, composed: true, detail });
  if (wrap.dispatchEvent(event)) return;
  const live = wrap.querySelector(':scope > [aria-live]');
  if (!live) return;
  live.textContent = '';
  setTimeout(() => (live.textContent = fill(texts(wrap).inserted, { label: detail.label })), 50);
}

/** Inserts the focused leaf on Control+Enter or Command+Enter, once per press. */
function insertKey(e, t) {
  if (e.key !== 'Enter' || !(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey || e.repeat) return false;
  const li = leafOf(t);
  if (!li) return false;
  e.preventDefault();
  insert(li);
  return true;
}

/** Moves the tree's Insert button, the pointer alternative to dragging, onto a leaf. */
function placeButton(li) {
  const wrap = li.closest('[data-zeyn-tree-drag]');
  const t = texts(wrap);
  wrap.zeynInsert ??= make('button', 'z-tree-insert', { type: 'button', tabindex: '-1', 'aria-hidden': 'true', text: t.insert });
  const label = fill(t.insert_label, { label: payload(li).label });
  wrap.zeynInsert.title = label;
  wrap.zeynInsert.setAttribute('aria-label', label);
  li.querySelector(':scope > .z-tree-row').append(wrap.zeynInsert);
}

document.addEventListener('dragstart', (e) => {
  const li = e.target instanceof Element && e.dataTransfer ? leafOf(e.target) : null;
  if (!li) return;
  const p = payload(li);
  e.dataTransfer.setData(MIME, JSON.stringify(p));
  e.dataTransfer.setData('text/plain', p.label);
  e.dataTransfer.effectAllowed = 'copy';
});

document.addEventListener('focusin', (e) => {
  const li = e.target instanceof Element ? leafOf(e.target) : null;
  if (li && !e.target.matches('.z-tree-insert')) placeButton(li);
});

keyHandlers.unshift(insertKey);

clickHandlers.unshift((e, t) => {
  if (!t.matches('[data-zeyn-tree-drag] .z-tree-insert')) return false;
  const li = leafOf(t);
  if (!li) return false;
  li.focus();
  insert(li);
  return true;
});
/* Moving items of a movable tree ------------------------------------------------------------ */


/** The item under `t` in a movable tree, or `null`. */
const movableOf = (t) => t.closest('[data-zeyn-tree-move] [role="treeitem"]');
/** The label of an item. */
const labelOf = (li) => li.querySelector(':scope > .z-tree-row > .z-tree-label').textContent;
/** The parent item of an item, or `null` at the top level. */
const parentOf = (li) => li.parentElement.closest('[role="treeitem"]');
/** The items next to `li` on its level. */
const siblings = (li) => [...li.parentElement.children].filter((x) => x.getAttribute('role') === 'treeitem');

/**
 * The moves the keys and the menu offer for `li`: up, down, out of its parent and into the
 * branch before it. Each is `{ name, target, place }` for the server.
 */
function movesOf(li) {
  const all = siblings(li);
  const i = all.indexOf(li);
  const out = [];
  if (all[i - 1]) out.push({ name: 'up', target: all[i - 1], place: 'before' });
  if (all[i + 1]) out.push({ name: 'down', target: all[i + 1], place: 'after' });
  const parent = parentOf(li);
  if (parent) out.push({ name: 'out', target: parent, place: 'after' });
  if (all[i - 1]?.hasAttribute('aria-expanded')) out.push({ name: 'into', target: all[i - 1], place: 'inside' });
  return out;
}

/** Sends a move to the server and loads the two levels it changed again. */
async function moveItem(li, target, place) {
  const wrap = li.closest('[data-zeyn-tree-move]');
  const t = texts(wrap);
  if (!target || target === li || li.contains(target)) return;
  const id = li.dataset.id;
  const label = labelOf(li);
  const r = await post('/_zeyn/data', { ...address(wrap), key: 'move', input: { id, target: target.dataset.id, place } });
  if (!r.ok) {
    const refused = fill(t.move_refused, { label });
    announce(refused);
    document.dispatchEvent(new CustomEvent('zeyn:run', { detail: { effects: [{ effect: 'toast', message: parse(r.body)?.message || refused, tone: 'danger' }] } }));
    return;
  }
  const from = parentOf(li)?.dataset.id ?? null;
  const to = place === 'inside' ? target.dataset.id : parentOf(target)?.dataset.id ?? null;
  wrap.dispatchEvent(new CustomEvent('zeyn:tree-reload', { bubbles: true, detail: { parents: [...new Set([from, to])], focus: id } }));
  announce(fill(t.moved, { label }));
}

let menu = null;

/** Closes the move menu. */
function closeMenu(refocus = true) {
  if (!menu) return;
  const { el, li } = menu;
  menu = null;
  el.remove();
  if (refocus) li.focus();
}

/** Opens the move menu of `li` under its Move button. */
function openMenu(li) {
  closeMenu(false);
  const t = texts(li.closest('[data-zeyn-tree-move]'));
  const el = make('ul', 'z-tree-menu', { role: 'menu', 'aria-label': fill(t.move_label, { label: labelOf(li) }) });
  const moves = movesOf(li);
  for (const m of moves) {
    const item = make('li', '', { role: 'menuitem', tabindex: '-1', text: t[`move_${m.name}`] });
    item.zeynMove = m;
    el.append(item);
  }
  if (!moves.length) el.append(make('li', 'z-tree-menu-none', { role: 'none', text: t.move_none }));
  li.querySelector(':scope > .z-tree-row').append(el);
  menu = { el, li };
  (el.querySelector('[role="menuitem"]') ?? li).focus();
}

/** Runs the chosen menu item. */
function choose(item) {
  const { li } = menu;
  const m = item.zeynMove;
  closeMenu();
  if (m) moveItem(li, m.target, m.place);
}

/** Keys inside the move menu (APG menu). */
function menuKey(e) {
  const items = [...menu.el.querySelectorAll('[role="menuitem"]')];
  const i = items.indexOf(document.activeElement);
  const go = (n) => items[(n + items.length) % items.length]?.focus();
  if (e.key === 'ArrowDown') go(i + 1);
  else if (e.key === 'ArrowUp') go(i - 1);
  else if (e.key === 'Home') go(0);
  else if (e.key === 'End') go(items.length - 1);
  else if (e.key === 'Escape') closeMenu();
  else if (e.key === 'Tab') return closeMenu(false), false;
  else if ((e.key === 'Enter' || e.key === ' ') && items[i]) choose(items[i]);
  else return false;
  e.preventDefault();
  e.stopPropagation();
  return true;
}

/** Alt with an arrow key moves the focused item: up, down, out (Left) or into the branch before it (Right). */
function moveKey(e, t) {
  if (menu?.el.contains(t)) return menuKey(e);
  if (!e.altKey || e.ctrlKey || e.metaKey || e.shiftKey) return false;
  const li = movableOf(t);
  if (!li || li !== t) return false;
  const name = { ArrowUp: 'up', ArrowDown: 'down', ArrowLeft: 'out', ArrowRight: 'into' }[readingKey(e.key, li)];
  if (!name) return false;
  e.preventDefault();
  const m = movesOf(li).find((x) => x.name === name);
  if (m) moveItem(li, m.target, m.place);
  return true;
}

/** Puts the tree's Move button, the single pointer way to move, onto the focused item. */
function placeMove(li) {
  const wrap = li.closest('[data-zeyn-tree-move]');
  const t = texts(wrap);
  wrap.zeynMove ??= make('button', 'z-tree-move', { type: 'button', tabindex: '-1', 'aria-hidden': 'true', 'aria-haspopup': 'menu', text: t.move });
  const label = fill(t.move_label, { label: labelOf(li) });
  wrap.zeynMove.title = label;
  wrap.zeynMove.setAttribute('aria-label', label);
  li.querySelector(':scope > .z-tree-row').append(wrap.zeynMove);
}

/** Where a drop at `y` over the row of `li` lands: before, inside (a branch) or after. */
function placeAt(li, y) {
  const r = li.querySelector(':scope > .z-tree-row').getBoundingClientRect();
  const f = (y - r.top) / r.height;
  if (li.hasAttribute('aria-expanded')) return f < 0.25 ? 'before' : f > 0.75 ? 'after' : 'inside';
  return f < 0.5 ? 'before' : 'after';
}

/** Drags `li` from the press `start` over the rows of its tree. */
function dragItem(li, start) {
  closeMenu(false);
  const tree = li.closest('[role="tree"]');
  const scroll = autoScroll(scrollerOf(tree));
  li.classList.add('z-tree-dragging');
  let spot = null;
  const mark = (next) => {
    if (spot) spot.li.removeAttribute('data-drop');
    spot = next;
    if (spot) spot.li.dataset.drop = spot.place;
  };
  const onMove = (e) => {
    scroll.at(e.clientX, e.clientY);
    const over = document.elementFromPoint(e.clientX, e.clientY)?.closest('[role="treeitem"]');
    mark(over && over.closest('[role="tree"]') === tree && !li.contains(over) ? { li: over, place: placeAt(over, e.clientY) } : null);
  };
  const onEnd = (e, cancelled) => {
    scroll.stop();
    li.classList.remove('z-tree-dragging');
    const at = spot;
    mark(null);
    if (cancelled) return;
    swallowClick();
    if (at) moveItem(li, at.li, at.place);
  };
  capture(li, start, onMove, onEnd);
}

document.addEventListener('pointerdown', (e) => {
  if (e.button !== 0 || e.pointerType === 'touch' || !(e.target instanceof Element) || e.target.closest('button, .z-tree-menu')) return;
  const li = movableOf(e.target);
  if (li) press(e, { onStart: (ev) => dragItem(li, ev) });
});

document.addEventListener('focusin', (e) => {
  const li = e.target instanceof Element ? movableOf(e.target) : null;
  if (li && li === e.target) placeMove(li);
});

keyHandlers.unshift(moveKey);

clickHandlers.unshift((e, t) => {
  const item = t.closest('.z-tree-menu [role="menuitem"]');
  if (item && menu) {
    choose(item);
    return true;
  }
  if (menu && !menu.el.contains(t)) closeMenu(false);
  if (!t.matches('[data-zeyn-tree-move] .z-tree-move')) return false;
  const li = movableOf(t);
  if (li) openMenu(li);
  return true;
});
