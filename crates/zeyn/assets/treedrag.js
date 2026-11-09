/**
 * Dragging leaves of a draggable tree (`data-zeyn-tree-drag`) onto a drop target, such as an
 * island. A drag carries `application/x-zeyn-tree` with the JSON `{ id, path, label }` and the
 * label as `text/plain`. Without dragging, Control+Enter (or Command+Enter) on a focused leaf and
 * the Insert button on the focused leaf dispatch the cancelable `zeyn:tree-insert` with the same
 * object as `detail`; a target accepts it with `preventDefault()`, and only then is the insert
 * announced in the tree's live region. One delegated listener serves every tree, however many
 * leaves it has, and each tree has one Insert button that moves to the focused leaf. Loaded only
 * by pages that hold a draggable tree.
 * @module treedrag
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { clickHandlers, fill, keyHandlers, make, texts } = K;

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
