/**
 * Filter chips of a grid (`DataGrid::filter_chips`): the active filters above the grid, one chip
 * per filtered column and one per condition group, each with a remove button, and "Clear all"
 * from two chips on. The grid runtime puts `{ grid, filters, groups, set }` on the chips box as
 * `zeynFilters` and sends `zeyn:filters` whenever its filters change; `set` replaces them, saves
 * the view and reloads the grid. Pressing a chip opens the filter builder when the grid has one,
 * otherwise it moves focus to the column's filter in the filter row.
 *
 * @module gridchips
 */

const BOX = '[data-zeyn-filter-chips]';

/** Fills `{name}` placeholders of a text. */
function fill(text, args) {
  return text.replace(/\{(\w+)\}/g, (m, k) => (k in args ? String(args[k]) : m));
}

/** Makes an element with a class and attributes. */
function make(tag, cls, attrs = {}) {
  const el = document.createElement(tag);
  if (cls) el.className = cls;
  for (const [k, v] of Object.entries(attrs)) el.setAttribute(k, v);
  return el;
}

/** The header cell of a column, which holds its filter kind and choice labels. */
function headOf(grid, field) {
  return grid.querySelector(`:scope > table > thead th[data-field="${CSS.escape(field)}"]`);
}

/** The label of a column from the grid's column list, or its field name. */
function labelOf(grid, field) {
  try {
    const c = JSON.parse(grid.dataset.zeynColumns || '[]').find((x) => x.field === field);
    return c?.label || headOf(grid, field)?.textContent.trim() || field;
  } catch {
    return field;
  }
}

/** A date, time or number bound in the language of the page. */
function bound(kind, v) {
  const lang = document.documentElement.lang || undefined;
  if (kind === 'date' && /^\d{4}-\d{2}-\d{2}$/.test(v)) return new Date(`${v}T00:00`).toLocaleDateString(lang);
  if (kind === 'datetime' && !Number.isNaN(Date.parse(v))) return new Date(v).toLocaleString(lang, { dateStyle: 'short', timeStyle: 'short' });
  if ((kind === 'number' || kind === 'money') && v !== '' && !Number.isNaN(Number(v))) return Number(v).toLocaleString(lang);
  return String(v);
}

/** The value part of a chip: the text, the picked choices, yes or no, or a range. */
function valueOf(box, grid, f) {
  const th = headOf(grid, f.field);
  const kind = th?.dataset.filter;
  if (f.contains != null) return `"${f.contains}"`;
  if (f.starts_with != null) return `${f.starts_with}…`;
  if (f.equals != null) return `= ${f.equals}`;
  if (f.any_of) {
    let options = [];
    try {
      options = JSON.parse(th?.dataset.options || '[]');
    } catch {}
    return f.any_of.map((v) => options.find((o) => String(o.value) === String(v))?.label ?? v).join(', ');
  }
  if (f.is != null) return f.is ? box.dataset.textYes : box.dataset.textNo;
  if (f.from != null && f.to != null) return `${bound(kind, f.from)} … ${bound(kind, f.to)}`;
  if (f.from != null) return `≥ ${bound(kind, f.from)}`;
  if (f.to != null) return `≤ ${bound(kind, f.to)}`;
  return '';
}

/** Number of conditions in a filter group and its nested groups. */
function count(g) {
  return (g.filters?.length ?? 0) + (g.groups ?? []).reduce((n, x) => n + count(x), 0);
}

/** One chip: a button with the text that edits the filter and a button that removes it. */
function chip(box, text, edit, remove) {
  const item = make('span', 'z-filter-chip');
  const label = make('button', 'z-filter-chip-edit', { type: 'button' });
  label.textContent = text;
  label.addEventListener('click', edit);
  const x = make('button', 'z-filter-chip-remove', { type: 'button', 'aria-label': fill(box.dataset.textRemove || 'Remove filter {label}', { label: text }) });
  x.append(make('span', 'z-icon z-icon-x', { 'aria-hidden': 'true' }));
  x.addEventListener('click', remove);
  item.append(label, x);
  return item;
}

/** Opens the grid's filter builder, or focuses the first control of a column's filter cell. */
function editor(grid, field) {
  return () => {
    const tools = grid.previousElementSibling?.matches('[data-zeyn-bulk]') ? grid.previousElementSibling.previousElementSibling : grid.previousElementSibling;
    const builder = tools?.querySelector('[data-zeyn-filter-builder]');
    if (builder) return builder.click();
    const cell = field && grid.querySelector(`:scope > table > thead .z-filter-cell[data-field="${CSS.escape(field)}"]`);
    cell?.querySelector('input, select, button')?.focus();
  };
}

/** Draws the chips of a box from the list the grid put on it and keeps focus near a removed chip. */
function draw(box) {
  const state = box.zeynFilters;
  if (!state) return;
  const { grid, filters, groups, set } = state;
  const at = [...box.querySelectorAll('button')].indexOf(document.activeElement);
  const items = [];
  filters.forEach((f, i) => {
    const text = `${labelOf(grid, f.field)}: ${valueOf(box, grid, f)}`;
    items.push(chip(box, text, editor(grid, f.field), () => set(filters.filter((_, j) => j !== i), groups)));
  });
  groups.forEach((g, i) => {
    const text = fill(box.dataset.textGroup || '{n} conditions', { n: count(g) });
    items.push(chip(box, text, editor(grid, null), () => set(filters, groups.filter((_, j) => j !== i))));
  });
  if (items.length > 1) {
    const clear = make('button', 'z-link z-filter-chips-clear', { type: 'button' });
    clear.textContent = box.dataset.textClear || 'Clear all';
    clear.addEventListener('click', () => {
      set([], []);
      grid.focus();
    });
    items.push(clear);
  }
  box.replaceChildren(...items);
  box.hidden = items.length === 0;
  if (at < 0) return;
  const buttons = [...box.querySelectorAll('button')];
  (buttons[Math.min(at, buttons.length - 1)] ?? grid).focus();
}

document.addEventListener('zeyn:filters', (e) => {
  if (e.target instanceof Element && e.target.matches(BOX)) draw(e.target);
});
document.querySelectorAll(BOX).forEach(draw);
document.addEventListener('zeyn:content', (e) => {
  const root = e.detail instanceof Element ? e.detail : document;
  root.querySelectorAll(BOX).forEach(draw);
});
