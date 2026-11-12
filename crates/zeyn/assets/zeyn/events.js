document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t) return;
  let el;
  if ((el = t.closest('input[data-zeyn-select]'))) return G?.pickRow(el, e.shiftKey);
  if ((el = t.closest('input[data-zeyn-select-all]'))) return G?.pickPage(el);
  if ((el = t.closest('[data-zeyn-select-matching]'))) {
    const grid = G?.gridOfBar(el);
    const s = grids.get(grid);
    if (s) s.allMatching = true;
    G?.syncSelection(grid);
    return G?.bulkBar(grid)?.querySelector('[data-zeyn-select-clear]')?.focus();
  }
  if ((el = t.closest('[data-zeyn-select-clear]'))) return G?.clearSelection(G?.gridOfBar(el));
  if ((el = t.closest('[data-zeyn-expand]'))) {
    const tr = el.closest('tr');
    return tr.dataset.zeynPath != null ? G?.toggleBranch(tr) : G?.toggleDetail(tr);
  }
  if ((el = t.closest('[data-zeyn-branch-more]'))) return G?.moreBranch(el);
  if ((el = t.closest('[data-zeyn-bulk-run]'))) return G?.runBulk(el);
  if ((el = t.closest('.z-menu-item'))) return runMenuItem(el);
  if ((el = t.closest('button[data-zeyn-run]'))) return G?.runRowItem(el);
  if ((el = t.closest('button.z-sort'))) return G?.toggleSort(el, e.shiftKey);
  if ((el = t.closest('button.z-col-menu'))) return G?.openColumnMenu(el);
  if ((el = t.closest('[data-zeyn-page]'))) return G?.loadGrid(el.closest('[data-zeyn-grid]'), el.dataset.zeynPage === 'prev' ? 'prev' : 'next');
  if ((el = t.closest('[data-zeyn-more] button'))) return G?.loadGrid(el.closest('[data-zeyn-grid]'), 'more');
  if ((el = t.closest('[data-zeyn-export]'))) return exportGrid(el);
  if ((el = t.closest('[data-zeyn-pdf]'))) return downloadPdf(el);
  if ((el = t.closest('[data-zeyn-close]'))) return el.closest('dialog')?.close();
  if ((el = t.closest('button[data-zeyn-effect], a[data-zeyn-effect]'))) {
    e.preventDefault();
    const effect = parseEffect(el);
    if (effect) runEffect(effect, el);
    return;
  }
  if ((el = t.closest('[data-zeyn-rows] > tr > td')) && !t.closest('button, a, input, select, label, .z-editing')) {
    const tr = el.parentElement;
    if (G?.dataRow(tr) && !el.classList.contains('z-col-gap')) G?.focusCell(tr.closest('[data-zeyn-grid]'), tr, G?.cellsOf(tr).indexOf(el), e.shiftKey);
  }
});

document.addEventListener('focusin', (e) => {
  const t = e.target;
  const tr = t instanceof HTMLTableCellElement ? t.parentElement : t;
  if (tr instanceof HTMLTableRowElement && tr.parentElement?.matches('[data-zeyn-rows]')) G?.activate(tr, false);
});

document.addEventListener('paste', (e) => {
  const td = e.target instanceof HTMLTableCellElement ? e.target : null;
  const grid = td?.parentElement?.parentElement?.matches('[data-zeyn-rows]') ? td.closest('[data-zeyn-grid]') : null;
  const text = e.clipboardData?.getData('text/plain');
  if (!grid || !text || !grids.get(grid)?.anchor) return;
  e.preventDefault();
  G?.pasteCells(grid, text);
});

document.addEventListener('dblclick', (e) => {
  const td = e.target instanceof Element ? e.target.closest('[data-zeyn-rows] > tr > td[data-v]') : null;
  if (td && !td.closest('.z-editing') && G?.startEdit(td.parentElement, td)) return;
  if (e.target instanceof Element && e.target.closest('.z-editing')) return;
  const tr = e.target instanceof Element ? e.target.closest('[data-zeyn-rows] > tr[data-zeyn-effect]') : null;
  const effect = tr && parseEffect(tr);
  if (effect) runEffect(effect, tr);
});

document.addEventListener('keydown', (e) => {
  const t = e.target;
  if (t instanceof Element && t.matches('.z-menu-item') && menuKey(e, t)) return;
  if (t instanceof Element && t.matches('.z-cell-input') && G?.editKey(e, t)) return;
  if (e.key === 'F2' && t instanceof HTMLTableRowElement && t.parentElement?.matches('[data-zeyn-rows]')) {
    if (G?.startEdit(t, null)) return e.preventDefault();
  }
  if (t instanceof HTMLTableCellElement && t.parentElement?.parentElement?.matches('[data-zeyn-rows]')) {
    const g = t.closest('[data-zeyn-grid]');
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && (e.key ?? '').toLowerCase() === 'c' && G?.copyCells(g)) return e.preventDefault();
    if (G?.cellKey(e, t)) return;
  }
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && (e.key ?? '').toLowerCase() === 'c' && !typing(t) && t instanceof Element) {
    const g = t.closest('[data-zeyn-grid]');
    const sel = window.getSelection();
    if (g && (!sel || sel.isCollapsed) && G?.copyRows(g, t.closest('[data-zeyn-rows] > tr'))) return e.preventDefault();
  }
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && (e.key ?? '').toLowerCase() === 'z' && !typing(t) && t instanceof Element) {
    const g = t.closest('[data-zeyn-grid]');
    if (g && grids.get(g)?.undo) {
      e.preventDefault();
      return G?.undoEdit(g);
    }
  }
  if (t instanceof Element && t.matches('.z-resize') && G?.resizeKey(e, t)) return;
  if (t instanceof Element && (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) && !typing(t)) {
    if (t.closest('thead th[data-field]')) {
      e.preventDefault();
      return G?.openColumnMenu(t.closest('th').querySelector('.z-col-menu'));
    }
    const owner = t.closest('[data-zeyn-rows] > tr[data-zeyn-menu], [data-zeyn-screen][data-zeyn-menu]');
    if (owner) {
      const box = (owner.matches('tr') ? owner : t).getBoundingClientRect();
      const x = getComputedStyle(owner).direction === 'rtl' ? box.right - 16 : box.left + 16;
      if (openContextMenu(owner, { x, y: box.bottom }, t)) return e.preventDefault();
    }
  }
  const vg = t instanceof Element ? t.closest('[data-zeyn-grid][data-zeyn-paging="virtual"]') : null;
  if (vg && (t === vg || t.parentElement?.matches('[data-zeyn-rows]')) && G?.virtualKey(e, vg)) return;
  if (t instanceof HTMLTableRowElement && t.parentElement?.matches('[data-zeyn-rows]') && G?.rowKey(e, t)) return;
  shortcut(e);
});

document.addEventListener('pointerdown', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t || e.button !== 0) return;
  const handle = t.closest('.z-resize');
  if (handle) return G?.startResize(e, handle);
  const sort = t.closest('[data-zeyn-grid] thead th[data-field] .z-sort');
  if (sort) G?.startColumnDrag(e, sort.closest('th'));
});

/**
 * Right click: our menu where one is defined (a column header, a row with `data-zeyn-menu`, a
 * screen with `data-zeyn-menu`), the browser's own menu elsewhere. Text inputs, text areas and
 * editable text always keep the browser menu. With `<html data-zeyn-native-menu="off">` the
 * browser menu is stopped everywhere else too.
 */
document.addEventListener('contextmenu', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t || typing(t) || t.closest('.z-menu')) return;
  const th = t.closest('[data-zeyn-grid] thead th[data-field]');
  if (th) {
    e.preventDefault();
    const grid = th.closest('[data-zeyn-grid]');
    if (!grids.get(grid)) return;
    const button = th.querySelector('.z-col-menu');
    return openMenu(G?.columnMenu(grid, th), { x: e.clientX, y: e.clientY, opener: button, label: button?.getAttribute('aria-label') });
  }
  const owner = t.closest('[data-zeyn-rows] > tr[data-zeyn-menu], [data-zeyn-screen][data-zeyn-menu]');
  if (owner && openContextMenu(owner, { x: e.clientX, y: e.clientY }, t)) return e.preventDefault();
  if (document.documentElement.dataset.zeynNativeMenu === 'off') e.preventDefault();
});

/**
 * Opens the context menu of a row or a screen. Effect items run in the browser; action items
 * ask the server to run them (`/_zeyn/grid/run` with the row key, `/_zeyn/menu` for screens).
 * Returns false when the owner has no items.
 */
function openContextMenu(owner, at, opener) {
  let items;
  try {
    items = JSON.parse(owner.dataset.zeynMenu || '[]');
  } catch {
    return false;
  }
  if (!items.length) return false;
  const row = owner.matches('tr');
  if (row) G?.activate(owner, false);
  const focusBack = row ? owner : opener?.closest(FOCUSABLE) ?? owner;
  const convert = (list) =>
    list.map((i) =>
      i.separator
        ? { separator: true }
        : {
            label: i.label,
            icon: i.icon,
            hint: i.hint,
            disabled: i.disabled,
            items: i.items ? convert(i.items) : undefined,
            run: i.effect
              ? () => runEffect(i.effect, focusBack)
              : i.run != null
                ? () => (row ? G?.runRowItem(owner, `m${i.run}`) : runScreenItem(owner, i.run))
                : undefined,
          },
    );
  openMenu(convert(items), { ...at, opener: focusBack, label: row ? owner.querySelector('td:not(.z-row-select)')?.textContent.trim() : owner.querySelector('h1, h2')?.textContent });
  return true;
}

/** Runs an action item of a screen's context menu with `POST /_zeyn/menu`. */
async function runScreenItem(screen, item) {
  const r = await post(BASE + '/_zeyn/menu', { screen: screen.dataset.zeynScreen, params: paramsOf(screen), item });
  if (!r.ok) return;
  await runEffects(r.data?.effects, screen);
  toast(screen.querySelector('[data-zeyn-grid]') ? grids.get(screen.querySelector('[data-zeyn-grid]'))?.texts.done || 'Done.' : 'Done.');
  await Promise.all([...screen.querySelectorAll('[data-zeyn-grid]')].map((g) => G?.loadGrid(g, 'reload')));
}

document.addEventListener('submit', (e) => {
  const form = e.target;
  if (!(form instanceof HTMLFormElement) || !form.matches('[data-zeyn-submit]')) return;
  e.preventDefault();
  submitForm(form, e.submitter);
});

document.addEventListener('input', (e) => {
  const t = e.target;
  if (!(t instanceof Element)) return;
  if (t.closest('.z-filter-cell')) return G?.onFilter(t);
  if (t.matches('[data-zeyn-grid-search]')) return G?.onSearch(t);
  if (t.getAttribute('aria-invalid') === 'true') {
    t.removeAttribute('aria-invalid');
    const box = t.id && document.getElementById(`${t.id}-error`);
    if (box) box.textContent = '';
  }
});

document.addEventListener('change', (e) => {
  if (!(e.target instanceof HTMLSelectElement)) return;
  if (e.target.matches('[data-zeyn-locale]')) changeLocale(e.target);
  else if (e.target.matches('[data-zeyn-timezone]')) changeTimeZone(e.target);
  else if (e.target.matches('[data-zeyn-theme-picker]')) changeTheme(e.target);
  else if (e.target.matches('[data-zeyn-density]')) changeDensity(e.target);
});

syncTimeZone();
dropOtherUsers();
relativeTimes(document);
setInterval(() => relativeTimes(document), 60_000);
initGrids(document);
syncAgentTools();

document.addEventListener('zeyn:leave', () => {
  for (const d of dialogs.slice().reverse()) finishDialog(d.dialog);
});

if (document.querySelector(ASSISTANT[1])) loadFile('assistant.js');

document.addEventListener('zeyn:content', (e) => {
  loadModules(e.detail ?? document);
  initGrids(e.detail);
  if (e.detail instanceof Element) relativeTimes(e.detail);
  syncAgentTools();
  dropOtherUsers();
});

/**
 * Filters a grid from another script (the assistant): `detail` is `{ filters, node? }` with
 * filters as the grid query sends them (`{ field, contains }`, `{ field, from, to }`, ...). The
 * grid of the main screen with that node path (the first one without it) shows the filters in
 * its header and reloads through `POST /_zeyn/grid`, where the server checks them as always.
 */
document.addEventListener('zeyn:filter', (e) => {
  const { filters, node } = e.detail ?? {};
  const all = [...document.querySelectorAll('.z-main [data-zeyn-grid]')];
  const grid = node == null ? all[0] : all.find((g) => nodeOf(g) === String(node));
  if (!grid || !Array.isArray(filters)) return;
  G?.initGrid(grid);
  const s = grids.get(grid);
  if (!s) return;
  e.preventDefault();
  s.filters = {};
  for (const f of filters) {
    const { field, ...rest } = f ?? {};
    if (typeof field === 'string') s.filters[field] = rest;
  }
  for (const cell of grid.querySelectorAll('.z-filter-cell[data-field]')) G?.fillFilter(cell, s.filters[cell.dataset.field], s.texts);
  s.allMatching = false;
  G?.loadGrid(grid, 'reset');
});

/**
 * `zeyn:reload` on an element reloads the grids inside it in place (`live.js` and `topics.js`
 * send it): the current page, or the first one with `detail.reset` (after a filter changed).
 */
document.addEventListener('zeyn:reload', (e) => {
  const root = e.target instanceof Element ? e.target : document;
  const how = e.detail?.reset ? 'reset' : 'reload';
  root.querySelectorAll('[data-zeyn-grid]').forEach((g) => G?.loadGrid(g, how));
});

/**
 * Lets other scripts (the command palette, menu bars) run effects and screen menu items through
 * the runtime: `detail` is `{effects, source}`, `{screen, item}` with a screen root element, or
 * `{error}` for an error toast.
 */
document.addEventListener('zeyn:run', (e) => {
  const { effects, source, screen, item, error } = e.detail ?? {};
  if (error) toast(String(error), 'error');
  else if (screen instanceof Element && item != null) runScreenItem(screen, String(item));
  else runEffects(Array.isArray(effects) ? effects : [], source);
});
