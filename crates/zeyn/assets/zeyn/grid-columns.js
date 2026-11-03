/** The grid's own sort order (`DataGrid::sort_by`), used when the user has not chosen one. */
function defaultSort(grid) {
  try {
    return JSON.parse(grid.dataset.zeynSortDefault || '[]');
  } catch {
    return [];
  }
}

/**
 * Cycles the sort of a column: none, ascending, descending. A plain click sorts by this column
 * only; Shift+click adds it to the sort keys (or changes or removes it there), up to four keys.
 * "None" means the grid's own order, so a column of that order only switches direction.
 */
function toggleSort(button, additive) {
  const th = button.closest('th');
  const grid = th.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  if (!s) return;
  const field = th.dataset.field;
  const current = s.sort.find((k) => k.field === field);
  const defaults = defaultSort(grid);
  let next = !current ? { field, desc: false } : !current.desc ? { field, desc: true } : null;
  if (!next && !additive && defaults[0]?.field === field) next = { field, desc: !current.desc };
  if (additive) {
    const rest = s.sort.filter((k) => k.field !== field);
    s.sort = next ? (current ? s.sort.map((k) => (k.field === field ? next : k)) : [...rest, next].slice(-4)) : rest;
  } else s.sort = next ? [next] : defaults;
  showSort(grid, s);
  saveView(grid);
  loadGrid(grid, 'reset');
}

/**
 * The view to start from: the server rendered one (`data-zeyn-view`), or, when views are not
 * saved on the server, the one kept in `localStorage` (then `local` is true and the grid reloads
 * in it).
 */
function savedView(grid) {
  const raw = grid.dataset.zeynView;
  if (raw) {
    try {
      return { view: JSON.parse(raw), local: false };
    } catch {
      return { view: null, local: false };
    }
  }
  if (grid.dataset.zeynViews === 'server') return { view: null, local: false };
  try {
    const stored = localStorage.getItem(viewKey(grid));
    const view = stored ? JSON.parse(stored) : null;
    return { view, local: !!view };
  } catch {
    return { view: null, local: false };
  }
}

/** `localStorage` key of a grid's view: screen and node. */
function viewKey(grid) {
  const a = address(grid);
  return `zeyn.view:${a.screen}#${a.node}`;
}

/** Every column of the grid, shown or not: `[{field, label, hidden, pin, width}]`. */
function columnsOf(grid) {
  try {
    return JSON.parse(grid.dataset.zeynColumns || '[]');
  } catch {
    return [];
  }
}

/** The shown columns as the header has them now: `[{field, width, pin}]`. */
function shownColumns(grid) {
  return [...(grid.querySelector('thead tr')?.cells ?? [])]
    .filter((th) => th.dataset.field)
    .map((th) => {
      const c = { field: th.dataset.field };
      if (th.dataset.userWidth) c.width = Number(th.dataset.userWidth);
      if (th.dataset.pin) c.pin = th.dataset.pin;
      return c;
    });
}

/**
 * Saves the view after a short pause: on the server when the grid says so
 * (`data-zeyn-views="server"`), otherwise in `localStorage`. `reset` deletes it.
 */
function saveView(grid, reset = false) {
  const s = grids.get(grid);
  if (!s) return;
  clearTimeout(s.viewTimer);
  s.viewTimer = setTimeout(() => {
    const view = reset ? null : { columns: s.columns ?? [], sort: s.sort, filters: filterList(s) };
    if (grid.dataset.zeynViews === 'server') {
      post(BASE + '/_zeyn/grid/view', { ...address(grid), view }, { quiet: true });
      return;
    }
    try {
      if (view) localStorage.setItem(viewKey(grid), JSON.stringify(view));
      else localStorage.removeItem(viewKey(grid));
    } catch {
      return;
    }
  }, VIEW_SAVE_DELAY_MS);
}

/** Changes the shown columns and reloads the grid so the server renders them. */
function setColumns(grid, columns) {
  const s = grids.get(grid);
  if (!s || !columns.length) return;
  s.columns = columns;
  saveView(grid);
  loadGrid(grid, 'layout');
}

/** Puts back the grid's own layout, sort and filters and forgets the saved view. */
function resetView(grid) {
  const s = grids.get(grid);
  if (!s) return;
  s.columns = null;
  s.sort = defaultSort(grid);
  s.filters = {};
  saveView(grid, true);
  loadGrid(grid, 'layout');
}

/** Replaces the header with the server rendered one and sets it up again. */
function applyHead(grid, s, html) {
  const thead = grid.querySelector('thead');
  if (!thead) return;
  const focusField = document.activeElement?.closest?.('thead th')?.dataset.field;
  thead.innerHTML = html;
  const table = grid.querySelector('table');
  table.classList.remove('z-fixed');
  table.style.width = '';
  setupHead(grid, s);
  if (s.paging === 'virtual' && s.filler) {
    const n = thead.rows[0]?.cells.length ?? 1;
    s.filler.colSpan = n;
    s.placeholder.innerHTML = '<td></td>'.repeat(n);
    s.holders.clear();
    s.key = '';
    measureVirtual(s);
  }
  initColumns(grid, s);
  if (focusField) thead.querySelector(`th[data-field="${CSS.escape(focusField)}"] .z-col-menu`)?.focus();
}

/**
 * Sets up the header row: sort buttons, column menu buttons, resize handles, the filter row
 * filled from the current filters, sort badges and pinned column offsets.
 */
function setupHead(grid, s) {
  const head = grid.querySelector('thead tr');
  if (!head) return;
  const t = s.texts;
  const widths = new Map([...head.cells].map((th) => [th, Math.round(th.getBoundingClientRect().width)]));
  for (const th of head.cells) {
    if (!th.dataset.field) continue;
    const label = th.textContent.trim();
    const sort = make('button', 'z-sort', { type: 'button' });
    sort.append(...th.childNodes);
    const menu = make('button', 'z-col-menu', {
      type: 'button',
      'aria-haspopup': 'menu',
      'aria-expanded': 'false',
      'aria-label': fill(t.menu_options || 'Column options: {label}', { label }),
    });
    const resize = make('span', 'z-resize', {
      role: 'separator',
      tabindex: '0',
      'aria-orientation': 'vertical',
      'aria-label': fill(t.menu_resize || 'Resize {label}', { label }),
      'aria-valuemin': String(MIN_COLUMN),
      'aria-valuemax': String(MAX_COLUMN),
      'aria-valuenow': String(Math.min(MAX_COLUMN, Math.max(MIN_COLUMN, widths.get(th) || MIN_COLUMN))),
    });
    const inner = make('div', 'z-th');
    inner.append(sort, menu);
    th.append(inner, resize);
    if (th.style.width) th.dataset.userWidth = String(parseInt(th.style.width, 10));
  }
  if (grid.querySelector(':scope > table > thead th[data-filter]')) {
    const row = make('tr', 'z-filter-row');
    for (const th of head.cells) {
      const cell = make('td');
      if (th.dataset.pin) cell.dataset.pin = th.dataset.pin;
      if (th.dataset.filter) {
        filterControls(cell, th, t);
        fillFilter(cell, s.filters[th.dataset.field], t);
      }
      row.append(cell);
    }
    head.after(row);
    document.dispatchEvent(new CustomEvent('zeyn:content', { detail: row }));
  }
  showSort(grid, s);
  requestAnimationFrame(() => pinOffsets(grid));
}

/** Puts a filter's values back into its controls after the header was rebuilt. */
function fillFilter(cell, f, t) {
  if (!f) return;
  const get = (bound) => cell.querySelector(`[data-bound="${bound}"]`);
  switch (cell.dataset.kind) {
    case 'text': {
      const op = ['contains', 'starts_with', 'equals'].find((k) => f[k]);
      if (op) {
        get('op').value = op;
        get('text').value = f[op];
      }
      break;
    }
    case 'choice':
      for (const box of cell.querySelectorAll('[data-bound="choice"]')) box.checked = (f.any_of ?? []).includes(box.value);
      choiceLabel(cell, t);
      break;
    case 'bool':
      if (f.is != null) get('is').value = String(f.is);
      break;
    default:
      for (const b of ['from', 'to']) if (f[b] != null && get(b)) get(b).value = f[b];
  }
}

/**
 * Pinned columns stay at their edge with `position: sticky`. The counts go on the grid root
 * (`data-pin-start`, `data-pin-end`) and the offsets into `--z-ps1..5` and `--z-pe1..3`, so the CSS
 * applies to every row, including virtual rows, without touching them.
 */
function pinOffsets(grid) {
  const cells = [...(grid.querySelector('thead tr')?.cells ?? [])];
  let start = 0;
  while (start < cells.length && cells[start].dataset.pin === 'start') start++;
  let end = 0;
  while (end < cells.length - start && cells[cells.length - 1 - end].dataset.pin === 'end') end++;
  start = Math.min(start, 5);
  end = Math.min(end, 3);
  let at = 0;
  for (let i = 0; i < start; i++) {
    grid.style.setProperty(`--z-ps${i + 1}`, `${at}px`);
    at += cells[i].offsetWidth;
  }
  at = 0;
  for (let i = 0; i < end; i++) {
    grid.style.setProperty(`--z-pe${i + 1}`, `${at}px`);
    at += cells[cells.length - 1 - i].offsetWidth;
  }
  if (start) grid.dataset.pinStart = start;
  else delete grid.dataset.pinStart;
  if (end) grid.dataset.pinEnd = end;
  else delete grid.dataset.pinEnd;
}

/**
 * Sets a column width. The first time, every column is frozen at its current width and the
 * table switches to a fixed layout so one column can change without moving the others.
 */
function setWidth(th, px) {
  const table = th.closest('table');
  const head = th.parentElement;
  const v = grids.get(th.closest('[data-zeyn-grid]'))?.colv;
  if (v) {
    const i = cellsOf(head).indexOf(th);
    const w = Math.round(Math.max(MIN_COLUMN, Math.min(MAX_COLUMN, px)));
    v.widths[i] = w;
    for (let k = 0; k < v.n; k++) v.x[k + 1] = v.x[k] + v.widths[k];
    table.querySelector(':scope > colgroup')?.children[i]?.style.setProperty('width', `${w}px`);
    table.style.width = `${v.x[v.n]}px`;
    th.style.width = `${w}px`;
    th.dataset.userWidth = String(w);
    th.querySelector('.z-resize')?.setAttribute('aria-valuenow', String(w));
    pinOffsets(th.closest('[data-zeyn-grid]'));
    return;
  }
  if (!table.classList.contains('z-fixed')) {
    for (const c of head.cells) c.style.width = `${c.offsetWidth}px`;
    table.classList.add('z-fixed');
  }
  const w = Math.round(Math.max(MIN_COLUMN, Math.min(MAX_COLUMN, px)));
  th.style.width = `${w}px`;
  th.dataset.userWidth = String(w);
  th.querySelector('.z-resize')?.setAttribute('aria-valuenow', String(w));
  table.style.width = `${[...head.cells].reduce((sum, c) => sum + (parseInt(c.style.width, 10) || c.offsetWidth), 0)}px`;
  pinOffsets(th.closest('[data-zeyn-grid]'));
}

/** Stores the shown columns with their widths after a resize. */
function widthChanged(grid) {
  const s = grids.get(grid);
  if (!s) return;
  s.columns = shownColumns(grid);
  saveView(grid);
}

/** Drags a resize handle; in right to left pages, dragging left makes the column wider. */
function startResize(e, handle) {
  e.preventDefault();
  const th = handle.closest('th');
  const grid = th.closest('[data-zeyn-grid]');
  const rtl = getComputedStyle(th).direction === 'rtl';
  const x0 = e.clientX;
  const w0 = th.offsetWidth;
  handle.setPointerCapture(e.pointerId);
  handle.classList.add('z-active');
  const move = (ev) => setWidth(th, w0 + (rtl ? x0 - ev.clientX : ev.clientX - x0));
  const up = () => {
    handle.removeEventListener('pointermove', move);
    handle.removeEventListener('pointerup', up);
    handle.removeEventListener('pointercancel', up);
    handle.classList.remove('z-active');
    widthChanged(grid);
  };
  handle.addEventListener('pointermove', move);
  handle.addEventListener('pointerup', up);
  handle.addEventListener('pointercancel', up);
}

/** Arrow keys resize a focused handle by 16 px (64 px with Shift). Returns true when handled. */
function resizeKey(e, handle) {
  const th = handle.closest('th');
  const rtl = getComputedStyle(th).direction === 'rtl';
  const step = e.shiftKey ? 64 : 16;
  const delta = { ArrowRight: step, ArrowLeft: -step }[e.key];
  if (delta === undefined) return false;
  e.preventDefault();
  setWidth(th, th.offsetWidth + (rtl ? -delta : delta));
  widthChanged(th.closest('[data-zeyn-grid]'));
  return true;
}

/** Items of the menu of one column: sort, pin, move, hide, choose columns, reset. */
function columnMenu(grid, th) {
  const s = grids.get(grid);
  const t = s.texts;
  const field = th.dataset.field;
  const shown = shownColumns(grid);
  const i = shown.findIndex((c) => c.field === field);
  const key = s.sort.find((k) => k.field === field);
  const sortBy = (desc) => () => {
    s.sort = desc == null ? s.sort.filter((k) => k.field !== field) : [{ field, desc }];
    if (!s.sort.length) s.sort = defaultSort(grid);
    showSort(grid, s);
    saveView(grid);
    loadGrid(grid, 'reset');
  };
  const pin = (side) => () => {
    const next = shown.map((c) => (c.field === field ? { ...c, pin: side ?? undefined } : c));
    setColumns(grid, next.map(({ pin: p, ...c }) => (p ? { ...c, pin: p } : c)));
  };
  const move = (by) => () => {
    const next = [...shown];
    const [c] = next.splice(i, 1);
    next.splice(i + by, 0, c);
    setColumns(grid, next);
  };
  const all = columnsOf(grid);
  return [
    { label: t.menu_sort_asc || 'Sort ascending', checked: key ? !key.desc : false, radio: true, run: sortBy(false) },
    { label: t.menu_sort_desc || 'Sort descending', checked: key ? key.desc : false, radio: true, run: sortBy(true) },
    { label: t.menu_sort_clear || 'Clear sort', disabled: !key, run: sortBy(null) },
    { separator: true },
    { label: t.menu_pin_start || 'Pin to start', checked: th.dataset.pin === 'start', radio: true, run: pin('start') },
    { label: t.menu_pin_end || 'Pin to end', checked: th.dataset.pin === 'end', radio: true, run: pin('end') },
    { label: t.menu_unpin || 'Unpin', disabled: !th.dataset.pin, run: pin(null) },
    { separator: true },
    { label: t.menu_move_prev || 'Move left', disabled: i <= 0, run: move(-1) },
    { label: t.menu_move_next || 'Move right', disabled: i < 0 || i >= shown.length - 1, run: move(1) },
    { label: t.menu_hide || 'Hide column', disabled: shown.length <= 1, run: () => setColumns(grid, shown.filter((c) => c.field !== field)) },
    {
      label: t.menu_filter || 'Filter',
      disabled: !th.dataset.filter,
      run: () => grid.querySelector(`:scope > table > thead .z-filter-cell[data-field="${CSS.escape(field)}"] .z-filter`)?.focus(),
    },
    {
      label: t.menu_columns || 'Columns',
      items: all.map((c) => {
        const on = shown.some((x) => x.field === c.field);
        return {
          label: c.label,
          checked: on,
          disabled: on && shown.length <= 1,
          run: () => setColumns(grid, on ? shown.filter((x) => x.field !== c.field) : [...shown, { field: c.field }]),
        };
      }),
    },
    { separator: true },
    { label: t.menu_reset || 'Reset view', run: () => resetView(grid) },
  ];
}

/** Opens the column menu under its button. */
function openColumnMenu(button) {
  const th = button.closest('th');
  const grid = th.closest('[data-zeyn-grid]');
  if (!grids.get(grid)) return;
  openMenu(columnMenu(grid, th), { anchor: button, label: button.getAttribute('aria-label') });
}

/** The menu that is open now, if any, with the element focus returns to. */
let openMenuState = null;

/**
 * Opens a menu (`role="menu"`) as an auto popover in the top layer, so no scroll container clips
 * it. Items: `{label, run, disabled, checked, radio, hint, items, separator}`; `items` makes a
 * submenu. Keyboard: arrows move (Right opens a submenu, Left closes it; mirrored in right to left
 * pages), Home and End jump, Enter and Space run, Escape closes, Tab closes. Focus returns to the
 * opener.
 * @param {object[]} items
 * @param {{anchor?: Element, x?: number, y?: number, label?: string, opener?: Element}} at
 */
function openMenu(items, at) {
  closeMenus();
  const opener = at.opener ?? at.anchor ?? document.activeElement;
  const host = dialogs.at(-1)?.dialog ?? document.body;
  const menu = buildMenu(items, at.label);
  host.append(menu);
  openMenuState = { menu, opener };
  at.anchor?.setAttribute('aria-expanded', 'true');
  menu.addEventListener('toggle', (e) => {
    if (e.newState !== 'closed') return;
    at.anchor?.setAttribute('aria-expanded', 'false');
    menu.remove();
    if (openMenuState?.menu === menu) openMenuState = null;
    if (!document.activeElement || document.activeElement === document.body || menu.contains(document.activeElement)) {
      if (opener?.isConnected) opener.focus();
    }
  });
  menu.showPopover();
  if (at.anchor) place(menu, at.anchor);
  else placeAt(menu, at.x ?? 0, at.y ?? 0);
  firstItem(menu)?.focus();
  return menu;
}

/** Closes the open menu, if any, without running anything. */
function closeMenus() {
  if (openMenuState?.menu.isConnected) openMenuState.menu.hidePopover();
  openMenuState = null;
}

/** Builds the menu element and its submenus. */
function buildMenu(items, label) {
  const menu = make('div', 'z-popover z-menu', { role: 'menu', popover: 'auto', tabindex: '-1' });
  if (label) menu.setAttribute('aria-label', label);
  for (const item of items) {
    if (item.separator) {
      menu.append(make('div', 'z-menu-sep', { role: 'separator' }));
      continue;
    }
    const role = item.checked == null ? 'menuitem' : item.radio ? 'menuitemradio' : 'menuitemcheckbox';
    const button = make('button', 'z-menu-item', { type: 'button', role, tabindex: '-1' });
    if (item.checked != null) button.setAttribute('aria-checked', String(!!item.checked));
    if (item.disabled) button.setAttribute('aria-disabled', 'true');
    if (item.icon) button.append(make('span', `z-icon z-icon-${item.icon}`, { 'aria-hidden': 'true' }));
    button.append(make('span', 'z-menu-label'));
    button.lastChild.textContent = item.label;
    if (item.hint) {
      const hint = make('span', 'z-menu-hint', { 'aria-hidden': 'true' });
      hint.textContent = item.hint;
      button.append(hint);
      button.setAttribute('aria-keyshortcuts', item.hint.replace('Ctrl', 'Control'));
    }
    if (item.items) {
      button.setAttribute('aria-haspopup', 'menu');
      button.setAttribute('aria-expanded', 'false');
      const sub = buildMenu(item.items, item.label);
      sub.popover = 'auto';
      button._submenu = sub;
      sub.addEventListener('toggle', (e) => button.setAttribute('aria-expanded', String(e.newState === 'open')));
    }
    button._item = item;
    menu.append(button);
    if (button._submenu) menu.append(button._submenu);
  }
  return menu;
}

/** Enabled items of one menu level. */
function menuItems(menu) {
  return [...menu.children].filter((c) => c.matches('.z-menu-item'));
}

function firstItem(menu) {
  return menuItems(menu).find((b) => b.getAttribute('aria-disabled') !== 'true') ?? menuItems(menu)[0];
}

/** Places a popover at a point (a right click), kept inside the viewport. */
function placeAt(panel, x, y) {
  const w = panel.offsetWidth;
  const h = panel.offsetHeight;
  panel.style.left = `${Math.max(4, Math.min(x, innerWidth - w - 4))}px`;
  panel.style.top = `${Math.max(4, Math.min(y, innerHeight - h - 4))}px`;
}

/** Opens the submenu of an item beside it and focuses its first item. */
function openSubmenu(button) {
  const sub = button._submenu;
  if (!sub) return;
  sub.showPopover();
  const r = button.getBoundingClientRect();
  const rtl = getComputedStyle(button).direction === 'rtl';
  const x = rtl ? r.left - sub.offsetWidth : r.right;
  placeAt(sub, x, r.top - 4);
  firstItem(sub)?.focus();
}

/** Runs a menu item: opens its submenu or runs it and closes every menu. */
function runMenuItem(button) {
  if (button.getAttribute('aria-disabled') === 'true') return;
  if (button._submenu) return openSubmenu(button);
  const item = button._item;
  const opener = openMenuState?.opener;
  closeMenus();
  if (opener?.isConnected) opener.focus();
  item?.run?.();
}

/** Keyboard inside a menu. Returns true when handled. */
function menuKey(e, button) {
  const menu = button.parentElement;
  const list = menuItems(menu);
  const i = list.indexOf(button);
  const rtl = getComputedStyle(menu).direction === 'rtl';
  const into = rtl ? 'ArrowLeft' : 'ArrowRight';
  const out = rtl ? 'ArrowRight' : 'ArrowLeft';
  let target = null;
  if (e.key === 'ArrowDown') target = list[(i + 1) % list.length];
  else if (e.key === 'ArrowUp') target = list[(i - 1 + list.length) % list.length];
  else if (e.key === 'Home') target = list[0];
  else if (e.key === 'End') target = list.at(-1);
  else if (e.key === into && button._submenu) {
    openSubmenu(button);
  } else if (e.key === out && menu.parentElement?.closest('.z-menu')) {
    const parent = menu.previousElementSibling;
    menu.hidePopover();
    parent?.focus();
  } else if (e.key === 'Enter' || e.key === ' ') {
    runMenuItem(button);
  } else if (e.key === 'Escape') {
    if (menu.parentElement?.closest('.z-menu')) {
      menu.hidePopover();
      menu.previousElementSibling?.focus();
    } else closeMenus();
  } else if (e.key === 'Tab') {
    closeMenus();
    return false;
  } else return false;
  e.preventDefault();
  e.stopPropagation();
  target?.focus();
  return true;
}

/** Sets `aria-sort` on every header and the sort order badge when several keys are active. */
function showSort(grid, s) {
  for (const th of grid.querySelectorAll(':scope > table > thead th[data-field]')) {
    const i = s.sort.findIndex((k) => k.field === th.dataset.field);
    th.setAttribute('aria-sort', i < 0 ? 'none' : s.sort[i].desc ? 'descending' : 'ascending');
    th.querySelector('.z-sort-order')?.remove();
    if (i < 0 || s.sort.length < 2) continue;
    const badge = document.createElement('span');
    badge.className = 'z-sort-order';
    const label = document.createElement('span');
    label.className = 'z-sr-only';
    label.textContent = fill(s.texts.sorted || 'Sort order {n}', { n: i + 1 });
    const number = document.createElement('span');
    number.setAttribute('aria-hidden', 'true');
    number.textContent = String(i + 1);
    badge.append(label, number);
    th.querySelector('.z-sort')?.append(badge);
  }
}

/** Reads the localized grid texts from `data-zeyn-texts`. */
function textsOf(grid) {
  try {
    return JSON.parse(grid.dataset.zeynTexts || '{}');
  } catch {
    return {};
  }
}

