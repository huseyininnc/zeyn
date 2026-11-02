/**
 * Zeyn browser runtime.
 *
 * Implements sections 3, 4 and 6 of `docs/runtime-protocol.md`: effects, windows, grids,
 * forms, keyboard shortcuts, export and language selection, plus optional WebMCP tools.
 * Interaction is wired with event delegation on `document`, so screens loaded into dialogs
 * work without rebinding. Only grids need per element setup, done by `initGrids`.
 * @module zeyn
 */

/** Path the app is served under, from `meta[name=zeyn-base]`; empty when served at the root. */
const BASE = (document.querySelector('meta[name="zeyn-base"]')?.content ?? '').replace(/\/$/, '');
const TOAST_MS = 4000;
const FILTER_DELAY_MS = 300;
const FOCUSABLE =
  'a[href], button:not([disabled]), input:not([type="hidden"]):not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])';
const UNREACHABLE = 'The server could not be reached.';
const V_CAP = 8_000_000;
const V_OVERSCAN = 20;
const V_CACHE = 20;
const V_FETCH_DELAY_MS = 40;
const VIEW_SAVE_DELAY_MS = 500;
const MIN_COLUMN = 48;
const MAX_COLUMN = 1200;
const MAC = /mac|iphone|ipad/i.test(navigator.userAgentData?.platform ?? navigator.platform ?? '');

/** @type {WeakMap<Element, object>} Per grid root state. */
const grids = new WeakMap();
/** @type {{dialog: HTMLDialogElement, opener: Element | null}[]} Open windows, top last. */
const dialogs = [];
let uid = 0;

/** Returns the CSRF token from `meta[name=zeyn-csrf]`. */
function csrf() {
  return document.querySelector('meta[name="zeyn-csrf"]')?.content ?? '';
}

/**
 * Parses a JSON response body; returns `null` for empty or invalid bodies.
 * @param {Response} res
 */
async function readJson(res) {
  if (res.status === 204) return null;
  try {
    return await res.json();
  } catch {
    return null;
  }
}

/**
 * Sends a JSON `POST` with the CSRF header. Error responses show their `message` in an error
 * toast, except `422` (handled by the caller) and when `quiet` is set.
 * @param {string} url
 * @param {object} body
 * @param {{quiet?: boolean, signal?: AbortSignal}} [options] An aborted request resolves
 *   silently with status 0.
 * @returns {Promise<{status: number, ok: boolean, data: any}>}
 */
async function post(url, body, { quiet = false, signal } = {}) {
  let res, data;
  try {
    res = await fetch(url, {
      method: 'POST',
      credentials: 'same-origin',
      headers: { 'content-type': 'application/json', 'x-zeyn-csrf': csrf() },
      body: JSON.stringify(body),
      signal,
    });
    data = await readJson(res);
  } catch (err) {
    if (!quiet && err?.name !== 'AbortError') toast(UNREACHABLE, 'error');
    return { status: 0, ok: false, data: null };
  }
  if (!res.ok && res.status !== 422 && !quiet) toast(data?.message || `Request failed (${res.status}).`, 'error');
  return { status: res.status, ok: res.ok, data };
}

/** Builds the `?params=` query string; empty when the screen has no params. */
function paramsQuery(params) {
  return params == null ? '' : `?params=${encodeURIComponent(JSON.stringify(params))}`;
}

/** Returns the closest screen root of an element. */
function screenOf(el) {
  return el?.closest('[data-zeyn-screen]') ?? null;
}

/** Parses `data-zeyn-params` of a screen root. */
function paramsOf(screen) {
  const raw = screen?.dataset.zeynParams;
  if (!raw) return null;
  try {
    return JSON.parse(raw);
  } catch {
    return null;
  }
}

/** Returns the node index of the closest `[data-zeyn-node]`, or `null`. */
function nodeOf(el) {
  const node = el.closest('[data-zeyn-node]');
  return node ? Number(node.dataset.zeynNode) : null;
}

/** Common request fields: screen name, params and node of an element. */
function address(el) {
  const screen = screenOf(el);
  return { screen: screen?.dataset.zeynScreen ?? null, params: paramsOf(screen), node: nodeOf(el) };
}

/** Reads a numeric data attribute; absent, empty or `null` gives `null`. */
function num(value) {
  return value == null || value === '' || value === 'null' ? null : Number(value);
}

/** Reads a string data attribute; absent, empty or `null` gives `null`. */
function str(value) {
  return value == null || value === '' || value === 'null' ? null : value;
}

/** Formats a number for the page language. */
function fmt(n) {
  return new Intl.NumberFormat(document.documentElement.lang || undefined).format(n);
}

/** Replaces `{name}` placeholders in a localized template. */
function fill(template, values) {
  return template.replace(/\{(\w+)\}/g, (m, k) => (k in values ? String(values[k]) : m));
}

/** Returns the toast region of the top dialog, or of the page, creating it when missing. */
function toastRegion() {
  const host = dialogs.at(-1)?.dialog ?? document.body;
  let region = [...host.querySelectorAll('.z-toasts')].find((r) => r.closest('dialog') === (host === document.body ? null : host));
  if (!region) {
    region = document.createElement('div');
    region.className = 'z-toasts';
    region.setAttribute('role', 'status');
    region.setAttribute('aria-live', 'polite');
    host.append(region);
  }
  return region;
}

/**
 * Shows a toast for four seconds. The timer pauses while the pointer is over the toast or it
 * holds focus (WCAG 2.2.1).
 * @param {string} message
 * @param {'info' | 'error'} [variant]
 */
function toast(message, variant = 'info') {
  const el = document.createElement('div');
  el.className = `z-toast z-toast-${variant}`;
  el.textContent = message;
  let left = TOAST_MS;
  let started = 0;
  let timer = 0;
  const run = () => {
    if (timer || el.matches(':hover, :focus-within')) return;
    started = Date.now();
    timer = setTimeout(() => el.remove(), left);
  };
  const pause = () => {
    if (!timer) return;
    clearTimeout(timer);
    timer = 0;
    left = Math.max(1000, left - (Date.now() - started));
  };
  el.addEventListener('pointerenter', pause);
  el.addEventListener('focusin', pause);
  el.addEventListener('pointerleave', () => setTimeout(run));
  el.addEventListener('focusout', () => setTimeout(run));
  toastRegion().append(el);
  run();
}

/** Parses the JSON effect of an element. */
function parseEffect(el) {
  try {
    return JSON.parse(el.dataset.zeynEffect);
  } catch {
    return null;
  }
}

/**
 * Runs one effect.
 * @param {{effect: string, screen?: string, params?: any, message?: string}} effect
 * @param {Element} [source] Element that triggered it; focus returns there when a window closes.
 */
async function runEffect(effect, source) {
  switch (effect?.effect) {
    case 'open_window':
      return openWindow(effect.screen, effect.params, source);
    case 'navigate':
      location.assign(`${BASE}/_zeyn/route/${encodeURIComponent(effect.screen)}${paramsQuery(effect.params)}`);
      return;
    case 'toast':
      return toast(effect.message ?? '');
    case 'close_and_refresh':
      return closeAndRefresh();
  }
}

/** Runs effects in order. */
async function runEffects(effects, source) {
  for (const effect of effects ?? []) await runEffect(effect, source);
}

/**
 * Loads a screen fragment into a modal `<dialog>` on top of the stack.
 * The native dialog carries the dialog role, so the role is removed from the inner window.
 */
async function openWindow(screen, params, opener) {
  let res;
  try {
    res = await fetch(`${BASE}/_zeyn/screen/${encodeURIComponent(screen)}${paramsQuery(params)}`, {
      credentials: 'same-origin',
      headers: { accept: 'text/html' },
    });
  } catch {
    return toast(UNREACHABLE, 'error');
  }
  if (!res.ok) {
    const data = await readJson(res);
    return toast(data?.message || `Request failed (${res.status}).`, 'error');
  }
  const dialog = document.createElement('dialog');
  dialog.className = 'z-dialog';
  dialog.innerHTML = await res.text();
  const win = dialog.querySelector('[data-zeyn-screen]');
  if (win) {
    win.removeAttribute('role');
    win.removeAttribute('aria-modal');
    win.tabIndex = -1;
    if (!win.hasAttribute('data-zeyn-params')) win.dataset.zeynParams = JSON.stringify(params ?? null);
    const label = win.getAttribute('aria-labelledby');
    if (label) dialog.setAttribute('aria-labelledby', label);
  }
  const close = document.createElement('button');
  close.type = 'button';
  close.className = 'z-close';
  close.dataset.zeynClose = '';
  close.setAttribute('aria-label', 'Close');
  close.textContent = '×';
  dialog.append(close);
  document.body.append(dialog);
  dialogs.push({ dialog, opener: opener ?? document.activeElement });
  dialog.addEventListener('close', () => finishDialog(dialog));
  dialog.showModal();
  initGrids(dialog);
  (win?.querySelector(FOCUSABLE) ?? win ?? close).focus();
  syncAgentTools();
}

/**
 * Removes a dialog from the stack right away, closes it and returns focus to its opener.
 * Runs again from the asynchronous `close` event, where it does nothing.
 */
function finishDialog(dialog) {
  const i = dialogs.findIndex((d) => d.dialog === dialog);
  if (i < 0) return;
  const [entry] = dialogs.splice(i, 1);
  if (dialog.open) dialog.close();
  dialog.remove();
  if (entry.opener?.isConnected) entry.opener.focus();
  syncAgentTools();
}

/** Closes the top dialog and reloads every grid of the screen under it. */
async function closeAndRefresh() {
  const under = dialogs.at(-2)?.dialog ?? document.querySelector('.z-main') ?? document.body;
  const top = dialogs.at(-1);
  if (top) finishDialog(top.dialog);
  await Promise.all([...under.querySelectorAll('[data-zeyn-grid]')].map((g) => loadGrid(g, 'reload')));
}

/** Rows currently in a grid. */
function rows(grid) {
  return [...(grid.querySelector('[data-zeyn-rows]')?.rows ?? [])];
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
  const sorted = [...grid.querySelectorAll('thead th[aria-sort="ascending"], thead th[aria-sort="descending"]')];
  const saved = savedView(grid);
  const state = {
    paging: d.zeynPaging || 'pages',
    size: Number(d.zeynPageSize) || 50,
    offset: num(d.zeynOffset) ?? 0,
    total: num(d.zeynTotal),
    next: str(d.zeynNext),
    count: rows(grid).length,
    sort: sorted.map((th) => ({ field: th.dataset.field, desc: th.getAttribute('aria-sort') === 'descending' })),
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
  if (saved.view) {
    state.sort = saved.view.sort ?? [];
    for (const f of saved.view.filters ?? []) {
      const { field, ...rest } = f;
      state.filters[field] = rest;
    }
    if (saved.view.columns?.length) state.columns = saved.view.columns;
  }
  setupHead(grid, state);
  if (saved.local) queueMicrotask(() => loadGrid(grid, 'layout'));
  new ResizeObserver(() => {
    grid.style.setProperty('--z-view-w', `${grid.clientWidth}px`);
    pinOffsets(grid);
  }).observe(grid);

  if (state.paging === 'virtual') return initVirtual(grid, state);
  const more = grid.querySelector('[data-zeyn-more]');
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
  if (!r.ok || !r.data) return;
  if (typeof r.data.head === 'string') applyHead(grid, s, r.data.head);

  const body = grid.querySelector('[data-zeyn-rows]');
  const hadFocus = body.contains(document.activeElement);
  if (append) body.insertAdjacentHTML('beforeend', r.data.rows ?? '');
  else {
    body.innerHTML = r.data.rows ?? '';
    if (kind !== 'reload' && !layout) s.active = 0;
  }
  s.offset = offset;
  s.count = body.rows.length;
  s.total = r.data.total ?? null;
  s.next = r.data.count === 0 && append ? null : (r.data.next ?? null);
  applyFoot(grid, r.data.foot);
  roving(grid, hadFocus);
  updateChrome(grid, append);
}

/**
 * Totals the grid footer shows, as `{field, op}` requests, read from `tfoot td[data-agg]`.
 * The server keeps only the ones the grid declares.
 */
function aggregatesOf(grid, all = false) {
  if (all) return grid.querySelector('tfoot') ? allAggregates(grid) : [];
  return [...grid.querySelectorAll('tfoot td[data-agg]')].flatMap((td) =>
    td.dataset.agg.split(/\s+/).filter(Boolean).map((op) => ({ field: td.dataset.field, op })),
  );
}

/** Every total any column may show; the server keeps the ones the grid declares. */
function allAggregates(grid) {
  return columnsOf(grid).flatMap((c) => ['sum', 'avg', 'min', 'max', 'count'].map((op) => ({ field: c.field, op })));
}

/** Replaces the footer row with server rendered totals, when the response has them. */
function applyFoot(grid, html) {
  const foot = grid.querySelector('tfoot[data-zeyn-foot]');
  if (foot && typeof html === 'string') foot.innerHTML = html;
}

/**
 * Buttons and links inside a row are reachable with Tab only in the active row, so the grid
 * stays one tab stop while each row's actions can still be used from the keyboard.
 */
function rowControls(tr, on) {
  for (const c of tr.querySelectorAll('button, a[href], input')) c.tabIndex = on ? 0 : -1;
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
  if (focus) list[s.active]?.focus();
}

/** Makes a row the active one and optionally focuses it. */
function activate(tr, focus) {
  const list = [...tr.parentElement.rows];
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
  const info = grid.querySelector('[data-zeyn-page-info]');
  if (s.paging === 'pages') {
    const prev = grid.querySelector('[data-zeyn-page="prev"]');
    const next = grid.querySelector('[data-zeyn-page="next"]');
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
    const more = grid.querySelector('[data-zeyn-more]');
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
 * The server rendered first block becomes the first cache entry.
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
  s.cache.set(first, [...body.rows].map((tr, k) => prepRow(s, tr, first * s.size + k)));
  s.total ??= body.rows.length;
  grid.tabIndex = -1;
  measureVirtual(s);
  grid.addEventListener('scroll', () => scheduleVirtual(grid), { passive: true });
  new ResizeObserver(() => {
    measureVirtual(s);
    scheduleVirtual(grid);
  }).observe(grid);
  renderVirtual(grid);
}

/** Reads the sticky header and footer heights; called on setup and on resize only. */
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
function renderVirtual(grid, focusIndex = null) {
  const s = grids.get(grid);
  s.raf = 0;
  const top = grid.scrollTop;
  const view = Math.max(s.rh, grid.clientHeight - s.headH - s.footH);
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
  const ahead = s.dir > 0 ? Math.max(...blocks) + 1 : Math.min(...blocks) - 1;
  if (ahead >= 0 && ahead * s.size < total) blocks.add(ahead);
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
  if (gen !== s.gen || controller.signal.aborted) return;
  if (!r.ok || !r.data) {
    s.failed.add(b);
    return;
  }
  if (head && typeof r.data.head === 'string') {
    s.needHead = false;
    applyHead(grid, s, r.data.head);
  }
  const t = document.createElement('template');
  t.innerHTML = `<table><tbody>${r.data.rows ?? ''}</tbody></table>`;
  s.cache.set(b, [...t.content.querySelectorAll('tr')].map((tr, k) => prepRow(s, tr, b * s.size + k)));
  relativeTimes(t.content);
  s.stale.delete(b);
  while (s.cache.size > V_CACHE) s.cache.delete(s.cache.keys().next().value);
  applyFoot(grid, r.data.foot);
  if (withTotal && r.data.total != null) {
    s.total = r.data.total;
    s.needTotal = false;
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

/** Relative time formatter for the page language, made once. */
let relativeFormat = null;

/**
 * Shows `time[data-zeyn-relative]` as relative time ("3 minutes ago") in the page language. The
 * server rendered exact time stays in the `title`. Called when rows arrive and once a minute.
 */
function relativeTimes(root) {
  const list = root.querySelectorAll('time[data-zeyn-relative]');
  if (!list.length || typeof Intl.RelativeTimeFormat !== 'function') return;
  relativeFormat ??= new Intl.RelativeTimeFormat(document.documentElement.lang || undefined, { numeric: 'auto' });
  const now = Date.now();
  const units = [
    ['year', 31536000],
    ['month', 2592000],
    ['week', 604800],
    ['day', 86400],
    ['hour', 3600],
    ['minute', 60],
  ];
  for (const el of list) {
    const at = Date.parse(el.getAttribute('datetime'));
    if (Number.isNaN(at)) continue;
    const secs = (at - now) / 1000;
    const [unit, size] = units.find(([, n]) => Math.abs(secs) >= n) ?? ['second', 1];
    el.textContent = relativeFormat.format(Math.round(secs / size), unit);
  }
}

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
 * Runs a bulk action with `POST /_zeyn/grid/bulk`: the picked keys, or, when every matching row
 * is selected, the filters so the server collects the keys itself.
 */
async function runBulk(button) {
  const grid = gridOfBar(button);
  const s = grid && grids.get(grid);
  if (!s) return;
  button.disabled = true;
  const r = await post(BASE + '/_zeyn/grid/bulk', {
    ...address(grid),
    action: Number(button.dataset.zeynBulkRun),
    keys: s.allMatching ? [] : [...s.selected],
    all: s.allMatching,
    query: { sort: s.sort, filters: filterList(s) },
  });
  button.disabled = false;
  if (!r.ok) return;
  await runEffects(r.data?.effects, button);
  toast(s.texts.done || 'Done.');
  clearSelection(grid);
  await loadGrid(grid, 'reload');
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
  if (button?.tagName === 'BUTTON') button.disabled = true;
  const r = await post(BASE + '/_zeyn/grid/run', { ...address(grid), key, item: item ?? button.dataset.zeynRun });
  if (button?.isConnected && button.tagName === 'BUTTON') button.disabled = false;
  if (!r.ok) return;
  await runEffects(r.data?.effects, button);
  toast(grids.get(grid)?.texts.done || 'Done.');
  await loadGrid(grid, 'reload');
  const again = grid.querySelector(`[data-zeyn-rows] tr[data-zeyn-key="${CSS.escape(key)}"]`);
  if (again && (!document.activeElement || document.activeElement === document.body)) {
    activate(again, false);
    (again.querySelector(`[data-zeyn-run="${CSS.escape(item ?? button.dataset.zeynRun)}"]`) ?? again).focus();
  }
}

/**
 * Cycles the sort of a column: none, ascending, descending. A plain click sorts by this column
 * only; Shift+click adds it to the sort keys (or changes or removes it there), up to four keys.
 */
function toggleSort(button, additive) {
  const th = button.closest('th');
  const grid = th.closest('[data-zeyn-grid]');
  const s = grids.get(grid);
  if (!s) return;
  const field = th.dataset.field;
  const current = s.sort.find((k) => k.field === field);
  const next = !current ? { field, desc: false } : !current.desc ? { field, desc: true } : null;
  if (additive) {
    const rest = s.sort.filter((k) => k.field !== field);
    s.sort = next ? (current ? s.sort.map((k) => (k.field === field ? next : k)) : [...rest, next].slice(-4)) : rest;
  } else s.sort = next ? [next] : [];
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
  s.sort = [];
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
    });
    const inner = make('div', 'z-th');
    inner.append(sort, menu);
    th.append(inner, resize);
    if (th.style.width) th.dataset.userWidth = String(parseInt(th.style.width, 10));
  }
  if (grid.querySelector('thead th[data-filter]')) {
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
      run: () => grid.querySelector(`.z-filter-cell[data-field="${CSS.escape(field)}"] .z-filter`)?.focus(),
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
  for (const th of grid.querySelectorAll('thead th[data-field]')) {
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

/** Creates an element with a class and optional attributes. */
function make(tag, className, attrs = {}) {
  const node = document.createElement(tag);
  if (className) node.className = className;
  for (const [k, v] of Object.entries(attrs)) node.setAttribute(k, v);
  return node;
}

/**
 * Builds the filter controls of one column in its filter row cell, by `th[data-filter]`:
 * `text` (match kind and text), `number`, `money`, `date`, `datetime`, `time` (from and to),
 * `choice` (a button opening a checkbox list) and `bool` (all, yes, no).
 */
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
    const op = make('select', 'z-filter z-filter-op', { 'aria-label': `${name}: ${t.match || 'Match'}` });
    op.dataset.bound = 'op';
    for (const k of ['contains', 'starts_with', 'equals']) op.append(new Option(t[k] || k, k));
    const text = make('input', 'z-filter', { type: 'search', autocomplete: 'off', 'aria-label': name });
    text.id = `z-filter-${++uid}`;
    text.dataset.bound = 'text';
    wrap.append(op, text);
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
    button.textContent = t.all || 'All';
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
    const select = make('select', 'z-filter', { 'aria-label': name });
    select.dataset.bound = 'is';
    select.append(new Option(t.all || 'All', ''), new Option(t.yes || 'Yes', 'true'), new Option(t.no || 'No', 'false'));
    cell.append(select);
  } else {
    const type = { number: 'number', money: 'number', date: 'date', datetime: 'date', time: 'time' }[kind] ?? 'search';
    const wrap = make('div', 'z-filter-pair z-filter-range');
    wrap.append(input(type, 'from', t.from || 'From'), input(type, 'to', t.to || 'To'));
    cell.append(wrap);
  }
}

/** Places a popover under its button, inside the viewport, at the inline start edge. */
function place(panel, button) {
  const r = button.getBoundingClientRect();
  const rtl = getComputedStyle(button).direction === 'rtl';
  const width = panel.offsetWidth;
  const left = rtl ? r.right - width : r.left;
  panel.style.left = `${Math.max(4, Math.min(left, innerWidth - width - 4))}px`;
  const below = r.bottom + 2;
  panel.style.top = `${below + panel.offsetHeight > innerHeight ? Math.max(4, r.top - panel.offsetHeight - 2) : below}px`;
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
      for (const b of ['from', 'to']) if (get(b)?.value) f[b] = get(b).value;
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
}

/** Active filters as the query sends them. */
function filterList(s) {
  return Object.entries(s.filters)
    .filter(([, f]) => f)
    .map(([field, f]) => ({ field, ...f }));
}

/** Debounces a filter control and reloads the grid from the start. */
function onFilter(control) {
  const cell = control.closest('.z-filter-cell');
  const grid = cell?.closest('[data-zeyn-grid]');
  const s = grid && grids.get(grid);
  if (!s) return;
  if (cell.dataset.kind === 'choice') choiceLabel(cell, s.texts);
  clearTimeout(s.timer);
  s.timer = setTimeout(() => {
    s.filters[cell.dataset.field] = readFilter(cell);
    s.allMatching = false;
    saveView(grid);
    loadGrid(grid, 'reset');
  }, FILTER_DELAY_MS);
}

/** Handles arrow keys, Home, End and Enter on a grid row. Returns true when handled. */
function rowKey(e, tr) {
  const list = [...tr.parentElement.rows];
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

/** True when the event target is a place where the user types text. */
function typing(target) {
  if (!(target instanceof Element)) return false;
  if (target.isContentEditable || target.closest('textarea, select')) return true;
  return target.matches('input') && !/^(checkbox|radio|button|submit|reset)$/.test(target.type);
}

/**
 * True when a key event matches one `aria-keyshortcuts` entry such as `Control+N`.
 * On macOS, Command (Meta) is accepted where Control is declared.
 */
function matchesShortcut(e, combo) {
  const parts = combo.split('+');
  const key = parts.pop().toLowerCase();
  const mods = new Set(parts.map((p) => p.toLowerCase()));
  const swap = MAC && mods.has('control') && !mods.has('meta');
  const ctrlMeta = swap
    ? e.ctrlKey !== e.metaKey
    : e.ctrlKey === mods.has('control') && e.metaKey === mods.has('meta');
  return e.key.toLowerCase() === key && ctrlMeta && e.shiftKey === mods.has('shift') && e.altKey === mods.has('alt');
}

/** Triggers the element whose `aria-keyshortcuts` matches, searching only the top dialog when one is open. */
function shortcut(e) {
  if (e.defaultPrevented || e.repeat || typing(e.target)) return;
  const scope = dialogs.at(-1)?.dialog ?? document;
  for (const el of scope.querySelectorAll('[aria-keyshortcuts]')) {
    if (el.disabled || (scope === document && el.closest('dialog'))) continue;
    if (el.getAttribute('aria-keyshortcuts').split(/\s+/).some((k) => k && matchesShortcut(e, k))) {
      e.preventDefault();
      el.click();
      return;
    }
  }
}

/**
 * Collects form values: booleans for checkboxes, strings otherwise. Skips read only inputs and
 * hidden inputs, except row versions (`data-zeyn-version`) used for conflict detection.
 */
function formValues(form) {
  const values = {};
  for (const el of form.elements) {
    if (!el.name || el.disabled || el.readOnly) continue;
    if (el.type === 'hidden' && !el.hasAttribute('data-zeyn-version')) continue;
    if (el.matches('button, fieldset, output, [type="submit"], [type="button"], [type="reset"]')) continue;
    if (el.type === 'checkbox') values[el.name] = el.checked;
    else if (el.type === 'radio') {
      if (el.checked) values[el.name] = el.value;
    } else values[el.name] = el.value;
  }
  return values;
}

/** Removes validation messages and `aria-invalid` from a form. */
function clearErrors(form) {
  form.querySelectorAll('.z-field-error').forEach((p) => (p.textContent = ''));
  form.querySelectorAll('[aria-invalid]').forEach((el) => el.removeAttribute('aria-invalid'));
  form.querySelector('.z-form-errors')?.remove();
}

/** Shows `422` errors under their fields, general errors on top, and focuses the first one. */
function showErrors(form, errors) {
  const scope = screenOf(form)?.dataset.zeynScreen;
  const general = [];
  let first = null;
  for (const err of errors) {
    const named = err.field ? form.elements.namedItem(err.field) : null;
    const input = named instanceof RadioNodeList ? named[0] : named;
    const id = input?.id ? `${input.id}-error` : `${scope}-${err.field}-error`;
    const box = input && form.querySelector(`#${CSS.escape(id)}`);
    if (!box) {
      general.push(err.message);
      continue;
    }
    box.textContent = box.textContent ? `${box.textContent} ${err.message}` : err.message;
    input.setAttribute('aria-invalid', 'true');
    first ??= input;
  }
  if (general.length) {
    const box = document.createElement('div');
    box.className = 'z-form-errors';
    box.setAttribute('role', 'alert');
    box.tabIndex = -1;
    for (const message of general) {
      const p = document.createElement('p');
      p.textContent = message;
      box.append(p);
    }
    form.prepend(box);
    first ??= box;
  }
  first?.focus();
}

/** Validates in the browser, posts to `/_zeyn/form`, then shows errors or runs effects. */
async function submitForm(form, submitter) {
  if (form.getAttribute('aria-busy') === 'true') return;
  clearErrors(form);
  if (!form.checkValidity()) {
    form.reportValidity();
    return;
  }
  const button = submitter ?? form.querySelector('[type="submit"]');
  if (button) button.disabled = true;
  form.setAttribute('aria-busy', 'true');
  try {
    const r = await post(BASE + '/_zeyn/form', { ...address(form), values: formValues(form) });
    if (r.status === 422) showErrors(form, r.data?.errors ?? []);
    else if (r.ok) await runEffects(r.data?.effects, form);
  } finally {
    if (button) button.disabled = false;
    form.removeAttribute('aria-busy');
  }
}

/** Quotes one CSV cell (RFC 4180) and defuses spreadsheet formulas in non numeric text. */
function csvCell(value) {
  let v = value;
  if (/^[=+\-@\t\r]/.test(v) && !/^[+-]?[\d.,\s]+$/.test(v)) v = `'${v}`;
  return /[",\r\n]/.test(v) ? `"${v.replace(/"/g, '""')}"` : v;
}

/** Downloads the rows loaded in the screen's first grid as CSV; XLSX is not available yet. */
function exportGrid(button) {
  if (button.dataset.zeynExport === 'xlsx') return toast('XLSX export is not available yet.');
  const screen = screenOf(button);
  const grid = screen?.querySelector('[data-zeyn-grid]');
  if (!grid) return;
  const head = [...grid.querySelectorAll('thead tr:first-child th')].map((th) => th.textContent.trim());
  const s = grids.get(grid);
  const loaded =
    s?.paging === 'virtual' ? [...s.cache.keys()].sort((a, b) => a - b).flatMap((b) => s.cache.get(b)) : rows(grid);
  const body = loaded.map((tr) => [...tr.cells].map((td) => td.textContent.trim()));
  const csv = [head, ...body].map((r) => r.map(csvCell).join(',')).join('\r\n') + '\r\n';
  const url = URL.createObjectURL(new Blob(['﻿', csv], { type: 'text/csv;charset=utf-8' }));
  const a = document.createElement('a');
  a.href = url;
  a.download = `${screen.dataset.zeynScreen || 'export'}.csv`;
  a.click();
  setTimeout(() => URL.revokeObjectURL(url), 1000);
}

/** Stores the chosen language with `POST /_zeyn/locale` and reloads the page. */
async function changeLocale(select) {
  const r = await post(BASE + '/_zeyn/locale', { locale: select.value });
  if (r.ok) location.reload();
}

/** Stores a hand picked time zone (`""` = automatic) with `POST /_zeyn/timezone` and reloads. */
async function changeTimeZone(select) {
  const r = await post(BASE + '/_zeyn/timezone', { zone: select.value });
  if (r.ok) location.reload();
}

/** Reads the page cookies into an object. */
function cookies() {
  const out = {};
  for (const part of document.cookie.split(';')) {
    const i = part.indexOf('=');
    if (i < 0) continue;
    try {
      out[part.slice(0, i).trim()] = decodeURIComponent(part.slice(i + 1).trim());
    } catch {
      continue;
    }
  }
  return out;
}

/**
 * Keeps the `zeyn_tz` cookie equal to the browser time zone. When the server rendered the page in
 * another zone (`meta[name=zeyn-tz]`) and the user did not pick one by hand (`zeyn_tz_manual`),
 * reloads once per session so the server can use the new cookie. No date math happens here.
 */
function syncTimeZone() {
  let zone;
  try {
    zone = Intl.DateTimeFormat().resolvedOptions().timeZone;
  } catch {
    return;
  }
  if (!zone) return;
  const jar = cookies();
  if (jar.zeyn_tz !== zone) {
    document.cookie = `zeyn_tz=${encodeURIComponent(zone)}; path=${BASE || '/'}; max-age=31536000; samesite=lax`;
  }
  const served = document.querySelector('meta[name="zeyn-tz"]')?.content;
  if (!served || served === zone || 'zeyn_tz_manual' in jar) return;
  try {
    if (sessionStorage.getItem('zeyn_tz_reloaded')) return;
    sessionStorage.setItem('zeyn_tz_reloaded', '1');
  } catch {
    return;
  }
  location.reload();
}

/** Abort controller of the currently registered WebMCP tools. */
let agentTools = { controller: null, names: [], handles: [], seq: 0 };

/**
 * Registers the WebMCP tools of the top visible screen and unregisters the previous ones.
 *
 * Uses `document.modelContext.registerTool(tool, { signal })` from the WebMCP draft
 * (unregistering by aborting the signal), and falls back to the older `navigator.modelContext`
 * with `unregisterTool(name)` or a returned `{ unregister() }` handle. Does nothing when the
 * browser has no model context or `GET /_zeyn/tools` answers with an error (agent tools off).
 */
async function syncAgentTools() {
  const mc = document.modelContext ?? navigator.modelContext;
  if (!mc?.registerTool) return;
  const seq = ++agentTools.seq;
  agentTools.controller?.abort();
  for (const h of agentTools.handles) h?.unregister?.();
  for (const name of agentTools.names) mc.unregisterTool?.(name);
  agentTools = { controller: null, names: [], handles: [], seq };

  const screen = dialogs.at(-1)?.dialog.querySelector('[data-zeyn-screen]') ?? document.querySelector('.z-main [data-zeyn-screen]');
  if (!screen) return;
  const name = screen.dataset.zeynScreen;
  const params = paramsOf(screen);
  let list;
  try {
    const q = params == null ? '' : `&params=${encodeURIComponent(JSON.stringify(params))}`;
    const res = await fetch(`${BASE}/_zeyn/tools?screen=${encodeURIComponent(name)}${q}`, { credentials: 'same-origin' });
    if (!res.ok) return;
    list = (await res.json())?.tools ?? [];
  } catch {
    return;
  }
  if (seq !== agentTools.seq) return;

  const controller = new AbortController();
  agentTools.controller = controller;
  for (const tool of list) {
    const base = { screen: name, params, node: tool.node };
    const execute = async (input) => {
      if (tool.kind === 'grid') {
        const r = await post(BASE + '/_zeyn/grid', { ...base, query: input }, { quiet: true });
        return { status: r.status, ...r.data, text: r.data?.rows ? rowsText(r.data.rows) : '' };
      }
      const r = await post(BASE + '/_zeyn/form', { ...base, values: input, via_agent: true }, { quiet: true });
      if (r.ok) await runEffects(r.data?.effects);
      return { status: r.status, ...r.data };
    };
    try {
      const handle = await mc.registerTool(
        { name: tool.name, description: tool.description, inputSchema: tool.inputSchema, execute },
        { signal: controller.signal },
      );
      agentTools.names.push(tool.name);
      agentTools.handles.push(handle);
    } catch {
      continue;
    }
  }
}

/** Turns rows HTML into tab separated text lines without running any markup. */
function rowsText(html) {
  const t = document.createElement('template');
  t.innerHTML = `<table><tbody>${html}</tbody></table>`;
  return [...t.content.querySelectorAll('tr')]
    .map((tr) => [...tr.cells].map((c) => c.textContent.trim()).join('\t'))
    .join('\n');
}

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
  if ((el = t.closest('[data-zeyn-bulk-run]'))) return runBulk(el);
  if ((el = t.closest('.z-menu-item'))) return runMenuItem(el);
  if ((el = t.closest('button[data-zeyn-run]'))) return runRowItem(el);
  if ((el = t.closest('button.z-sort'))) return toggleSort(el, e.shiftKey);
  if ((el = t.closest('button.z-col-menu'))) return openColumnMenu(el);
  if ((el = t.closest('[data-zeyn-page]'))) return loadGrid(el.closest('[data-zeyn-grid]'), el.dataset.zeynPage === 'prev' ? 'prev' : 'next');
  if ((el = t.closest('[data-zeyn-more] button'))) return loadGrid(el.closest('[data-zeyn-grid]'), 'more');
  if ((el = t.closest('[data-zeyn-export]'))) return exportGrid(el);
  if ((el = t.closest('[data-zeyn-close]'))) return el.closest('dialog')?.close();
  if ((el = t.closest('button[data-zeyn-effect], a[data-zeyn-effect]'))) {
    e.preventDefault();
    const effect = parseEffect(el);
    if (effect) runEffect(effect, el);
  }
});

document.addEventListener('focusin', (e) => {
  const tr = e.target;
  if (tr instanceof HTMLTableRowElement && tr.parentElement?.matches('[data-zeyn-rows]')) activate(tr, false);
});

document.addEventListener('dblclick', (e) => {
  const tr = e.target instanceof Element ? e.target.closest('[data-zeyn-rows] > tr[data-zeyn-effect]') : null;
  const effect = tr && parseEffect(tr);
  if (effect) runEffect(effect, tr);
});

document.addEventListener('keydown', (e) => {
  const t = e.target;
  if (t instanceof Element && t.matches('.z-menu-item') && menuKey(e, t)) return;
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
  const handle = e.target instanceof Element ? e.target.closest('.z-resize') : null;
  if (handle && e.button === 0) startResize(e, handle);
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
  openMenu(convert(items), { ...at, opener: focusBack, label: row ? owner.querySelector('td:not(.z-select)')?.textContent.trim() : owner.querySelector('h1, h2')?.textContent });
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
});

syncTimeZone();
setInterval(() => relativeTimes(document), 60_000);
initGrids(document);
syncAgentTools();

document.addEventListener('zeyn:content', (e) => {
  initGrids(e.detail);
  syncAgentTools();
});
