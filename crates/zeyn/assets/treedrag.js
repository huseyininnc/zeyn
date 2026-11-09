/**
 * Dragging leaves of a draggable tree (`data-zeyn-tree-drag`) onto a drop target, such as an
 * island. A drag carries `application/x-zeyn-tree` with the JSON `{ id, path, label }` and the
 * label as `text/plain`. Control+Enter (or Command+Enter) on a focused leaf is the keyboard
 * alternative (WCAG 2.5.7): it dispatches `zeyn:tree-insert` with the same object as `detail`
 * and announces the insert in the tree's live region. One delegated listener serves every tree,
 * however many leaves it has. Loaded only by pages that hold a draggable tree.
 * @module treedrag
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { fill, initializers, keyHandlers, texts, uid } = K;

const MIME = 'application/x-zeyn-tree';

/** The leaf under `t` in a draggable tree, or `null`. */
function leafOf(t) {
  const li = t.closest('[data-zeyn-tree-drag] [role="treeitem"]');
  return li && !li.hasAttribute('aria-expanded') ? li : null;
}

/** What a drag or an insert carries: the leaf's id, the ids from the top level down, its label. */
function payload(li) {
  const path = [];
  for (let at = li; at; at = at.parentElement.closest('[role="treeitem"]')) path.unshift(at.dataset.id);
  return { id: li.dataset.id, path, label: li.querySelector(':scope > .z-tree-row > .z-tree-label').textContent };
}

/** Points each new draggable tree at its keyboard hint. */
function initDrag(root) {
  root.querySelectorAll('[data-zeyn-tree-drag]:not([data-drag-ready])').forEach((wrap) => {
    wrap.dataset.dragReady = '';
    const hint = wrap.querySelector(':scope > .z-tree-keys');
    if (!hint) return;
    hint.id = `z-tree-keys-${uid()}`;
    wrap.querySelector('[role="tree"]').setAttribute('aria-describedby', hint.id);
  });
}

/** Inserts the focused leaf on Control+Enter or Command+Enter. */
function insertKey(e, t) {
  if (e.key !== 'Enter' || !(e.ctrlKey || e.metaKey) || e.altKey || e.shiftKey) return false;
  const li = leafOf(t);
  if (!li) return false;
  e.preventDefault();
  const wrap = li.closest('[data-zeyn-tree-drag]');
  const detail = payload(li);
  wrap.dispatchEvent(new CustomEvent('zeyn:tree-insert', { bubbles: true, composed: true, detail }));
  const live = wrap.querySelector(':scope > [aria-live]');
  if (live) live.textContent = fill(texts(wrap).inserted, { label: detail.label });
  return true;
}

document.addEventListener('dragstart', (e) => {
  const li = e.target instanceof Element && e.dataTransfer ? leafOf(e.target) : null;
  if (!li) return;
  const p = payload(li);
  e.dataTransfer.setData(MIME, JSON.stringify(p));
  e.dataTransfer.setData('text/plain', p.label);
  e.dataTransfer.effectAllowed = 'copy';
});

keyHandlers.unshift(insertKey);
initializers.push(initDrag);
initDrag(document);
