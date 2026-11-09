/**
 * Trees (W3C APG tree view) with lazy children. Loaded only by pages that hold one
 * (`data-zeyn-tree`). Leaves of a draggable tree (`data-zeyn-tree-drag`) get `draggable`;
 * `treedrag.js` handles the drag and the keyboard insert.
 * @module tree
 */
const K = await import(new URL(`components.js${new URL(import.meta.url).search}`, import.meta.url).href);
const { clickHandlers, fetchData, initializers, keyHandlers, make, readingKey, texts, typeAhead } = K;

/* Tree --------------------------------------------------------------------------------------- */

/** Builds the tree items of one level from the action's items. */
function treeItems(wrap, items, level) {
  const max = Number(wrap.dataset.max) || 1000;
  const list = items.slice(0, max);
  return list.map((item, i) => {
    const li = make('li', 'z-tree-item', {
      role: 'treeitem',
      id: `${wrap.zeynId}-${K.uid()}`,
      tabindex: '-1',
      'aria-level': String(level),
      'aria-setsize': String(list.length),
      'aria-posinset': String(i + 1),
      'aria-selected': 'false',
      'data-id': item.id,
    });
    li.style.setProperty('--z-level', String(level - 1));
    if (item.has_children) {
      li.setAttribute('aria-expanded', 'false');
      li.dataset.lazy = '';
    } else if (wrap.dataset.zeynTreeDrag !== undefined) li.draggable = true;
    const row = make('span', 'z-tree-row');
    row.append(make('span', 'z-tree-toggle', { 'aria-hidden': 'true' }), make('span', 'z-tree-label', { text: item.label }));
    if (item.hint) row.append(make('span', 'z-tree-hint', { text: item.hint }));
    li.append(row);
    return li;
  });
}

/** Loads one level of a tree into `list` (the tree or a group). */
async function loadTree(wrap, list, parent, level) {
  const t = texts(wrap);
  list.setAttribute('aria-busy', 'true');
  const items = await fetchData(wrap, '', { parent });
  list.removeAttribute('aria-busy');
  if (!items) {
    list.replaceChildren(make('li', 'z-tree-note', { role: 'none', text: t.failed }));
    return false;
  }
  const nodes = treeItems(wrap, items, level);
  list.replaceChildren(...(nodes.length ? nodes : [make('li', 'z-tree-note', { role: 'none', text: t.empty })]));
  return true;
}

/** Loads the top level of every tree inside `root`. */
function initTrees(root) {
  root.querySelectorAll('[data-zeyn-tree]:not([data-ready])').forEach(async (wrap) => {
    wrap.dataset.ready = '';
    wrap.zeynId = `z-tree-${K.uid()}`;
    const tree = wrap.querySelector('[role="tree"]');
    await loadTree(wrap, tree, null, 1);
    const first = tree.querySelector('[role="treeitem"]');
    if (first) first.tabIndex = 0;
  });
}

/** Visible tree items in order. */
function visibleItems(tree) {
  return [...tree.querySelectorAll('[role="treeitem"]')].filter((li) => !li.parentElement.closest('[role="treeitem"][aria-expanded="false"]'));
}

/** Moves the roving focus to a tree item. */
function focusItem(li) {
  const tree = li.closest('[role="tree"]');
  tree.querySelectorAll('[role="treeitem"][tabindex="0"]').forEach((x) => (x.tabIndex = -1));
  li.tabIndex = 0;
  li.focus();
}

/** Opens or closes a branch, loading its children the first time. */
async function toggleItem(li, open) {
  if (!li.hasAttribute('aria-expanded')) return;
  const want = open ?? li.getAttribute('aria-expanded') !== 'true';
  li.setAttribute('aria-expanded', String(want));
  if (want && li.dataset.lazy !== undefined) {
    delete li.dataset.lazy;
    const group = make('ul', 'z-tree-group', { role: 'group' });
    li.append(group);
    li.setAttribute('aria-busy', 'true');
    group.append(make('li', 'z-tree-note', { role: 'none', text: texts(li.closest('[data-zeyn-tree]')).loading }));
    const ok = await loadTree(li.closest('[data-zeyn-tree]'), group, li.dataset.id, Number(li.getAttribute('aria-level')) + 1);
    li.removeAttribute('aria-busy');
    if (!ok) li.dataset.lazy = '';
  }
}

/** Selects one item (single selection follows Enter, Space and clicks). */
function selectItem(li) {
  const tree = li.closest('[role="tree"]');
  tree.querySelectorAll('[aria-selected="true"]').forEach((x) => x.setAttribute('aria-selected', 'false'));
  li.setAttribute('aria-selected', 'true');
  li.closest('[data-zeyn-tree]').dispatchEvent(new CustomEvent('zeyn:tree-select', { bubbles: true, detail: { id: li.dataset.id } }));
}

/** Keyboard of a tree (ARIA tree pattern). */
function treeKey(e, li) {
  const tree = li.closest('[role="tree"]');
  const items = visibleItems(tree);
  const i = items.indexOf(li);
  const expanded = li.getAttribute('aria-expanded');
  switch (readingKey(e.key, li)) {
    case 'ArrowDown':
      if (items[i + 1]) focusItem(items[i + 1]);
      break;
    case 'ArrowUp':
      if (items[i - 1]) focusItem(items[i - 1]);
      break;
    case 'Home':
      focusItem(items[0]);
      break;
    case 'End':
      focusItem(items.at(-1));
      break;
    case 'ArrowRight':
      if (expanded === 'false') toggleItem(li, true);
      else if (expanded === 'true') {
        const child = li.querySelector(':scope > [role="group"] > [role="treeitem"]');
        if (child) focusItem(child);
      }
      break;
    case 'ArrowLeft':
      if (expanded === 'true') toggleItem(li, false);
      else {
        const parent = li.parentElement.closest('[role="treeitem"]');
        if (parent) focusItem(parent);
      }
      break;
    case 'Enter':
    case ' ':
      selectItem(li);
      break;
    default: {
      const at = typeAhead(e, items.map((x) => ({ label: x.querySelector('.z-tree-label').textContent })), i);
      if (at < 0) return false;
      focusItem(items[at]);
    }
  }
  e.preventDefault();
  return true;
}

/* Setup -------------------------------------------------------------------------------------- */

initializers.push(initTrees);

keyHandlers.push((e, t) => t.matches('[data-zeyn-tree] [role="treeitem"]') && treeKey(e, t));

clickHandlers.push((e, t) => {
  const row = t.closest('[data-zeyn-tree] .z-tree-row');
  if (!row) return false;
  const li = row.parentElement;
  focusItem(li);
  if (t.closest('.z-tree-toggle')) toggleItem(li);
  else {
    selectItem(li);
    if (li.hasAttribute('aria-expanded')) toggleItem(li);
  }
  return true;
});

initTrees(document);
