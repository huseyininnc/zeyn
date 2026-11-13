/**
 * Opens or closes the detail of a row (`data-zeyn-detail`). Opening adds a row under it and
 * loads the detail screen into it from `GET /_zeyn/screen/{screen}`; closing removes that row, so
 * the next opening loads fresh data. Grids inside the detail are set up like any other.
 */
async function toggleDetail(tr, open = tr.getAttribute('aria-expanded') !== 'true') {
  const grid = tr.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  let target;
  try {
    target = JSON.parse(tr.dataset.zeynDetail);
  } catch {
    return;
  }
  const button = tr.querySelector(':scope > td > [data-zeyn-expand]');
  const shown = tr.nextElementSibling?.classList.contains('z-detail') ? tr.nextElementSibling : null;
  tr.setAttribute('aria-expanded', String(open));
  button?.setAttribute('aria-expanded', String(open));
  if (!open) {
    const inside = shown?.contains(document.activeElement);
    shown?.remove();
    if (inside) tr.focus();
    return;
  }
  if (shown) return;
  const t = s?.texts ?? {};
  const label = tr.querySelector(':scope > td:not(.z-expand):not(.z-row-select)')?.textContent.trim() ?? '';
  const row = make('tr', 'z-detail');
  if (tr.dataset.zeynLevel != null) row.dataset.zeynLevel = tr.dataset.zeynLevel;
  const cell = make('td');
  cell.colSpan = own(grid, 'thead tr:not(.z-band-row)')?.cells.length ?? 1;
  const box = make('div', 'z-detail-body', { role: 'region', 'aria-label': `${t.details || 'Details'}: ${label}`, 'aria-busy': 'true' });
  box.id = `z-detail-${++uid}`;
  box.textContent = t.loading || 'Loading...';
  button?.setAttribute('aria-controls', box.id);
  cell.append(box);
  row.append(cell);
  tr.after(row);
  let res;
  try {
    res = await fetch(`${BASE}/_zeyn/screen/${encodeURIComponent(target.screen)}${paramsQuery(target.params)}`, {
      credentials: 'same-origin',
      headers: { accept: 'text/html' },
    });
  } catch {
    res = null;
  }
  if (!row.isConnected) return;
  box.removeAttribute('aria-busy');
  if (!res?.ok) {
    const data = res ? await readJson(res) : null;
    box.setAttribute('role', 'alert');
    box.textContent = data?.message || t.error || UNREACHABLE;
    return;
  }
  box.innerHTML = await res.text();
  const win = box.querySelector('[data-zeyn-screen]');
  if (win) {
    win.removeAttribute('role');
    win.removeAttribute('aria-modal');
    if (!win.hasAttribute('data-zeyn-params')) win.dataset.zeynParams = JSON.stringify(target.params ?? null);
    const heading = win.querySelector(':scope > h1, :scope > h2');
    if (heading) {
      const h = make('h3');
      h.id = heading.id;
      h.append(...heading.childNodes);
      heading.replaceWith(h);
    }
  }
  initGrids(box);
  relativeTimes(box);
}

/**
 * Every cell of a row in column order, also the ones column virtualization took out of the page.
 * @param {HTMLTableRowElement} tr
 * @returns {HTMLTableCellElement[]}
 */
function cellsOf(tr) {
  return tr._zc ?? [...tr.cells];
}

/** True for a row of data cells: not a group, detail, "Load more" or placeholder row. */
function dataRow(tr) {
  return !!tr && !tr.matches('.z-group, .z-detail, .z-branch-more, .z-placeholder');
}

/** Index of the first data cell: the expander and selection cells come before it. */
function leadOf(tr) {
  let i = 0;
  const cells = cellsOf(tr);
  while (i < cells.length && cells[i].matches('.z-expand, .z-row-select')) i++;
  return i;
}

/** Position of a row for ranges: its absolute index in a virtual grid, else its place in the DOM. */
function rowIndex(grid, tr) {
  return tr.dataset.zeynIndex != null ? Number(tr.dataset.zeynIndex) : rows(grid).indexOf(tr);
}

/** The row at a range position, from the DOM or, in a virtual grid, from the block cache. */
function rowAt(grid, i) {
  const s = grids.get(grid);
  if (s?.paging !== 'virtual') return rows(grid)[i] ?? null;
  const b = Math.floor(i / s.size);
  return (s.cache.get(b) ?? s.stale.get(b))?.[i - b * s.size] ?? null;
}

/**
 * Moves the cell cursor to column `col` of `tr` and focuses that cell. With `extend` the range
 * from the anchor cell grows to it; otherwise the cell becomes the new anchor.
 */
function focusCell(grid, tr, col, extend = false) {
  const s = grids.get(grid);
  if (!s || !dataRow(tr)) return false;
  const cells = cellsOf(tr);
  const c = Math.max(leadOf(tr), Math.min(cells.length - 1, col));
  showColumn(grid, c);
  const td = cellsOf(tr)[c];
  if (!td?.isConnected) return false;
  activate(tr, false);
  td.tabIndex = -1;
  td.focus();
  const at = { row: rowIndex(grid, tr), col: c };
  if (!extend || !s.anchor) s.anchor = at;
  s.cursor = at;
  paintRange(grid);
  return true;
}

/** The selected rectangle: first and last row position and column, anchor and cursor included. */
function rangeOf(s) {
  if (!s?.anchor || !s.cursor) return null;
  return {
    r0: Math.min(s.anchor.row, s.cursor.row),
    r1: Math.max(s.anchor.row, s.cursor.row),
    c0: Math.min(s.anchor.col, s.cursor.col),
    c1: Math.max(s.anchor.col, s.cursor.col),
  };
}

/** Marks the cells of the range in the DOM (`aria-selected`), when it is more than one cell. */
function paintRange(grid) {
  const s = grids.get(grid);
  for (const td of grid.querySelectorAll(':scope > table > tbody > tr > td[aria-selected]')) td.removeAttribute('aria-selected');
  const r = rangeOf(s);
  if (!r || (r.r0 === r.r1 && r.c0 === r.c1)) return;
  for (let i = r.r0; i <= r.r1; i++) {
    const tr = rowAt(grid, i);
    if (!dataRow(tr)) continue;
    const cells = cellsOf(tr);
    for (let c = r.c0; c <= r.c1; c++) cells[c]?.setAttribute('aria-selected', 'true');
  }
}

/** Leaves cell mode: drops the range and focuses the row. */
function leaveCells(grid, tr) {
  const s = grids.get(grid);
  if (s) s.anchor = s.cursor = null;
  paintRange(grid);
  activate(tr, true);
}

/**
 * Keys on a focused cell, as in a spreadsheet: arrows move (Shift grows the range), Home and End
 * go to the ends of the row (with Ctrl, of the grid), Tab and Shift+Tab move along the row and on
 * to the next one and leave the grid at its ends, Enter or F2 edits an editable cell or presses
 * the cell's only button or link, Escape drops the range. Left on the first cell goes back to the
 * row. Returns true when handled.
 */
function cellKey(e, td) {
  const tr = td.parentElement;
  const grid = tr.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  if (!s || !dataRow(tr)) return false;
  const col = cellsOf(tr).indexOf(td);
  const last = cellsOf(tr).length - 1;
  const rtl = getComputedStyle(td).direction === 'rtl';
  const ctrl = e.ctrlKey || e.metaKey;
  const go = (row, c, extend = e.shiftKey) => {
    e.preventDefault();
    if (row && dataRow(row)) focusCell(grid, row, c, extend);
    else if (row) activate(row, true);
    return true;
  };
  const vertical = (step) => {
    if (s.paging === 'virtual') {
      s.active = Number(tr.dataset.zeynIndex);
      if (!virtualKey(e, grid)) return false;
      const row = s.body.querySelector(`[data-zeyn-index="${s.active}"]`);
      if (row) focusCell(grid, row, col, e.shiftKey);
      return true;
    }
    const list = rows(grid);
    const i = list.indexOf(tr);
    const target = typeof step === 'number' ? list[i + step] : step;
    if (!target) return (e.preventDefault(), true);
    return go(target, col);
  };
  switch (e.key) {
    case 'ArrowLeft':
    case 'ArrowRight': {
      const dir = (e.key === 'ArrowRight' ? 1 : -1) * (rtl ? -1 : 1);
      if (dir < 0 && col <= leadOf(tr) && !e.shiftKey) {
        e.preventDefault();
        leaveCells(grid, tr);
        return true;
      }
      return go(tr, col + dir);
    }
    case 'ArrowUp':
      return vertical(-1);
    case 'ArrowDown':
      return vertical(1);
    case 'PageUp':
    case 'PageDown':
      return s.paging === 'virtual' ? vertical(0) : false;
    case 'Home':
    case 'End': {
      const end = e.key === 'End';
      if (ctrl && s.paging === 'virtual') {
        if (!vertical(0)) return false;
        const row = s.body.querySelector(`[data-zeyn-index="${s.active}"]`);
        if (row) focusCell(grid, row, end ? Infinity : 0, e.shiftKey);
        return true;
      }
      const list = rows(grid).filter(dataRow);
      const row = ctrl ? (end ? list.at(-1) : list[0]) : tr;
      return go(row, end ? Infinity : 0);
    }
    case 'Tab': {
      const list = rows(grid).filter(dataRow);
      const i = list.indexOf(tr);
      const back = e.shiftKey;
      if (!back && col < last) return go(tr, col + 1, false);
      if (back && col > leadOf(tr)) return go(tr, col - 1, false);
      const row = list[i + (back ? -1 : 1)];
      if (!row) {
        s.anchor = s.cursor = null;
        paintRange(grid);
        return false;
      }
      return go(row, back ? Infinity : 0, false);
    }
    case 'Enter':
    case 'F2': {
      e.preventDefault();
      if (td.dataset.v != null) return startEdit(tr, td) || true;
      const controls = td.querySelectorAll('button, a[href]');
      if (e.key === 'Enter' && controls.length === 1) controls[0].click();
      else if (e.key === 'Enter' && tr.dataset.zeynEffect) {
        const effect = parseEffect(tr);
        if (effect) runEffect(effect, td);
      }
      return true;
    }
    case 'Escape':
      if (!s.anchor) return false;
      e.preventDefault();
      s.anchor = s.cursor;
      paintRange(grid);
      return true;
  }
  return false;
}

/** Text of a cell as the user reads it, on one line. */
function cellText(cell) {
  return (cell.textContent.trim() || cell.querySelector('[role="img"][aria-label]')?.getAttribute('aria-label') || '').replace(/[\t\r\n]+/g, ' ');
}

/** Copies the range of cells (or the focused cell) as tab separated text. Returns true when copied. */
function copyCells(grid) {
  const s = grids.get(grid);
  const r = rangeOf(s);
  if (!r || !navigator.clipboard?.writeText) return false;
  const lines = [];
  let n = 0;
  for (let i = r.r0; i <= r.r1; i++) {
    const tr = rowAt(grid, i);
    if (!dataRow(tr)) continue;
    const cells = cellsOf(tr).slice(r.c0, r.c1 + 1);
    n += cells.length;
    lines.push(cells.map(cellText).join('\t'));
  }
  navigator.clipboard.writeText(lines.join('\r\n') + '\r\n').then(
    () => toast(fill(s.texts.copied_cells || '{n} cells copied.', { n: fmt(n) })),
    () => {},
  );
  return true;
}

/** Reads tab separated text from a spreadsheet: rows of cells, quoted cells may hold tabs and line breaks. */
function parseTsv(text) {
  const out = [];
  let row = [];
  let cell = '';
  let quoted = false;
  for (let i = 0; i < text.length; i++) {
    const c = text[i];
    if (quoted) {
      if (c !== '"') cell += c;
      else if (text[i + 1] === '"') {
        cell += '"';
        i++;
      } else quoted = false;
    } else if (c === '"' && cell === '') quoted = true;
    else if (c === '\t') {
      row.push(cell);
      cell = '';
    } else if (c === '\n' || c === '\r') {
      if (c === '\r' && text[i + 1] === '\n') i++;
      row.push(cell);
      out.push(row);
      row = [];
      cell = '';
    } else cell += c;
  }
  if (cell !== '' || row.length) {
    row.push(cell);
    out.push(row);
  }
  return out;
}

/**
 * Turns pasted text into the value an input of the column would hold: choices by value or label,
 * yes and no in any of the page's words, money without the currency symbol. Anything else goes to
 * the server as it is, which reads and validates it.
 */
function pastedValue(raw, th, t) {
  const v = raw.trim();
  const low = v.toLocaleLowerCase(document.documentElement.lang || undefined);
  switch (th.dataset.edit) {
    case 'bool':
      if (['true', '1', 'yes', 'x', (t.yes || '').toLocaleLowerCase()].includes(low)) return true;
      if (['false', '0', 'no', '', (t.no || '').toLocaleLowerCase()].includes(low)) return false;
      return v;
    case 'choice': {
      let options = [];
      try {
        options = JSON.parse(th.dataset.options || '[]');
      } catch {
        options = [];
      }
      const lower = (x) => String(x).toLocaleLowerCase(document.documentElement.lang || undefined);
      return options.find((o) => lower(o.value) === low || lower(o.label) === low)?.value ?? v;
    }
    case 'money':
      return v.replace(/[^\d.,\-]/g, '');
    case 'number':
      return v.replace(/[\s ]/g, '');
    case 'text':
      return raw.replace(/\r$/, '');
    default:
      return v;
  }
}

/** Most rows one paste may change, and how many it may change without asking first. */
const PASTE_MAX = 500;
const PASTE_ASK = 10;

/**
 * Pastes tab separated text into editable cells from the cursor (one value fills the whole
 * range). Each changed row is saved through `POST /_zeyn/grid/edit`, one after the other, with its
 * row version, so the server reads, validates and checks it like an inline edit. Errors show in
 * their cells; a summary toast tells how many rows were saved. More than ten rows ask first.
 */
async function pasteCells(grid, text) {
  const s = grids.get(grid);
  const range = rangeOf(s);
  const table = parseTsv(text);
  if (!s || !range || !table.length) return;
  const t = s.texts;
  const heads = cellsOf(own(grid, 'thead tr:not(.z-band-row)'));
  const one = table.length === 1 && table[0].length === 1;
  const height = one ? range.r1 - range.r0 + 1 : table.length;
  const width = one ? range.c1 - range.c0 + 1 : Math.max(...table.map((r) => r.length));
  const plans = [];
  let editable = 0;
  for (let k = 0, i = range.r0; k < height; i++) {
    const tr = rowAt(grid, i);
    if (!tr) break;
    if (!dataRow(tr) || tr.dataset.zeynKey == null) continue;
    const cells = cellsOf(tr);
    const values = {};
    const before = {};
    for (let j = 0; j < width; j++) {
      const c = range.c0 + j;
      const th = heads[c];
      const raw = one ? table[0][0] : table[k][j];
      if (!th?.dataset.edit || cells[c]?.dataset.v == null || raw == null) continue;
      editable++;
      const value = pastedValue(raw, th, t);
      const old = th.dataset.edit === 'bool' ? cells[c].dataset.v === 'true' : cells[c].dataset.v;
      if (String(value) === String(old)) continue;
      values[th.dataset.field] = value;
      before[th.dataset.field] = old;
    }
    if (Object.keys(values).length) plans.push({ tr, values, before });
    k++;
  }
  if (!editable) return toast(t.paste_readonly || 'These cells cannot be edited.', 'error');
  if (!plans.length) return;
  if (plans.length > PASTE_MAX) return toast(fill(t.paste_too_many || 'Paste into at most {n} rows at a time.', { n: fmt(PASTE_MAX) }), 'error');
  if (plans.length > PASTE_ASK && !confirm(fill(t.paste_confirm || 'Change {n} rows?', { n: fmt(plans.length) }))) return;
  grid.setAttribute('aria-busy', 'true');
  const back = { key: rowAt(grid, s.cursor.row)?.dataset.zeynKey, col: s.cursor.col, anchor: s.anchor, cursor: s.cursor };
  const vf = grid.dataset.zeynVersionField;
  const undo = [];
  let failed = 0;
  for (const p of plans) {
    const tr = p.tr.isConnected || s.paging === 'virtual' ? p.tr : null;
    if (!tr) continue;
    const values = { ...p.values };
    if (vf && tr.dataset.zeynVersion != null) values[vf] = tr.dataset.zeynVersion;
    const r = await post(
      BASE + '/_zeyn/grid/edit',
      { ...address(grid), key: tr.dataset.zeynKey, values, view: viewOf(grid, s) },
      { quiet: true },
    );
    if (r.ok && typeof r.data?.row === 'string') {
      swapRow(grid, tr, r.data.row);
      undo.push({ key: tr.dataset.zeynKey, values: p.before });
      continue;
    }
    failed++;
    const errors = r.status === 422 ? (r.data?.errors ?? []) : Object.keys(p.values).map((field) => ({ field, message: r.data?.message || UNREACHABLE }));
    for (const err of errors) {
      const c = heads.findIndex((th) => th.dataset.field === err.field);
      const td = cellsOf(tr)[c] ?? cellsOf(tr)[leadOf(tr)];
      td.setAttribute('aria-invalid', 'true');
      td.title = err.message;
      td.querySelector('.z-cell-error')?.remove();
      const box = make('span', 'z-cell-error');
      box.textContent = err.message;
      td.append(box);
    }
  }
  grid.removeAttribute('aria-busy');
  const here = back.key != null && grid.querySelector(`:scope > table > [data-zeyn-rows] > tr[data-zeyn-key="${CSS.escape(back.key)}"]`);
  if (here && (!grid.contains(document.activeElement) || document.activeElement === grid)) {
    focusCell(grid, here, back.col);
    Object.assign(s, { anchor: back.anchor, cursor: back.cursor });
    paintRange(grid);
  }
  if (undo.length) {
    s.undo = undo;
    toast(fill(t.pasted || '{n} rows saved.', { n: fmt(undo.length) }), 'info', { label: t.undo || 'Undo', run: () => undoEdit(grid) });
  }
  if (failed) toast(fill(t.paste_failed || '{n} rows could not be saved.', { n: fmt(failed) }), 'error');
  if (s.paging === 'virtual') scheduleVirtual(grid);
}

/**
 * Puts a saved row from the server in place of `tr`, keeping its place in a tree, a virtual
 * block and the column window. Returns the new row.
 */
function swapRow(grid, tr, html) {
  const s = grids.get(grid);
  const t = document.createElement('template');
  t.innerHTML = `<table><tbody>${html}</tbody></table>`;
  const fresh = t.content.querySelector('tr');
  if (!fresh) return null;
  if (tr.dataset.zeynIndex != null) prepRow(s, fresh, Number(tr.dataset.zeynIndex));
  keepBranch(tr, fresh);
  if (tr.isConnected) tr.replaceWith(fresh);
  if (s?.cache) {
    for (const block of s.cache.values()) {
      const i = block.indexOf(tr);
      if (i >= 0) block[i] = fresh;
    }
  }
  relativeTimes(fresh);
  if (s?.colv) applyColumns(grid, true);
  return fresh;
}

/**
 * Fewest columns before only the visible ones (and a few around them) stay in the page. The
 * server's `VIRTUAL_WIDE_COLUMNS` is the same number: a virtual grid this wide comes with every
 * row of its first block in the template, so no row is laid out before the window is set.
 */
const COLV_MIN = 40;
const COLV_BUFFER = 6;

/**
 * Column virtualization for wide grids: with at least 40 columns, every column gets a fixed width
 * from a `colgroup`, and each row keeps in the page only its pinned cells and the cells in view
 * plus up to six on each side; one spacer cell with `colspan` stands for the columns left out on each
 * side. Rows keep their full list of cells (`cellsOf`), so copy, paste and editing see every
 * column. Called on setup and after the header changes.
 */
function initColumns(grid, s) {
  const head = own(grid, 'thead tr:not(.z-band-row)');
  const table = own(grid, 'table');
  if (!head || !table) return;
  s.colv = null;
  table.querySelector(':scope > colgroup')?.remove();
  const cells = cellsOf(head);
  if (cells.length < COLV_MIN) return;
  for (const tr of table.querySelectorAll(':scope > * > tr')) {
    if (tr._zc) tr.replaceChildren(...tr._zc);
    tr._zc = null;
    tr._zw = '';
  }
  const widths = cells.map((c) => parseInt(c.style.width, 10) || c.offsetWidth || 96);
  const x = [0];
  for (const w of widths) x.push(x.at(-1) + w);
  const group = make('colgroup');
  for (const w of widths) group.append(make('col', '', { style: `width:${w}px` }));
  table.prepend(group);
  table.classList.add('z-fixed');
  table.style.width = `${x.at(-1)}px`;
  let pinS = 0;
  while (pinS < cells.length && cells[pinS].dataset.pin === 'start') pinS++;
  let pinE = 0;
  while (pinE < cells.length - pinS && cells[cells.length - 1 - pinE].dataset.pin === 'end') pinE++;
  s.colv = { n: cells.length, widths, x, pinS, pinE, first: -1, last: -1 };
  if (!s.colvBound) {
    s.colvBound = true;
    let raf = 0;
    grid.addEventListener(
      'scroll',
      () => {
        if (!raf) raf = requestAnimationFrame(() => ((raf = 0), applyColumns(grid)));
      },
      { passive: true },
    );
  }
  applyColumns(grid, true, s.view0);
}

/**
 * Sets the window of columns from the horizontal scroll and applies it to the rows of the grid.
 * The window moves only when a column outside it comes into view, then it takes the visible
 * columns and six more on each side, so most scroll frames change nothing. `rows` walks the rows
 * even when the window stays, for rows that just arrived.
 */
function applyColumns(grid, rows = false, geometry = null) {
  const s = grids.get(grid);
  const v = s?.colv;
  if (!v) return;
  const scrolled = geometry ? geometry.left : Math.abs(grid.scrollLeft);
  const left = scrolled + v.x[v.pinS];
  const right = scrolled + (geometry ? geometry.width : grid.clientWidth) - (v.x[v.n] - v.x[v.n - v.pinE]);
  const lastFree = v.n - v.pinE - 1;
  let first = v.pinS;
  while (first < lastFree && v.x[first + 1] <= left) first++;
  let last = first;
  while (last < lastFree && v.x[last + 1] < right) last++;
  const moved = v.first < 0 || first < v.first || last > v.last;
  if (moved) {
    v.first = Math.max(v.pinS, first - COLV_BUFFER);
    v.last = Math.min(lastFree, last + COLV_BUFFER);
  }
  if (!moved && !rows) return;
  const table = own(grid, 'table');
  for (const tr of table.querySelectorAll(':scope > * > tr')) windowRow(tr, v);
}

/** Keeps the pinned cells and the cells of the column window of one row in the page. */
function windowRow(tr, v) {
  const key = `${v.first}:${v.last}`;
  if (tr._zw === key || tr.classList.contains('z-editing')) return;
  const all = tr._zc ?? [...tr.cells];
  if (all.length !== v.n) return;
  if (!tr._zc) all.forEach((c, i) => c.setAttribute('aria-colindex', String(i + 1)));
  tr._zc = all;
  const gap = (side, span) => {
    const cell = (tr[side] ??= make('td', 'z-col-gap', { 'aria-hidden': 'true' }));
    cell.colSpan = span;
    return cell;
  };
  const end = v.n - v.pinE;
  const want = all.slice(0, v.pinS);
  if (v.first > v.pinS) want.push(gap('_zgapA', v.first - v.pinS));
  want.push(...all.slice(v.first, v.last + 1));
  if (v.last < end - 1) want.push(gap('_zgapB', end - 1 - v.last));
  want.push(...all.slice(end));
  tr.replaceChildren(...want);
  tr._zw = key;
}

/** Puts every cell of a row back, for editing it. */
function unwindow(tr) {
  if (!tr._zc) return;
  tr.replaceChildren(...tr._zc);
  tr._zw = 'all';
}

/** Scrolls a virtualized grid sideways so column `col` is in view, and updates the window at once. */
function showColumn(grid, col) {
  const s = grids.get(grid);
  const v = s?.colv;
  if (!v || col < v.pinS || col >= v.n - v.pinE) return;
  const sign = getComputedStyle(grid).direction === 'rtl' ? -1 : 1;
  const left = Math.abs(grid.scrollLeft);
  const startW = v.x[v.pinS];
  const endW = v.x[v.n] - v.x[v.n - v.pinE];
  if (v.x[col] < left + startW) grid.scrollLeft = sign * (v.x[col] - startW);
  else if (v.x[col + 1] > left + grid.clientWidth - endW) grid.scrollLeft = sign * (v.x[col + 1] - grid.clientWidth + endW);
  applyColumns(grid);
}

/** Level of a row in a grouped or tree grid (`data-zeyn-level`); -1 in flat grids. */
function levelOf(tr) {
  return tr?.dataset.zeynLevel != null ? Number(tr.dataset.zeynLevel) : -1;
}

/** Removes the rows under a group or tree row: every following row deeper than it. */
function collapseBranch(tr) {
  const level = levelOf(tr);
  const inside = [];
  for (let n = tr.nextElementSibling; n && levelOf(n) > level; n = n.nextElementSibling) inside.push(n);
  const focused = inside.some((n) => n.contains(document.activeElement));
  for (const n of inside) n.remove();
  if (focused) tr.focus();
}

/**
 * Opens or closes a group row or a tree row (`data-zeyn-path`). Opening loads the first page of
 * its subgroups, rows or children right under it; closing removes them.
 */
async function toggleBranch(tr, open = tr.getAttribute('aria-expanded') !== 'true') {
  const grid = tr.closest('[data-zeyn-grid]');
  tr.setAttribute('aria-expanded', String(open));
  tr.querySelector('[data-zeyn-expand]')?.setAttribute('aria-expanded', String(open));
  collapseBranch(tr);
  if (!open) return;
  let path;
  try {
    path = JSON.parse(tr.dataset.zeynPath);
  } catch {
    return;
  }
  await loadBranch(grid, tr, levelOf(tr) + 1, path, 0);
}

/**
 * Loads one page of a branch with `POST /_zeyn/grid` and `path`, and puts the rows after
 * `anchor`. When more remain, a row with a "Load more" button follows them.
 */
async function loadBranch(grid, anchor, level, path, offset) {
  const s = grids.get(grid);
  if (!s) return;
  anchor.setAttribute('aria-busy', 'true');
  const r = await post(BASE + '/_zeyn/grid', {
    ...address(grid),
    path,
    query: { page: { type: 'offset', offset, limit: s.size }, sort: s.sort, filters: filterList(s), q: s.q, with_total: true, aggregates: [] },
    view: viewOf(grid, s),
  });
  anchor.removeAttribute('aria-busy');
  if (!r.ok || !r.data || !anchor.isConnected) return;
  const t = document.createElement('template');
  t.innerHTML = `<table><tbody>${r.data.rows ?? ''}</tbody></table>`;
  const fresh = [...t.content.querySelectorAll('tbody > tr')];
  const shown = offset + (r.data.count ?? fresh.length);
  if (r.data.total != null && shown < r.data.total) {
    const more = make('tr', 'z-branch-more', { 'data-zeyn-level': String(level), 'aria-level': String(level + 1) });
    more.dataset.zeynPath = JSON.stringify(path);
    more.dataset.zeynOffset = String(shown);
    const cell = make('td');
    cell.colSpan = own(grid, 'thead tr:not(.z-band-row)')?.cells.length ?? 1;
    const button = make('button', 'z-link', { type: 'button', 'data-zeyn-branch-more': '', tabindex: '-1' });
    button.textContent = `${s.texts.load_more || 'Load more'} (${fmt(shown)} / ${fmt(r.data.total)})`;
    cell.append(button);
    more.append(cell);
    fresh.push(more);
  }
  anchor.after(...fresh);
  roving(grid, false);
}

/** Loads the next page of a branch in place of its "Load more" row. */
async function moreBranch(button) {
  const row = button.closest('tr');
  const grid = row.closest('[data-zeyn-grid]');
  const anchor = row.previousElementSibling;
  let path;
  try {
    path = JSON.parse(row.dataset.zeynPath);
  } catch {
    return;
  }
  row.remove();
  await loadBranch(grid, anchor, levelOf(row), path, Number(row.dataset.zeynOffset) || 0);
  const next = anchor.nextElementSibling;
  if (next && !next.classList.contains('z-detail')) activate(next, true);
}

/** Handles arrow keys, Home, End and Enter on a grid row. Returns true when handled. */
function rowKey(e, tr) {
  const list = rows(tr.closest('[data-zeyn-grid]'));
  if (e.key === 'ArrowRight' || e.key === 'ArrowLeft') {
    const rtl = getComputedStyle(tr).direction === 'rtl';
    const open = (e.key === 'ArrowRight') !== rtl;
    const branch = tr.dataset.zeynPath != null || tr.dataset.zeynDetail != null;
    if (branch && open !== (tr.getAttribute('aria-expanded') === 'true')) {
      e.preventDefault();
      if (tr.dataset.zeynPath != null) toggleBranch(tr, open);
      else toggleDetail(tr, open);
      return true;
    }
    if (open && dataRow(tr) && !e.shiftKey) {
      e.preventDefault();
      focusCell(tr.closest('[data-zeyn-grid]'), tr, leadOf(tr));
      return true;
    }
    if (!open && levelOf(tr) > 0) {
      const i = list.indexOf(tr);
      const parent = list.slice(0, i).reverse().find((r) => levelOf(r) < levelOf(tr));
      if (parent) {
        e.preventDefault();
        activate(parent, true);
        return true;
      }
    }
  }
  if (e.key === 'Enter' && tr.classList.contains('z-branch-more')) {
    e.preventDefault();
    tr.querySelector('[data-zeyn-branch-more]')?.click();
    return true;
  }
  const i = list.indexOf(tr);
  const target = { ArrowDown: i + 1, ArrowUp: i - 1, Home: 0, End: list.length - 1 }[e.key];
  if (target !== undefined) {
    e.preventDefault();
    if (list[target]) activate(list[target], true);
    return true;
  }
  const box = tr.querySelector('[data-zeyn-select]');
  if (e.key === ' ' && box) {
    e.preventDefault();
    box.checked = !box.checked;
    pickRow(box, e.shiftKey);
    return true;
  }
  if (e.key === 'Enter' && tr.dataset.zeynEffect) {
    e.preventDefault();
    const effect = parseEffect(tr);
    if (effect) runEffect(effect, tr);
    return true;
  }
  return false;
}

