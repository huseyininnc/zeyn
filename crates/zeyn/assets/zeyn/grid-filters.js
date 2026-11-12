/**
 * Builds the filter controls of one column in its filter row cell, by `th[data-filter]`:
 * `text` (match kind and text), `number`, `money`, `date`, `datetime`, `time` (from and to),
 * `choice` (a button opening a checkbox list) and `bool` (all, yes, no).
 */
/**
 * Wraps a native select in the Zeyn select control; components.js turns it into a combobox and
 * keeps the native select as the value holder. `none` is shown for the empty value.
 */
function zselect(select, label, none = '') {
  select.id ||= `z-select-${++uid}`;
  const wrap = make('div', 'z-select z-filter-select');
  wrap.dataset.textNone = none;
  const name = make('span', 'z-sr-only', { id: `${select.id}-label` });
  name.textContent = label;
  wrap.append(name, select);
  return wrap;
}

function filterControls(cell, th, t) {
  const kind = th.dataset.filter;
  const label = th.textContent.trim();
  const name = fill(t.filter || 'Filter {label}', { label });
  cell.className = 'z-filter-cell';
  cell.dataset.field = th.dataset.field;
  cell.dataset.kind = kind;
  const input = (type, bound, text) => {
    const i = make('input', 'z-filter', { type, autocomplete: 'off', 'aria-label': `${name}: ${text}` });
    i.id = `z-filter-${++uid}`;
    i.dataset.bound = bound;
    if (type === 'number') {
      i.step = 'any';
      i.inputMode = 'decimal';
    }
    if (type !== 'date' && type !== 'time') i.placeholder = text;
    return i;
  };
  if (kind === 'text') {
    const wrap = make('div', 'z-filter-pair');
    const op = make('select', 'z-filter z-filter-op');
    op.dataset.bound = 'op';
    for (const k of ['contains', 'starts_with', 'equals']) op.append(new Option(t[k] || k, k));
    const text = make('input', 'z-filter', { type: 'search', autocomplete: 'off', 'aria-label': name });
    text.id = `z-filter-${++uid}`;
    text.dataset.bound = 'text';
    wrap.append(zselect(op, `${name}: ${t.match || 'Match'}`), text);
    cell.append(wrap);
  } else if (kind === 'choice') {
    let options = [];
    try {
      options = JSON.parse(th.dataset.options || '[]');
    } catch {
      options = [];
    }
    const id = `z-choice-${++uid}`;
    const button = make('button', 'z-filter z-filter-choice', {
      type: 'button',
      'aria-haspopup': 'true',
      'aria-expanded': 'false',
      'aria-controls': id,
      'aria-label': name,
      popovertarget: id,
    });
    button.dataset.zeynChoice = '';
    button.dataset.name = name;
    button.textContent = t.all || 'All';
    button.setAttribute('aria-label', `${name}: ${button.textContent}`);
    const panel = make('div', 'z-popover z-choice-panel', { id, role: 'group', 'aria-label': name, popover: 'auto' });
    for (const o of options) {
      const row = make('label', 'z-check');
      const box = make('input', '', { type: 'checkbox' });
      box.value = o.value;
      box.dataset.bound = 'choice';
      row.append(box, document.createTextNode(o.label));
      panel.append(row);
    }
    panel.addEventListener('toggle', (e) => {
      button.setAttribute('aria-expanded', String(e.newState === 'open'));
      if (e.newState === 'open') {
        place(panel, button);
        panel.querySelector('input')?.focus();
      } else if (panel.contains(document.activeElement) || document.activeElement === document.body) button.focus();
    });
    cell.append(button, panel);
  } else if (kind === 'bool') {
    const select = make('select', 'z-filter');
    select.dataset.bound = 'is';
    select.append(new Option(t.all || 'All', ''), new Option(t.yes || 'Yes', 'true'), new Option(t.no || 'No', 'false'));
    cell.append(zselect(select, name, t.all || 'All'));
  } else {
    const type = { number: 'number', money: 'number', date: 'text', datetime: 'text', time: 'text' }[kind] ?? 'search';
    const wrap = make('div', 'z-filter-pair z-filter-range');
    wrap.append(input(type, 'from', t.from || 'From'), input(type, 'to', t.to || 'To'));
    if (type === 'text') wrap.querySelectorAll('input').forEach((i) => (i.dataset.zeynFilterDate = kind === 'time' ? 'time' : 'date'));
    cell.append(wrap);
  }
  loadModules(cell);
}

/** Reads the controls of a filter cell into a filter object without `field`; `null` when empty. */
function readFilter(cell) {
  const get = (bound) => cell.querySelector(`[data-bound="${bound}"]`);
  const f = {};
  switch (cell.dataset.kind) {
    case 'text': {
      const text = get('text').value.trim();
      if (text) f[get('op').value] = text;
      break;
    }
    case 'choice': {
      const picked = [...cell.querySelectorAll('[data-bound="choice"]:checked')].map((b) => b.value);
      if (picked.length) f.any_of = picked;
      break;
    }
    case 'bool':
      if (get('is').value) f.is = get('is').value === 'true';
      break;
    default:
      for (const b of ['from', 'to']) {
        const el = get(b);
        const v = el?.dataset.zeynFilterDate ? el.dataset.iso : el?.value;
        if (v) f[b] = v;
      }
  }
  return Object.keys(f).length ? f : null;
}

/** Updates the label of a choice filter button: all, the one picked, or how many. */
function choiceLabel(cell, t) {
  const button = cell.querySelector('[data-zeyn-choice]');
  if (!button) return;
  const picked = [...cell.querySelectorAll('[data-bound="choice"]:checked')];
  button.textContent =
    picked.length === 0 ? t.all || 'All' : picked.length === 1 ? picked[0].parentElement.textContent : fill(t.selected || '{n} selected', { n: picked.length });
  button.classList.toggle('z-active', picked.length > 0);
  button.setAttribute('aria-label', `${button.dataset.name}: ${button.textContent}`);
}

/** Active filters as the query sends them. */
function filterList(s) {
  return Object.entries(s.filters)
    .filter(([, f]) => f)
    .map(([field, f]) => ({ field, ...f }));
}

/**
 * Debounces a filter control and reloads the grid from the start. The grid shows busy (rows
 * fade) in the frame of the key press, not only when the request leaves.
 */
function onFilter(control) {
  const cell = control.closest('.z-filter-cell');
  const grid = cell?.closest('[data-zeyn-grid]');
  const s = grid && grids.get(grid);
  if (!s) return;
  if (cell.dataset.kind === 'choice') choiceLabel(cell, s.texts);
  grid.setAttribute('aria-busy', 'true');
  clearTimeout(s.timer);
  s.timer = setTimeout(() => {
    s.filters[cell.dataset.field] = readFilter(cell);
    s.allMatching = false;
    showFilters(grid, s);
    saveView(grid);
    loadGrid(grid, 'reset');
  }, FILTER_DELAY_MS);
}


/**
 * The grid of an element in the tools above it (`.z-grid-tools`, before the selection bar and
 * the grid), or `null`.
 */
function gridOfTools(el) {
  let grid = el.closest('.z-grid-tools')?.nextElementSibling;
  if (grid?.matches('[data-zeyn-bulk]')) grid = grid.nextElementSibling;
  return grid?.matches('[data-zeyn-grid]') ? grid : null;
}

/**
 * Debounces the search box of a grid (`input[data-zeyn-grid-search]`, in the tools before the
 * grid) and reloads the grid from the start with the text as `q`. Each reload cancels
 * the one before it, and the result is announced in the grid's live region.
 */
function onSearch(input) {
  const grid = gridOfTools(input);
  if (!grid) return;
  initGrid(grid);
  const s = grids.get(grid);
  grid.setAttribute('aria-busy', 'true');
  clearTimeout(s.timer);
  s.timer = setTimeout(() => {
    const q = input.value.trim();
    if (q === s.q) return grid.removeAttribute('aria-busy');
    s.q = q;
    s.allMatching = false;
    s.searched = true;
    loadGrid(grid, 'reset');
  }, FILTER_DELAY_MS);
}

/** Empties the controls of a filter cell, before `fillFilter` puts other values in. */
function clearFilter(cell, t) {
  for (const el of cell.querySelectorAll('input, select')) {
    if (el.type === 'checkbox') el.checked = false;
    else if (el.tagName === 'SELECT') el.selectedIndex = 0;
    else el.value = '';
    delete el.dataset.iso;
  }
  choiceLabel(cell, t);
}

/** Puts the grid's filters into its filter row and the count on its "Filter" button. */
function showFilters(grid, s) {
  for (const cell of grid.querySelectorAll(':scope > table > thead .z-filter-cell[data-field]')) {
    clearFilter(cell, s.texts);
    fillFilter(cell, s.filters[cell.dataset.field], s.texts);
  }
  const badge = toolsOf(grid)?.querySelector('[data-zeyn-filter-count]');
  if (!badge) return;
  const n = filterList(s).length;
  badge.hidden = !n;
  badge.textContent = n ? fmt(n) : '';
}

/** Focuses the column picker of a filter builder row. */
function focusPick(line) {
  line?.querySelector(':scope > select')?.focus();
}

/**
 * Opens the filter builder of a grid: a modal dialog with one row per condition, each a column
 * picker and that column's filter inputs (the filter row's own controls). Apply replaces the
 * grid's filters and reloads it; Escape or Cancel leave them as they were, and focus returns to
 * the button.
 */
function openBuilder(button) {
  const grid = gridOfTools(button);
  const s = grid && grids.get(grid);
  if (!s) return;
  const t = s.texts;
  const heads = [...grid.querySelectorAll(':scope > table > thead th[data-filter]')];
  if (!heads.length) return;
  const dialog = make('dialog', 'z-dialog', { 'aria-labelledby': `z-fb-${++uid}` });
  const body = make('div', 'z-window z-filter-builder');
  const title = make('h2', '', { id: `z-fb-${uid}` });
  title.textContent = t.builder_title || 'Filter rows';
  const list = make('div', 'z-fb-list');
  const row = (field, f) => {
    const line = make('div', 'z-fb-row');
    const pick = make('select', 'z-filter', { 'aria-label': t.column || 'Column' });
    for (const th of heads) pick.append(new Option(th.textContent.trim(), th.dataset.field));
    pick.value = field;
    const cell = make('div');
    const fill = () => {
      cell.replaceChildren();
      const th = heads.find((h) => h.dataset.field === pick.value);
      filterControls(cell, th, t);
      fillFilter(cell, f, t);
      f = null;
    };
    pick.addEventListener('change', fill);
    fill();
    const remove = make('button', 'z-button z-fb-remove', { type: 'button', 'aria-label': t.remove_condition || 'Remove condition' });
    remove.append(make('span', 'z-icon z-icon-x', { 'aria-hidden': 'true' }));
    remove.addEventListener('click', () => {
      focusPick(line.nextElementSibling ?? line.previousElementSibling);
      line.remove();
      if (!list.children.length) add.focus();
    });
    line.append(pick, cell, remove);
    list.append(line);
    return line;
  };
  for (const f of filterList(s)) {
    const { field, ...rest } = f;
    if (heads.some((h) => h.dataset.field === field)) row(field, rest);
  }
  const add = make('button', 'z-button', { type: 'button' });
  add.textContent = t.add_condition || 'Add condition';
  add.addEventListener('click', () => focusPick(row(heads[0].dataset.field, null)));
  const clear = make('button', 'z-button', { type: 'button' });
  clear.textContent = t.clear_all || 'Clear all';
  clear.addEventListener('click', () => {
    list.replaceChildren();
    add.focus();
  });
  const cancel = make('button', 'z-button', { type: 'button' });
  cancel.textContent = t.confirm_cancel || 'Cancel';
  cancel.addEventListener('click', () => dialog.close());
  const apply = make('button', 'z-button z-primary', { type: 'button' });
  apply.textContent = t.apply || 'Apply';
  apply.addEventListener('click', () => {
    s.filters = {};
    for (const cell of list.querySelectorAll('.z-filter-cell')) {
      const f = readFilter(cell);
      if (f) s.filters[cell.dataset.field] = f;
    }
    s.allMatching = false;
    showFilters(grid, s);
    saveView(grid);
    dialog.close();
    loadGrid(grid, 'reset');
  });
  const actions = make('div', 'z-fb-actions');
  actions.append(add, clear, make('span', 'z-fb-gap'), cancel, apply);
  body.append(title, list, actions);
  dialog.append(body);
  dialog.addEventListener('change', (e) => {
    const cell = e.target.closest?.('.z-filter-cell');
    if (cell?.dataset.kind === 'choice') choiceLabel(cell, t);
  });
  dialog.addEventListener('close', () => {
    dialog.remove();
    button.focus();
  });
  document.body.append(dialog);
  loadModules(dialog);
  dialog.showModal();
  if (!list.children.length) row(heads[0].dataset.field, null);
  focusPick(list.firstElementChild);
}
