/** The selection bar of a grid, rendered right before it. */
function bulkBar(grid) {
  const bar = grid.previousElementSibling;
  return bar?.matches('[data-zeyn-bulk]') ? bar : null;
}

/**
 * Shows the selection on the rows in the DOM (checkbox and `aria-selected`), the header box
 * (checked, mixed or empty) and the selection bar with its count and the offer to select every
 * matching row.
 */
function syncSelection(grid) {
  const s = grids.get(grid);
  const bar = bulkBar(grid);
  if (!s || !bar) return;
  const list = rows(grid).filter((tr) => tr.dataset.zeynKey != null);
  let on = 0;
  for (const tr of list) {
    const picked = s.allMatching || s.selected.has(tr.dataset.zeynKey);
    const box = tr.querySelector('[data-zeyn-select]');
    if (box && box.checked !== picked) box.checked = picked;
    if (picked) on++;
    if (picked) tr.setAttribute('aria-selected', 'true');
    else tr.removeAttribute('aria-selected');
  }
  const all = grid.querySelector('[data-zeyn-select-all]');
  if (all) {
    all.checked = list.length > 0 && on === list.length;
    all.indeterminate = on > 0 && on < list.length;
  }
  const count = s.allMatching ? s.total ?? 0 : s.selected.size;
  bar.hidden = count === 0;
  const d = bar.dataset;
  bar.querySelector('.z-bulk-count').textContent = s.allMatching
    ? fill(d.textAllSelected || 'All {total} matching rows selected', { total: fmt(s.total ?? 0) })
    : fill(d.textSelected || '{n} selected', { n: fmt(count) });
  const matching = bar.querySelector('[data-zeyn-select-matching]');
  const offer = !s.allMatching && list.length > 0 && on === list.length && (s.total ?? 0) > s.selected.size;
  matching.hidden = !offer;
  if (offer) matching.textContent = fill(d.textAll || 'Select all {total} matching rows', { total: fmt(s.total ?? 0) });
}

/**
 * Picks or drops a row from its checkbox. With Shift, every row between the last picked one and
 * this one (rows in the DOM) takes the same state.
 */
function pickRow(box, range) {
  const tr = box.closest('tr');
  const grid = tr.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  if (!s) return;
  const on = box.checked;
  if (s.allMatching && !on) {
    s.allMatching = false;
    for (const r of rows(grid)) if (r.dataset.zeynKey != null) s.selected.add(r.dataset.zeynKey);
  }
  const list = rows(grid);
  const i = list.indexOf(tr);
  const j = range && s.lastPick != null ? list.findIndex((r) => r.dataset.zeynKey === s.lastPick) : -1;
  const span = j < 0 ? [tr] : list.slice(Math.min(i, j), Math.max(i, j) + 1);
  for (const r of span) {
    const key = r.dataset.zeynKey;
    if (key == null) continue;
    if (on) s.selected.add(key);
    else s.selected.delete(key);
  }
  s.lastPick = tr.dataset.zeynKey;
  syncSelection(grid);
}

/** Header box: picks or drops every row in the DOM. */
function pickPage(box) {
  const grid = box.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  if (!s) return;
  if (!box.checked) s.allMatching = false;
  for (const r of rows(grid)) {
    if (r.dataset.zeynKey == null) continue;
    if (box.checked) s.selected.add(r.dataset.zeynKey);
    else s.selected.delete(r.dataset.zeynKey);
  }
  syncSelection(grid);
}

/** Grid of a selection bar control. */
function gridOfBar(el) {
  return el.closest('[data-zeyn-bulk]')?.nextElementSibling ?? null;
}

/** Drops the selection of a grid. */
function clearSelection(grid) {
  const s = grids.get(grid);
  if (!s) return;
  s.selected.clear();
  s.allMatching = false;
  s.lastPick = null;
  syncSelection(grid);
}

/**
 * Asks before a row or bulk action whose button carries `data-zeyn-confirm`, in the page's
 * confirmation dialog (`zeyn:confirm`), or the browser's own when that script is missing.
 * Resolves `true` at once for buttons without a question.
 */
function confirmed(button, s) {
  const message = button?.dataset.zeynConfirm;
  if (!message) return Promise.resolve(true);
  const t = s?.texts ?? {};
  return loadModule('components').then(() => new Promise((resolve) => {
    const detail = {
      title: t.confirm_title || '',
      message,
      ok: button.textContent.trim(),
      cancel: t.confirm_cancel || 'Cancel',
      danger: button.matches('.z-danger, .z-danger-button'),
      resolve,
    };
    if (!ask('zeyn:confirm', detail)) resolve(window.confirm(message));
  }));
}

/**
 * Runs a bulk action with `POST /_zeyn/grid/bulk`: the picked keys, or, when every matching row
 * is selected, the filters so the server collects the keys itself.
 */
async function runBulk(button) {
  const grid = gridOfBar(button);
  const s = grid && grids.get(grid);
  if (!s || !(await confirmed(button, s))) return;
  button.disabled = true;
  const r = await post(BASE + '/_zeyn/grid/bulk', {
    ...address(grid),
    action: Number(button.dataset.zeynBulkRun),
    keys: s.allMatching ? [] : [...s.selected],
    all: s.allMatching,
    query: { sort: s.sort, filters: filterList(s), q: s.q },
  });
  button.disabled = false;
  if (!r.ok) return;
  await runEffects(r.data?.effects, button);
  toast(s.texts.done || 'Done.');
  clearSelection(grid);
  await loadGrid(grid, 'reload');
}

/** Editable header cells by column position: `th[data-edit]` or `null`. */
function editableHeads(grid) {
  const head = own(grid, 'thead tr:not(.z-band-row)');
  return (head ? cellsOf(head) : []).map((th) => (th.dataset.edit ? th : null));
}

/**
 * Puts a row in edit mode: every editable cell (`td[data-v]`) gets an input of its column's kind
 * holding the server's input value. Focus goes to the cell `start`, or the first input.
 */
function startEdit(tr, start) {
  const grid = tr.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  if (!s || tr.classList.contains('z-editing') || (tr.dataset.zeynKey == null && !tr.classList.contains('z-new-row'))) return false;
  unwindow(tr);
  const heads = editableHeads(grid);
  let first = null;
  let wanted = null;
  [...tr.cells].forEach((td, i) => {
    const th = heads[i];
    if (!th || td.dataset.v == null) return;
    td._orig = td.innerHTML;
    const kind = th.dataset.edit;
    const label = `${th.textContent.trim()}`;
    let input;
    if (kind === 'choice') {
      input = make('select', 'z-cell-input', { 'aria-label': label });
      let options = [];
      try {
        options = JSON.parse(th.dataset.options || '[]');
      } catch {
        options = [];
      }
      if (th.dataset.optional) input.append(new Option('', ''));
      for (const o of options) input.append(new Option(o.label, o.value));
      input.value = td.dataset.v;
    } else if (kind === 'bool') {
      input = make('input', 'z-cell-input', { type: 'checkbox', 'aria-label': label });
      input.checked = td.dataset.v === 'true';
    } else {
      const type = { number: 'number', date: 'date', datetime: 'datetime-local', time: 'time' }[kind] ?? 'text';
      input = make('input', 'z-cell-input', { type, 'aria-label': label, autocomplete: 'off' });
      if (type === 'number') input.step = '1';
      if (type === 'datetime-local' || type === 'time') input.step = '1';
      if (kind === 'money') input.inputMode = 'decimal';
      input.value = td.dataset.v;
    }
    input.dataset.field = th.dataset.field;
    td.replaceChildren(input);
    first ??= input;
    if (td === start) wanted = input;
  });
  if (!first) return false;
  tr.classList.add('z-editing');
  (wanted ?? first).focus();
  (wanted ?? first).select?.();
  return true;
}

/** Leaves edit mode without saving and puts the cells back; a new row goes away. */
function cancelEdit(tr) {
  const grid = tr.closest('[data-zeyn-grid]');
  if (tr.classList.contains('z-new-row')) {
    tr.remove();
    if (!rows(grid).length) gridState(grid, 'empty');
    toolsOf(grid)?.querySelector('[data-zeyn-grid-add]')?.focus();
    return;
  }
  delete tr.dataset.zeynDirty;
  for (const td of tr.cells) {
    if (td._orig != null) {
      td.innerHTML = td._orig;
      td._orig = null;
    }
  }
  tr.classList.remove('z-editing');
  const s = grids.get(grid);
  if (s?.colv) {
    tr._zw = '';
    windowRow(tr, s.colv);
  }
  tr.focus();
}

/** Values of a row in edit mode, as a form sends them, with the row version. */
function editValues(tr) {
  const grid = tr.closest('[data-zeyn-grid]');
  const values = {};
  for (const input of tr.querySelectorAll('.z-cell-input')) {
    values[input.dataset.field] = input.type === 'checkbox' ? input.checked : input.value;
  }
  const vf = grid.dataset.zeynVersionField;
  if (vf && tr.dataset.zeynVersion != null) values[vf] = tr.dataset.zeynVersion;
  return values;
}

/**
 * Sends values of a row to `POST /_zeyn/grid/edit` and swaps in the saved row. On `422` the
 * messages show in their cells (`aria-invalid`, `role="alert"`) and focus moves to the first one.
 * Returns the saved row or `null`.
 */
async function sendEdit(grid, tr, values) {
  const s = grids.get(grid);
  for (const e of tr.querySelectorAll('.z-cell-error')) e.remove();
  tr.setAttribute('aria-busy', 'true');
  const r = await post(BASE + '/_zeyn/grid/edit', {
    ...address(grid),
    key: tr.dataset.zeynKey,
    values,
    view: s ? viewOf(grid, s) : null,
  });
  tr.removeAttribute('aria-busy');
  if (r.status === 422) {
    cellErrors(tr, r.data?.errors);
    return null;
  }
  if (!r.ok || typeof r.data?.row !== 'string') {
    if (r.status === 409) await loadGrid(grid, 'reload');
    return null;
  }
  const fresh = swapRow(grid, tr, r.data.row);
  if (!fresh) return null;
  activate(fresh, true);
  syncSelection(grid);
  return fresh;
}

/**
 * Shows validation errors of a row in edit mode under their inputs (`aria-invalid`,
 * `role="alert"`) and focuses the first in the row; errors without an input become toasts.
 */
function cellErrors(tr, errors) {
  for (const input of tr.querySelectorAll('.z-cell-input[aria-invalid]')) input.removeAttribute('aria-invalid');
  for (const err of errors ?? []) {
    const input = tr.querySelector(`.z-cell-input[data-field="${CSS.escape(err.field)}"]`);
    if (!input) {
      toast(err.message, 'error');
      continue;
    }
    input.setAttribute('aria-invalid', 'true');
    const box = make('span', 'z-cell-error', { role: 'alert' });
    box.id = `z-cell-error-${++uid}`;
    box.textContent = err.message;
    input.setAttribute('aria-describedby', box.id);
    input.after(box);
  }
  tr.querySelector('.z-cell-input[aria-invalid="true"]')?.focus();
}

/**
 * Carries the place of a row in a grouped or tree grid over to its fresh copy from the server,
 * which is rendered as a top row: level, path, open state and the tree toggle.
 */
function keepBranch(tr, fresh) {
  for (const a of ['aria-level', 'data-zeyn-level', 'data-zeyn-path', 'aria-expanded']) {
    if (tr.hasAttribute(a)) fresh.setAttribute(a, tr.getAttribute(a));
  }
  const old = tr.querySelector(':scope > .z-tree-cell');
  const cell = fresh.querySelector(':scope > .z-tree-cell > .z-tree');
  if (!old || !cell) return;
  const src = old._orig != null ? Object.assign(document.createElement('td'), { innerHTML: old._orig }) : old;
  const tree = src.querySelector('.z-tree');
  if (!tree) return;
  cell.style.cssText = tree.style.cssText;
  if (tree.firstElementChild) cell.firstElementChild?.replaceWith(tree.firstElementChild);
}

/** Values of a row in edit mode from before the edit, by field. */
function editBefore(tr) {
  const before = {};
  for (const td of tr.cells) {
    const input = td.querySelector('.z-cell-input');
    if (input) before[input.dataset.field] = input.type === 'checkbox' ? td.dataset.v === 'true' : td.dataset.v;
  }
  return before;
}

/**
 * Saves a row in edit mode and offers to undo the change. A new row is created instead, and in
 * a batch grid a changed row stays in edit mode, marked, until the user saves every change.
 */
async function saveEdit(tr) {
  const grid = tr.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  if (!s || tr.getAttribute('aria-busy') === 'true') return;
  if (tr.classList.contains('z-new-row')) return saveNew(grid, tr);
  const before = editBefore(tr);
  const values = editValues(tr);
  const changed = Object.keys(before).some((k) => String(before[k]) !== String(values[k]));
  if (!changed) {
    cancelEdit(tr);
    return batchBar(grid);
  }
  if (grid.hasAttribute('data-zeyn-batch')) {
    tr.dataset.zeynDirty = '';
    tr.focus();
    return batchBar(grid);
  }
  const fresh = await sendEdit(grid, tr, values);
  if (!fresh) return;
  const key = fresh.dataset.zeynKey;
  s.undo = { key, values: before };
  toast(s.texts.saved || 'Saved.', 'info', { label: s.texts.undo || 'Undo', run: () => undoEdit(grid) });
}

/** The tools above a grid (`.z-grid-tools`), or `null`. */
function toolsOf(grid) {
  let el = grid.previousElementSibling;
  if (el?.matches('[data-zeyn-bulk]')) el = el.previousElementSibling;
  return el?.matches('.z-grid-tools') ? el : null;
}

/**
 * Opens an empty row at the top of the grid with an input for every editable column, or focuses
 * the one already open.
 */
function addRow(button) {
  const grid = gridOfTools(button);
  const s = grid && grids.get(grid);
  const body = grid?.querySelector(':scope > table > [data-zeyn-rows]');
  if (!s || !body) return;
  const open = body.querySelector(':scope > tr.z-new-row');
  if (open) return open.querySelector('.z-cell-input')?.focus();
  const tr = make('tr', 'z-new-row');
  for (const th of cellsOf(own(grid, 'thead tr:not(.z-band-row)'))) {
    const td = make('td');
    if (th.dataset.edit) td.dataset.v = '';
    tr.append(td);
  }
  body.prepend(tr);
  gridState(grid, null);
  startEdit(tr, null);
}

/** Creates the new row through `POST /_zeyn/grid/add` and reloads the grid. */
async function saveNew(grid, tr) {
  for (const e of tr.querySelectorAll('.z-cell-error')) e.remove();
  tr.setAttribute('aria-busy', 'true');
  const r = await post(BASE + '/_zeyn/grid/add', { ...address(grid), values: editValues(tr) });
  tr.removeAttribute('aria-busy');
  if (r.status === 422) return cellErrors(tr, r.data?.errors);
  if (!r.ok) return;
  tr.remove();
  toast(grids.get(grid)?.texts.added || 'Row added.');
  await loadGrid(grid, 'reload');
  rows(grid)[0]?.focus();
}

/** Changed rows of a batch grid that wait to be saved. */
function dirtyRows(grid) {
  return rows(grid).filter((tr) => tr.dataset.zeynDirty != null && tr.classList.contains('z-editing'));
}

/** Shows the count of changed rows in the batch bar, or hides the bar when there are none. */
function batchBar(grid) {
  const bar = toolsOf(grid)?.querySelector('[data-zeyn-batch-bar]');
  if (!bar) return;
  const n = dirtyRows(grid).length;
  bar.hidden = !n;
  bar.firstElementChild.textContent = n ? fill(bar.dataset.textCount || 'Changed rows: {n}', { n: fmt(n) }) : '';
}

/** Saves every changed row of a batch grid, one after another; failed rows keep their changes. */
async function saveBatch(button) {
  const grid = gridOfTools(button);
  const s = grid && grids.get(grid);
  if (!s) return;
  button.disabled = true;
  let saved = 0;
  for (const tr of dirtyRows(grid)) if (await sendEdit(grid, tr, editValues(tr))) saved++;
  button.disabled = false;
  batchBar(grid);
  if (saved) toast(fill(s.texts.batch_saved || '{n} rows saved.', { n: fmt(saved) }));
  const left = dirtyRows(grid).length;
  if (left) toast(fill(s.texts.paste_failed || '{n} rows could not be saved.', { n: fmt(left) }), 'error');
}

/** Drops every unsaved change of a batch grid. */
function discardBatch(button) {
  const grid = gridOfTools(button);
  if (!grid) return;
  for (const tr of dirtyRows(grid)) cancelEdit(tr);
  batchBar(grid);
}

/**
 * Whether the grid may load other rows: a batch grid with unsaved changes asks first and drops
 * them when the user agrees.
 */
function mayLeave(grid) {
  const dirty = grid.hasAttribute('data-zeyn-batch') ? dirtyRows(grid) : [];
  if (!dirty.length) return true;
  if (!confirm(fill(grids.get(grid)?.texts.discard_confirm || 'Discard the changes in {n} rows?', { n: fmt(dirty.length) }))) return false;
  for (const tr of dirty) cancelEdit(tr);
  batchBar(grid);
  return true;
}

/**
 * The Delete key on a row of a grid with `delete_with`: deletes the selected rows, or the row
 * itself when none is selected, after the action's confirmation. Returns true when handled.
 */
function deleteKey(tr) {
  const grid = tr.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  const button = bulkBar(grid)?.querySelector('[data-zeyn-delete]');
  if (!s || !button || tr.dataset.zeynKey == null) return false;
  if (!s.selected.size && !s.allMatching) {
    const box = tr.querySelector('input[data-zeyn-select]');
    if (!box) return false;
    box.checked = true;
    pickRow(box, false);
  }
  runBulk(button);
  return true;
}

/**
 * Puts back the values from before the last inline save or paste (every row it changed), with
 * each row's current version.
 */
async function undoEdit(grid) {
  const s = grids.get(grid);
  const undo = s?.undo;
  if (!undo) return;
  s.undo = null;
  let done = 0;
  for (const u of Array.isArray(undo) ? undo : [undo]) {
    const tr = grid.querySelector(`:scope > table > [data-zeyn-rows] > tr[data-zeyn-key="${CSS.escape(u.key)}"]`);
    if (!tr) continue;
    const values = { ...u.values };
    const vf = grid.dataset.zeynVersionField;
    if (vf && tr.dataset.zeynVersion != null) values[vf] = tr.dataset.zeynVersion;
    if (await sendEdit(grid, tr, values)) done++;
  }
  if (done) toast(s.texts.undone || 'Change undone.');
}

/**
 * Copies the selected rows in the DOM (or the focused row) as tab separated text for
 * spreadsheets, in the shown column order, with a header line when there is more than one row.
 * Selection and action columns are left out. Returns true when something was copied.
 */
function copyRows(grid, focused) {
  const s = grids.get(grid);
  if (!s || !navigator.clipboard?.writeText) return false;
  const picked = rows(grid).filter((tr) => tr.getAttribute('aria-selected') === 'true');
  const list = picked.length ? picked : focused ? [focused] : [];
  if (!list.length) return false;
  const keep = (cell) => !cell.matches('.z-row-select, .z-select-col, .z-row-actions, .z-actions-col, .z-expand, .z-expand-col');
  const text = (cell) =>
    (cell.textContent.trim() || cell.querySelector('[role="img"][aria-label]')?.getAttribute('aria-label') || '').replace(/[\t\r\n]+/g, ' ');
  const lines = list.map((tr) => cellsOf(tr).filter(keep).map(text).join('\t'));
  if (list.length > 1) lines.unshift(cellsOf(own(grid, 'thead tr:not(.z-band-row)')).filter(keep).map(text).join('\t'));
  navigator.clipboard.writeText(lines.join('\r\n') + '\r\n').then(
    () => toast(fill(s.texts.copied || '{n} rows copied.', { n: fmt(list.length) })),
    () => {},
  );
  return true;
}

/** Keys in a row in edit mode: Enter saves, Escape cancels. Returns true when handled. */
function editKey(e, input) {
  const tr = input.closest('tr');
  if (e.key === 'Escape') {
    e.preventDefault();
    e.stopPropagation();
    cancelEdit(tr);
    return true;
  }
  if (e.key === 'Enter' && !e.isComposing) {
    e.preventDefault();
    saveEdit(tr);
    return true;
  }
  return false;
}

/**
 * Runs a row item (`data-zeyn-run`) with `POST /_zeyn/grid/run`, sending only the row key and the
 * item. On success shows a short confirmation and reloads the grid.
 */
async function runRowItem(button, item) {
  const tr = button?.closest('tr[data-zeyn-key]') ?? null;
  const grid = (tr ?? button)?.closest('[data-zeyn-grid]');
  const key = tr?.dataset.zeynKey ?? button?.dataset.zeynKey;
  if (!grid || key == null) return;
  if (!item && !(await confirmed(button, grids.get(grid)))) return;
  if (button?.tagName === 'BUTTON') button.disabled = true;
  const r = await post(BASE + '/_zeyn/grid/run', { ...address(grid), key, item: item ?? button.dataset.zeynRun });
  if (button?.isConnected && button.tagName === 'BUTTON') button.disabled = false;
  if (!r.ok) return;
  await runEffects(r.data?.effects, button);
  toast(grids.get(grid)?.texts.done || 'Done.');
  await loadGrid(grid, 'reload');
  const again = grid.querySelector(`:scope > table > [data-zeyn-rows] > tr[data-zeyn-key="${CSS.escape(key)}"]`);
  if (again && (!document.activeElement || document.activeElement === document.body)) {
    activate(again, false);
    (again.querySelector(`[data-zeyn-run="${CSS.escape(item ?? button.dataset.zeynRun)}"]`) ?? again).focus();
  }
}

