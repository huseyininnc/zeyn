/** Rows currently in a grid, without the rows that hold an opened detail. */
function rows(grid) {
  return [...(grid.querySelector('[data-zeyn-rows]')?.rows ?? [])].filter((tr) => !tr.classList.contains('z-detail'));
}

/**
 * The first element matching `selector` that belongs to `grid` itself, not to a grid nested in
 * one of its detail rows.
 */
function own(grid, selector) {
  for (const el of grid.querySelectorAll(selector)) if (el.closest('[data-zeyn-grid]') === grid) return el;
  return null;
}

/** Sets up every grid inside `root`. Safe to call more than once. */
function initGrids(root) {
  root.querySelectorAll('[data-zeyn-grid]').forEach(initGrid);
}

/**
 * Sets up one grid: state from the server rendered first page, sort buttons, filter row,
 * infinite scroll observer and roving tabindex. Does not fetch.
 */
function initGrid(grid) {
  if (grids.has(grid)) return;
  const d = grid.dataset;
  const sorted = [...grid.querySelectorAll(':scope > table > thead th[aria-sort="ascending"], :scope > table > thead th[aria-sort="descending"]')];
  const saved = savedView(grid);
  const state = {
    paging: d.zeynPaging || 'pages',
    size: Number(d.zeynPageSize) || 50,
    offset: num(d.zeynOffset) ?? 0,
    total: num(d.zeynTotal),
    next: str(d.zeynNext),
    count: rows(grid).length,
    sort: sorted.length ? sorted.map((th) => ({ field: th.dataset.field, desc: th.getAttribute('aria-sort') === 'descending' })) : defaultSort(grid),
    filters: {},
    texts: textsOf(grid),
    columns: null,
    needHead: false,
    selected: new Set(),
    allMatching: false,
    lastPick: null,
    viewTimer: 0,
    seq: 0,
    timer: 0,
    active: 0,
    loading: false,
    observer: null,
  };
  grids.set(grid, state);
  state.view0 = { left: Math.abs(grid.scrollLeft), width: grid.clientWidth, top: grid.scrollTop, height: grid.clientHeight };
  if (saved.view) {
    state.sort = saved.view.sort?.length ? saved.view.sort : defaultSort(grid);
    for (const f of saved.view.filters ?? []) {
      const { field, ...rest } = f;
      state.filters[field] = rest;
    }
    if (saved.view.columns?.length) state.columns = saved.view.columns;
  }
  setupHead(grid, state);
  initColumns(grid, state);
  if (saved.local) queueMicrotask(() => loadGrid(grid, 'layout'));
  new ResizeObserver(() => {
    const width = grid.clientWidth;
    pinOffsets(grid);
    grid.style.setProperty('--z-view-w', `${width}px`);
  }).observe(grid);

  const waiting = grid.querySelector(':scope > template[data-zeyn-rows-later]');
  if (!rows(grid).length && !waiting && grid.dataset.zeynOffset != null) gridState(grid, 'empty');
  if (state.paging === 'virtual') {
    initVirtual(grid, state);
    state.view0 = null;
    return;
  }
  state.view0 = null;
  const more = own(grid, '[data-zeyn-more]');
  if (state.paging === 'infinite' && more && 'IntersectionObserver' in window) {
    state.observer = new IntersectionObserver((entries) => {
      if (entries.some((e) => e.isIntersecting)) loadGrid(grid, 'more');
    });
  }
  roving(grid, false);
  updateChrome(grid);
}

/**
 * Loads rows from `POST /_zeyn/grid`.
 * @param {Element} grid Grid root.
 * @param {'reset' | 'reload' | 'prev' | 'next' | 'more'} kind `reset` starts over (sort, filter),
 *   `reload` keeps the current page, `more` appends the next infinite batch.
 */
async function loadGrid(grid, kind) {
  if (!grid) return;
  initGrid(grid);
  const s = grids.get(grid);
  if (s.paging === 'virtual') return resetVirtual(grid, s, kind);
  const infinite = s.paging === 'infinite';
  const append = kind === 'more';
  if (append && (s.loading || s.next == null)) return;
  let offset = s.offset;
  if (kind === 'reset' || infinite) offset = 0;
  else if (kind === 'next') offset = s.next != null && /^\d+$/.test(s.next) ? Number(s.next) : s.offset + s.count;
  else if (kind === 'prev') offset = Math.max(0, s.offset - s.size);
  const page = infinite
    ? { type: 'after', cursor: append ? s.next : null, limit: s.size }
    : { type: 'offset', offset, limit: s.size };
  const filters = filterList(s);
  const seq = ++s.seq;
  s.loading = true;
  grid.setAttribute('aria-busy', 'true');
  const layout = kind === 'layout';
  const totals = kind === 'reset' || kind === 'reload' || layout || (infinite && !append);
  const r = await post(BASE + '/_zeyn/grid', {
    ...address(grid),
    query: { page, sort: s.sort, filters, with_total: !infinite, aggregates: totals ? aggregatesOf(grid, layout) : [] },
    view: s.columns ? { columns: s.columns } : null,
    head: layout,
  });
  if (seq !== s.seq) return;
  s.loading = false;
  grid.removeAttribute('aria-busy');
  if (!r.ok || !r.data) {
    if (!append) gridState(grid, 'error');
    return;
  }
  if (typeof r.data.head === 'string') applyHead(grid, s, r.data.head);

  const body = grid.querySelector('[data-zeyn-rows]');
  const hadFocus = body.contains(document.activeElement);
  if (append) body.insertAdjacentHTML('beforeend', r.data.rows ?? '');
  else {
    body.innerHTML = r.data.rows ?? '';
    s.anchor = s.cursor = null;
    if (kind !== 'reload' && !layout) s.active = 0;
  }
  s.offset = offset;
  s.count = rows(grid).length;
  s.total = r.data.total ?? null;
  s.next = r.data.count === 0 && append ? null : (r.data.next ?? null);
  applyFoot(grid, r.data.foot);
  gridState(grid, s.count ? null : 'empty');
  roving(grid, hadFocus);
  updateChrome(grid, append);
}

/**
 * Totals the grid footer shows, as `{field, op}` requests, read from `tfoot td[data-agg]`.
 * The server keeps only the ones the grid declares.
 */
function aggregatesOf(grid, all = false) {
  if (all) return own(grid, 'tfoot') ? allAggregates(grid) : [];
  return [...grid.querySelectorAll(':scope > table > tfoot td[data-agg]')].flatMap((td) =>
    td.dataset.agg.split(/\s+/).filter(Boolean).map((op) => ({ field: td.dataset.field, op })),
  );
}

/** Every total any column may show; the server keeps the ones the grid declares. */
function allAggregates(grid) {
  return columnsOf(grid).flatMap((c) => ['sum', 'avg', 'min', 'max', 'count'].map((op) => ({ field: c.field, op })));
}

/** Replaces the footer row with server rendered totals, when the response has them. */
function applyFoot(grid, html) {
  const foot = own(grid, 'tfoot[data-zeyn-foot]');
  if (foot && typeof html === 'string') foot.innerHTML = html;
}

/**
 * Buttons and links inside a row are reachable with Tab only in the active row, so the grid
 * stays one tab stop while each row's actions can still be used from the keyboard.
 */
function rowControls(tr, on) {
  for (const c of tr.querySelectorAll('button, a[href], input')) c.tabIndex = on ? 0 : -1;
}

/**
 * Shows the empty or error state of a grid under its header, or hides it (`kind` null). The
 * error state has a retry button that reloads the grid.
 */
function gridState(grid, kind) {
  let box = grid.querySelector(':scope > .z-grid-state');
  if (!kind) {
    if (box) box.hidden = true;
    return;
  }
  const t = grids.get(grid)?.texts ?? {};
  if (!box) {
    box = make('div', 'z-grid-state', { role: 'status' });
    grid.querySelector('table')?.after(box);
  }
  box.hidden = false;
  box.dataset.kind = kind;
  box.replaceChildren(document.createTextNode(kind === 'error' ? t.error || 'The rows could not be loaded.' : t.empty || 'No records.'));
  if (kind === 'error') {
    const retry = make('button', 'z-button', { type: 'button' });
    retry.textContent = t.retry || 'Try again';
    retry.addEventListener('click', () => loadGrid(grid, 'reload'));
    box.append(' ', retry);
  }
}

/** Applies the roving tabindex: the active row gets 0, the others -1. */
function roving(grid, focus) {
  const s = grids.get(grid);
  const list = rows(grid);
  s.active = Math.min(s.active, Math.max(0, list.length - 1));
  list.forEach((tr, i) => {
    tr.tabIndex = i === s.active ? 0 : -1;
    rowControls(tr, i === s.active);
  });
  relativeTimes(grid);
  syncSelection(grid);
  if (s.colv) applyColumns(grid, true);
  if (focus) list[s.active]?.focus();
}

/** Makes a row the active one and optionally focuses it. */
function activate(tr, focus) {
  const list = rows(tr.closest('[data-zeyn-grid]'));
  list.forEach((r) => {
    r.tabIndex = r === tr ? 0 : -1;
    rowControls(r, r === tr);
  });
  const s = grids.get(tr.closest('[data-zeyn-grid]'));
  if (s) s.active = tr.dataset.zeynIndex != null ? Number(tr.dataset.zeynIndex) : list.indexOf(tr);
  if (focus) tr.focus();
}

/** Updates pager buttons, page info, the "Load more" button and the scroll observer. */
function updateChrome(grid, announce = false) {
  const s = grids.get(grid);
  const d = grid.dataset;
  const info = own(grid, '[data-zeyn-page-info]');
  if (s.paging === 'pages') {
    const prev = own(grid, '[data-zeyn-page="prev"]');
    const next = own(grid, '[data-zeyn-page="next"]');
    const hasNext = s.total != null ? s.offset + s.count < s.total : s.next != null;
    const focused = document.activeElement;
    if (prev) prev.disabled = s.offset <= 0;
    if (next) next.disabled = !hasNext;
    if (focused === prev && prev.disabled) next?.focus();
    if (focused === next && next.disabled) prev?.focus();
    if (info) {
      const values = { from: fmt(s.count ? s.offset + 1 : 0), to: fmt(s.offset + s.count), total: fmt(s.total ?? 0) };
      const template = s.total != null ? d.zeynTextRangeTotal || '{from}-{to} of {total}' : d.zeynTextRange || '{from}-{to}';
      info.textContent = fill(template, values);
    }
  } else if (s.paging === 'infinite') {
    const more = own(grid, '[data-zeyn-more]');
    const button = more?.querySelector('button');
    if (button) button.hidden = s.next == null;
    if (info && announce) info.textContent = fill(d.zeynTextLoaded || '{count} rows loaded', { count: fmt(s.count) });
    if (s.observer && more) {
      s.observer.unobserve(more);
      if (s.next != null) s.observer.observe(more);
    }
  }
}

/**
 * Sets up a virtual grid (protocol 4.6). The grid root is the scroll container; a spacer
 * `tbody` after the rows gives the table its full height (capped at 8,000,000 px), and the rows
 * `tbody` holds only the visible rows plus 20 above and below, moved with `translateY`.
 * The server rendered first block becomes the first cache entry, with the rows the server put
 * in a `<template data-zeyn-rows-later>` (inert until they scroll into view).
 */
function initVirtual(grid, s) {
  const body = grid.querySelector('[data-zeyn-rows]');
  const head = grid.querySelector('thead');
  const columns = head.rows[0]?.cells.length ?? 1;
  const spacer = document.createElement('tbody');
  spacer.className = 'z-spacer';
  spacer.setAttribute('aria-hidden', 'true');
  spacer.innerHTML = `<tr><td colspan="${columns}"></td></tr>`;
  body.after(spacer);
  const placeholder = document.createElement('tr');
  placeholder.className = 'z-placeholder';
  placeholder.setAttribute('aria-busy', 'true');
  placeholder.innerHTML = '<td></td>'.repeat(columns);
  Object.assign(s, {
    rh: Number(grid.dataset.zeynRowHeight) || 32,
    body,
    head,
    foot: grid.querySelector('tfoot'),
    table: grid.querySelector('table'),
    filler: spacer.rows[0].cells[0],
    placeholder,
    cache: new Map(),
    stale: new Map(),
    inflight: new Map(),
    failed: new Set(),
    holders: new Map(),
    gen: 0,
    version: 0,
    key: '',
    needTotal: s.total == null,
    exact: 0,
    dir: 1,
    view: 0,
    height: 0,
    fillerHeight: -1,
    rowCount: -1,
    raf: 0,
    fetchTimer: 0,
    infoTimer: 0,
  });
  const first = Math.floor(s.offset / s.size);
  const later = grid.querySelector(':scope > template[data-zeyn-rows-later]');
  const rows = [...body.rows, ...(later ? later.content.children : [])];
  later?.remove();
  s.cache.set(first, rows.map((tr, k) => prepRow(s, tr, first * s.size + k)));
  s.total ??= rows.length;
  grid.tabIndex = -1;
  s.headH = 0;
  s.footH = 0;
  grid.addEventListener('scroll', () => scheduleVirtual(grid), { passive: true });
  new ResizeObserver(() => {
    s.remeasure = true;
    scheduleVirtual(grid);
  }).observe(grid);
  renderVirtual(grid, null, s.view0);
}

/**
 * Reads the sticky header and footer heights. Runs at the start of the frame after the grid was
 * set up or resized (and after a new header), before that frame writes anything.
 */
function measureVirtual(s) {
  s.headH = s.head.offsetHeight;
  s.footH = s.foot?.offsetHeight ?? 0;
}

/** Marks a row with its absolute index, `aria-rowindex` and zebra parity. */
function prepRow(s, tr, i) {
  tr.dataset.zeynIndex = i;
  tr.setAttribute('aria-rowindex', s.head.rows.length + i + 1);
  tr.tabIndex = -1;
  tr.classList.toggle('z-alt', i % 2 === 1);
  return tr;
}

/** Schedules one render on the next animation frame. */
function scheduleVirtual(grid) {
  const s = grids.get(grid);
  if (!s.raf) s.raf = requestAnimationFrame(() => renderVirtual(grid));
}

/**
 * Returns the row for index `i` from the cache, the stale cache or a placeholder. Placeholders
 * that left the window are reused from `spare` instead of cloned.
 */
function virtualRow(s, i, spare) {
  const b = Math.floor(i / s.size);
  const tr = (s.cache.get(b) ?? s.stale.get(b))?.[i - b * s.size];
  if (tr) return tr;
  let holder = s.holders.get(i);
  if (!holder) {
    holder = prepRow(s, spare.pop() ?? s.placeholder.cloneNode(true), i);
    s.holders.set(i, holder);
  }
  return holder;
}

/**
 * Renders the visible window. Reads scroll geometry first, then writes; touches the DOM rows
 * only when the window or the loaded blocks changed. Focus stays on the focused row, or moves to
 * `focusIndex` when given.
 */
function renderVirtual(grid, focusIndex = null, geometry = null) {
  const s = grids.get(grid);
  s.raf = 0;
  if (s.remeasure && !geometry) {
    measureVirtual(s);
    s.remeasure = false;
  }
  const top = geometry ? geometry.top : grid.scrollTop;
  const across = s.colv && !geometry ? { left: Math.abs(grid.scrollLeft), width: grid.clientWidth } : geometry;
  const view = Math.max(s.rh, (geometry ? geometry.height : grid.clientHeight) - s.headH - s.footH);
  const total = s.total ?? 0;
  const full = total * s.rh;
  const height = Math.min(full, V_CAP);
  const visible = view / s.rh;
  const range = Math.max(1, height - view);
  const exact = full <= V_CAP ? top / s.rh : (Math.min(top, range) / range) * Math.max(0, total - visible);
  const start = Math.max(0, Math.floor(exact) - V_OVERSCAN);
  const shift = top - (exact - start) * s.rh;
  const fit = Math.max(0, Math.floor((height - shift) / s.rh + 0.01));
  const end = Math.min(total, Math.ceil(exact + visible) + V_OVERSCAN, start + fit);
  if (exact !== s.exact) s.dir = exact > s.exact ? 1 : -1;
  Object.assign(s, { exact, view, height });

  s.body.style.transform = `translateY(${shift}px)`;
  const key = `${start}:${end}:${s.version}:${s.gen}`;
  if (key !== s.key) {
    s.key = key;
    const focused =
      focusIndex ?? (s.body.contains(document.activeElement) ? document.activeElement.dataset.zeynIndex : null);
    const spare = [];
    for (const [i, holder] of s.holders) {
      if (i < start || i >= end) {
        s.holders.delete(i);
        spare.push(holder);
      }
    }
    const nodes = [];
    for (let i = start; i < end; i++) nodes.push(virtualRow(s, i, spare));
    const roving = nodes.find((n) => Number(n.dataset.zeynIndex) === s.active) ?? nodes[0];
    for (const n of nodes) {
      if (n.tabIndex !== (n === roving ? 0 : -1)) {
        n.tabIndex = n === roving ? 0 : -1;
        rowControls(n, n === roving);
      }
    }
    const want = new Set(nodes);
    const active = document.activeElement;
    const kept = s.body.contains(active) && want.has(active);
    if (s.body.contains(active) && !kept) grid.focus({ preventScroll: true });
    for (const n of [...s.body.children]) if (!want.has(n)) n.remove();
    let ref = s.body.firstChild;
    for (const n of nodes) {
      if (n === ref) ref = ref.nextSibling;
      else s.body.insertBefore(n, ref);
    }
    syncSelection(grid);
    if (s.colv) applyColumns(grid, true, across);
    const filler = Math.max(0, height - (end - start) * s.rh);
    if (filler !== s.fillerHeight) s.filler.style.height = `${(s.fillerHeight = filler)}px`;
    const target = focused != null && s.body.querySelector(`[data-zeyn-index="${focused}"]`);
    if (focused != null && !(kept && target === active)) (target || grid).focus({ preventScroll: true });
    const rowCount = s.head.rows.length + total;
    if (rowCount !== s.rowCount) s.table.setAttribute('aria-rowcount', (s.rowCount = rowCount));
    for (let b = Math.floor(start / s.size); b * s.size < end; b++) {
      const block = s.cache.get(b);
      if (block) {
        s.cache.delete(b);
        s.cache.set(b, block);
      }
    }
  }

  const blocks = new Set();
  for (let b = Math.floor(start / s.size); b * s.size < Math.max(end, start + 1); b++) blocks.add(b);
  const shown = [...blocks].every((b) => s.cache.has(b));
  const ahead = s.dir > 0 ? Math.max(...blocks) + 1 : Math.min(...blocks) - 1;
  const near = s.dir > 0 ? ahead * s.size - end : start - (ahead + 1) * s.size;
  if (shown && ahead >= 0 && ahead * s.size < total && near < s.size / 2) blocks.add(ahead);
  for (const [b, controller] of s.inflight) {
    if (!blocks.has(b)) {
      controller.abort();
      s.inflight.delete(b);
    }
  }
  clearTimeout(s.fetchTimer);
  s.fetchTimer = setTimeout(() => {
    for (const b of blocks) if (!s.cache.has(b) && !s.inflight.has(b) && !s.failed.has(b)) fetchBlock(grid, s, b);
  }, V_FETCH_DELAY_MS);

  clearTimeout(s.infoTimer);
  s.infoTimer = setTimeout(() => {
    const info = grid.querySelector('[data-zeyn-page-info]');
    if (!info) return;
    const from = Math.min(total, Math.floor(exact) + 1);
    const to = Math.min(total, Math.floor(exact + visible));
    info.textContent = fill(grid.dataset.zeynTextRangeTotal || '{from}-{to} of {total}', {
      from: fmt(from),
      to: fmt(to),
      total: fmt(total),
    });
  }, 400);
}

/** Fetches one block by offset; stale or aborted answers are dropped. */
async function fetchBlock(grid, s, b) {
  const controller = new AbortController();
  s.inflight.set(b, controller);
  const gen = s.gen;
  const withTotal = s.needTotal;
  const head = s.needHead;
  s.needTotal = false;
  s.needHead = false;
  const filters = filterList(s);
  const page = { type: 'offset', offset: b * s.size, limit: s.size };
  const r = await post(
    BASE + '/_zeyn/grid',
    {
      ...address(grid),
      query: { page, sort: s.sort, filters, with_total: withTotal, aggregates: withTotal ? aggregatesOf(grid, head) : [] },
      view: s.columns ? { columns: s.columns } : null,
      head,
    },
    { signal: controller.signal },
  );
  if (s.inflight.get(b) === controller) s.inflight.delete(b);
  if (gen === s.gen && (controller.signal.aborted || !r.ok || !r.data || (head && typeof r.data.head !== 'string'))) {
    s.needTotal ||= withTotal;
    s.needHead ||= head;
  }
  if (gen !== s.gen || controller.signal.aborted) return;
  if (!r.ok || !r.data) {
    s.failed.add(b);
    if (!s.cache.size) gridState(grid, 'error');
    return;
  }
  if (head && typeof r.data.head === 'string') {
    applyHead(grid, s, r.data.head);
  }
  const t = document.createElement('template');
  t.innerHTML = `<table><tbody>${r.data.rows ?? ''}</tbody></table>`;
  s.cache.set(b, [...t.content.querySelectorAll('tr')].map((tr, k) => prepRow(s, tr, b * s.size + k)));
  if (s.colv) for (const tr of s.cache.get(b)) windowRow(tr, s.colv);
  relativeTimes(t.content);
  s.stale.delete(b);
  while (s.cache.size > V_CACHE) s.cache.delete(s.cache.keys().next().value);
  applyFoot(grid, r.data.foot);
  if (withTotal && r.data.total == null) s.needTotal = true;
  if (withTotal && r.data.total != null) {
    s.total = r.data.total;
    gridState(grid, s.total ? null : 'empty');
  }
  s.version++;
  scheduleVirtual(grid);
}

/**
 * Drops the cache of a virtual grid. Sort and filter changes scroll to the top; `reload`
 * keeps the position and shows the old rows until fresh blocks arrive.
 */
function resetVirtual(grid, s, kind) {
  for (const controller of s.inflight.values()) controller.abort();
  s.inflight.clear();
  s.failed.clear();
  s.holders.clear();
  s.stale = kind === 'reload' ? new Map([...s.stale, ...s.cache]) : new Map();
  s.cache = new Map();
  s.gen++;
  s.needTotal = true;
  if (kind === 'layout') s.needHead = true;
  if (kind !== 'reload' && kind !== 'layout') {
    s.active = 0;
    grid.scrollTop = 0;
  }
  renderVirtual(grid);
}

/** Handles Arrow, Page Up, Page Down, Home and End in a virtual grid. Returns true when handled. */
function virtualKey(e, grid) {
  const s = grids.get(grid);
  const total = s?.total ?? 0;
  if (!total) return false;
  const page = Math.max(1, Math.floor(s.view / s.rh) - 1);
  const step = { ArrowDown: 1, ArrowUp: -1, PageDown: page, PageUp: -page }[e.key];
  let i;
  if (step !== undefined) i = s.active + step;
  else if (e.key === 'Home') i = 0;
  else if (e.key === 'End') i = total - 1;
  else return false;
  e.preventDefault();
  renderVirtual(grid);
  s.active = Math.max(0, Math.min(total - 1, i));
  const visible = s.view / s.rh;
  let target = null;
  if (s.active < s.exact) target = s.active;
  else if (s.active + 1 > s.exact + visible) target = s.active + 1 - visible;
  if (target !== null) {
    const full = total * s.rh;
    const round = target === s.active ? Math.floor : Math.ceil;
    grid.scrollTop =
      full <= V_CAP ? target * s.rh : round((target / Math.max(1, total - visible)) * Math.max(1, s.height - s.view));
  }
  s.key = '';
  renderVirtual(grid, s.active);
  return true;
}

