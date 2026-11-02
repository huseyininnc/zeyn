document.addEventListener('click', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t) return;
  let el;
  if ((el = t.closest('input[data-zeyn-select]'))) return pickRow(el, e.shiftKey);
  if ((el = t.closest('input[data-zeyn-select-all]'))) return pickPage(el);
  if ((el = t.closest('[data-zeyn-select-matching]'))) {
    const grid = gridOfBar(el);
    const s = grids.get(grid);
    if (s) s.allMatching = true;
    syncSelection(grid);
    return bulkBar(grid)?.querySelector('[data-zeyn-select-clear]')?.focus();
  }
  if ((el = t.closest('[data-zeyn-select-clear]'))) return clearSelection(gridOfBar(el));
  if ((el = t.closest('[data-zeyn-expand]'))) {
    const tr = el.closest('tr');
    return tr.dataset.zeynPath != null ? toggleBranch(tr) : toggleDetail(tr);
  }
  if ((el = t.closest('[data-zeyn-branch-more]'))) return moreBranch(el);
  if ((el = t.closest('[data-zeyn-bulk-run]'))) return runBulk(el);
  if ((el = t.closest('.z-menu-item'))) return runMenuItem(el);
  if ((el = t.closest('button[data-zeyn-run]'))) return runRowItem(el);
  if ((el = t.closest('button.z-sort'))) return toggleSort(el, e.shiftKey);
  if ((el = t.closest('button.z-col-menu'))) return openColumnMenu(el);
  if ((el = t.closest('[data-zeyn-page]'))) return loadGrid(el.closest('[data-zeyn-grid]'), el.dataset.zeynPage === 'prev' ? 'prev' : 'next');
  if ((el = t.closest('[data-zeyn-more] button'))) return loadGrid(el.closest('[data-zeyn-grid]'), 'more');
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
    if (dataRow(tr) && !el.classList.contains('z-col-gap')) focusCell(tr.closest('[data-zeyn-grid]'), tr, cellsOf(tr).indexOf(el), e.shiftKey);
  }
});

document.addEventListener('focusin', (e) => {
  const t = e.target;
  const tr = t instanceof HTMLTableCellElement ? t.parentElement : t;
  if (tr instanceof HTMLTableRowElement && tr.parentElement?.matches('[data-zeyn-rows]')) activate(tr, false);
});

document.addEventListener('paste', (e) => {
  const td = e.target instanceof HTMLTableCellElement ? e.target : null;
  const grid = td?.parentElement?.parentElement?.matches('[data-zeyn-rows]') ? td.closest('[data-zeyn-grid]') : null;
  const text = e.clipboardData?.getData('text/plain');
  if (!grid || !text || !grids.get(grid)?.anchor) return;
  e.preventDefault();
  pasteCells(grid, text);
});

document.addEventListener('dblclick', (e) => {
  const td = e.target instanceof Element ? e.target.closest('[data-zeyn-rows] > tr > td[data-v]') : null;
  if (td && !td.closest('.z-editing') && startEdit(td.parentElement, td)) return;
  if (e.target instanceof Element && e.target.closest('.z-editing')) return;
  const tr = e.target instanceof Element ? e.target.closest('[data-zeyn-rows] > tr[data-zeyn-effect]') : null;
  const effect = tr && parseEffect(tr);
  if (effect) runEffect(effect, tr);
});

document.addEventListener('keydown', (e) => {
  const t = e.target;
  if (t instanceof Element && t.matches('.z-menu-item') && menuKey(e, t)) return;
  if (t instanceof Element && t.matches('.z-cell-input') && editKey(e, t)) return;
  if (e.key === 'F2' && t instanceof HTMLTableRowElement && t.parentElement?.matches('[data-zeyn-rows]')) {
    if (startEdit(t, null)) return e.preventDefault();
  }
  if (t instanceof HTMLTableCellElement && t.parentElement?.parentElement?.matches('[data-zeyn-rows]')) {
    const g = t.closest('[data-zeyn-grid]');
    if ((e.ctrlKey || e.metaKey) && !e.shiftKey && (e.key ?? '').toLowerCase() === 'c' && copyCells(g)) return e.preventDefault();
    if (cellKey(e, t)) return;
  }
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && (e.key ?? '').toLowerCase() === 'c' && !typing(t) && t instanceof Element) {
    const g = t.closest('[data-zeyn-grid]');
    const sel = window.getSelection();
    if (g && (!sel || sel.isCollapsed) && copyRows(g, t.closest('[data-zeyn-rows] > tr'))) return e.preventDefault();
  }
  if ((e.ctrlKey || e.metaKey) && !e.shiftKey && (e.key ?? '').toLowerCase() === 'z' && !typing(t) && t instanceof Element) {
    const g = t.closest('[data-zeyn-grid]');
    if (g && grids.get(g)?.undo) {
      e.preventDefault();
      return undoEdit(g);
    }
  }
  if (t instanceof Element && t.matches('.z-resize') && resizeKey(e, t)) return;
  if (t instanceof Element && (e.key === 'ContextMenu' || (e.shiftKey && e.key === 'F10')) && !typing(t)) {
    if (t.closest('thead th[data-field]')) {
      e.preventDefault();
      return openColumnMenu(t.closest('th').querySelector('.z-col-menu'));
    }
    const owner = t.closest('[data-zeyn-rows] > tr[data-zeyn-menu], [data-zeyn-screen][data-zeyn-menu]');
    if (owner) {
      const box = (owner.matches('tr') ? owner : t).getBoundingClientRect();
      if (openContextMenu(owner, { x: box.left + 16, y: box.bottom }, t)) return e.preventDefault();
    }
  }
  const vg = t instanceof Element ? t.closest('[data-zeyn-grid][data-zeyn-paging="virtual"]') : null;
  if (vg && (t === vg || t.parentElement?.matches('[data-zeyn-rows]')) && virtualKey(e, vg)) return;
  if (t instanceof HTMLTableRowElement && t.parentElement?.matches('[data-zeyn-rows]') && rowKey(e, t)) return;
  shortcut(e);
});

document.addEventListener('pointerdown', (e) => {
  const t = e.target instanceof Element ? e.target : null;
  if (!t || e.button !== 0) return;
  const handle = t.closest('.z-resize');
  if (handle) return startResize(e, handle);
  const sort = t.closest('[data-zeyn-grid] thead th[data-field] .z-sort');
  if (sort) startColumnDrag(e, sort.closest('th'));
});

/**
 * Moves a column by dragging its header onto another header of the same pinned group. A press
 * that moves less than a few pixels is still a click and sorts. The column menu (Move left,
 * Move right) does the same from the keyboard.
 */
function startColumnDrag(e, th) {
  const grid = th.closest('[data-zeyn-grid]');
  if (!grids.get(grid)) return;
  const x0 = e.clientX;
  const y0 = e.clientY;
  const rtl = getComputedStyle(grid).direction === 'rtl';
  const group = () =>
    [...th.parentElement.cells].filter((h) => h.dataset.field && (h.dataset.pin || '') === (th.dataset.pin || ''));
  let dragging = false;
  let target = null;
  let after = false;
  const mark = (on) => target?.classList.toggle(after ? 'z-col-drop-end' : 'z-col-drop-start', on);
  const move = (ev) => {
    if (!dragging) {
      if (Math.abs(ev.clientX - x0) < 6 && Math.abs(ev.clientY - y0) < 6) return;
      dragging = true;
      th.classList.add('z-col-dragging');
      document.documentElement.classList.add('z-col-moving');
    }
    mark(false);
    target = group().find((h) => {
      const r = h.getBoundingClientRect();
      return ev.clientX >= r.left && ev.clientX <= r.right;
    }) ?? null;
    if (target === th) target = null;
    if (target) {
      const r = target.getBoundingClientRect();
      after = ev.clientX > r.left + r.width / 2 !== rtl;
      mark(true);
    }
  };
  const end = (ev) => {
    document.removeEventListener('pointermove', move);
    document.removeEventListener('pointerup', end);
    document.removeEventListener('pointercancel', end);
    mark(false);
    th.classList.remove('z-col-dragging');
    document.documentElement.classList.remove('z-col-moving');
    if (!dragging) return;
    const swallow = (c) => {
      c.preventDefault();
      c.stopPropagation();
    };
    document.addEventListener('click', swallow, { capture: true, once: true });
    setTimeout(() => document.removeEventListener('click', swallow, true), 0);
    if (!target || ev.type === 'pointercancel') return;
    const shown = shownColumns(grid);
    const [col] = shown.splice(shown.findIndex((c) => c.field === th.dataset.field), 1);
    const at = shown.findIndex((c) => c.field === target.dataset.field) + (after ? 1 : 0);
    shown.splice(at, 0, col);
    setColumns(grid, shown);
  };
  document.addEventListener('pointermove', move);
  document.addEventListener('pointerup', end);
  document.addEventListener('pointercancel', end);
}

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
    return openMenu(columnMenu(grid, th), { x: e.clientX, y: e.clientY, opener: button, label: button?.getAttribute('aria-label') });
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
  if (row) activate(owner, false);
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
                ? () => (row ? runRowItem(owner, `m${i.run}`) : runScreenItem(owner, i.run))
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
  await Promise.all([...screen.querySelectorAll('[data-zeyn-grid]')].map((g) => loadGrid(g, 'reload')));
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
  if (t.closest('.z-filter-cell')) return onFilter(t);
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
});

syncTimeZone();
setInterval(() => relativeTimes(document), 60_000);
initGrids(document);
syncAgentTools();

document.addEventListener('zeyn:leave', () => {
  for (const d of dialogs.slice().reverse()) finishDialog(d.dialog);
});

document.addEventListener('zeyn:content', (e) => {
  initGrids(e.detail);
  syncAgentTools();
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
  initGrid(grid);
  const s = grids.get(grid);
  if (!s) return;
  e.preventDefault();
  s.filters = {};
  for (const f of filters) {
    const { field, ...rest } = f ?? {};
    if (typeof field === 'string') s.filters[field] = rest;
  }
  for (const cell of grid.querySelectorAll('.z-filter-cell[data-field]')) fillFilter(cell, s.filters[cell.dataset.field], s.texts);
  s.allMatching = false;
  loadGrid(grid, 'reset');
});

/** `zeyn:reload` on an element reloads the grids inside it in place (`live.js` sends it). */
document.addEventListener('zeyn:reload', (e) => {
  const root = e.target instanceof Element ? e.target : document;
  root.querySelectorAll('[data-zeyn-grid]').forEach((g) => loadGrid(g, 'reload'));
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
